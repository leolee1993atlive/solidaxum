use std::sync::Arc;

use axum::{
    Extension, Router,
    routing::{delete, get},
};

use super::handler::{
    batch_logout_removed, delete_my_session, force_logout,
    get_current_user_online_info, list_online_users,
};
use crate::{
    OperLogState,
    middleware::oper_log::{OperLogLayer, OperLogMeta},
    operlog::model::BusinessType,
};

pub fn online_user_route(
    oper_log_layer: OperLogLayer,
) -> Router<Arc<OperLogState>> {
    Router::new()
        .route("/monitor/online/list", get(list_online_users))
        .layer(oper_log_layer.clone())
        .merge(
            Router::new()
                .route("/monitor/online", get(get_current_user_online_info))
                .layer(oper_log_layer.clone()),
        )
        .merge(
            Router::new()
                .route("/monitor/online/{tokenId}", delete(force_logout))
                .layer(oper_log_layer.clone())
                .layer(Extension(OperLogMeta::new(
                    "强退在线用户",
                    BusinessType::FORCE,
                ))),
        )
        .merge(
            Router::new()
                .route(
                    "/monitor/online/myself/{tokenId}",
                    delete(delete_my_session),
                )
                .layer(oper_log_layer.clone()),
        )
        .merge(
            Router::new()
                .route(
                    "/monitor/online/batchLogout",
                    delete(batch_logout_removed),
                )
                .layer(oper_log_layer.clone()),
        )
}
