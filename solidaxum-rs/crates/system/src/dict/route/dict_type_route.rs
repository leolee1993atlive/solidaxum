use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, post},
};

use crate::{
    app::AppState,
    dict::handler::dict_type_handler::{
        create, delete as delete_dict_type, export, get_by_id, optionselect,
        page, refresh_cache, update,
    },
};

pub fn dict_type_route() -> Router<Arc<AppState>> {
    Router::new()
        .route("/system/dict/type/list", get(page))
        .route("/system/dict/type/optionselect", get(optionselect))
        .route("/system/dict/type/export", post(export))
        .route("/system/dict/type/refreshCache", delete(refresh_cache))
        .route(
            "/system/dict/type/{id}",
            get(get_by_id).delete(delete_dict_type),
        )
        .route("/system/dict/type", post(create).put(update))
}
