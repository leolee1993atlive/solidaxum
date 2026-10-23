use std::sync::Arc;

use crate::handler_trait::{JobHandler, JobRegistry};

pub struct SysDefaultJobHandler;

#[async_trait::async_trait]
impl JobHandler for SysDefaultJobHandler {
    async fn execute(&self, params: &str) -> Result<String, String> {
        tracing::info!(params, "SysDefaultJob 执行");
        Ok(format!("SysDefaultJob executed with params: {}", params))
    }
}

/// 注册所有内置 handler
pub fn register_builtin_handlers(registry: &mut JobRegistry) {
    registry.register("SysDefaultJob", Arc::new(SysDefaultJobHandler));
}
