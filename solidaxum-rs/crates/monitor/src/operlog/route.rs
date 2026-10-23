use std::sync::Arc;

use axum::{
    Extension, Router,
    routing::{delete, get, post},
};

use super::handler::{
    clean_oper_log, delete_oper_log, export_oper_log, list_oper_log,
};
use crate::{
    OperLogState,
    middleware::oper_log::{OperLogLayer, OperLogMeta},
    operlog::model::BusinessType,
};

pub fn oper_log_route(
    oper_log_layer: OperLogLayer,
) -> Router<Arc<OperLogState>> {
    Router::new()
        .route("/monitor/operlog/list", get(list_oper_log))
        .route("/monitor/operlog/export", post(export_oper_log))
        .layer(oper_log_layer.clone())
        .merge(
            Router::new()
                .route("/monitor/operlog/{operIds}", delete(delete_oper_log))
                .layer(oper_log_layer.clone())
                .layer(Extension(OperLogMeta::new(
                    "删除操作日志",
                    BusinessType::DELETE,
                ))),
        )
        .merge(
            Router::new()
                .route("/monitor/operlog/clean", delete(clean_oper_log))
                .layer(oper_log_layer.clone())
                .layer(Extension(OperLogMeta::new(
                    "清空操作日志",
                    BusinessType::CLEAN,
                ))),
        )
}
