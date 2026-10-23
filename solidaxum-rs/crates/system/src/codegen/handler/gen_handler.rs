use std::{collections::HashMap, sync::Arc};

use app_core::{
    error::AppError,
    extract::AppJson,
    middleware::extractors::CurrentUser,
    model::login_user::LoginUser,
    vo::{page::TableDataInfo, response::R},
};
use app_data::{DataContext, DataSource};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderValue, header},
};
use validator::Validate;

use crate::{
    app::AppState,
    codegen::model::{
        gen_column::GenTableColumn,
        gen_table::{
            DbTableQueryBo, GenTable, GenTableQueryBo, GenTableUpdateBo,
        },
    },
    permission::guard,
};

/// ctx 请求期构造并透传。codegen 是全
/// 豁免模块(GenTableMapper/GenTableColumnMapper 类级
/// @InterceptorIgnore(dataPermission) —— 代码生成元数据非行级),
/// 读在 repo 内显式 without_row_scope,写由漏斗按 principal 盖审计。
fn request_ctx<'a>(
    ds: &'a DataSource,
    login_user: &LoginUser,
) -> DataContext<'a> {
    ds.context(app_data::Principal::from(login_user))
}

#[tracing::instrument(skip(state, query))]
pub async fn gen_list(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<GenTableQueryBo>,
) -> Result<
    R<TableDataInfo<crate::codegen::model::gen_table::GenTable>>,
    AppError,
> {
    guard::require_permission(&login_user, "tool:gen:list")?;
    let ds = state.infra.data_source();
    let (rows, total) = state
        .services
        .gen_table_service
        .list_page(&request_ctx(&ds, &login_user), &query)
        .await?;
    Ok(R::ok_with_data(TableDataInfo::new(rows, total as i64)))
}

#[tracing::instrument(skip(state))]
pub async fn get_gen_table(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(table_id): Path<i64>,
) -> Result<R<serde_json::Value>, AppError> {
    guard::require_permission(&login_user, "tool:gen:query")?;
    let ds = state.infra.data_source();
    let (table, columns) = state
        .services
        .gen_table_service
        .get_detail(&request_ctx(&ds, &login_user), table_id)
        .await?;
    let map = serde_json::json!({
        "info": table,
        "rows": columns,
    });
    Ok(R::ok_with_data(map))
}

#[tracing::instrument(skip(state, query))]
pub async fn db_list(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<DbTableQueryBo>,
) -> Result<
    R<TableDataInfo<crate::codegen::model::gen_table::DbTableVo>>,
    AppError,
> {
    guard::require_permission(&login_user, "tool:gen:list")?;
    let ds = state.infra.data_source();
    let (rows, total) = state
        .services
        .gen_table_service
        .list_db_tables(&request_ctx(&ds, &login_user), &query)
        .await?;
    Ok(R::ok_with_data(TableDataInfo::new(rows, total as i64)))
}

#[tracing::instrument(skip(state))]
pub async fn column_list(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(table_id): Path<i64>,
) -> Result<
    R<TableDataInfo<crate::codegen::model::gen_column::GenTableColumn>>,
    AppError,
> {
    guard::require_permission(&login_user, "tool:gen:list")?;
    let ds = state.infra.data_source();
    let columns = state
        .services
        .gen_table_service
        .list_columns(&request_ctx(&ds, &login_user), table_id)
        .await?;
    let total = columns.len() as i64;
    Ok(R::ok_with_data(TableDataInfo::new(columns, total)))
}

#[tracing::instrument(skip(state, params))]
pub async fn import_table(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&login_user, "tool:gen:import")?;
    let tables = params.get("tables").cloned().unwrap_or_default();
    let data_name = params
        .get("dataName")
        .cloned()
        .unwrap_or_else(|| "master".to_string());

    if tables.is_empty() {
        return Err(AppError::BadRequest("表名不能为空".to_string()));
    }
    let table_names: Vec<String> = tables
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let ds = state.infra.data_source();
    state
        .services
        .gen_table_service
        .import_tables(&request_ctx(&ds, &login_user), &table_names, &data_name)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state, bo))]
pub async fn edit_save(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<GenTableUpdateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&login_user, "tool:gen:edit")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    state
        .services
        .gen_table_service
        .update(&request_ctx(&ds, &login_user), bo)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state))]
pub async fn remove(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(table_ids): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&login_user, "tool:gen:remove")?;
    let ids: Vec<i64> = table_ids
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();
    if ids.is_empty() {
        return Err(AppError::BadRequest("ID 不能为空".to_string()));
    }
    let ds = state.infra.data_source();
    state
        .services
        .gen_table_service
        .delete_by_ids(&request_ctx(&ds, &login_user), &ids)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state))]
pub async fn preview(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(table_id): Path<i64>,
) -> Result<R<HashMap<String, String>>, AppError> {
    guard::require_permission(&login_user, "tool:gen:preview")?;
    let ds = state.infra.data_source();
    let (mut table, columns) = state
        .services
        .gen_table_service
        .get_detail(&request_ctx(&ds, &login_user), table_id)
        .await?;
    table.pk_column = columns.iter().find(|c| c.is_pk()).cloned();
    let result = state
        .services
        .code_gen_engine
        .preview_code(&table, &columns)?;
    Ok(R::ok_with_data(result))
}

#[tracing::instrument(skip(state))]
pub async fn download(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(table_id): Path<i64>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guard::require_permission(&login_user, "tool:gen:code")?;
    let ds = state.infra.data_source();
    let (mut table, columns) = state
        .services
        .gen_table_service
        .get_detail(&request_ctx(&ds, &login_user), table_id)
        .await?;
    table.pk_column = columns.iter().find(|c| c.is_pk()).cloned();
    let data = state
        .services
        .code_gen_engine
        .download_code(&table, &columns)?;

    let content_length = data.len().to_string();
    let headers = [
        (
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream; charset=UTF-8"),
        ),
        (
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=\"ruoyi.zip\""),
        ),
        (
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&content_length)
                .expect("usize 字符串必为合法 HeaderValue"),
        ),
        (
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        ),
        (
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            HeaderValue::from_static("Content-Disposition"),
        ),
    ];
    Ok((headers, data))
}

#[tracing::instrument(skip(state, params))]
pub async fn batch_gen_code(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    guard::require_permission(&login_user, "tool:gen:code")?;
    let table_id_str = params.get("tableIdStr").cloned().unwrap_or_default();
    let ids: Vec<i64> = table_id_str
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();

    let ds = state.infra.data_source();
    let ctx = request_ctx(&ds, &login_user);
    let mut pairs: Vec<(GenTable, Vec<GenTableColumn>)> = Vec::new();
    for id in &ids {
        let (mut table, columns) = state
            .services
            .gen_table_service
            .get_detail(&ctx, *id)
            .await?;
        table.pk_column = columns.iter().find(|c| c.is_pk()).cloned();
        pairs.push((table, columns));
    }

    let table_refs: Vec<(&GenTable, &[GenTableColumn])> =
        pairs.iter().map(|(t, c)| (t, c.as_slice())).collect();
    let all_data = state
        .services
        .code_gen_engine
        .batch_download_code(&table_refs)?;

    let content_length = all_data.len().to_string();
    let headers = [
        (
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream; charset=UTF-8"),
        ),
        (
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=\"ruoyi.zip\""),
        ),
        (
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&content_length)
                .expect("usize 字符串必为合法 HeaderValue"),
        ),
        (
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        ),
        (
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            HeaderValue::from_static("Content-Disposition"),
        ),
    ];
    Ok((headers, all_data))
}

#[tracing::instrument(skip(state))]
pub async fn synch_db(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(table_id): Path<i64>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&login_user, "tool:gen:edit")?;
    let ds = state.infra.data_source();
    state
        .services
        .gen_table_service
        .synch_db(&request_ctx(&ds, &login_user), table_id)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(_state))]
/// 获取已注册数据源名称列表。
/// 当前单数据源设计,直接返回 `["master"]`。
/// 多数据源支持为未来扩展点(audit #84)。
pub async fn get_data_names(
    State(_state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<Vec<String>>, AppError> {
    guard::require_permission(&login_user, "tool:gen:list")?;
    Ok(R::ok_with_data(vec!["master".to_string()]))
}
