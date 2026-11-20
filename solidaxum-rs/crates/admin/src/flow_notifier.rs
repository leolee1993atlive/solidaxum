use std::sync::Arc;

use app_core::{error::AppResult, util::snowflake};
use flow::notifier::FlowNotifier;
use system::push::{message::PushMessage, sse::PushHub};

/// 基于 system PushHub 的工作流通知实现。
#[derive(Clone)]
pub struct PushHubFlowNotifier {
    hub: PushHub,
    /// 可选发送日志数据源;未注入时只推 SSE 不落日志(兼容既有装配)。
    log_ds: Option<Arc<app_data::DataSource>>,
}

impl PushHubFlowNotifier {
    pub fn new(hub: PushHub) -> Self {
        Self { hub, log_ds: None }
    }

    /// 注入数据源后,每次流程通知额外写一行 `flow_notify` SSE 成功日志。
    pub fn with_send_log(mut self, ds: Arc<app_data::DataSource>) -> Self {
        self.log_ds = Some(ds);
        self
    }
}

#[async_trait::async_trait]
impl FlowNotifier for PushHubFlowNotifier {
    async fn notify_users(
        &self,
        user_ids: &[i64],
        message: &str,
        path: Option<&str>,
    ) -> AppResult<()> {
        if user_ids.is_empty() {
            return Ok(());
        }
        let path = path
            .map(String::from)
            .or_else(|| Some("/task/taskWaiting".to_string()));
        for &uid in user_ids {
            let msg = PushMessage::workflow(
                snowflake::next_id(),
                message.to_string(),
                path.clone(),
            );
            self.hub.push_to_user(uid, msg);
        }
        if let Some(ds) = &self.log_ds {
            let ctx = ds.system_context();
            system::push::push_service::write_sse_log(
                &ctx,
                "flow_notify",
                None,
                None,
                user_ids,
                message,
            )
            .await;
        }
        Ok(())
    }
}

/// 便于测试注入自定义 notifier。
pub fn arc_notifier(hub: PushHub) -> Arc<dyn FlowNotifier> {
    Arc::new(PushHubFlowNotifier::new(hub))
}

/// 带发送日志的 notifier(生产装配用)。
pub fn arc_notifier_with_send_log(
    hub: PushHub,
    ds: Arc<app_data::DataSource>,
) -> Arc<dyn FlowNotifier> {
    Arc::new(PushHubFlowNotifier::new(hub).with_send_log(ds))
}
