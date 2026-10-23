use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post, put},
};

use crate::{
    app::AppState,
    client::handler::client_handler::{
        change_status, create_client, delete_client, export_client,
        get_client_by_id, list_client, update_client,
    },
};

pub fn client_route() -> Router<Arc<AppState>> {
    Router::new()
        .route("/system/client/list", get(list_client))
        .route(
            "/system/client/{ids}",
            get(get_client_by_id).delete(delete_client),
        )
        .route("/system/client", post(create_client).put(update_client))
        .route("/system/client/changeStatus", put(change_status))
        .route("/system/client/export", post(export_client))
}
