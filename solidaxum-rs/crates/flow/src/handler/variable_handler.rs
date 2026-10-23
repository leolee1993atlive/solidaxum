use std::sync::Arc;

use app_core::{
    error::AppError,
    extract::AppJson,
    middleware::extractors::CurrentUser,
    vo::{
        page::{PageQuery, PageResult},
        response::R,
    },
};
use axum::extract::{Path, Query, State};
use validator::Validate;

use crate::{
    handler::{FlowState, user_ctx},
    model::dto::{VariableAddBo, VariableEditBo, VariableVo},
};

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariablePageQuery {
    #[serde(flatten)]
    pub page: PageQuery,
    pub variable_name: Option<String>,
    pub label: Option<String>,
    pub status: Option<String>,
}

#[tracing::instrument(skip(state, login_user, query))]
pub async fn list_variable(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<VariablePageQuery>,
) -> Result<R<PageResult<VariableVo>>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:variable:list",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let (page_no, page_size, _offset) =
        app_core::vo::page::normalize_paging(&query.page);
    let page = state
        .variable_service
        .list_page(
            &dctx,
            query.variable_name.as_deref(),
            query.label.as_deref(),
            query.status.as_deref(),
            page_no as u64,
            page_size as u64,
        )
        .await?;
    let vos: Vec<VariableVo> = page.rows.iter().map(VariableVo::from).collect();
    Ok(R::ok_with_data(PageResult::from_page_rows(&page, vos)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn get_variable(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<VariableVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:variable:query",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let model = state
        .variable_service
        .get_by_id(&dctx, id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("流程变量不存在 (id={})", id))
        })?;
    Ok(R::ok_with_data(VariableVo::from(&model)))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn add_variable(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<VariableAddBo>,
) -> Result<R<VariableVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:variable:add",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    bo.validate()?;
    let model = state
        .variable_service
        .add(&dctx, bo, &CurrentUser(login_user))
        .await?;
    Ok(R::ok_with_data(VariableVo::from(&model)))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn edit_variable(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<VariableEditBo>,
) -> Result<R<VariableVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:variable:edit",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    bo.validate()?;
    let model = state
        .variable_service
        .edit(&dctx, bo, &CurrentUser(login_user))
        .await?;
    Ok(R::ok_with_data(VariableVo::from(&model)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn delete_variable(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:variable:remove",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let mut id_list: Vec<i64> = Vec::new();
    for id_str in ids.split(',') {
        if id_str.is_empty() {
            continue;
        }
        let id: i64 = id_str.parse().map_err(|_| {
            AppError::BadRequest(format!("无效的ID: {}", id_str))
        })?;
        id_list.push(id);
    }
    state.variable_service.delete(&dctx, &id_list).await?;
    Ok(R::ok())
}
