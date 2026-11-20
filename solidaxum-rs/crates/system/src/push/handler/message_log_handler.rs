//! 消息发送历史查询。
//!
//! 发送日志是全局审计流水,不施加行级数据范围;接口仍要求
//! `system:messageChannel:list` 权限。

use std::sync::Arc;

use app_core::vo::page::normalize_paging;
use axum::extract::{Query, State};

use crate::{
    app::AppState,
    permission::guard,
    push::repository::sys_message_send_log::{
        SendLogPageQuery, SendLogRepository,
    },
};
use app_core::{
    error::AppResult,
    middleware::extractors::CurrentUser,
    vo::{page::TableDataInfo, response::R},
};

/// `GET /system/messageLog/list` — 发送历史分页(全局审计流水)。
#[tracing::instrument(skip(state, login_user))]
pub async fn list_send_logs(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<SendLogPageQuery>,
) -> AppResult<R<TableDataInfo<crate::push::repository::sys_message_send_log::Model>>>
{
    guard::require_permission(&login_user, "system:messageChannel:list")?;
    let (current, size, _) = normalize_paging(&query.page);
    let ds = state.infra.data_source();
    let ctx = ds.system_context();
    let page = SendLogRepository::new()
        .select_page(&ctx, &query, current as u64, size as u64)
        .await?;
    Ok(R::ok_with_data(page))
}
