use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    app::AppState,
    dict::handler::dict_data_handler::{
        create, delete as delete_dict_data, export, get_by_id,
        get_dict_data_by_type, page, update,
    },
};

pub fn dict_data_route() -> Router<Arc<AppState>> {
    Router::new()
        .route("/system/dict/data/list", get(page))
        .route("/system/dict/data/export", post(export))
        .route(
            "/system/dict/data/type/{dict_type}",
            get(get_dict_data_by_type),
        )
        .route(
            "/system/dict/data/{id}",
            get(get_by_id).delete(delete_dict_data),
        )
        .route("/system/dict/data", post(create).put(update))
}
