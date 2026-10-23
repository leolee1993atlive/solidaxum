use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::{
    app::AppState,
    config::handler::config_handler::{
        create, delete as delete_config, export_config, get_by_id,
        get_config_by_key, page, refresh_cache, sys_info, update,
        update_config_by_key,
    },
};

pub fn config_route() -> Router<Arc<AppState>> {
    Router::new()
        .route("/system/config/list", get(page))
        .route("/system/config/configKey/{key}", get(get_config_by_key))
        .route("/system/config/sysInfo", get(sys_info))
        .route("/system/config/updateByKey", put(update_config_by_key))
        .route("/system/config/refreshCache", delete(refresh_cache))
        .route("/system/config/export", post(export_config))
        .route("/system/config/{id}", get(get_by_id).delete(delete_config))
        .route("/system/config", post(create).put(update))
}
