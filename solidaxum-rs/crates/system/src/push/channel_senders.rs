//! 企业应用定向消息发送器(钉钉 / 企微 / 飞书)。
//!
//! 三件事:
//! 1. **点对点**:按平台 userid 定向(touser / userIds),不是群机器人广播;
//! 2. **手机号回写**:`sys_user.*_user_id` 为空时按手机号查平台 userid
//!    (本任务只提供查询能力,回写编排在 B5 Task 2);
//! 3. **token 缓存**:各平台 token 有 2h 有效期,进程内按渠道缓存至过期前 5min。
//!
//! 配置来源:`sys_message_channel`(DB 优先,空则回退同名环境变量,见
//! [`resolve_config`])。

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use app_core::error::{AppError, AppResult};
use serde_json::{Value, json};

use crate::push::repository::sys_message_channel::ChannelConfig;

/// 渠道标识常量(与 `sys_message_channel.channel` 一致)。
pub const CH_DINGTALK: &str = "DINGTALK";
pub const CH_WECHAT: &str = "WECHAT";
pub const CH_FEISHU: &str = "FEISHU";

const DING_BASE: &str = "https://api.dingtalk.com";
const DING_OAPI: &str = "https://oapi.dingtalk.com";
const WECOM_BASE: &str = "https://qyapi.weixin.qq.com";
const FEISHU_BASE: &str = "https://open.feishu.cn";

fn http() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(format!("HTTP 客户端构建失败: {e}")))
}

fn http_err(e: reqwest::Error) -> AppError {
    // 脱敏:reqwest Display 会带完整 URL(可能含 token 查询串)
    let s = e.to_string();
    let s = match s.find(" for url ") {
        Some(i) => format!("{}(URL 已脱敏)", &s[..i]),
        None => s,
    };
    AppError::Internal(format!("渠道请求失败: {s}"))
}

fn body_err(channel: &str, body: &Value) -> AppError {
    AppError::Internal(format!("{channel} 接口返回: {body}"))
}

// ─────────────────────────── token 缓存 ───────────────────────────

struct CachedToken {
    token: String,
    expires_at: Instant,
}

fn token_cache()
-> &'static Mutex<std::collections::HashMap<String, CachedToken>> {
    static CACHE: std::sync::OnceLock<
        Mutex<std::collections::HashMap<String, CachedToken>>,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn cache_get(key: &str) -> Option<String> {
    let map = token_cache().lock().ok()?;
    map.get(key)
        .filter(|t| t.expires_at > Instant::now())
        .map(|t| t.token.clone())
}

fn cache_put(key: &str, token: &str, ttl_secs: u64) {
    if let Ok(mut map) = token_cache().lock() {
        // 提前 5 分钟过期,避免边界失效
        let ttl = ttl_secs.saturating_sub(300).max(60);
        map.insert(
            key.to_string(),
            CachedToken {
                token: token.to_string(),
                expires_at: Instant::now() + Duration::from_secs(ttl),
            },
        );
    }
}

// ─────────────────────────── 钉钉 ───────────────────────────

/// 钉钉企业内部应用:取 accessToken(新版 /v1.0/oauth2/accessToken)。
pub async fn dingtalk_token(cfg: &ChannelConfig) -> AppResult<String> {
    let key = format!("{CH_DINGTALK}:{}", cfg.client_id);
    if let Some(t) = cache_get(&key) {
        return Ok(t);
    }
    let url = format!("{}/v1.0/oauth2/accessToken", cfg.api_base(DING_BASE));
    let resp = http()?
        .post(&url)
        .json(&json!({
            "appKey": cfg.client_id,
            "appSecret": cfg.client_secret,
        }))
        .send()
        .await
        .map_err(http_err)?;
    let body: Value = resp.json().await.map_err(http_err)?;
    let token = body
        .get("accessToken")
        .and_then(Value::as_str)
        .ok_or_else(|| body_err("钉钉取 token", &body))?;
    cache_put(
        &key,
        token,
        body.get("expireIn").and_then(Value::as_u64).unwrap_or(7200),
    );
    Ok(token.to_string())
}

/// 钉钉手机号 → userid(锚 `queryUserIdByMobile`)。
pub async fn dingtalk_userid_by_mobile(
    cfg: &ChannelConfig,
    token: &str,
    mobile: &str,
) -> AppResult<Option<String>> {
    let url = format!(
        "{}/topapi/v2/user/getbymobile?access_token={token}",
        cfg.api_base(DING_OAPI)
    );
    let resp = http()?
        .post(&url)
        .json(&json!({ "mobile": mobile }))
        .send()
        .await
        .map_err(http_err)?;
    let body: Value = resp.json().await.map_err(http_err)?;
    if body.get("errcode").and_then(Value::as_i64) != Some(0) {
        return Ok(None);
    }
    Ok(body
        .pointer("/result/userid")
        .and_then(Value::as_str)
        .map(str::to_string))
}

/// 钉钉工作通知(定向):用 `robot/oToMessages/batchSend`,
/// robotCode = clientId。
pub async fn dingtalk_send(
    cfg: &ChannelConfig,
    user_ids: &[String],
    content: &str,
) -> AppResult<Value> {
    let token = dingtalk_token(cfg).await?;
    let url = format!(
        "{}/v1.0/robot/oToMessages/batchSend",
        cfg.api_base(DING_BASE)
    );
    let resp = http()?
        .post(&url)
        .header("x-acs-dingtalk-access-token", &token)
        .json(&json!({
            "robotCode": cfg.client_id,
            "userIds": user_ids,
            "msgKey": "sampleText",
            "msgParam": json!({ "content": content }).to_string(),
        }))
        .send()
        .await
        .map_err(http_err)?;
    let body: Value = resp.json().await.map_err(http_err)?;
    if body.get("invalidStaffIdList").is_some()
        || body.get("flowControlledStaffIdList").is_some()
        || body.get("processQueryKey").is_some()
    {
        return Ok(body);
    }
    Err(body_err("钉钉发送", &body))
}

// ─────────────────────────── 企业微信 ───────────────────────────

/// 企微:取 access_token(corpid + corpsecret)。
pub async fn wechat_token(cfg: &ChannelConfig) -> AppResult<String> {
    let key = format!("{CH_WECHAT}:{}", cfg.client_id);
    if let Some(t) = cache_get(&key) {
        return Ok(t);
    }
    let url = format!(
        "{}/cgi-bin/gettoken?corpid={}&corpsecret={}",
        cfg.api_base(WECOM_BASE),
        cfg.client_id,
        cfg.client_secret
    );
    let resp = http()?.get(&url).send().await.map_err(http_err)?;
    let body: Value = resp.json().await.map_err(http_err)?;
    let token = body
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| body_err("企微取 token", &body))?;
    cache_put(
        &key,
        token,
        body.get("expires_in")
            .and_then(Value::as_u64)
            .unwrap_or(7200),
    );
    Ok(token.to_string())
}

/// 企微手机号 → userid(锚 `queryUserIdByMobile`)。
pub async fn wechat_userid_by_mobile(
    cfg: &ChannelConfig,
    token: &str,
    mobile: &str,
) -> AppResult<Option<String>> {
    let url = format!(
        "{}/cgi-bin/user/getuserid?access_token={token}",
        cfg.api_base(WECOM_BASE)
    );
    let resp = http()?
        .post(&url)
        .json(&json!({ "mobile": mobile }))
        .send()
        .await
        .map_err(http_err)?;
    let body: Value = resp.json().await.map_err(http_err)?;
    if body.get("errcode").and_then(Value::as_i64) != Some(0) {
        return Ok(None);
    }
    Ok(body
        .get("userid")
        .and_then(Value::as_str)
        .map(str::to_string))
}

/// 企微应用消息(定向):`/cgi-bin/message/send`,touser 竖线分隔。
pub async fn wechat_send(
    cfg: &ChannelConfig,
    user_ids: &[String],
    content: &str,
) -> AppResult<Value> {
    // 企微 message/send 的 agentid 必填;连通性自检只取 token 不查它,
    // 所以这里前置拦截,给出可读文案而不是平台报错。
    if cfg.agent_id.trim().is_empty() {
        return Err(AppError::BadRequest(
            "企微发送需要 AgentId(企业微信后台-应用管理页可查)".to_string(),
        ));
    }
    let token = wechat_token(cfg).await?;
    let url = format!(
        "{}/cgi-bin/message/send?access_token={token}",
        cfg.api_base(WECOM_BASE)
    );
    let agent: Value = cfg
        .agent_id
        .trim()
        .parse::<i64>()
        .map(Value::from)
        .unwrap_or(Value::String(cfg.agent_id.clone()));
    let resp = http()?
        .post(&url)
        .json(&json!({
            "touser": user_ids.join("|"),
            "msgtype": "text",
            "agentid": agent,
            "text": { "content": content },
            "safe": 0,
        }))
        .send()
        .await
        .map_err(http_err)?;
    let body: Value = resp.json().await.map_err(http_err)?;
    if body.get("errcode").and_then(Value::as_i64) == Some(0) {
        Ok(body)
    } else {
        Err(body_err("企微发送", &body))
    }
}

// ─────────────────────────── 飞书 ───────────────────────────

/// 飞书自建应用:tenant_access_token(app_id + app_secret)。
pub async fn feishu_token(cfg: &ChannelConfig) -> AppResult<String> {
    let key = format!("{CH_FEISHU}:{}", cfg.client_id);
    if let Some(t) = cache_get(&key) {
        return Ok(t);
    }
    let url = format!(
        "{}/open-apis/auth/v3/tenant_access_token/internal",
        cfg.api_base(FEISHU_BASE)
    );
    let resp = http()?
        .post(&url)
        .json(&json!({
            "app_id": cfg.client_id,
            "app_secret": cfg.client_secret,
        }))
        .send()
        .await
        .map_err(http_err)?;
    let body: Value = resp.json().await.map_err(http_err)?;
    let token = body
        .get("tenant_access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| body_err("飞书取 token", &body))?;
    cache_put(
        &key,
        token,
        body.get("expire").and_then(Value::as_u64).unwrap_or(7200),
    );
    Ok(token.to_string())
}

/// 飞书手机号 → open_id(联系方式的 batch_get_id)。
pub async fn feishu_userid_by_mobile(
    cfg: &ChannelConfig,
    token: &str,
    mobile: &str,
) -> AppResult<Option<String>> {
    let url = format!(
        "{}/open-apis/contact/v3/users/batch_get_id?user_id_type=open_id",
        cfg.api_base(FEISHU_BASE)
    );
    let resp = http()?
        .post(&url)
        .bearer_auth(token)
        .json(&json!({ "mobiles": [mobile] }))
        .send()
        .await
        .map_err(http_err)?;
    let body: Value = resp.json().await.map_err(http_err)?;
    if body.get("code").and_then(Value::as_i64) != Some(0) {
        return Ok(None);
    }
    Ok(body
        .pointer("/data/user_list/0/user_id")
        .and_then(Value::as_str)
        .map(str::to_string))
}

/// 飞书消息(定向):`/open-apis/im/v1/messages?receive_id_type=open_id`。
pub async fn feishu_send(
    cfg: &ChannelConfig,
    open_ids: &[String],
    content: &str,
) -> AppResult<Value> {
    let token = feishu_token(cfg).await?;
    let url = format!(
        "{}/open-apis/im/v1/messages?receive_id_type=open_id",
        cfg.api_base(FEISHU_BASE)
    );
    let mut last = Value::Null;
    for id in open_ids {
        let resp = http()?
            .post(&url)
            .bearer_auth(&token)
            .json(&json!({
                "receive_id": id,
                "msg_type": "text",
                "content": json!({ "text": content }).to_string(),
            }))
            .send()
            .await
            .map_err(http_err)?;
        let body: Value = resp.json().await.map_err(http_err)?;
        if body.get("code").and_then(Value::as_i64) != Some(0) {
            return Err(body_err("飞书发送", &body));
        }
        last = body;
    }
    Ok(last)
}
