use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post, put},
};

use crate::{
    app::AppState,
    role::handler::role_handler::{
        allocated_user_list, cancel_auth_user, cancel_auth_users,
        change_status, create_role, delete_role, export_role, get_role_by_id,
        get_role_dept_tree, list_role, list_role_page, option_select,
        select_auth_users, unallocated_user_list, update_role,
        update_role_permission,
    },
};

pub fn role_route() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/system/role",
            get(list_role).post(create_role).put(update_role),
        )
        .route("/system/role/page", get(list_role_page))
        .route("/system/role/list", get(list_role_page))
        .route("/system/role/{id}", get(get_role_by_id).delete(delete_role))
        .route("/system/role/permission", put(update_role_permission))
        .route("/system/role/deptTree/{role_id}", get(get_role_dept_tree))
        .route("/system/role/changeStatus", put(change_status))
        .route("/system/role/export", post(export_role))
        .route("/system/role/optionselect", get(option_select))
        .route(
            "/system/role/authUser/allocatedList",
            get(allocated_user_list),
        )
        .route(
            "/system/role/authUser/unallocatedList",
            get(unallocated_user_list),
        )
        .route("/system/role/authUser/cancel", put(cancel_auth_user))
        .route("/system/role/authUser/cancelAll", put(cancel_auth_users))
        .route("/system/role/authUser/selectAll", put(select_auth_users))
}
