//! 节点配置单点更新 handler(对齐参考实现 FlowNodeConfController,
//! 路径按本仓 workflow 前缀定为 /workflow/nodeConf/*)。
//!
//! 清单查询给表达式构造器提供下拉数据源;两个 PUT 走显式 set 单字段
//! 更新(支持置空),与整图重建 save-json 互补。
use std::sync::Arc;

use app_core::{
    error::AppError, extract::AppJson, middleware::extractors::CurrentUser,
    vo::response::R,
};
use axum::extract::{Query, State};
use validator::Validate;

use crate::{
    handler::{FlowState, user_ctx},
    model::dto::{
        NodeConfPermissionBo, NodeConfQuery, NodeConfVo, SkipConfConditionBo,
        SkipConfVo,
    },
};

#[tracing::instrument(skip(state, login_user, query))]
pub async fn list_nodes(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<NodeConfQuery>,
) -> Result<R<Vec<NodeConfVo>>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:nodeConf:list",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let nodes = state
        .def_service
        .node_conf_nodes(&dctx, query.definition_id)
        .await?;
    let vos: Vec<NodeConfVo> = nodes.iter().map(NodeConfVo::from).collect();
    Ok(R::ok_with_data(vos))
}

#[tracing::instrument(skip(state, login_user, query))]
pub async fn list_skips(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<NodeConfQuery>,
) -> Result<R<Vec<SkipConfVo>>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:nodeConf:list",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let skips = state
        .def_service
        .node_conf_skips(&dctx, query.definition_id)
        .await?;
    let vos: Vec<SkipConfVo> = skips.iter().map(SkipConfVo::from).collect();
    Ok(R::ok_with_data(vos))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn update_permission_flag(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<NodeConfPermissionBo>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:nodeConf:edit",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    bo.validate()?;
    state
        .def_service
        .update_permission_flag(
            &dctx,
            bo.definition_id,
            &bo.node_code,
            bo.permission_flag,
        )
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn update_skip_condition(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<SkipConfConditionBo>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:nodeConf:edit",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    bo.validate()?;
    state
        .def_service
        .update_skip_condition(&dctx, bo.skip_id, bo.skip_condition)
        .await?;
    Ok(R::ok())
}
