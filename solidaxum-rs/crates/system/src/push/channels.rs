//! 多渠道路由(钉钉/企微/飞书定向 + 短信/APP 入口)。
//!
//! **定向语义**:钉钉/企微/飞书走企业应用点对点消息(按平台 userid /
//! open_id),不是群机器人广播。
//!
//! 渠道配置来源见 [`crate::push::channel_config_service::resolve_config`]:
//! DB(`sys_message_channel`,密钥 AES)优先,空则回退环境变量。
//!
//! **短信 / APP 主线只留入口**:两者按院方定制开发。此处保留渠道枚举、
//! 发送日志与 `SmsSender` / `AppSender` 两个 trait —— 未注册实现时落
//! `SKIPPED(no_driver)`,院方实现后注册即可,主线不含任何厂商专有代码。
//!
//! 发送结果(SUCCESS/FAILED/SKIPPED)一律写 `sys_message_send_log`。

use std::{
    sync::atomic::{AtomicI64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use app_data::{DataContext, DataError, sea_orm::ActiveValue::Set};
use serde_json::{Value, json};

use crate::push::repository::sys_message_send_log::{
    ActiveModel, SendLogRepository,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    App,
    Sms,
    Wechat,
    Dingtalk,
    Feishu,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::App => "APP",
            Self::Sms => "SMS",
            Self::Wechat => "WECHAT",
            Self::Dingtalk => "DINGTALK",
            Self::Feishu => "FEISHU",
        }
    }

    /// 环境变量启用开关(DB 配置启用优先,见 resolve_config)。
    pub fn enable_key(self) -> &'static str {
        match self {
            Self::App => "APP_MSG_ENABLE",
            Self::Sms => "SMS_MSG_ENABLE",
            Self::Wechat => "WECHAT_MSG_ENABLE",
            Self::Dingtalk => "DINGTALK_MSG_ENABLE",
            Self::Feishu => "FEISHU_MSG_ENABLE",
        }
    }
}

// ───────────────────────── 短信 / APP 扩展点 ─────────────────────────

/// 短信发送器(院方定制实现;主线未注册任何实现)。
#[allow(clippy::double_must_use)]
#[async_trait::async_trait]
pub trait SmsSender: Send + Sync {
    async fn send(
        &self,
        phones: &[String],
        content: &Value,
    ) -> Result<Value, String>;
}

/// APP 推送发送器(院方定制实现;主线未注册任何实现)。
#[allow(clippy::double_must_use)]
#[async_trait::async_trait]
pub trait AppSender: Send + Sync {
    async fn send(
        &self,
        user_names: &[String],
        content: &str,
    ) -> Result<Value, String>;
}

/// 未注册实现的占位(dispatch 见到即落 SKIPPED(no_driver))。
pub struct NotConfiguredSender;

#[async_trait::async_trait]
impl SmsSender for NotConfiguredSender {
    async fn send(&self, _p: &[String], _c: &Value) -> Result<Value, String> {
        Err("no_driver".to_string())
    }
}

#[async_trait::async_trait]
impl AppSender for NotConfiguredSender {
    async fn send(&self, _u: &[String], _c: &str) -> Result<Value, String> {
        Err("no_driver".to_string())
    }
}

/// 进程级发送器注册表(院方接入时装配期 set 一次)。
#[derive(Default)]
pub struct SenderRegistry {
    pub sms: Option<std::sync::Arc<dyn SmsSender>>,
    pub app: Option<std::sync::Arc<dyn AppSender>>,
}

fn registry() -> &'static std::sync::Mutex<SenderRegistry> {
    static R: std::sync::OnceLock<std::sync::Mutex<SenderRegistry>> =
        std::sync::OnceLock::new();
    R.get_or_init(|| std::sync::Mutex::new(SenderRegistry::default()))
}

/// 注册短信/APP 发送器(院方定制实现的接入点;重复调用以后者为准)。
pub fn register_senders(
    sms: Option<std::sync::Arc<dyn SmsSender>>,
    app: Option<std::sync::Arc<dyn AppSender>>,
) {
    if let Ok(mut r) = registry().lock() {
        r.sms = sms;
        r.app = app;
    }
}

fn sms_sender() -> Option<std::sync::Arc<dyn SmsSender>> {
    registry().lock().ok().and_then(|r| r.sms.clone())
}

fn app_sender() -> Option<std::sync::Arc<dyn AppSender>> {
    registry().lock().ok().and_then(|r| r.app.clone())
}

// ───────────────────────── 日志 ─────────────────────────

static LOG_SEQ: AtomicI64 = AtomicI64::new(0);

/// 发送日志主键:毫秒 <<16 | 16 位进程内序号(多实例同毫秒不撞)。
fn next_log_id() -> i64 {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let seq = LOG_SEQ.fetch_add(1, Ordering::Relaxed) & 0xFFFF;
    (ms << 16) | seq
}

/// 错误串脱敏(可能带 token 查询串)。
fn sanitize_error_str(s: &str) -> String {
    match s.find(" for url ") {
        Some(i) => format!("{}(URL 已脱敏)", &s[..i]),
        None => s.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn write_log(
    ctx: &DataContext<'_>,
    channel: &str,
    biz_type: &str,
    user_ids: &[i64],
    content: &str,
    status: &str,
    skip_reason: Option<&str>,
    error: Option<&str>,
    provider: Option<&str>,
    batch_id: Option<&str>,
    biz_id: Option<&str>,
) {
    let mut am = ActiveModel::from_new();
    am.log_id = Set(next_log_id());
    am.channel = Set(channel.to_string());
    am.batch_id = Set(batch_id.map(str::to_string));
    am.biz_type = Set(Some(biz_type.to_string()));
    am.biz_id = Set(biz_id.map(str::to_string));
    am.recipient_user_ids = Set(Some(
        serde_json::to_string(user_ids).unwrap_or_else(|_| "[]".into()),
    ));
    am.content = Set(Some(content.to_string()));
    am.status = Set(status.to_string());
    am.skip_reason = Set(skip_reason.map(str::to_string));
    am.error_message = Set(error.map(str::to_string));
    am.provider_response =
        Set(provider.map(|s| s.chars().take(500).collect::<String>()));
    if let Err(e) = SendLogRepository::new().insert(ctx, am).await {
        tracing::warn!(
            channel,
            error = %e,
            "发送日志写入失败"
        );
    }
}

/// 一次渠道发送的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendOutcome {
    Success,
    Failed,
    Skipped,
}

/// 渠道发送入参。
pub struct SendRequest<'a> {
    /// 业务类型(由调用方参数化,不再硬编码单一业务)
    pub biz_type: &'a str,
    /// 生效配置(钉钉/企微/飞书);短信/APP 为 None
    pub config:
        Option<&'a crate::push::repository::sys_message_channel::ChannelConfig>,
    pub user_ids: &'a [i64],
    pub content: &'a str,
    /// 短信结构化内容(院方网关契约,由实现解释)
    pub content_map: Option<&'a Value>,
    /// 平台收件人:钉钉/企微 userid、飞书 open_id、短信手机号、APP 用户名
    pub platform_ids: &'a [String],
    pub batch_id: &'a str,
    pub biz_id: &'a str,
}

/// 发送一条渠道消息。失败只落日志,不向上抛。
pub async fn dispatch(
    ctx: &DataContext<'_>,
    channel: Channel,
    req: SendRequest<'_>,
) -> Result<SendOutcome, DataError> {
    macro_rules! skip {
        ($reason:expr) => {{
            write_log(
                ctx,
                channel.as_str(),
                req.biz_type,
                req.user_ids,
                req.content,
                "SKIPPED",
                Some($reason),
                None,
                None,
                Some(req.batch_id),
                Some(req.biz_id),
            )
            .await;
            return Ok(SendOutcome::Skipped);
        }};
    }

    if req.user_ids.is_empty() {
        skip!("empty_user_ids");
    }

    match channel {
        Channel::Dingtalk | Channel::Wechat | Channel::Feishu => {
            let Some(cfg) = req.config else {
                skip!("not_configured");
            };
            if !cfg.enabled || !cfg.is_configured() {
                skip!("not_configured");
            }
            if req.platform_ids.is_empty() {
                skip!("no_platform_user_id");
            }
            let res = match channel {
                Channel::Dingtalk => {
                    crate::push::channel_senders::dingtalk_send(
                        cfg,
                        req.platform_ids,
                        req.content,
                    )
                    .await
                }
                Channel::Wechat => {
                    crate::push::channel_senders::wechat_send(
                        cfg,
                        req.platform_ids,
                        req.content,
                    )
                    .await
                }
                _ => {
                    crate::push::channel_senders::feishu_send(
                        cfg,
                        req.platform_ids,
                        req.content,
                    )
                    .await
                }
            };
            match res {
                Ok(body) => {
                    write_log(
                        ctx,
                        channel.as_str(),
                        req.biz_type,
                        req.user_ids,
                        req.content,
                        "SUCCESS",
                        None,
                        None,
                        Some(&body.to_string()),
                        Some(req.batch_id),
                        Some(req.biz_id),
                    )
                    .await;
                    Ok(SendOutcome::Success)
                }
                Err(e) => {
                    let msg = sanitize_error_str(&e.to_string());
                    write_log(
                        ctx,
                        channel.as_str(),
                        req.biz_type,
                        req.user_ids,
                        req.content,
                        "FAILED",
                        None,
                        Some(&msg),
                        None,
                        Some(req.batch_id),
                        Some(req.biz_id),
                    )
                    .await;
                    Ok(SendOutcome::Failed)
                }
            }
        }
        Channel::Sms => {
            let Some(sender) = sms_sender() else {
                skip!("no_driver");
            };
            if req.platform_ids.is_empty() {
                skip!("no_phone");
            }
            let content = req
                .content_map
                .cloned()
                .unwrap_or_else(|| json!({ "text": req.content }));
            match sender.send(req.platform_ids, &content).await {
                Ok(body) => {
                    write_log(
                        ctx,
                        channel.as_str(),
                        req.biz_type,
                        req.user_ids,
                        req.content,
                        "SUCCESS",
                        None,
                        None,
                        Some(&body.to_string()),
                        Some(req.batch_id),
                        Some(req.biz_id),
                    )
                    .await;
                    Ok(SendOutcome::Success)
                }
                Err(e) => {
                    let msg = sanitize_error_str(&e);
                    write_log(
                        ctx,
                        channel.as_str(),
                        req.biz_type,
                        req.user_ids,
                        req.content,
                        "FAILED",
                        None,
                        Some(&msg),
                        None,
                        Some(req.batch_id),
                        Some(req.biz_id),
                    )
                    .await;
                    Ok(SendOutcome::Failed)
                }
            }
        }
        Channel::App => {
            let Some(sender) = app_sender() else {
                skip!("no_driver");
            };
            if req.platform_ids.is_empty() {
                skip!("no_platform_user_id");
            }
            match sender.send(req.platform_ids, req.content).await {
                Ok(body) => {
                    write_log(
                        ctx,
                        channel.as_str(),
                        req.biz_type,
                        req.user_ids,
                        req.content,
                        "SUCCESS",
                        None,
                        None,
                        Some(&body.to_string()),
                        Some(req.batch_id),
                        Some(req.biz_id),
                    )
                    .await;
                    Ok(SendOutcome::Success)
                }
                Err(e) => {
                    let msg = sanitize_error_str(&e);
                    write_log(
                        ctx,
                        channel.as_str(),
                        req.biz_type,
                        req.user_ids,
                        req.content,
                        "FAILED",
                        None,
                        Some(&msg),
                        None,
                        Some(req.batch_id),
                        Some(req.biz_id),
                    )
                    .await;
                    Ok(SendOutcome::Failed)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use app_data::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    use super::*;
    use crate::push::repository::sys_message_send_log::{
        Column, Entity as SendLogEntity,
    };

    #[test]
    fn channel_names_match_send_log_contract() {
        assert_eq!(Channel::Sms.as_str(), "SMS");
        assert_eq!(Channel::Wechat.as_str(), "WECHAT");
        assert_eq!(Channel::Dingtalk.as_str(), "DINGTALK");
        assert_eq!(Channel::App.as_str(), "APP");
        assert_eq!(Channel::Feishu.as_str(), "FEISHU");
    }

    #[test]
    fn log_id_no_inline_collision() {
        let a = next_log_id();
        let b = next_log_id();
        assert_ne!(a, b, "同毫秒内两条日志主键不得碰撞");
    }

    #[test]
    fn sanitize_strips_url_query() {
        let s = "error sending request for url \
                 (https://oapi.dingtalk.com/x?access_token=SEC123)";
        let out = sanitize_error_str(s);
        assert!(!out.contains("SEC123"), "{out}");
        assert!(out.contains("URL 已脱敏"), "{out}");
        assert_eq!(sanitize_error_str("HTTP 502"), "HTTP 502");
    }

    #[tokio::test]
    async fn not_configured_senders_report_no_driver() {
        let sms = NotConfiguredSender;
        let app = NotConfiguredSender;
        assert_eq!(
            SmsSender::send(&sms, &[], &json!({})).await.unwrap_err(),
            "no_driver"
        );
        assert_eq!(
            AppSender::send(&app, &[], "x").await.unwrap_err(),
            "no_driver"
        );
    }

    /// SKIPPED 链路:空收件人优先于渠道判断,落一行可追溯日志。
    #[tokio::test]
    async fn dispatch_empty_users_writes_skipped_log() {
        let ds = app_data::testing::sqlite_memory_empty().await;
        ds.create_table(SendLogEntity).await.unwrap();
        let ctx = ds.system_context();
        let out = dispatch(
            &ctx,
            Channel::Dingtalk,
            SendRequest {
                biz_type: "TEST_BIZ",
                config: None,
                user_ids: &[],
                content: "内容",
                content_map: None,
                platform_ids: &[],
                batch_id: "batch-1",
                biz_id: "biz-1",
            },
        )
        .await
        .unwrap();
        assert_eq!(out, SendOutcome::Skipped);
        let rows = ctx
            .select(SendLogEntity::find().filter(Column::Status.eq("SKIPPED")))
            .without_row_scope()
            .all()
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].skip_reason.as_deref(), Some("empty_user_ids"));
        assert_eq!(rows[0].biz_type.as_deref(), Some("TEST_BIZ"));
        assert_eq!(rows[0].biz_id.as_deref(), Some("biz-1"));
    }

    /// 短信/APP 未注册驱动 → SKIPPED(no_driver),不依赖网络。
    #[tokio::test]
    async fn dispatch_sms_without_driver_writes_no_driver_log() {
        register_senders(None, None);
        let ds = app_data::testing::sqlite_memory_empty().await;
        ds.create_table(SendLogEntity).await.unwrap();
        let ctx = ds.system_context();
        let out = dispatch(
            &ctx,
            Channel::Sms,
            SendRequest {
                biz_type: "TEST_BIZ",
                config: None,
                user_ids: &[7],
                content: "内容",
                content_map: None,
                platform_ids: &["13800000000".to_string()],
                batch_id: "batch-1",
                biz_id: "biz-1",
            },
        )
        .await
        .unwrap();
        assert_eq!(out, SendOutcome::Skipped);
        let rows = ctx
            .select(SendLogEntity::find())
            .without_row_scope()
            .all()
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].channel, "SMS");
        assert_eq!(rows[0].skip_reason.as_deref(), Some("no_driver"));
    }
}
