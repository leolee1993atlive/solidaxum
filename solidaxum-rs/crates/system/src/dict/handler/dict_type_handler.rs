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
    dict::model::dict_type::{
        DictTypeCreateBo, DictTypeExportRow, DictTypeListVo, DictTypePageQuery,
        DictTypeUpdateBo,
    },
    permission::guard,
};

/// 字典类型分页查询 GET /system/dict/type/list
#[tracing::instrument(skip(state), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
pub async fn page(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<DictTypePageQuery>,
) -> Result<R<TableDataInfo<DictTypeListVo>>, AppError> {
    guard::require_permission(&login_user, "system:dict:list")?;
    // ctx 请求期构造;dict 是全豁免模块(SysDictTypeMapper 无
    // @DataPermission),读在 repo 内显式 without_row_scope,ctx 只承载
    // 审计 principal(写路径用)。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let result = state.services.dict_type_service.page(&ctx, query).await?;
    Ok(R::ok_with_data(result))
}

/// 字典类型下拉选择 GET /system/dict/type/optionselect
#[tracing::instrument(skip(state))]
pub async fn optionselect(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<Vec<DictTypeListVo>>, AppError> {
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let list = state.services.dict_type_service.list(&ctx).await?;
    Ok(R::ok_with_data(list))
}

/// 根据ID查询字典类型 GET /system/dict/type/{id}
#[tracing::instrument(skip(state), fields(dict_id = id))]
pub async fn get_by_id(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<DictTypeListVo>, AppError> {
    guard::require_permission(&login_user, "system:dict:query")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let dict_type =
        state.services.dict_type_service.get_by_id(&ctx, id).await?;
    Ok(R::ok_with_data(dict_type.to_list_vo()))
}

/// 新增字典类型 POST /system/dict/type
#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, dict_type = ?bo.dict_type)
)]
pub async fn create(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<DictTypeCreateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:dict:add")?;
    bo.validate()?;
    // 审计盖章人取 ctx 的 principal(漏斗 insert 统一盖)
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .dict_type_service
        .insert_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

/// 修改字典类型 PUT /system/dict/type
#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, dict_id = ?bo.dict_id)
)]
pub async fn update(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<DictTypeUpdateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:dict:edit")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .dict_type_service
        .update_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

/// 删除字典类型 DELETE /system/dict/type/{ids}
#[tracing::instrument(
    skip(state),
    fields(operator = %current_user.0.username, dict_ids = ids)
)]
pub async fn delete(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:dict:remove")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    for id_str in ids.split(',') {
        if id_str.is_empty() {
            continue;
        }
        let id: i64 = id_str.parse().map_err(|_| {
            AppError::BadRequest(format!(
                "删除字典类型失败: 无效的字典类型ID: {}",
                id_str
            ))
        })?;
        state
            .services
            .dict_type_service
            .delete_by_id(&ctx, id, &current_user)
            .await?;
    }
    Ok(R::ok())
}

/// 刷新字典缓存 DELETE /system/dict/type/refreshCache
#[tracing::instrument(skip(state), fields(operator = %login_user.username))]
pub async fn refresh_cache(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<()>, AppError> {
    guard::require_permission(&login_user, "system:dict:remove")?;
    state.services.dict_type_service.refresh_cache().await?;
    Ok(R::ok())
}

/// 导出字典类型 xlsx POST /system/dict/type/export
/// 对齐 Java SysDictTypeController.export:56-62。
#[tracing::instrument(skip(state, login_user))]
pub async fn export(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    // Vue download() 发 application/x-www-form-urlencoded(见 request.ts:259),
    // 故用 Form 提取(与 GET list 的 Query 同走 serde_urlencoded),不能用 AppJson。
    Form(_query): Form<DictTypePageQuery>,
) -> Result<(HeaderMap, Body), AppError> {
    guard::require_permission(&login_user, "system:dict:export")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let list = state.services.dict_type_service.list(&ctx).await?;
    let rows: Vec<DictTypeExportRow> = list
        .iter()
        .map(|v| DictTypeExportRow {
            dict_id: v.dict_id,
            dict_name: v.dict_name.clone(),
            dict_type: v.dict_type.clone(),
            remark: v.remark.clone().unwrap_or_default(),
            create_time: v
                .base_vo
                .create_time
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
        })
        .collect();
    let bytes = ExcelBuilder::<DictTypeExportRow>::export(&rows)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"dict_type.xlsx\""),
    );
    Ok((headers, Body::from(bytes)))
}
