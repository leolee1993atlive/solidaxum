use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post, put},
};

use crate::{
    app::AppState,
    user::handler::user_handler::{
        auth_role_assign, change_status, create_user, delete_user, dept_tree,
        export_user, get_add_user_info, get_auth_role, get_info,
        get_user_by_id, get_user_info, import_data, import_template, list_user,
        list_user_by_dept, list_user_page, option_select, reset_password,
        unlock, update_password, update_user,
    },
};

pub fn user_route() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/system/user",
            get(list_user).post(create_user).put(update_user),
        )
        .route("/system/user/page", get(list_user_page))
        .route("/system/user/list", get(list_user_page))
        .route("/system/user/list/dept/{dept_id}", get(list_user_by_dept))
        // 新建表单初始化(无 userId,末尾斜杠);对齐 Java @GetMapping({"/","/{userId}"})
        .route("/system/user/", get(get_add_user_info))
        .route("/system/user/{id}", get(get_user_by_id).delete(delete_user))
        .route("/system/user/password", put(update_password))
        .route("/system/user/resetPwd", put(reset_password))
        .route("/system/user/changeStatus", put(change_status))
        .route("/system/user/unlock/{user_id}", get(unlock))
        .route("/system/user/optionselect", get(option_select))
        .route("/system/user/deptTree", get(dept_tree))
        .route("/system/user/importData", post(import_data))
        .route("/system/user/importTemplate", get(import_template))
        .route("/system/user/export", post(export_user))
        .route("/system/user/authRole/{user_id}", get(get_auth_role))
        .route("/system/user/authRole", put(auth_role_assign))
        .route("/system/user/info", get(get_user_info))
        .route("/system/user/getInfo", get(get_info))
}
