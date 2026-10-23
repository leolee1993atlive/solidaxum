use std::sync::Arc;

use app_core::{
    error::{AppError, AppResult},
    extract::AppJson,
    middleware::extractors::CurrentUser,
    permission::require_permission,
    vo::response::R,
};
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};

use crate::{
    JobServices, entity::sys_job::ActiveModel, map_service_err,
    scheduler::SchedulerService,
};

#[derive(Clone)]
pub struct JobHandlerState {
    pub services: JobServices,
    pub scheduler: Arc<SchedulerService>,
    /// 请求期 ctx 构造源(与 infra.data_source() 同一 Arc,
    /// 装配注释见 admin/src/router.rs 构造点)。
    pub ds: Arc<app_data::DataSource>,
}

impl std::fmt::Debug for JobHandlerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobHandlerState").finish()
    }
}

#[derive(Debug, Deserialize)]
pub struct JobPageQuery {
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

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateJobBody {
    pub job_name: String,
    pub job_group: String,
    pub invoke_target: String,
    pub cron_expression: String,
    pub job_params: Option<String>,
    pub misfire_policy: Option<i8>,
    pub concurrent: Option<i8>,
    pub status: Option<String>,
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateJobBody {
    pub job_id: i64,
    #[serde(flatten)]
    pub data: CreateJobBody,
}

#[derive(Debug, Deserialize)]
pub struct ChangeStatusBody {
    pub job_id: i64,
    pub status: String,
}

pub async fn list_jobs(
    State(state): State<Arc<JobHandlerState>>,
    CurrentUser(login_user): CurrentUser,
    Query(q): Query<JobPageQuery>,
) -> AppResult<R<PageData<serde_json::Value>>> {
    require_permission(&login_user, "monitor:job:list")?;
    let page = q.page_num.unwrap_or(1);
    let size = q.page_size.unwrap_or(10);
    // ctx 请求期构造(job 全豁免,读不解析 resolver)
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    let page_result = state
        .services
        .job_service
        .page(&ctx, page, size, q.job_name.as_deref(), q.status.as_deref())
        .await
        .map_err(map_service_err)?;
    let rows: Vec<_> = page_result
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

pub async fn get_job(
    State(state): State<Arc<JobHandlerState>>,
    CurrentUser(login_user): CurrentUser,
    Path(job_id): Path<i64>,
) -> AppResult<R<serde_json::Value>> {
    require_permission(&login_user, "monitor:job:query")?;
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    // 已删/不存在:service 已收口为 DataError::NotFound,
    // map_service_err 映 AppError::NotFound
    let job = state
        .services
        .job_service
        .get(&ctx, job_id)
        .await
        .map_err(map_service_err)?;
    Ok(R::ok_with_data(serde_json::to_value(job).unwrap()))
}

pub async fn create_job(
    State(state): State<Arc<JobHandlerState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(body): AppJson<CreateJobBody>,
) -> AppResult<R<()>> {
    require_permission(&login_user, "monitor:job:add")?;
    use app_data::sea_orm::ActiveValue::Set;
    let model = ActiveModel {
        job_name: Set(body.job_name),
        job_group: Set(body.job_group),
        invoke_target: Set(body.invoke_target),
        cron_expression: Set(body.cron_expression),
        job_params: Set(body.job_params),
        misfire_policy: Set(body.misfire_policy.unwrap_or(1)),
        concurrent: Set(body.concurrent.unwrap_or(1)),
        status: Set(body.status.clone().unwrap_or_else(|| "0".to_string())),
        remark: Set(body.remark),
        ..Default::default()
    };
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    // operator 平价锚:Java SysJobServiceImpl.insertByBo 经
    // LoginHelper.getUsername() 盖 create_by/update_by。
    let created = state
        .services
        .job_service
        .create(&ctx, model, &login_user.username)
        .await
        .map_err(map_service_err)?;

    // 若状态为运行中则立即调度
    if created.status == "1" {
        let _ = state.scheduler.schedule_job(&created).await;
    }
    Ok(R::ok())
}

pub async fn update_job(
    State(state): State<Arc<JobHandlerState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(body): AppJson<UpdateJobBody>,
) -> AppResult<R<()>> {
    require_permission(&login_user, "monitor:job:edit")?;
    use app_data::sea_orm::ActiveValue::Set;
    let model = ActiveModel {
        job_id: Set(body.job_id),
        job_name: Set(body.data.job_name),
        job_group: Set(body.data.job_group),
        invoke_target: Set(body.data.invoke_target),
        cron_expression: Set(body.data.cron_expression),
        job_params: Set(body.data.job_params),
        misfire_policy: Set(body.data.misfire_policy.unwrap_or(1)),
        concurrent: Set(body.data.concurrent.unwrap_or(1)),
        status: Set(body
            .data
            .status
            .clone()
            .unwrap_or_else(|| "0".to_string())),
        remark: Set(body.data.remark),
        ..Default::default()
    };
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    // operator 平价锚,同 create_job 注。
    let updated = state
        .services
        .job_service
        .update(&ctx, model, &login_user.username)
        .await
        .map_err(map_service_err)?;

    // DB 写成功后再动调度 —— update 对不存在/已删任务回 404 整体
    // 回滚,先摘调度会致"用户见失败而调度已漂移";remove_job 对不存在
    // 任务幂等,无副作用放大。
    let _ = state.scheduler.remove_job(body.job_id).await;
    if updated.status == "1" {
        let _ = state.scheduler.schedule_job(&updated).await;
    }
    Ok(R::ok())
}

pub async fn delete_jobs(
    State(state): State<Arc<JobHandlerState>>,
    CurrentUser(login_user): CurrentUser,
    Path(ids): Path<String>,
) -> AppResult<R<()>> {
    require_permission(&login_user, "monitor:job:remove")?;
    let job_ids: Vec<i64> =
        ids.split(',').filter_map(|s| s.parse().ok()).collect();
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    // 批删 = 逐条软删(包事务);operator 盖 update_by(对齐 Java
    // MetaObjectHandler 在 logic delete 时回填)。
    state
        .services
        .job_service
        .delete(&ctx, &job_ids, &login_user.username)
        .await
        .map_err(map_service_err)?;
    // DB 写成功后才摘调度(delete 对不存在/已删 id 回 404 整体回滚;
    // 先摘会致"失败但调度已漂移")。
    for id in &job_ids {
        let _ = state.scheduler.remove_job(*id).await;
    }
    Ok(R::ok())
}

pub async fn change_status(
    State(state): State<Arc<JobHandlerState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(body): AppJson<ChangeStatusBody>,
) -> AppResult<R<()>> {
    require_permission(&login_user, "monitor:job:changeStatus")?;
    let ctx = state.ds.context(app_data::Principal::from(&login_user));
    // operator 平价锚,同 create_job 注;不存在/已删 → NotFound。
    state
        .services
        .job_service
        .change_status(&ctx, body.job_id, &body.status, &login_user.username)
        .await
        .map_err(map_service_err)?;

    // DB 写成功后按目标状态摘/挂调度(对不存在/已删任务回 404,
    // 先摘调度会漂移)。
    if body.status == "0" {
        let _ = state.scheduler.remove_job(body.job_id).await;
    }
    if body.status == "1"
        && let Ok(job) = state.services.job_service.get(&ctx, body.job_id).await
    {
        let _ = state.scheduler.schedule_job(&job).await;
    }
    Ok(R::ok())
}

pub async fn run_job(
    State(state): State<Arc<JobHandlerState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(body): AppJson<serde_json::Value>,
) -> AppResult<R<String>> {
    require_permission(&login_user, "monitor:job:changeStatus")?;
    let job_id = body["jobId"]
        .as_i64()
        .ok_or_else(|| AppError::BadRequest("缺少或非法的 jobId".into()))?;
    let msg = state.scheduler.run_once(job_id).await.map_err(|e| {
        tracing::error!("手动触发任务失败: {e}");
        AppError::Internal(e.to_string())
    })?;
    Ok(R::ok_with_data(msg))
}
