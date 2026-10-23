use std::sync::Arc;

use app_core::{
    error::AppError,
    extract::AppJson,
    middleware::extractors::CurrentUser,
    vo::{page::TableDataInfo, response::R},
};
use axum::{
    extract::{Multipart, Path, Query, State},
    http::{StatusCode, header},
};
use validator::Validate;

use crate::{
    app::AppState,
    oss::model::oss::{
        OssConfigChangeStatusBo, OssConfigCreateBo, OssConfigQueryBo,
        OssConfigUpdateBo, OssQueryBo, SysOssVo,
    },
    permission::guard,
};

/// 批量填充 createByName(对齐 Java @Translation USER_ID_TO_NAME)。
/// ctx 由调用方传入(翻译读豁免:翻译数据非行级,ctx 的范围不影响
/// 解析口径)。
async fn fill_oss_create_by_names(
    state: &Arc<AppState>,
    ctx: &app_data::DataContext<'_>,
    vos: &mut [SysOssVo],
) -> Result<(), AppError> {
    let creator_ids: Vec<i64> =
        vos.iter().filter_map(|v| v.base_vo.create_by).collect();
    if creator_ids.is_empty() {
        return Ok(());
    }
    let name_map = state
        .services
        .translation_service
        .user_id_to_name(ctx, &creator_ids)
        .await?;
    for vo in vos.iter_mut() {
        if let Some(id) = vo.base_vo.create_by {
            vo.create_by_name = name_map.get(&id).cloned();
        }
    }
    Ok(())
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadQuery {
    pub service: Option<String>,
}

#[tracing::instrument(skip(state), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
pub async fn list_oss(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<OssQueryBo>,
) -> Result<R<TableDataInfo<SysOssVo>>, AppError> {
    guard::require_permission(&login_user, "system:oss:list")?;
    // ctx 请求期构造,repo 读与翻译读复用同一 ctx
    //(oss 全豁免,翻译读同豁免)。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let service = state.services.oss_service.clone();
    let result = service.list(&ctx, query).await?;
    let mut vos: Vec<SysOssVo> =
        result.rows.iter().map(|o| o.to_vo()).collect();
    fill_oss_create_by_names(&state, &ctx, &mut vos).await?;
    Ok(R::ok_with_data(result.with_rows(vos)))
}

#[tracing::instrument(skip(state))]
pub async fn list_oss_by_ids(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(oss_ids): Path<String>,
) -> Result<R<Vec<crate::oss::model::oss::SysOss>>, AppError> {
    guard::require_permission(&login_user, "system:oss:query")?;
    let ids = parse_ids(&oss_ids)?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let service = state.services.oss_service.clone();
    let result = service.list_by_ids(&ctx, &ids).await?;
    Ok(R::ok_with_data(result))
}

#[tracing::instrument(skip(state, multipart), fields(operator = %current_user.0.username))]
pub async fn upload_oss(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<UploadQuery>,
    multipart: Multipart,
) -> Result<R<crate::oss::model::oss::OssUploadVo>, AppError> {
    guard::require_permission(&current_user.0, "system:oss:upload")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    let service = state.services.oss_service.clone();
    let result = service
        .upload(&ctx, multipart, query.service, &current_user)
        .await?;
    Ok(R::ok_with_data(result))
}

#[allow(clippy::type_complexity)]
#[tracing::instrument(skip(state), fields(oss_id = id))]
pub async fn download_oss(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<(StatusCode, [(header::HeaderName, String); 2], Vec<u8>), AppError>
{
    guard::require_permission(&login_user, "system:oss:download")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let service = state.services.oss_service.clone();
    let (data, filename, content_type) = service.download(&ctx, id).await?;
    let disposition =
        format!("attachment; filename=\"{}\"", filename.replace('"', "\\\""));
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        data,
    ))
}

#[tracing::instrument(skip(state), fields(operator = %current_user.0.username, oss_ids = ids))]
pub async fn delete_oss(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:oss:remove")?;
    let ids = parse_ids(&ids)?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    let service = state.services.oss_service.clone();
    service.delete_by_ids(&ctx, &ids).await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
pub async fn list_oss_config(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<OssConfigQueryBo>,
) -> Result<R<TableDataInfo<crate::oss::model::oss::OssConfigVo>>, AppError> {
    guard::require_permission(&login_user, "system:ossConfig:list")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let service = state.services.oss_service.clone();
    let result = service.config_list(&ctx, query).await?;
    Ok(R::ok_with_data(result))
}

#[tracing::instrument(skip(state), fields(oss_config_id = id))]
pub async fn get_oss_config_by_id(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<crate::oss::model::oss::OssConfigVo>, AppError> {
    guard::require_permission(&login_user, "system:ossConfig:list")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let service = state.services.oss_service.clone();
    let result = service.config_get_by_id(&ctx, id).await?;
    Ok(R::ok_with_data(result))
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, config_key = ?bo.config_key)
)]
pub async fn create_oss_config(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<OssConfigCreateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:ossConfig:add")?;
    bo.validate()?;
    // 审计盖章人取 ctx 的 principal(漏斗 insert 统一盖)
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    let service = state.services.oss_service.clone();
    service.config_add(&ctx, bo, &current_user).await?;
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, oss_config_id = ?bo.oss_config_id)
)]
pub async fn update_oss_config(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<OssConfigUpdateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:ossConfig:edit")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    let service = state.services.oss_service.clone();
    service.config_update(&ctx, bo, &current_user).await?;
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state),
    fields(operator = %current_user.0.username, oss_config_ids = ids)
)]
pub async fn delete_oss_config(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:ossConfig:remove")?;
    let ids = parse_ids(&ids)?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    let service = state.services.oss_service.clone();
    for id in ids {
        service.config_delete_by_id(&ctx, id).await?;
    }
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state, bo),
    fields(oss_config_id = ?bo.oss_config_id)
)]
pub async fn change_oss_config_status(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<OssConfigChangeStatusBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:ossConfig:edit")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    let service = state.services.oss_service.clone();
    service.config_change_status(&ctx, bo).await?;
    Ok(R::ok())
}

fn parse_ids(ids: &str) -> Result<Vec<i64>, AppError> {
    ids.split(',')
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse()
                .map_err(|_| AppError::BadRequest(format!("无效的ID: {}", s)))
        })
        .collect()
}
