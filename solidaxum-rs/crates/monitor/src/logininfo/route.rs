use std::sync::Arc;

use axum::{
    Extension, Router,
    routing::{delete, get, post},
};

use super::handler::{
    clean_login_info, delete_login_info, export_login_info, list_login_info,
    unlock_user,
};
use crate::{
    OperLogState,
    middleware::oper_log::{OperLogLayer, OperLogMeta},
    operlog::model::BusinessType,
};

pub fn login_info_route(
    oper_log_layer: OperLogLayer,
) -> Router<Arc<OperLogState>> {
    Router::new()
        .route("/monitor/loginInfo/list", get(list_login_info))
        .route("/monitor/loginInfo/export", post(export_login_info))
        .layer(oper_log_layer.clone())
        .merge(
            Router::new()
                .route(
                    "/monitor/loginInfo/{infoIds}",
                    delete(delete_login_info),
                )
                .layer(oper_log_layer.clone())
                .layer(Extension(OperLogMeta::new(
                    "删除登录日志",
                    BusinessType::DELETE,
                ))),
        )
        .merge(
            Router::new()
                .route("/monitor/loginInfo/clean", delete(clean_login_info))
                .layer(oper_log_layer.clone())
                .layer(Extension(OperLogMeta::new(
                    "清空登录日志",
                    BusinessType::CLEAN,
                ))),
        )
        .merge(
            Router::new()
                .route("/monitor/loginInfo/unlock/{userName}", get(unlock_user))
                .layer(oper_log_layer.clone())
                .layer(Extension(OperLogMeta::new(
                    "账户解锁",
                    BusinessType::OTHER,
                ))),
        )
}
