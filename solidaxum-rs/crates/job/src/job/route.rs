use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post, put},
};

use super::handler::{
    JobHandlerState, change_status, create_job, delete_jobs, get_job,
    list_jobs, run_job, update_job,
};

pub fn job_route(state: Arc<JobHandlerState>) -> Router {
    Router::new()
        .route("/list", get(list_jobs))
        .route("/", post(create_job).put(update_job))
        .route("/changeStatus", put(change_status))
        .route("/run", put(run_job))
        // GET 详情与 DELETE 批量共用同一路径段(axum 0.8 不允许同位置不同
        // 参数名的两个捕获);Path 按位置提取,handler 参数名无关。
        .route("/{jobId}", get(get_job).delete(delete_jobs))
        .with_state(state)
}
