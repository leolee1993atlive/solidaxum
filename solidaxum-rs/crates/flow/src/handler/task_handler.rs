use std::sync::Arc;

use app_core::{
    error::AppError,
    extract::AppJson,
    middleware::extractors::CurrentUser,
    vo::{
        page::{PageQuery, PageResult},
        response::R,
    },
};
use axum::extract::{Path, Query, State};

use crate::{
    dto::flow_params::FlowParams,
    handler::{FlowState, to_ctx, user_ctx},
    model::dto::{
        BackNodeVo, BackProcessBo, CompleteTaskBo, CurrentTaskUserVo,
        FlowNextNodeBo, FlowNextNodeVo, FlowTaskVo, FlowTerminationBo,
        FlowUrgeTaskBo, StartProcessBo, StartProcessReturnDTO, TaskOperationBo,
    },
    service::assignee_resync::{
        AssigneeResyncService, ResyncFilter, ResyncReport,
    },
};

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPageQuery {
    #[serde(flatten)]
    pub page: PageQuery,
    /// 申请人 id 列表（前端「选择申请人」多选，逗号分隔）。
    pub create_by_ids: Option<String>,
    /// 当前节点名称模糊匹配。
    pub node_name: Option<String>,
    /// 流程定义名称模糊匹配。
    pub flow_name: Option<String>,
    /// 流程定义编码模糊匹配（对齐 Java `FlwTaskMapper` 的 `likeIfText`）。
    pub flow_code: Option<String>,
}

impl TaskPageQuery {
    pub fn to_filter(&self) -> crate::model::dto::TaskPageFilter {
        crate::model::dto::TaskPageFilter {
            create_by_ids: self.create_by_ids.clone(),
            node_name: self.node_name.clone(),
            flow_name: self.flow_name.clone(),
            flow_code: self.flow_code.clone(),
        }
    }
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn start_work_flow(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<StartProcessBo>,
) -> Result<R<StartProcessReturnDTO>, AppError> {
    let initiator = login_user.user_id;
    let initiator_dept_id = login_user.dept_id;
    let current_user = CurrentUser(login_user);
    let ctx = to_ctx(&current_user);
    let dctx = user_ctx(&state, &current_user);

    let dto = state
        .ins_service
        .start_work_flow(&dctx, &bo, initiator, initiator_dept_id, &ctx)
        .await?;

    Ok(R::ok_with_data(dto))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn complete_task(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<CompleteTaskBo>,
) -> Result<R<()>, AppError> {
    let ctx = to_ctx(&CurrentUser(login_user.clone()));
    let dctx = user_ctx(&state, &CurrentUser(login_user));
    // flowCopyList 写入抄送表
    state
        .task_service
        .complete(
            &dctx,
            bo.task_id,
            bo.message.clone(),
            bo.variables.clone(),
            &bo.flow_copy_list,
            bo.assignee_map.clone(),
            &ctx,
        )
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state, login_user))]
pub async fn page_by_task_wait(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<TaskPageQuery>,
) -> Result<R<PageResult<FlowTaskVo>>, AppError> {
    let (page_no, page_size, _offset) =
        app_core::vo::page::normalize_paging(&query.page);
    let handler = login_user.user_id.to_string();
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let page = state
        .task_service
        .page_wait(
            &dctx,
            &handler,
            &query.to_filter(),
            page_no as u64,
            page_size as u64,
        )
        .await?;
    Ok(R::ok_with_data(PageResult::from(page)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn page_by_task_finish(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<TaskPageQuery>,
) -> Result<R<PageResult<crate::model::dto::FlowHisTaskVo>>, AppError> {
    let (page_no, page_size, _offset) =
        app_core::vo::page::normalize_paging(&query.page);
    let handler = login_user.user_id.to_string();
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let page = state
        .task_service
        .page_finish(
            &dctx,
            &handler,
            &query.to_filter(),
            page_no as u64,
            page_size as u64,
        )
        .await?;
    Ok(R::ok_with_data(PageResult::from(page)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn page_by_all_task_wait(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<TaskPageQuery>,
) -> Result<R<PageResult<FlowTaskVo>>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:task:list",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let (page_no, page_size, _offset) =
        app_core::vo::page::normalize_paging(&query.page);
    let page = state
        .task_service
        .page_all_wait(
            &dctx,
            &query.to_filter(),
            page_no as u64,
            page_size as u64,
        )
        .await?;
    Ok(R::ok_with_data(PageResult::from(page)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn page_by_all_task_finish(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<TaskPageQuery>,
) -> Result<R<PageResult<crate::model::dto::FlowHisTaskVo>>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:task:list",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let (page_no, page_size, _offset) =
        app_core::vo::page::normalize_paging(&query.page);
    let page = state
        .task_service
        .page_all_finish(
            &dctx,
            &query.to_filter(),
            page_no as u64,
            page_size as u64,
        )
        .await?;
    Ok(R::ok_with_data(PageResult::from(page)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn page_by_task_copy(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<TaskPageQuery>,
) -> Result<R<PageResult<FlowTaskVo>>, AppError> {
    let (page_no, page_size, _offset) =
        app_core::vo::page::normalize_paging(&query.page);
    let handler = login_user.user_id.to_string();
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let page = state
        .task_service
        .page_copy(
            &dctx,
            &handler,
            &query.to_filter(),
            page_no as u64,
            page_size as u64,
        )
        .await?;
    Ok(R::ok_with_data(PageResult::from(page)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn get_task_vo(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(task_id): Path<i64>,
) -> Result<R<FlowTaskVo>, AppError> {
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let vo = state
        .task_service
        .get_task_vo(&dctx, task_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("任务不存在 (id={})", task_id))
        })?;
    Ok(R::ok_with_data(vo))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn get_next_node_list(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<FlowNextNodeBo>,
) -> Result<R<Vec<FlowNextNodeVo>>, AppError> {
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let list = state.task_service.get_next_nodes(&dctx, bo.task_id).await?;
    Ok(R::ok_with_data(list))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn termination_task(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<FlowTerminationBo>,
) -> Result<R<crate::model::instance::Instance>, AppError> {
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let task = state
        .task_service
        .get_task(&dctx, bo.task_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("任务不存在 (id={})", bo.task_id))
        })?;
    let current_user = CurrentUser(login_user);
    let ctx = to_ctx(&current_user);
    let dctx = user_ctx(&state, &current_user);
    let params = FlowParams {
        message: bo.message.clone(),
        ..Default::default()
    };
    let instance = state
        .ins_service
        .termination(&dctx, task.instance_id, &params, &ctx)
        .await?;
    Ok(R::ok_with_data(instance))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn task_operation(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(op): Path<String>,
    AppJson(bo): AppJson<TaskOperationBo>,
) -> Result<R<crate::model::instance::Instance>, AppError> {
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let ctx = to_ctx(&CurrentUser(login_user));
    let instance = state
        .task_service
        .task_operation(&dctx, &op, &bo, &ctx)
        .await?;
    Ok(R::ok_with_data(instance))
}

#[tracing::instrument(skip(state, login_user, task_ids))]
pub async fn update_assignee(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(user_id): Path<i64>,
    AppJson(task_ids): AppJson<Vec<i64>>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:task:edit",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let ctx = to_ctx(&CurrentUser(login_user));
    state
        .task_service
        .update_assignee(&dctx, &task_ids, user_id, &ctx)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn back_process(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<BackProcessBo>,
) -> Result<R<crate::model::instance::Instance>, AppError> {
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let ctx = to_ctx(&CurrentUser(login_user));
    let instance = state
        .task_service
        .back_process(
            &dctx,
            bo.task_id,
            &bo.node_code,
            bo.message.clone(),
            &ctx,
        )
        .await?;
    Ok(R::ok_with_data(instance))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn get_back_task_node(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path((task_id, now_node_code)): Path<(i64, String)>,
) -> Result<R<Vec<BackNodeVo>>, AppError> {
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let list = state
        .task_service
        .get_back_nodes(&dctx, task_id, &now_node_code)
        .await?;
    Ok(R::ok_with_data(list))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn current_task_all_user(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(task_id): Path<i64>,
) -> Result<R<Vec<CurrentTaskUserVo>>, AppError> {
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let users = state
        .task_service
        .current_task_all_user(&dctx, task_id)
        .await?;
    Ok(R::ok_with_data(users))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn urge_task(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<FlowUrgeTaskBo>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:task:edit",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user.clone()));
    let ctx = to_ctx(&CurrentUser(login_user));
    state.task_service.urge_task(&dctx, &bo, &ctx).await?;
    Ok(R::ok())
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReResolveBo {
    /// 指定实例只重解析该实例; 缺省全量运行中实例
    pub instance_id: Option<i64>,
}

/// 重解析运行中实例办理人(管理端兜底运维手段, 对齐旧系统手工 SQL
/// 清数的正规化)。直接复用 D3 补偿服务的全量/单实例两条路径,
/// 应用语义与 worker 消费完全一致(含转办/委派人工介入跳过规则)。
#[tracing::instrument(skip(state, login_user, bo))]
pub async fn re_resolve_assignee(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<ReResolveBo>,
) -> Result<R<ResyncReport>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:task:reResolve",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user));
    let svc = AssigneeResyncService::new(state.resolver.clone());
    let report = svc
        .resync(
            &dctx,
            ResyncFilter {
                instance_id: bo.instance_id,
                event_user_ids: None,
            },
        )
        .await?;
    Ok(R::ok_with_data(report))
}
