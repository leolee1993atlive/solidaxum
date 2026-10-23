use std::sync::Arc;

use axum::{Router, routing::get};

use crate::{
    app::AppState, message::handler::message_handler::get_message_box,
};

pub fn message_route() -> Router<Arc<AppState>> {
    Router::new().route("/resource/message/box", get(get_message_box))
}
