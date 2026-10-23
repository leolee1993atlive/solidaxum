use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    app::AppState,
    notice::handler::notice_handler::{
        create_notice, delete_notice, get_notice_by_id, list_notice_page,
        update_notice,
    },
};

pub fn notice_route() -> Router<Arc<AppState>> {
    Router::new()
        .route("/system/notice/list", get(list_notice_page))
        .route(
            "/system/notice/{id}",
            get(get_notice_by_id).delete(delete_notice),
        )
        .route("/system/notice", post(create_notice).put(update_notice))
}
