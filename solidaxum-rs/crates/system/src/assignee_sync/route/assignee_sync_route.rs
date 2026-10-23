//! flow_assignee_sync 补偿任务查询路由(参考实现)。

use std::sync::Arc;

use axum::{Router, routing::get};

use crate::{
    app::AppState,
    assignee_sync::handler::assignee_sync_handler::list_assignee_sync,
};

/// 补偿记录查询接口。合并进 admin router 的 system 路由组
/// (与 system 路由同款形态:apply_auth + oper_log 由装配层叠加)。
pub fn assignee_sync_route() -> Router<Arc<AppState>> {
    Router::new().route("/system/assigneeSync/list", get(list_assignee_sync))
}
