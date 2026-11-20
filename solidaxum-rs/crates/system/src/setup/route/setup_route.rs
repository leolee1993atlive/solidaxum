use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    app::AppState,
    setup::handler::setup_handler::{setup_complete, setup_status},
};

pub fn setup_route() -> Router<Arc<AppState>> {
    Router::new()
        // 探测(仅需登录)与聚合完成(仅超管+未完成)。
        .route("/system/setup/status", get(setup_status))
        .route("/system/setup/complete", post(setup_complete))
}
