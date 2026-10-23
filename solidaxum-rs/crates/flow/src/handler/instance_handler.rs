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
use serde::Deserialize;

use crate::{
    dto::flow_params::FlowParams,
    handler::{FlowState, to_ctx, user_ctx},
    model::{dto::FlowInstanceVo, instance::Instance},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstancePageQuery {
    #[serde(flatten)]
    pub page: PageQuery,
    pub flow_code: Option<String>,
    pub business_id: Option<String>,
    /// 流程分类(前端左侧分类树点击时传入)。
    pub category: Option<String>,
    /// 发起人 id 列表(逗号分隔)。
    pub create_by_ids: Option<String>,
    /// 当前节点名称模糊匹配。
    pub node_name: Option<String>,
    /// 流程定义名称模糊匹配。
    pub flow_name: Option<String>,
}

impl InstancePageQuery {
    pub fn to_filter(&self) -> crate::model::dto::InstancePageFilter {
        crate::model::dto::InstancePageFilter {
            flow_code: self.flow_code.clone(),
            business_id: self.business_id.clone(),
            category: self.category.clone(),
            create_by_ids: self.create_by_ids.clone(),
            node_name: self.node_name.clone(),
            flow_name: self.flow_name.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowCancelBo {
    pub business_id: Option<String>,
    pub instance_id: Option<i64>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowInvalidBo {
    pub business_id: Option<String>,
    pub instance_id: Option<i64>,
    /// 作废原因（对齐 Java，持久化到历史任务 message 字段）。
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowVariableBo {
    pub instance_id: i64,
    pub variable: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveQuery {
    #[serde(deserialize_with = "deserialize_bool_str")]
    pub active: bool,
}

fn deserialize_bool_str<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(D::Error::custom(format!("无效的 active 值: {}", s))),
    }
}

/// 把实例列表转为富 VO 并逐行填充 join/translation 字段,对齐 Java
/// `selectRunning/Finish/CurrentInstanceList` 均返回 `FlowInstanceVo`。
/// 注:逐行 `fill_instance_vo` 存在 N+1;分页规模小暂可接受,后续可批量化。
async fn enrich_instance_rows(
    state: &FlowState,
    login_user: &CurrentUser,
    instances: &[Instance],
) -> Result<Vec<FlowInstanceVo>, AppError> {
    let dctx = user_ctx(state, login_user);
    let mut rows = Vec::with_capacity(instances.len());
    for inst in instances {
        let mut vo = FlowInstanceVo::from(inst);
        state.ins_service.fill_instance_vo(&dctx, &mut vo).await?;
        rows.push(vo);
    }
    Ok(rows)
}

#[tracing::instrument(skip(state, login_user))]
pub async fn page_by_running(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<InstancePageQuery>,
) -> Result<R<PageResult<FlowInstanceVo>>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:instance:list",
    )?;
    let (page_no, page_size, _offset) =
        app_core::vo::page::normalize_paging(&query.page);
    let current_user = CurrentUser(login_user);
    let dctx = user_ctx(&state, &current_user);
    let page = state
        .ins_service
        .page_running(
            &dctx,
            &query.to_filter(),
            page_no as u64,
            page_size as u64,
        )
        .await?;
    let rows = enrich_instance_rows(&state, &current_user, &page.rows).await?;
    Ok(R::ok_with_data(PageResult::from_page_rows(&page, rows)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn page_by_finish(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<InstancePageQuery>,
) -> Result<R<PageResult<FlowInstanceVo>>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:instance:list",
    )?;
    let (page_no, page_size, _offset) =
        app_core::vo::page::normalize_paging(&query.page);
    let current_user = CurrentUser(login_user);
    let dctx = user_ctx(&state, &current_user);
    let page = state
        .ins_service
        .page_finish(
            &dctx,
            &query.to_filter(),
            page_no as u64,
            page_size as u64,
        )
        .await?;
    let rows = enrich_instance_rows(&state, &current_user, &page.rows).await?;
    Ok(R::ok_with_data(PageResult::from_page_rows(&page, rows)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn page_by_current(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<InstancePageQuery>,
) -> Result<R<PageResult<FlowInstanceVo>>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:instance:currentList",
    )?;
    let current_user = CurrentUser(login_user);
    let user_id = current_user.get_user_id().to_string();
    let (page_no, page_size, _offset) =
        app_core::vo::page::normalize_paging(&query.page);
    let dctx = user_ctx(&state, &current_user);
    let page = state
        .ins_service
        .page_current(
            &dctx,
            &user_id,
            &query.to_filter(),
            page_no as u64,
            page_size as u64,
        )
        .await?;
    let rows = enrich_instance_rows(&state, &current_user, &page.rows).await?;
    Ok(R::ok_with_data(PageResult::from_page_rows(&page, rows)))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn get_info(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(business_id): Path<String>,
) -> Result<R<FlowInstanceVo>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:instance:query",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user));
    let instance = state
        .ins_service
        .get_by_business_id(&dctx, &business_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "实例不存在 (business_id={})",
                business_id
            ))
        })?;
    let mut vo = FlowInstanceVo::from(&instance);
    state.ins_service.fill_instance_vo(&dctx, &mut vo).await?;
    Ok(R::ok_with_data(vo))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn delete_by_business_ids(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:instance:remove",
    )?;
    let id_list: Vec<String> = ids
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    if id_list.is_empty() {
        return Err(AppError::BadRequest("业务ID不能为空".to_string()));
    }
    let dctx = user_ctx(&state, &CurrentUser(login_user));
    state
        .ins_service
        .delete_by_business_ids(&dctx, &id_list)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state, login_user))]
pub async fn delete_by_instance_ids(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:instance:remove",
    )?;
    let id_list: Vec<i64> = ids
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse()
                .map_err(|_| AppError::BadRequest(format!("无效的ID: {}", s)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if id_list.is_empty() {
        return Err(AppError::BadRequest("实例ID不能为空".to_string()));
    }
    let dctx = user_ctx(&state, &CurrentUser(login_user));
    state
        .ins_service
        .delete_by_instance_ids(&dctx, &id_list)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state, login_user))]
pub async fn delete_his_by_instance_ids(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(ids): Path<String>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:instance:remove",
    )?;
    let id_list: Vec<i64> = ids
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse()
                .map_err(|_| AppError::BadRequest(format!("无效的ID: {}", s)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if id_list.is_empty() {
        return Err(AppError::BadRequest("实例ID不能为空".to_string()));
    }
    // 删除历史任务走连接路径软删,不盖审计(his_task_repo 文件头裁定),
    // 登录用户仅用于权限校验,不再取 OperatorContext。
    let dctx = user_ctx(&state, &CurrentUser(login_user));
    state
        .ins_service
        .delete_his_by_instance_ids(&dctx, &id_list)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn cancel_process_apply(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<FlowCancelBo>,
) -> Result<R<Instance>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:instance:cancel",
    )?;
    let current_user = CurrentUser(login_user);
    let op = to_ctx(&current_user);
    let dctx = user_ctx(&state, &current_user);
    let instance = match (bo.business_id, bo.instance_id) {
        (Some(bid), _) => {
            state.ins_service.get_by_business_id(&dctx, &bid).await?
        }
        (None, Some(id)) => state.ins_service.get_by_id(&dctx, id).await?,
        (None, None) => {
            return Err(AppError::BadRequest("缺少业务ID或实例ID".to_string()));
        }
    }
    .ok_or_else(|| AppError::NotFound("实例不存在".to_string()))?;

    let params = FlowParams {
        message: bo.message,
        ..Default::default()
    };
    let instance = state
        .ins_service
        .revoke(&dctx, instance.id.unwrap_or(0), &params, &op)
        .await?;
    Ok(R::ok_with_data(instance))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn active_instance(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(id): Path<i64>,
    Query(query): Query<ActiveQuery>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:instance:active",
    )?;
    let current_user = CurrentUser(login_user);
    let op = to_ctx(&current_user);
    let dctx = user_ctx(&state, &current_user);
    state
        .ins_service
        .set_active(&dctx, id, query.active, &op)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state, login_user))]
pub async fn flow_his_task_list(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(business_id): Path<String>,
) -> Result<R<serde_json::Value>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:instance:query",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user));
    let result = state
        .ins_service
        .flow_his_task_list(&dctx, &business_id)
        .await?;
    Ok(R::ok_with_data(result))
}

#[tracing::instrument(skip(state, login_user))]
pub async fn instance_variable(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(instance_id): Path<i64>,
) -> Result<R<serde_json::Value>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:instance:variableQuery",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user));
    let variable = state.ins_service.get_variable(&dctx, instance_id).await?;
    Ok(R::ok_with_data(variable))
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn update_variable(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<FlowVariableBo>,
) -> Result<R<()>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:instance:variable",
    )?;
    let current_user = CurrentUser(login_user);
    let op = to_ctx(&current_user);
    let dctx = user_ctx(&state, &current_user);
    state
        .ins_service
        .update_variable(&dctx, bo.instance_id, &bo.variable, &op)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state, login_user, bo))]
pub async fn invalid_instance(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    AppJson(bo): AppJson<FlowInvalidBo>,
) -> Result<R<Instance>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:instance:invalid",
    )?;
    let current_user = CurrentUser(login_user);
    let op = to_ctx(&current_user);
    let dctx = user_ctx(&state, &current_user);
    let instance = state
        .ins_service
        .invalid(
            &dctx,
            bo.business_id.as_deref(),
            bo.instance_id,
            bo.comment.as_deref(),
            &op,
        )
        .await?;
    Ok(R::ok_with_data(instance))
}
