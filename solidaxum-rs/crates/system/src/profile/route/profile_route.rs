use std::sync::Arc;

use axum::{
    Router,
    routing::{get, put},
};

use crate::{
    app::AppState,
    profile::handler::profile_handler::{
        get_profile, update_password, update_profile,
    },
};

pub fn profile_route() -> Router<Arc<AppState>> {
    Router::new()
        .route("/system/user/profile", get(get_profile).put(update_profile))
        .route("/system/user/profile/updatePwd", put(update_password))
}
