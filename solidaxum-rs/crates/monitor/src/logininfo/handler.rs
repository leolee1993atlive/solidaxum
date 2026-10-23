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

use super::model::{LoginInfo, LoginInfoExportRow, LoginInfoQuery};
use crate::OperLogState;

#[tracing::instrument(skip(state))]
pub async fn list_login_info(
    State(state): State<Arc<OperLogState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<LoginInfoQuery>,
) -> Result<R<TableDataInfo<LoginInfo>>, AppError> {
    guard::require_permission(&login_user, "monitor:logininfo:list")?;
    // ctx 请求期构造(system 各模块同款;logininfo 全豁免,
    // 读不解析 resolver,写无审计列可盖)。
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    let result = state.services.login_info_service.page(&ctx, query).await?;
    Ok(R::ok_with_data(result))
}

/// 导出登录日志 xlsx POST /monitor/loginInfo/export
/// 对齐 Java SysLoginInfoController.export:59-62。
#[tracing::instrument(skip(state, login_user))]
pub async fn export_login_info(
    State(state): State<Arc<OperLogState>>,
    CurrentUser(login_user): CurrentUser,
    // Vue download() 发 application/x-www-form-urlencoded(见 request.ts:259),
    // 故用 Form 提取(与 GET list 的 Query 同走 serde_urlencoded),不能用 AppJson。
    Form(query): Form<LoginInfoQuery>,
) -> Result<(HeaderMap, Body), AppError> {
    guard::require_permission(&login_user, "monitor:logininfo:export")?;
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    let list = state
        .services
        .login_info_service
        .list_all(&ctx, query)
        .await?;
    let rows: Vec<LoginInfoExportRow> = list
        .iter()
        .map(LoginInfoExportRow::from_login_info)
        .collect();
    let bytes = ExcelBuilder::<LoginInfoExportRow>::export(&rows)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"login_info.xlsx\""),
    );
    Ok((headers, Body::from(bytes)))
}

#[tracing::instrument(skip(state), fields(info_ids = ids))]
pub async fn delete_login_info(
    State(state): State<Arc<OperLogState>>,
    CurrentUser(login_user): CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&login_user, "monitor:logininfo:remove")?;
    let id_list: Vec<i64> = ids
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<i64>().map_err(|_| {
                AppError::BadRequest(format!("无效的登录日志ID: {}", s))
            })
        })
        .collect::<Result<_, _>>()?;
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    state
        .services
        .login_info_service
        .delete_by_ids(&ctx, id_list)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state))]
pub async fn clean_login_info(
    State(state): State<Arc<OperLogState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<()>, AppError> {
    guard::require_permission(&login_user, "monitor:logininfo:remove")?;
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    state.services.login_info_service.clean_all(&ctx).await?;
    Ok(R::ok())
}

#[tracing::instrument(fields(user_name = %user_name))]
pub async fn unlock_user(
    State(state): State<Arc<OperLogState>>,
    CurrentUser(login_user): CurrentUser,
    Path(user_name): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&login_user, "monitor:logininfo:unlock")?;
    let key = format!("pwd_err_cnt:{}", user_name);
    let mut conn = state
        .services
        .online_user_service
        .redis_pool
        .get()
        .await
        .map_err(|e| AppError::Internal(format!("Redis连接失败: {}", e)))?;
    let _: () = redis::cmd("DEL")
        .arg(&key)
        .query_async(&mut *conn)
        .await
        .unwrap_or(());
    tracing::info!(user_name = %user_name, "账户解锁:清除密码错误计数");
    Ok(R::ok())
}
