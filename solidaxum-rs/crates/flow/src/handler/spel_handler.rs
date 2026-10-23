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
    model::dto::{SpelAddBo, SpelEditBo, SpelVo},
};

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpelPageQuery {
    #[serde(flatten)]
    pub page: PageQuery,
    pub component_name: Option<String>,
    pub method_name: Option<String>,
    pub status: Option<String>,
}

#[tracing::instrument(skip(state, login_user, query))]
pub async fn list_spel(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<SpelPageQuery>,
) -> Result<R<PageResult<SpelVo>>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:spel:list",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let (page_no, page_size, _offset) =
        app_core::vo::page::normalize_paging(&query.page);
    let page = state
        .spel_service
        .list_page(
            &dctx,
            query.component_name.as_deref(),
            query.method_name.as_deref(),
            query.status.as_deref(),
            page_no as u64,
            page_size as u64,
        )
        .await?;
    let vos: Vec<SpelVo> = page.rows.iter().map(SpelVo::from).collect();
    Ok(R::ok_with_data(PageResult::from_page_rows(&page, vos)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn get_spel(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<SpelVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:spel:query",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let model =
        state
            .spel_service
            .get_by_id(&dctx, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("表达式不存在 (id={})", id))
            })?;
    Ok(R::ok_with_data(SpelVo::from(&model)))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn add_spel(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<SpelAddBo>,
) -> Result<R<SpelVo>, AppError> {
    app_core::permission::require_permission(&login_user, "workflow:spel:add")?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    bo.validate()?;
    let model = state
        .spel_service
        .add(&dctx, bo, &CurrentUser(login_user))
        .await?;
    Ok(R::ok_with_data(SpelVo::from(&model)))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn edit_spel(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<SpelEditBo>,
) -> Result<R<SpelVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:spel:edit",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    bo.validate()?;
    let model = state
        .spel_service
        .edit(&dctx, bo, &CurrentUser(login_user))
        .await?;
    Ok(R::ok_with_data(SpelVo::from(&model)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn delete_spel(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:spel:remove",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    for id_str in ids.split(',') {
        if id_str.is_empty() {
            continue;
        }
        let id: i64 = id_str.parse().map_err(|_| {
            AppError::BadRequest(format!("无效的ID: {}", id_str))
        })?;
        state
            .spel_service
            .delete(&dctx, id, &CurrentUser(login_user.clone()))
            .await?;
    }
    Ok(R::ok())
}
