use std::sync::Arc;

use app_core::{
    error::AppError,
    extract::AppJson,
    middleware::extractors::CurrentUser,
    util::excel::ExcelBuilder,
    vo::{page::TableDataInfo, response::R},
};
use axum::{
    body::Body,
    extract::{Form, Path, Query, State},
    http::{HeaderMap, HeaderValue, header},
};
use validator::Validate;

use crate::{
    app::AppState,
    client::model::client::{
        ClientChangeStatusBo, ClientCreateBo, ClientExportRow, ClientListVo,
        ClientPageQuery, ClientUpdateBo,
    },
    permission::guard,
};

#[tracing::instrument(skip(state), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
pub async fn list_client(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<ClientPageQuery>,
) -> Result<R<TableDataInfo<ClientListVo>>, AppError> {
    guard::require_permission(&login_user, "system:client:list")?;
    // ctx 请求期构造;client 是全豁免模块(SysClientMapper 无
    // @DataPermission),读在 repo 内显式 without_row_scope。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let result = state.services.client_service.page(&ctx, query).await?;
    Ok(R::ok_with_data(result))
}

#[tracing::instrument(skip(state), fields(client_id = id))]
pub async fn get_client_by_id(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<ClientListVo>, AppError> {
    guard::require_permission(&login_user, "system:client:query")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let client = state.services.client_service.get_by_id(&ctx, id).await?;
    Ok(R::ok_with_data(client.to_list_vo()))
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, client_key = ?bo.client_key)
)]
pub async fn create_client(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<ClientCreateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:client:add")?;
    bo.validate()?;
    // 审计盖章人取 ctx 的 principal(漏斗 insert 统一盖)
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .client_service
        .insert_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, client_id = ?bo.id)
)]
pub async fn update_client(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<ClientUpdateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:client:edit")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .client_service
        .update_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state),
    fields(operator = %current_user.0.username, client_ids = ids)
)]
pub async fn delete_client(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:client:remove")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    let mut id_list: Vec<i64> = ids
        .split(',')
        .map(|s| {
            s.trim().parse::<i64>().map_err(|_| {
                AppError::BadRequest(format!(
                    "删除客户端失败: 无效的客户端ID: {}",
                    s
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if id_list.is_empty() {
        return Err(AppError::BadRequest(
            "删除客户端失败: 客户端ID为空".into(),
        ));
    }
    id_list.sort();
    id_list.dedup();
    for id in id_list {
        state.services.client_service.delete_by_id(&ctx, id).await?;
    }
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, client_id = %bo.client_id)
)]
pub async fn change_status(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<ClientChangeStatusBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:client:edit")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .client_service
        .change_status(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

/// 导出客户端 xlsx POST /system/client/export
/// 对齐 Java SysClientController.export。**刻意不含 clientSecret**。
#[tracing::instrument(skip(state, login_user))]
pub async fn export_client(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    // Vue download() 发 application/x-www-form-urlencoded(见 request.ts:259),
    // 故用 Form 提取(与 GET list 的 Query 同走 serde_urlencoded),不能用 AppJson。
    Form(query): Form<ClientPageQuery>,
) -> Result<(HeaderMap, Body), AppError> {
    guard::require_permission(&login_user, "system:client:export")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let list = state.services.client_service.list_all(&ctx, query).await?;
    let rows: Vec<ClientExportRow> =
        list.iter().map(ClientExportRow::from_client).collect();
    let bytes = ExcelBuilder::<ClientExportRow>::export(&rows)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"client.xlsx\""),
    );
    Ok((headers, Body::from(bytes)))
}
