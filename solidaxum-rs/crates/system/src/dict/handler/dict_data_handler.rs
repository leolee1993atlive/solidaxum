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
    dict::model::dict_data::{
        DictDataCreateBo, DictDataExportRow, DictDataListVo, DictDataPageQuery,
        DictDataUpdateBo,
    },
    permission::guard,
};

/// 字典数据分页查询 GET /system/dict/data/list
#[tracing::instrument(skip(state), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
pub async fn page(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<DictDataPageQuery>,
) -> Result<R<TableDataInfo<DictDataListVo>>, AppError> {
    guard::require_permission(&login_user, "system:dict:list")?;
    // ctx 请求期构造;dict 是全豁免模块(SysDictDataMapper 无
    // @DataPermission),读在 repo 内显式 without_row_scope。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let result = state.services.dict_data_service.page(&ctx, query).await?;
    Ok(R::ok_with_data(result))
}

/// 根据字典类型查询字典数据 GET /system/dict/data/type/{dict_type}
/// 仅需登录，无需特定权限
#[tracing::instrument(skip(state), fields(dict_type = %dict_type))]
pub async fn get_dict_data_by_type(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(dict_type): Path<String>,
) -> Result<R<Vec<DictDataListVo>>, AppError> {
    // 对位 Java `@Cacheable(SYS_DICT)` selectDictDataByType 的 HTTP 面
    // (ServiceImpl:106):缓存命中不落库,未命中的落库读是豁免读。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let list = state
        .services
        .dict_data_service
        .get_by_type(&ctx, &dict_type)
        .await?;
    Ok(R::ok_with_data(list))
}

/// 根据ID查询字典数据 GET /system/dict/data/{id}
#[tracing::instrument(skip(state), fields(dict_code = id))]
pub async fn get_by_id(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<DictDataListVo>, AppError> {
    guard::require_permission(&login_user, "system:dict:query")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let dict_data =
        state.services.dict_data_service.get_by_id(&ctx, id).await?;
    Ok(R::ok_with_data(dict_data.to_list_vo()))
}

/// 新增字典数据 POST /system/dict/data
#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, dict_type = ?bo.dict_type)
)]
pub async fn create(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<DictDataCreateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:dict:add")?;
    bo.validate()?;
    // 审计盖章人取 ctx 的 principal(漏斗 insert 统一盖)
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .dict_data_service
        .insert_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

/// 修改字典数据 PUT /system/dict/data
#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, dict_code = ?bo.dict_code)
)]
pub async fn update(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<DictDataUpdateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:dict:edit")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .dict_data_service
        .update_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

/// 删除字典数据 DELETE /system/dict/data/{ids}
#[tracing::instrument(
    skip(state),
    fields(operator = %current_user.0.username, dict_codes = ids)
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
                "删除字典数据失败: 无效的字典编码: {}",
                id_str
            ))
        })?;
        state
            .services
            .dict_data_service
            .delete_by_id(&ctx, id, &current_user)
            .await?;
    }
    Ok(R::ok())
}

/// 导出字典数据 xlsx POST /system/dict/data/export
/// 对齐 Java SysDictDataController.export:59-65。
#[tracing::instrument(skip(state, login_user))]
pub async fn export(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    // Vue download() 发 application/x-www-form-urlencoded(见 request.ts:259),
    // 故用 Form 提取(与 GET list 的 Query 同走 serde_urlencoded),不能用 AppJson。
    Form(_query): Form<DictDataPageQuery>,
) -> Result<(HeaderMap, Body), AppError> {
    guard::require_permission(&login_user, "system:dict:export")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let list = state.services.dict_data_service.list_all(&ctx).await?;
    let rows: Vec<DictDataExportRow> = list
        .iter()
        .map(|v| DictDataExportRow {
            dict_code: v.dict_code,
            dict_sort: v.dict_sort,
            dict_label: v.dict_label.clone(),
            dict_value: v.dict_value.clone(),
            dict_type: v.dict_type.clone(),
            is_default: v.is_default.clone(),
            remark: v.remark.clone().unwrap_or_default(),
        })
        .collect();
    let bytes = ExcelBuilder::<DictDataExportRow>::export(&rows)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"dict_data.xlsx\""),
    );
    Ok((headers, Body::from(bytes)))
}
