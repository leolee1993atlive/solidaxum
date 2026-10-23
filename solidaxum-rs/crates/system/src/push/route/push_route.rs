use std::sync::Arc;

use axum::{Router, routing::get};

use crate::{app::AppState, push::sse::sse_handler};

pub fn push_route() -> Router<Arc<AppState>> {
    Router::new().route("/resource/message", get(sse_handler))
}
