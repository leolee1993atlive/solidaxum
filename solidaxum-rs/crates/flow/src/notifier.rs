use app_core::error::AppResult;

/// 工作流消息推送抽象（由 admin 层对接 PushHub，避免 flow → system 依赖）。
#[async_trait::async_trait]
pub trait FlowNotifier: Send + Sync {
    /// 向指定用户推送工作流站内消息（SSE）。
    async fn notify_users(
        &self,
        user_ids: &[i64],
        message: &str,
        path: Option<&str>,
    ) -> AppResult<()>;
}

/// 默认空实现。
pub struct NoopFlowNotifier;

#[async_trait::async_trait]
impl FlowNotifier for NoopFlowNotifier {
    async fn notify_users(
        &self,
        _user_ids: &[i64],
        _message: &str,
        _path: Option<&str>,
    ) -> AppResult<()> {
        Ok(())
    }
}
