use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, post},
};

use crate::{
    app::AppState,
    menu::handler::menu_handler::{
        add_menu, delete_menu, delete_menu_cascade, edit_menu, get_menu,
        get_routers, list_menu, role_menu_treeselect, treeselect,
    },
};

pub fn menu_route() -> Router<Arc<AppState>> {
    Router::new()
        .route("/system/menu/getRouters", get(get_routers))
        .route("/system/menu/list", get(list_menu))
        .route("/system/menu/treeselect", get(treeselect))
        .route(
            "/system/menu/roleMenuTreeselect/{role_id}",
            get(role_menu_treeselect),
        )
        .route("/system/menu", post(add_menu).put(edit_menu))
        .route(
            "/system/menu/cascade/{menu_ids}",
            delete(delete_menu_cascade),
        )
        .route("/system/menu/{menu_id}", get(get_menu).delete(delete_menu))
}
