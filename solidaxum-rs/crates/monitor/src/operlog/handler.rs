use std::sync::Arc;

use app_core::{
    error::AppError,
    middleware::extractors::CurrentUser,
    util::excel::ExcelBuilder,
    vo::{page::TableDataInfo, response::R},
};
use axum::{
    body::Body,
    extract::{Form, Path, Query, State},
    http::{HeaderMap, HeaderValue, header},
};
use system::permission::guard;

use super::model::{OperLogExportRow, OperLogQuery};
use crate::OperLogState;

#[tracing::instrument(skip(state))]
pub async fn list_oper_log(
    State(state): State<Arc<OperLogState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<OperLogQuery>,
) -> Result<R<TableDataInfo<crate::operlog::model::OperLog>>, AppError> {
    guard::require_permission(&login_user, "monitor:operlog:list")?;
    // ctx 请求期构造(system 各模块同款;operlog 全豁免,
    // 读不解析 resolver,写无审计列可盖)。
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    let result = state.services.oper_log_service.page(&ctx, query).await?;
    Ok(R::ok_with_data(result))
}

/// 导出操作日志 xlsx POST /monitor/operlog/export
/// 对齐 Java SysOperlogController.export:56-60。
#[tracing::instrument(skip(state, login_user))]
pub async fn export_oper_log(
    State(state): State<Arc<OperLogState>>,
    CurrentUser(login_user): CurrentUser,
    // Vue download() 发 application/x-www-form-urlencoded(见 request.ts:259),
    // 故用 Form 提取(与 GET list 的 Query 同走 serde_urlencoded),不能用 AppJson。
    Form(query): Form<OperLogQuery>,
) -> Result<(HeaderMap, Body), AppError> {
    guard::require_permission(&login_user, "monitor:operlog:export")?;
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    let list = state
        .services
        .oper_log_service
        .list_all(&ctx, query)
        .await?;
    let rows: Vec<OperLogExportRow> =
        list.iter().map(OperLogExportRow::from_oper_log).collect();
    let bytes = ExcelBuilder::<OperLogExportRow>::export(&rows)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"operlog.xlsx\""),
    );
    Ok((headers, Body::from(bytes)))
}

#[tracing::instrument(skip(state), fields(oper_ids = ids))]
pub async fn delete_oper_log(
    State(state): State<Arc<OperLogState>>,
    CurrentUser(login_user): CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&login_user, "monitor:operlog:remove")?;
    let id_list: Vec<i64> = ids
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<i64>().map_err(|_| {
                AppError::BadRequest(format!("无效的操作日志ID: {}", s))
            })
        })
        .collect::<Result<_, _>>()?;
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    state
        .services
        .oper_log_service
        .delete_by_ids(&ctx, id_list)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state))]
pub async fn clean_oper_log(
    State(state): State<Arc<OperLogState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<()>, AppError> {
    guard::require_permission(&login_user, "monitor:operlog:remove")?;
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    state.services.oper_log_service.clean_all(&ctx).await?;
    Ok(R::ok())
}
