use std::sync::Arc;

use axum::{Router, routing::get};

use super::handler::get_cache_info;
use crate::{OperLogState, middleware::oper_log::OperLogLayer};

pub fn cache_route(oper_log_layer: OperLogLayer) -> Router<Arc<OperLogState>> {
    Router::new()
        .route("/monitor/cache", get(get_cache_info))
        .layer(oper_log_layer)
}
