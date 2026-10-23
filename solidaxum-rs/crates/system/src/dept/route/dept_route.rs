use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    app::AppState,
    dept::handler::dept_handler::{
        create_dept, delete_dept, get_dept_by_id, list_dept, list_dept_exclude,
        option_select, update_dept,
    },
};

pub fn dept_route() -> Router<Arc<AppState>> {
    Router::new()
        .route("/system/dept/list", get(list_dept))
        .route("/system/dept/list/exclude/{id}", get(list_dept_exclude))
        .route("/system/dept/optionselect", get(option_select))
        .route("/system/dept/{id}", get(get_dept_by_id).delete(delete_dept))
        .route("/system/dept", post(create_dept).put(update_dept))
}
