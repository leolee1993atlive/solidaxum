//! 渠道配置服务:读取(DB 优先 / env 回退)、保存(密钥 AES 加密)、
//! 对外脱敏、连通性自检。
//!
//! 配置优先级:**DB 有已配置行则用 DB,否则回退环境变量** —— 既有 env
//! 部署不破,新装可全在页面维护。

use std::sync::Arc;

use serde::Serialize;
use serde_json::{Value, json};

use app_core::error::{AppError, AppResult};
use app_core::serde::encrypt_field::FieldEncryptor;
use app_data::DataContext;

use crate::push::channel_senders as senders;
use crate::push::repository::sys_message_channel::{
    self, ChannelConfig, ChannelRepository,
};

/// 支持的渠道(短信与 APP 按院方定制,不在配置页维护)。
pub const MANAGED_CHANNELS: [(&str, &str); 3] = [
    (senders::CH_DINGTALK, "钉钉"),
    (senders::CH_WECHAT, "企业微信"),
    (senders::CH_FEISHU, "飞书"),
];

/// 对外回读视图(密钥一律脱敏;`configured` 告知是否已配齐)。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelVo {
    pub channel: String,
    pub channel_name: String,
    pub enabled: bool,
    pub client_id: String,
    /// 脱敏后的密钥(已配置显示 ******,未配置为空串)
    pub client_secret: String,
    pub agent_id: String,
    pub agent_secret: String,
    pub base_url: String,
    pub domain: String,
    pub remark: Option<String>,
    /// 凭据是否齐全(前端据此显示"未配置")
    pub configured: bool,
    /// 密钥来源:db / env / none(便于排查"为什么改了不生效")
    pub source: String,
}

/// 保存入参。**字段缺省(None)= 保留现值**,便于只改部分字段
/// (如只改备注);显式传空串 = 清空该字段。密钥另约定空串 = 保留
/// 现值(不回填则不动),显式清空密钥目前不支持。
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelSaveBo {
    /// 缺省 = 保留现值;全新渠道缺省 = 停用
    pub enabled: Option<bool>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub agent_id: Option<String>,
    pub agent_secret: Option<String>,
    pub base_url: Option<String>,
    pub domain: Option<String>,
    pub remark: Option<String>,
}

fn env_of(ch: &str, field: &str) -> String {
    let key = match (ch, field) {
        (senders::CH_DINGTALK, "client_id") => "DINGTALK_CLIENT_ID",
        (senders::CH_DINGTALK, "client_secret") => "DINGTALK_CLIENT_SECRET",
        (senders::CH_DINGTALK, "agent_id") => "DINGTALK_AGENT_ID",
        (senders::CH_WECHAT, "client_id") => "WECHAT_CORP_ID",
        (senders::CH_WECHAT, "client_secret") => "WECHAT_CORP_SECRET",
        (senders::CH_WECHAT, "agent_id") => "WECHAT_AGENT_ID",
        (senders::CH_FEISHU, "client_id") => "FEISHU_APP_ID",
        (senders::CH_FEISHU, "client_secret") => "FEISHU_APP_SECRET",
        (senders::CH_FEISHU, "agent_id") => "FEISHU_APP_ID",
        _ => return String::new(),
    };
    std::env::var(key).unwrap_or_default()
}

/// 解析某渠道的生效配置:**DB 已配置则用 DB(解密),否则 env 回退**。
/// 返回 (配置, 来源)。
pub async fn resolve_config(
    ctx: &DataContext<'_>,
    encryptor: Option<&Arc<dyn FieldEncryptor>>,
    channel: &str,
) -> AppResult<(ChannelConfig, String)> {
    // 表读取失败(存量部署未跑迁移/DB 抖动)不再向上抛 —— 否则会杀死
    // 整个推送编排,连 env 配置的渠道也发不出。降级回退 env。
    let row = match ChannelRepository::new()
        .select_by_channel(ctx, channel)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                channel,
                error = %e,
                "渠道配置表读取失败(未迁移?),回退环境变量"
            );
            None
        }
    };
    if let Some(m) = row {
        let dec = |v: &Option<String>| -> String {
            v.as_deref()
                .filter(|s| !s.is_empty())
                .map(|s| match encryptor {
                    Some(e) => e.decrypt(s).unwrap_or_else(|_| s.to_string()),
                    None => s.to_string(),
                })
                .unwrap_or_default()
        };
        let cfg = ChannelConfig {
            channel: m.channel.clone(),
            enabled: m.enabled == "1",
            client_id: m.client_id.clone().unwrap_or_default(),
            client_secret: dec(&m.client_secret),
            agent_id: m.agent_id.clone().unwrap_or_default(),
            agent_secret: dec(&m.agent_secret),
            base_url: m.base_url.clone().unwrap_or_default(),
            domain: m.domain.clone().unwrap_or_default(),
        };
        if cfg.is_configured() {
            return Ok((cfg, "db".to_string()));
        }
    }
    // env 回退
    let cfg = ChannelConfig {
        channel: channel.to_string(),
        enabled: true,
        client_id: env_of(channel, "client_id"),
        client_secret: env_of(channel, "client_secret"),
        agent_id: env_of(channel, "agent_id"),
        agent_secret: String::new(),
        base_url: std::env::var("MSG_CHANNEL_BASE_URL").unwrap_or_default(),
        domain: String::new(),
    };
    let source = if cfg.is_configured() { "env" } else { "none" };
    Ok((cfg, source.to_string()))
}

/// 渠道清单(脱敏)。
pub async fn list_channels(
    ctx: &DataContext<'_>,
    encryptor: Option<&Arc<dyn FieldEncryptor>>,
) -> AppResult<Vec<ChannelVo>> {
    let rows = ChannelRepository::new().select_all(ctx).await?;
    let mut out = Vec::new();
    for (ch, name) in MANAGED_CHANNELS {
        let row = rows.iter().find(|r| r.channel == ch);
        let (cfg, source) = resolve_config(ctx, encryptor, ch).await?;
        let has_secret = !cfg.client_secret.is_empty();
        out.push(ChannelVo {
            channel: ch.to_string(),
            channel_name: row
                .map(|r| r.channel_name.clone())
                .unwrap_or_else(|| name.to_string()),
            enabled: cfg.enabled,
            client_id: cfg.client_id.clone(),
            client_secret: if has_secret {
                "******".to_string()
            } else {
                String::new()
            },
            agent_id: cfg.agent_id.clone(),
            agent_secret: if cfg.agent_secret.is_empty() {
                String::new()
            } else {
                "******".to_string()
            },
            base_url: cfg.base_url.clone(),
            domain: cfg.domain.clone(),
            remark: row.and_then(|r| r.remark.clone()),
            configured: cfg.is_configured(),
            source,
        });
    }
    Ok(out)
}

fn enc(
    encryptor: Option<&Arc<dyn FieldEncryptor>>,
    v: &str,
) -> AppResult<String> {
    match encryptor {
        Some(e) => e
            .encrypt(v)
            .map_err(|err| AppError::Internal(format!("密钥加密失败: {err}"))),
        None => {
            // 与 haems 同口径:未配置 aes_key 时降级明文,但必须留痕。
            tracing::warn!("字段加密器未配置,消息渠道密钥将以明文落库");
            Ok(v.to_string())
        }
    }
}

/// 保存某渠道配置。字段缺省(None)= 保留现值(便于只改部分字段);
/// 密钥空串 = 保留现值(不回填则不动)。
pub async fn save_channel(
    ctx: &DataContext<'_>,
    encryptor: Option<&Arc<dyn FieldEncryptor>>,
    channel: &str,
    bo: ChannelSaveBo,
) -> AppResult<()> {
    if !MANAGED_CHANNELS.iter().any(|(c, _)| *c == channel) {
        return Err(AppError::BadRequest(format!(
            "不支持的渠道:{channel}(短信/APP 按院方定制,不在本表维护)"
        )));
    }
    let repo = ChannelRepository::new();
    let existing = repo.select_by_channel(ctx, channel).await?;
    // 缺省 = 保留现值;显式传入则 trim,空串落 NULL(显式清空)。
    let keep = |incoming: Option<String>,
                f: fn(&sys_message_channel::Model) -> Option<String>|
     -> Option<String> {
        match incoming {
            None => existing.as_ref().and_then(f),
            Some(v) => {
                let t = v.trim().to_string();
                if t.is_empty() { None } else { Some(t) }
            }
        }
    };
    let old = |f: fn(&sys_message_channel::Model) -> Option<String>| {
        existing.as_ref().and_then(f)
    };
    let enabled = bo
        .enabled
        .or_else(|| existing.as_ref().map(|m| m.enabled == "1"))
        .unwrap_or(false);
    let secret_in = bo.client_secret.as_deref().unwrap_or("");
    let client_secret = if secret_in.trim().is_empty() {
        old(|m| m.client_secret.clone())
    } else {
        Some(enc(encryptor, secret_in.trim())?)
    };
    let agent_secret_in = bo.agent_secret.as_deref().unwrap_or("");
    let agent_secret = if agent_secret_in.trim().is_empty() {
        old(|m| m.agent_secret.clone())
    } else {
        Some(enc(encryptor, agent_secret_in.trim())?)
    };
    let am = sys_message_channel::narrow_update(
        channel,
        enabled,
        keep(bo.client_id, |m| m.client_id.clone()),
        client_secret,
        keep(bo.agent_id, |m| m.agent_id.clone()),
        agent_secret,
        keep(bo.base_url, |m| m.base_url.clone()),
        keep(bo.domain, |m| m.domain.clone()),
        keep(bo.remark, |m| m.remark.clone()),
    );
    repo.update_by_channel(ctx, am).await?;
    Ok(())
}

/// 连通性自检:取 token(不发送任何消息)。
pub async fn test_channel(
    ctx: &DataContext<'_>,
    encryptor: Option<&Arc<dyn FieldEncryptor>>,
    channel: &str,
) -> AppResult<Value> {
    let (cfg, source) = resolve_config(ctx, encryptor, channel).await?;
    if !cfg.is_configured() {
        return Err(AppError::BadRequest(format!(
            "{channel} 未配置(需 clientId + clientSecret)"
        )));
    }
    let token = match channel {
        senders::CH_DINGTALK => senders::dingtalk_token(&cfg).await,
        senders::CH_WECHAT => senders::wechat_token(&cfg).await,
        senders::CH_FEISHU => senders::feishu_token(&cfg).await,
        _ => Err(AppError::BadRequest(format!("不支持的渠道:{channel}"))),
    }?;
    Ok(json!({
        "ok": true,
        "channel": channel,
        "source": source,
        "tokenPrefix": token.chars().take(6).collect::<String>(),
        "message": "连接成功(仅取 token,未发送消息)",
    }))
}
