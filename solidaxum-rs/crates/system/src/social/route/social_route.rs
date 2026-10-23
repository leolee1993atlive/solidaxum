use std::sync::Arc;

use axum::{Router, routing::get};

use crate::{
    app::AppState, social::handler::social_handler::list_social_accounts,
};

pub fn social_route() -> Router<Arc<AppState>> {
    Router::new().route("/system/social/list", get(list_social_accounts))
}
