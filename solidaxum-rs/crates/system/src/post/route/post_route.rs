use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    app::AppState,
    post::handler::post_handler::{
        create_post, delete_post, dept_tree, export_post, get_post_by_id,
        list_post, option_select, update_post,
    },
};

pub fn post_route() -> Router<Arc<AppState>> {
    Router::new()
        .route("/system/post/list", get(list_post))
        .route("/system/post/optionselect", get(option_select))
        .route("/system/post/deptTree", get(dept_tree))
        .route("/system/post/export", post(export_post))
        .route("/system/post/{id}", get(get_post_by_id).delete(delete_post))
        .route("/system/post", post(create_post).put(update_post))
}
