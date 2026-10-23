//! flow_assignee_sync 补偿任务查询 handler(参考实现)。

use std::sync::Arc;

use app_core::{
    error::AppError,
    middleware::extractors::CurrentUser,
    vo::{page::TableDataInfo, response::R},
};
use axum::extract::{Query, State};

use crate::{
    app::AppState,
    assignee_sync::{
        model::assignee_sync::{AssigneeSync, AssigneeSyncPageQuery},
        service::assignee_sync_service::AssigneeSyncService,
    },
    permission::guard,
};

/// 补偿任务分页查询。权限点 `system:assigneeSync:list`(本表无 Java
/// 对应物,权限点为参考实现自有命名)。
///
/// 表全豁免(模块头裁定),ctx 仅供连接路径执行;行级范围不施加。
#[tracing::instrument(skip(state, login_user, query))]
pub async fn list_assignee_sync(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<AssigneeSyncPageQuery>,
) -> Result<R<TableDataInfo<AssigneeSync>>, AppError> {
    guard::require_permission(&login_user, "system:assigneeSync:list")?;
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let (page_no, page_size, _offset) =
        app_core::vo::page::normalize_paging(&query.page);
    let page = AssigneeSyncService::page(
        &ctx,
        &query,
        page_no as u64,
        page_size as u64,
    )
    .await?;
    Ok(R::ok_with_data(page))
}
