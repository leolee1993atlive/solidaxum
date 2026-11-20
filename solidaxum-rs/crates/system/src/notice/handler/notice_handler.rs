use std::sync::Arc;

use app_core::{
    error::AppError,
    extract::AppJson,
    middleware::extractors::CurrentUser,
    vo::{page::TableDataInfo, response::R},
};
use axum::extract::{Path, Query, State};
use validator::Validate;

use crate::{
    app::AppState,
    notice::model::notice::{
        NoticeCreateBo, NoticeListVo, NoticeMyQuery, NoticeMyVo,
        NoticePageQuery, NoticeUpdateBo,
    },
    permission::guard,
};

#[tracing::instrument(skip(state), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
pub async fn list_notice_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<NoticePageQuery>,
) -> Result<R<TableDataInfo<NoticeListVo>>, AppError> {
    guard::require_permission(&login_user, "system:notice:list")?;
    // ctx 请求期构造;notice 是全豁免模块(SysNoticeMapper 无
    // @DataPermission),读在 repo 内显式 without_row_scope。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let mut result = state.services.notice_service.page(&ctx, query).await?;

    // 批量翻译 create_by (user_id) → create_by_name
    // (翻译读同豁免:翻译数据非行级,ctx 的范围不影响解析口径)
    let user_ids: Vec<i64> = result
        .rows
        .iter()
        .filter_map(|r| r.base_vo.create_by)
        .collect();
    if !user_ids.is_empty() {
        let name_map = state
            .services
            .translation_service
            .user_id_to_name(&ctx, &user_ids)
            .await?;
        for row in &mut result.rows {
            if let Some(uid) = row.base_vo.create_by {
                row.create_by_name = name_map.get(&uid).cloned();
            }
        }
    }

    Ok(R::ok_with_data(result))
}

#[tracing::instrument(skip(state), fields(notice_id = id))]
pub async fn get_notice_by_id(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<NoticeListVo>, AppError> {
    guard::require_permission(&login_user, "system:notice:query")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let notice = state.services.notice_service.get_by_id(&ctx, id).await?;
    Ok(R::ok_with_data(notice.to_list_vo()))
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, notice_title = ?bo.notice_title)
)]
pub async fn create_notice(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<NoticeCreateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:notice:add")?;
    bo.validate()?;
    // 审计盖章人取 ctx 的 principal(漏斗 insert 统一盖)
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    // 新建为草稿,不推送;发布(POST /system/notice/{id}/publish)时才通知。
    state
        .services
        .notice_service
        .insert_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state, bo),
    fields(operator = %current_user.0.username, notice_id = ?bo.notice_id)
)]
pub async fn update_notice(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<NoticeUpdateBo>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:notice:edit")?;
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .notice_service
        .update_by_bo(&ctx, bo, &current_user)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(
    skip(state),
    fields(operator = %current_user.0.username, notice_ids = ids)
)]
pub async fn delete_notice(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:notice:remove")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    for id_str in ids.split(',') {
        if id_str.is_empty() {
            continue;
        }
        let id: i64 = id_str.parse().map_err(|_| {
            AppError::BadRequest(format!(
                "删除通知公告失败: 无效的通知公告ID: {}",
                id_str
            ))
        })?;
        state.services.notice_service.delete_by_id(&ctx, id).await?;
    }
    Ok(R::ok())
}

/// 发布公告(发布时按目标发送 SSE,USER 目标补建回执行)。
#[tracing::instrument(
    skip(state),
    fields(operator = %current_user.0.username, notice_id = id)
)]
pub async fn publish_notice(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&current_user.0, "system:notice:edit")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&current_user.0));
    state
        .services
        .notice_service
        .publish(&ctx, id, &current_user)
        .await?;
    Ok(R::ok())
}

/// 当前用户可见的已发布公告(登录即可,无额外权限)。
#[tracing::instrument(skip(state), fields(user_id = %login_user.user_id))]
pub async fn list_my_notice(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<NoticeMyQuery>,
) -> Result<R<TableDataInfo<NoticeMyVo>>, AppError> {
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let result = state
        .services
        .notice_service
        .my_page(&ctx, login_user.user_id, query)
        .await?;
    Ok(R::ok_with_data(result))
}

/// 标记单条公告已读(幂等)。
#[tracing::instrument(skip(state), fields(user_id = %login_user.user_id, notice_id = id))]
pub async fn mark_notice_read(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> Result<R<()>, AppError> {
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    state
        .services
        .notice_service
        .mark_read(&ctx, id, login_user.user_id)
        .await?;
    Ok(R::ok())
}

/// 全部已读:当前用户可见的已发布公告。
#[tracing::instrument(skip(state), fields(user_id = %login_user.user_id))]
pub async fn mark_all_notice_read(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<()>, AppError> {
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    state
        .services
        .notice_service
        .mark_all_read(&ctx, login_user.user_id)
        .await?;
    Ok(R::ok())
}
