use std::sync::Arc;

use app_core::{
    error::AppError,
    extract::AppJson,
    middleware::extractors::CurrentUser,
    util::excel::{ExcelBuilder, guard_export_rows},
    vo::{
        page::{PageQuery, PageResult},
        response::R,
    },
};
use axum::{
    body::Body,
    extract::{Form, Path, Query, State},
    http::{HeaderMap, HeaderValue, header},
};
use serde::Deserialize;
use validator::Validate;

use crate::{
    handler::{FlowState, user_ctx},
    model::dto::{
        LeaveAddBo, LeaveEditBo, LeaveExportRow, LeaveQueryBo, LeaveVo,
    },
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeavePageQuery {
    #[serde(flatten)]
    pub page: PageQuery,
    pub leave_type: Option<String>,
    pub status: Option<String>,
    pub start_leave_days: Option<i32>,
    pub end_leave_days: Option<i32>,
}

#[tracing::instrument(skip(state, login_user, query))]
pub async fn list_leave(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<LeavePageQuery>,
) -> Result<R<PageResult<LeaveVo>>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:leave:list",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let (page_no, page_size, _offset) =
        app_core::vo::page::normalize_paging(&query.page);
    let page = state
        .leave_service
        .list_page(
            &dctx,
            query.leave_type.as_deref(),
            query.status.as_deref(),
            query.start_leave_days,
            query.end_leave_days,
            page_no as u64,
            page_size as u64,
        )
        .await?;
    let vos: Vec<LeaveVo> = page.rows.iter().map(LeaveVo::from).collect();
    Ok(R::ok_with_data(PageResult::from_page_rows(&page, vos)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn get_leave(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<LeaveVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:leave:query",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let model =
        state
            .leave_service
            .get_by_id(&dctx, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("请假记录不存在 (id={})", id))
            })?;
    Ok(R::ok_with_data(LeaveVo::from(&model)))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn add_leave(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<LeaveAddBo>,
) -> Result<R<LeaveVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:leave:add",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    bo.validate()?;
    let model = state
        .leave_service
        .add(&dctx, bo, &CurrentUser(login_user))
        .await?;
    Ok(R::ok_with_data(LeaveVo::from(&model)))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn edit_leave(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<LeaveEditBo>,
) -> Result<R<LeaveVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:leave:edit",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    bo.validate()?;
    let model = state
        .leave_service
        .edit(&dctx, bo, &CurrentUser(login_user))
        .await?;
    Ok(R::ok_with_data(LeaveVo::from(&model)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn delete_leave(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:leave:remove",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let id_list: Vec<i64> = ids
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    if id_list.is_empty() {
        return Err(AppError::BadRequest("缺少待删除 ID".to_string()));
    }
    state
        .leave_service
        .delete(&dctx, &id_list, &CurrentUser(login_user))
        .await?;
    Ok(R::ok())
}

/// 导出请假 xlsx POST /workflow/leave/export
/// 形状对齐全站其余导出端点:Form 提取(Vue download 发 form-urlencoded)+ xlsx + 吃筛选。
/// 交付前用 AppJson 提取器(Vue 发 form → 415/400)、忽略筛选恒全量、输出 .json —— 全不对。
#[tracing::instrument(skip(state, login_user, query))]
pub async fn export_leave(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Form(query): Form<LeaveQueryBo>,
) -> Result<(HeaderMap, Body), AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:leave:export",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    // 与 list 共用同一套 filter —— 此前写死 None 导致导出恒全量,
    // 与屏幕上看到的不一致。
    //
    // 形状对齐 logininfo / operlog / user 三处导出:**先 COUNT 把住闸门,
    // 再不分页地全取**。这里踩过两个坑,都不报错、只是数据悄悄不对:
    //
    // 1. 闸门读分页的 total —— 分页 total 是上限计数(默认 1 万)而闸门线是
    //    5 万,10000 恒小于 50000,闸门永远放行;
    // 2. 取数走 `select_page(.., 1, EXPORT_ROW_LIMIT)` —— `Paging` 把
    //    page_size 钳在 1..=200,所以不管上限写多大,**实际只取回 200 行**。
    //
    // 两条合起来就是:一张两万行的表点导出,得到一个 200 行的 xlsx,没有任何提示。
    let total = state
        .leave_service
        .count_for_export(
            &dctx,
            query.leave_type.as_deref(),
            query.status.as_deref(),
            query.start_leave_days,
            query.end_leave_days,
        )
        .await?;
    guard_export_rows(total as usize)?;
    let models = state
        .leave_service
        .list_all_for_export(
            &dctx,
            query.leave_type.as_deref(),
            query.status.as_deref(),
            query.start_leave_days,
            query.end_leave_days,
        )
        .await?;
    let vos: Vec<LeaveVo> = models.iter().map(LeaveVo::from).collect();
    let export_rows: Vec<LeaveExportRow> =
        vos.iter().map(LeaveExportRow::from_vo).collect();
    let bytes = ExcelBuilder::<LeaveExportRow>::export(&export_rows)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"leave.xlsx\""),
    );
    Ok((headers, Body::from(bytes)))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn submit_and_start(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<LeaveAddBo>,
) -> Result<R<LeaveVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:leave:add",
    )?;
    bo.validate()?;
    let current_user = CurrentUser(login_user);
    let dctx = crate::handler::user_ctx(&state, &current_user);
    let model = state
        .leave_service
        .submit_and_start(&dctx, bo, &current_user)
        .await?;
    Ok(R::ok_with_data(LeaveVo::from(&model)))
}
