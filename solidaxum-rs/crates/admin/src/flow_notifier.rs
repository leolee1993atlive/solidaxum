use std::sync::Arc;

use app_core::{error::AppResult, util::snowflake};
use flow::notifier::FlowNotifier;
use system::push::{message::PushMessage, sse::PushHub};

/// 基于 system PushHub 的工作流通知实现。
#[derive(Clone)]
pub struct PushHubFlowNotifier {
    hub: PushHub,
}

impl PushHubFlowNotifier {
    pub fn new(hub: PushHub) -> Self {
        Self { hub }
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
        Ok(())
    }
}

/// 便于测试注入自定义 notifier。
pub fn arc_notifier(hub: PushHub) -> Arc<dyn FlowNotifier> {
    Arc::new(PushHubFlowNotifier::new(hub))
}
