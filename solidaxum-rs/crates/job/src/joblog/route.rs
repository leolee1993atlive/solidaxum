use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get},
};

use super::handler::{
    JobLogHandlerState, clean_logs, delete_logs, get_log, list_logs,
};

pub fn job_log_route(state: Arc<JobLogHandlerState>) -> Router {
    Router::new()
        .route("/list", get(list_logs))
        .route("/clean", delete(clean_logs))
        // GET 详情与 DELETE 批量共用同一路径段(axum 0.8 限制),Path 按位置提取。
        .route("/{jobLogId}", get(get_log).delete(delete_logs))
        .with_state(state)
}
