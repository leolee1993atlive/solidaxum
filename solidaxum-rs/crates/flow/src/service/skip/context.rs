use std::collections::HashMap;

use app_core::error::{AppError, AppResult};
use app_data::DataContext;
use serde_json::Value;

use crate::{
    dto::flow_params::FlowParams,
    enums::{ActivityStatus, NodeType},
    model::{
        definition::Definition, instance::Instance, node::Node, skip::Skip,
        task::Task, user::User,
    },
    repository::{
        definition_repo::DefinitionRepository,
        instance_repo::InstanceRepository, node_repo::NodeRepository,
        skip_repo::SkipRepository, task_repo::TaskRepository,
        user_repo::UserRepository,
    },
    service::definition_service::OperatorContext,
};

/// 一次 `skip` 流转全程共享的上下文。
///
/// `load` 成功后所有字段都是确定值:没有 `Option` 占位,
/// 也就没有"上一步填好了下一步才能 unwrap"的隐式契约。
///
/// `load` 收 `&DataContext<'_>`,在**事务外**执行(守卫提升,形态论证见
/// task_service 文件头)——其 NotFound/BadRequest 文案(任务不存在/挂起/
/// 已结束)在闭包外以原文案抛出,闭包内只剩写路径的 DataError。
/// `active_tasks` 字段:汇聚网关 AND-join 检测所需的当前活跃任务快照,
/// load 时预读,resolve_next 据此在事务外判定(READ COMMITTED 下无锁读,
/// 提升不改变结果)。
pub(crate) struct SkipContext {
    pub task: Task,
    pub instance: Instance,
    pub definition: Definition,
    pub now_node: Node,
    pub all_nodes: Vec<Node>,
    pub all_skips: Vec<Skip>,
    pub task_users: Vec<User>,
    /// 实例当前活跃任务(join 检测快照,load 时读取)
    pub active_tasks: Vec<Task>,
    /// `handler` 已回填为当前操作人
    pub params: FlowParams,
    pub variables: HashMap<String, Value>,
    /// `variables` 的 JSON 序列化结果,写 his / instance 时复用
    pub variables_json: String,
    /// commit_advance 在事务闭包内采集的新任务简报
    /// (node_name, 最终 permission_list);闭包提交成功后由调用方经
    /// `notify_new_tasks` 推送 SSE(事务外,失败仅告警)。
    pub created_task_briefs: Vec<(String, Vec<String>)>,
}

impl SkipContext {
    pub(crate) async fn load(
        ctx: &DataContext<'_>,
        task_id: i64,
        flow_params: &FlowParams,
        op: &OperatorContext,
    ) -> AppResult<Self> {
        let task = TaskRepository::select_by_id(ctx, task_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("任务不存在 (id={})", task_id))
            })?;

        let instance = InstanceRepository::select_by_id(ctx, task.instance_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "实例不存在 (id={})",
                    task.instance_id
                ))
            })?;

        let definition =
            DefinitionRepository::select_by_id(ctx, task.definition_id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!(
                        "流程定义不存在 (id={})",
                        task.definition_id
                    ))
                })?;

        if definition.activity_status != ActivityStatus::Active as i8 {
            return Err(AppError::BadRequest("流程定义已挂起".to_string()));
        }
        if instance.activity_status != ActivityStatus::Active as i8 {
            return Err(AppError::BadRequest("流程实例已挂起".to_string()));
        }
        if NodeType::from_i8(instance.node_type).is_some_and(|t| t.is_end()) {
            return Err(AppError::BadRequest(
                "流程已结束，不能跳转".to_string(),
            ));
        }

        let mut variables: HashMap<String, Value> = match &instance.variable {
            Some(v) if !v.is_empty() => {
                serde_json::from_str(v).unwrap_or_default()
            }
            _ => HashMap::new(),
        };
        for (k, v) in &flow_params.variable {
            variables.insert(k.clone(), v.clone());
        }

        // 弹窗选人合并(对齐 Java CompletePrepareComponent.setPopAssigneeMap):
        // assignee_map 的 key 是下一节点编码;与存量裸键变量并集后双写
        // pass:{code} / back:{code},建任务时按办理方向消费
        if let Some(assignee_map) = &flow_params.assignee_map {
            for (code, val) in assignee_map {
                let mut ids = crate::expression::handler::value_to_id_list(val);
                if let Some(existing) = variables.get(code) {
                    ids.extend(crate::expression::handler::value_to_id_list(
                        existing,
                    ));
                }
                ids.sort_unstable();
                ids.dedup();
                let joined = ids.join(",");
                variables.insert(
                    format!("pass:{code}"),
                    Value::String(joined.clone()),
                );
                variables.insert(format!("back:{code}"), Value::String(joined));
            }
        }

        let variables_json = serde_json::to_string(&variables)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let all_nodes =
            NodeRepository::select_by_definition_id(ctx, task.definition_id)
                .await?;
        let all_skips =
            SkipRepository::select_by_definition_id(ctx, task.definition_id)
                .await?;

        let now_node = all_nodes
            .iter()
            .find(|n: &&Node| n.node_code == task.node_code)
            .cloned()
            .ok_or_else(|| {
                AppError::NotFound(format!("节点不存在: {}", task.node_code))
            })?;

        let task_users =
            UserRepository::select_by_task_id(ctx, task_id).await?;

        let active_tasks =
            TaskRepository::select_by_instance_id(ctx, task.instance_id)
                .await?;

        let mut params = flow_params.clone();
        if params.handler.is_none() {
            params.handler = Some(op.user_id.clone());
        }

        Ok(Self {
            task,
            instance,
            definition,
            now_node,
            all_nodes,
            all_skips,
            task_users,
            active_tasks,
            params,
            variables,
            variables_json,
            created_task_briefs: Vec::new(),
        })
    }

    /// 当前办理人。`load` 保证 `params.handler` 非 `None`。
    pub(crate) fn handler(&self) -> &str {
        self.params.handler.as_deref().unwrap_or("")
    }
}
