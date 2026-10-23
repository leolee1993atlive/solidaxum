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
    config::model::config::{
        ConfigCreateBo, ConfigExportRow, ConfigListVo, ConfigPageQuery,
        ConfigUpdateBo, ConfigUpdateByKeyBo,
    },
    permission::guard,
};

/// 参数配置分页查询 GET /system/config/list
#[tracing::instrument(skip(state), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
pub async fn page(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<ConfigPageQuery>,
) -> Result<R<TableDataInfo<ConfigListVo>>, AppError> {
    guard::require_permission(&login_user, "system:config:list")?;
    // ctx 请求期构造;config 是全豁免模块(SysConfigMapper 无
    // @DataPermission),读在 repo 内显式 without_row_scope。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let result = state.services.sys_config_service.page(&ctx, query).await?;
    Ok(R::ok_with_data(result))
}

/// 根据参数键名查询参数值 GET /system/config/configKey/{key}
/// 注意: 此接口仅需登录，不需要特定权限
/// (对位 Java `@Cacheable(SYS_CONFIG)` selectConfigByKey 的 HTTP 面)
#[tracing::instrument(skip(state), fields(config_key = key))]
pub async fn get_config_by_key(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(key): Path<String>,
) -> Result<R<String>, AppError> {
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let value = state
        .services
        .sys_config_service
        .get_by_key(&ctx, &key)
        .await?;
    Ok(R::ok_with_data(value))
}

/// 系统信息(标题/图标) GET /system/config/sysInfo
///
/// 锚:原 Java 版「系统信息」配置页 —— 运营可自定义产品标题
/// (通常为医院全称+系统名)与图标(通常为医院标),顶栏标题与侧栏图标
/// 的运行时来源。存 sys_config 键 `sys.title`/`sys.icon`(icon 为 OSS
/// URL);仅需登录,不给任何已登录用户发 system:config:query。
/// 键缺失(如老库存)不报错,降级为空串由前端兜底。
#[tracing::instrument(skip(state))]
pub async fn sys_info(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<SysInfoVo>, AppError> {
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let svc = &state.services.sys_config_service;
    // 缺键/未初始化 → 空串兜底(NotFound 仅用于配置面,展示面应安静)
    let title = svc
        .get_by_key(&ctx, SYS_INFO_TITLE_KEY)
        .await
        .unwrap_or_default();
    let icon = svc
        .get_by_key(&ctx, SYS_INFO_ICON_KEY)
        .await
        .unwrap_or_default();
    Ok(R::ok_with_data(SysInfoVo { title, icon }))
}

/// 系统信息存量的 sys_config 键名(前端保存亦走 updateByKey 这两个键)。
pub const SYS_INFO_TITLE_KEY: &str = "sys.title";
pub const SYS_INFO_ICON_KEY: &str = "sys.icon";

/// 系统信息 Vo(仅登录可读;title=顶栏标题,icon=侧栏图标 URL)。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SysInfoVo {
    pub title: String,
    pub icon: String,
}

/// 根据ID查询参数配置 GET /system/config/{id}
#[tracing::instrument(skip(state), fields(config_id = id))]
pub async fn get_by_id(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<ConfigListVo>, AppError> {
    guard::require_permission(&login_user, "system:config:query")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let config = state
        .services
        .sys_config_service
        .get_by_id(&ctx, id)
        .await?;
    Ok(R::ok_with_data(config.to_list_vo()))
}

/// 新增参数配置 POST /system/config
#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, config_key = ?bo.config_key)
)]
pub async fn create(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<ConfigCreateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:config:add")?;
    bo.validate()?;
    // 审计盖章人取 ctx 的 principal(漏斗 insert 统一盖)
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .sys_config_service
        .insert_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

/// 修改参数配置 PUT /system/config
#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, config_id = ?bo.config_id)
)]
pub async fn update(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<ConfigUpdateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:config:edit")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .sys_config_service
        .update_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

/// 根据参数键名更新参数值 PUT /system/config/updateByKey
#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, config_key = ?bo.config_key)
)]
pub async fn update_config_by_key(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<ConfigUpdateByKeyBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:config:edit")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .sys_config_service
        .update_by_key(&ctx, &bo.config_key, &bo.config_value, &current_user)
        .await?;
    Ok(R::ok())
}

/// 删除参数配置 DELETE /system/config/{ids}
#[tracing::instrument(
    skip(state),
    fields(operator = %current_user.0.username, config_ids = ids)
)]
pub async fn delete(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:config:remove")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    for id_str in ids.split(',') {
        if id_str.is_empty() {
            continue;
        }
        let id: i64 = id_str.parse().map_err(|_| {
            AppError::BadRequest(format!(
                "删除参数配置失败: 无效的参数ID: {}",
                id_str
            ))
        })?;
        state
            .services
            .sys_config_service
            .delete_by_id(&ctx, id, &current_user)
            .await?;
    }
    Ok(R::ok())
}

/// 刷新参数缓存 DELETE /system/config/refreshCache
#[tracing::instrument(skip(state), fields(operator = %login_user.username))]
pub async fn refresh_cache(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<()>, AppError> {
    guard::require_permission(&login_user, "system:config:remove")?;
    state.services.sys_config_service.refresh_cache().await?;
    Ok(R::ok())
}

/// 导出参数配置 xlsx POST /system/config/export
/// 对齐 Java SysConfigController.export。
#[tracing::instrument(skip(state, login_user))]
pub async fn export_config(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    // Vue download() 发 application/x-www-form-urlencoded(见 request.ts:259),
    // 故用 Form 提取(与 GET list 的 Query 同走 serde_urlencoded),不能用 AppJson。
    Form(query): Form<ConfigPageQuery>,
) -> Result<(HeaderMap, Body), AppError> {
    guard::require_permission(&login_user, "system:config:export")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let list = state
        .services
        .sys_config_service
        .list_all(&ctx, query)
        .await?;
    let rows: Vec<ConfigExportRow> =
        list.iter().map(ConfigExportRow::from_config).collect();
    let bytes = ExcelBuilder::<ConfigExportRow>::export(&rows)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"config.xlsx\""),
    );
    Ok((headers, Body::from(bytes)))
}
