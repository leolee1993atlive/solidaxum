use std::sync::Arc;

use app_core::{
    error::{AppError, AppResult},
    middleware::extractors::CurrentUser,
    permission::require_permission,
    vo::response::R,
};
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};

use crate::{JobServices, map_service_err};

#[derive(Clone)]
pub struct JobLogHandlerState {
    pub services: JobServices,
    /// 请求期 ctx 构造源(与 infra.data_source() 同一 Arc,
    /// 装配注释见 admin/src/router.rs 构造点)。
    pub ds: Arc<app_data::DataSource>,
}

// DataSource 未实现 Debug,与 JobHandlerState 同款手写。
impl std::fmt::Debug for JobLogHandlerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobLogHandlerState").finish()
    }
}

#[derive(Debug, Deserialize)]
pub struct LogPageQuery {
    pub page_num: Option<u64>,
    pub page_size: Option<u64>,
    pub job_name: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PageData<T> {
    pub rows: Vec<T>,
    pub total: u64,
    /// total 触到计数上限被截断:真实总数 ≥ total(前端渲染成 “10000+”)。
    /// 与 app_core 的 TableDataInfo 同口径 —— job 这两个列表历史上自带 VO,
    /// 字段跟着补,免得只有这两条列表把截断标记丢在半路。
    pub total_capped: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_approx: Option<i64>,
}

pub async fn list_logs(
    State(state): State<Arc<JobLogHandlerState>>,
    CurrentUser(login_user): CurrentUser,
    Query(q): Query<LogPageQuery>,
) -> AppResult<R<PageData<serde_json::Value>>> {
    require_permission(&login_user, "monitor:job:list")?;
    // ctx 请求期构造(joblog 全豁免,读不解析 resolver)
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    let page_result = state
        .services
        .job_log_service
        .page(
            &ctx,
            q.page_num.unwrap_or(1),
            q.page_size.unwrap_or(10),
            q.job_name.as_deref(),
            q.status.as_deref(),
        )
        .await
        .map_err(map_service_err)?;
    let rows = page_result
        .rows
        .into_iter()
        .map(|m| serde_json::to_value(m).unwrap())
        .collect();
    Ok(R::ok_with_data(PageData {
        rows,
        total: page_result.total.max(0) as u64,
        total_capped: page_result.total_capped,
        total_approx: page_result.total_approx,
    }))
}

pub async fn get_log(
    State(state): State<Arc<JobLogHandlerState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
) -> AppResult<R<serde_json::Value>> {
    require_permission(&login_user, "monitor:job:query")?;
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    let log = state
        .services
        .job_log_service
        .get(&ctx, id)
        .await
        .map_err(map_service_err)?
        .ok_or_else(|| AppError::NotFound("调度日志不存在".into()))?;
    Ok(R::ok_with_data(serde_json::to_value(log).unwrap()))
}

pub async fn delete_logs(
    State(state): State<Arc<JobLogHandlerState>>,
    CurrentUser(login_user): CurrentUser,
    Path(ids): Path<String>,
) -> AppResult<R<()>> {
    require_permission(&login_user, "monitor:job:remove")?;
    let ids: Vec<i64> = ids.split(',').filter_map(|s| s.parse().ok()).collect();
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    state
        .services
        .job_log_service
        .delete(&ctx, &ids)
        .await
        .map_err(map_service_err)?;
    Ok(R::ok())
}

pub async fn clean_logs(
    State(state): State<Arc<JobLogHandlerState>>,
    CurrentUser(login_user): CurrentUser,
) -> AppResult<R<()>> {
    require_permission(&login_user, "monitor:job:remove")?;
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    state
        .services
        .job_log_service
        .clean(&ctx)
        .await
        .map_err(map_service_err)?;
    Ok(R::ok())
}
