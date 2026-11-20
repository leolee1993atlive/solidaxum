use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post, put},
};

use crate::{
    app::AppState,
    notice::handler::notice_handler::{
        create_notice, delete_notice, get_notice_by_id, list_my_notice,
        list_notice_page, mark_all_notice_read, mark_notice_read,
        publish_notice, update_notice,
    },
};

pub fn notice_route() -> Router<Arc<AppState>> {
    Router::new()
        .route("/system/notice/list", get(list_notice_page))
        .route("/system/notice/my", get(list_my_notice))
        .route("/system/notice/readAll", put(mark_all_notice_read))
        .route("/system/notice/{id}/read", put(mark_notice_read))
        .route("/system/notice/{id}/publish", post(publish_notice))
        .route(
            "/system/notice/{id}",
            get(get_notice_by_id).delete(delete_notice),
        )
        .route("/system/notice", post(create_notice).put(update_notice))
}
