use std::sync::Arc;

use app_core::{
    error::AppError, extract::AppJson, middleware::extractors::CurrentUser,
    vo::response::R,
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, header},
};
use validator::Validate;

use crate::{
    handler::{FlowState, user_ctx},
    model::dto::{
        CategoryAddBo, CategoryEditBo, CategoryQueryBo, CategoryTreeVo,
        CategoryVo,
    },
};

#[tracing::instrument(skip(state, login_user, query))]
pub async fn list_category(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<CategoryQueryBo>,
) -> Result<R<Vec<CategoryVo>>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:category:list",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let rows = state
        .category_service
        .list_all_filtered(&dctx, query.category_name.as_deref())
        .await?;
    let vos: Vec<CategoryVo> = rows.iter().map(CategoryVo::from).collect();
    Ok(R::ok_with_data(vos))
}

#[tracing::instrument(skip(state, login_user, _query))]
pub async fn export_category(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(_query): AppJson<CategoryQueryBo>,
) -> Result<(HeaderMap, Body), AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:category:export",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let rows = state.category_service.list_all(&dctx).await?;
    let vos: Vec<CategoryVo> = rows.iter().map(CategoryVo::from).collect();
    let json = serde_json::to_string(&vos)
        .map_err(|e| AppError::Internal(format!("序列化失败: {}", e)))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static(
            "attachment; filename=\"category_export.json\"",
        ),
    );
    Ok((headers, Body::from(json)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn get_category(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<CategoryVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:category:query",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let model = state
        .category_service
        .get_by_id(&dctx, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("分类不存在 (id={})", id)))?;
    Ok(R::ok_with_data(CategoryVo::from(&model)))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn add_category(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<CategoryAddBo>,
) -> Result<R<CategoryVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:category:add",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    bo.validate()?;
    let model = state
        .category_service
        .add(&dctx, bo, &CurrentUser(login_user))
        .await?;
    Ok(R::ok_with_data(CategoryVo::from(&model)))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn edit_category(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<CategoryEditBo>,
) -> Result<R<CategoryVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:category:edit",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    bo.validate()?;
    let model = state
        .category_service
        .edit(&dctx, bo, &CurrentUser(login_user))
        .await?;
    Ok(R::ok_with_data(CategoryVo::from(&model)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn delete_category(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:category:remove",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    state
        .category_service
        .delete(&dctx, id, &CurrentUser(login_user))
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state, login_user))]
pub async fn category_tree(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<Vec<CategoryTreeVo>>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:category:list",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let rows = state.category_service.list_all(&dctx).await?;
    let tree = CategoryTreeVo::build_tree(
        &rows,
        crate::service::category_service::ROOT_PARENT_ID,
    );
    Ok(R::ok_with_data(tree))
}
