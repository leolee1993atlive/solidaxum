//! 流程实例服务 —— instance 链经 app-data 漏斗执行。
//!
//! # 事务形状:守卫提升事务外,写半边单闭包
//!
//! 统一形状与 definition_service 相同:**守卫(纯读)提升事务外,
//! 写半边单闭包**(menu_handler 删除级联先例:READ COMMITTED 是语句级快照,
//! 守卫写在事务内同样挡不住 check-then-act 竞态,移出不改变任何结果;
//! 带前端契约文案的业务错误在闭包外以原文案抛出,闭包内只剩 repo 调用的
//! DataError,经 `From<DataError>` 恢复 HTTP 语义)。
//!
//! # 0 行语义与删除形态
//!
//! - **任务/办理人清理:逻辑删**(`soft_delete_by_*`,`@TableLogic` 平价,
//!   理由见 task_repo/user_repo 文件头):0 行 → Ok(0) 被忽略;同事务重读走
//!   `_in_txn` canonical 变体(软删存活过滤),读不见被删行,与硬删后重读等价。
//! - `update_by_id` / `update_by_id_in_txn`(revoke/termination/invalid/
//!   set_active/update_variable):漏斗 update 的 WHERE = 主键 + 软删存活,
//!   0 行 → `DataError::NotFound`(404;触发点都是「读后被并发删除」的
//!   竞争窗口);更新已删实例不会静默复活。
//! - `soft_delete_by_ids` / `soft_delete_by_business_ids`:
//!   `soft_delete_where` 自带 alive 过滤,重删已删实例 rows_affected=0
//!   (instance_repo_test 判例钉住)。
//! - `delete_his_by_instance_ids`:`soft_delete_where` 逻辑删(@TableLogic
//!   平价,理由见 his_task_repo 文件头;墓碑行保留,读侧全滤 del_flag='0')。
//!
//! # 回调错误通道
//!
//! `on_started` / `notify_business_status` 返回 `AppResult`,闭包错误类型是
//! `DataError` —— 按 shared.rs 判例同款映射 `DataError::Unsupported(
//! format!("业务回写失败:{e}"))`:回滚触发不变(闭包 Err ⇒ rollback),
//! 错误呈现不变(同为 500)。
//!
//! # 审计盖章
//!
//! A 组实体的审计列类型与漏斗盖章签名不匹配(policy.rs 双组裁定)→
//! `OperatorContext` 保留,审计字段由本服务手工盖。
//! `DataContext` 只负责执行面(连接/事务/软删接管)。

use std::{collections::HashMap, sync::Arc};

use app_core::{
    error::{AppError, AppResult},
    util::snowflake,
};
use app_data::{DataContext, DataError};
use serde_json::Value;

use crate::{
    dto::flow_params::FlowParams,
    engine::{chart, graph, runtime::one_vote_veto, transition::PathWayData},
    enums::{
        ActivityStatus, CooperateType, FlowStatus, NodeType, PublishStatus,
        SkipType, UserType,
    },
    expression::{
        condition::prefetch_spel_context,
        handler::{apply_next_handler, expand_handlers, resolve_handlers},
    },
    listener::{
        trigger::trigger_listeners,
        types::{LISTENER_ASSIGNMENT, LISTENER_CREATE, ListenerVariable},
    },
    model::{
        convert::entities_to_def_json, dto::InstancePageFilter,
        instance::Instance, task::Task, user::User,
    },
    notifier::FlowNotifier,
    repository::{
        definition_repo::DefinitionRepository,
        his_task_repo::HisTaskRepository, instance_repo::InstanceRepository,
        node_repo::NodeRepository, skip_repo::SkipRepository,
        task_repo::TaskRepository, user_repo::UserRepository,
    },
    resolver::{BusinessStatusCallback, UserResolver},
    service::{
        definition_service::OperatorContext,
        his_task_service::HisTaskService,
        shared::{
            final_status_label, notify_business_status, notify_new_tasks,
            set_flow_status,
        },
        skip::persist::write_back_task_permissions,
    },
};

/// 构建启动流程的初始变量。
///
/// `initiator` / `initiatorDeptId` / `businessId` 由服务端注入,并**覆盖**调用方
/// 传入的同名变量。首节点办理人表达式 `${initiator}` 依赖它——缺失会导致
/// 表达式留为字面量,发起人办理首节点被判 403。
pub(crate) fn build_start_variables(
    mut variables: HashMap<String, Value>,
    initiator: i64,
    initiator_dept_id: Option<i64>,
    business_id: &str,
) -> HashMap<String, Value> {
    variables.insert(
        "initiator".to_string(),
        Value::String(initiator.to_string()),
    );
    if let Some(dept_id) = initiator_dept_id {
        variables.insert(
            "initiatorDeptId".to_string(),
            Value::Number(dept_id.into()),
        );
    }
    variables.insert(
        "businessId".to_string(),
        Value::String(business_id.to_string()),
    );
    variables
}

/// 执行面全部经请求期 `DataContext`(handler 按登录用户构造);
/// 本服务只持有流程编排依赖,不再持有数据库连接。
#[derive(Clone)]
pub struct InstanceService {
    registry: Arc<crate::listener::ListenerRegistry>,
    permission: Arc<dyn crate::permission::PermissionHandler>,
    resolver: Arc<dyn UserResolver>,
    business_callback: Arc<dyn BusinessStatusCallback>,
    notifier: Arc<dyn FlowNotifier>,
}

impl InstanceService {
    pub fn new(
        registry: Arc<crate::listener::ListenerRegistry>,
        permission: Arc<dyn crate::permission::PermissionHandler>,
        resolver: Arc<dyn UserResolver>,
        business_callback: Arc<dyn BusinessStatusCallback>,
        notifier: Arc<dyn FlowNotifier>,
    ) -> Self {
        Self {
            registry,
            permission,
            resolver,
            business_callback,
            notifier,
        }
    }

    /// 获取权限处理器（允许外部访问以进行权限扩展）
    pub fn permission(&self) -> &Arc<dyn crate::permission::PermissionHandler> {
        &self.permission
    }

    /// 保存流程实例业务扩展信息。
    pub async fn save_biz_ext(
        &self,
        ctx: &DataContext<'_>,
        instance_id: i64,
        business_id: &str,
        biz_ext: &crate::model::dto::FlowInstanceBizExtBo,
        op: &OperatorContext,
    ) -> AppResult<()> {
        crate::service::business_fields::save_instance_biz_ext(
            ctx,
            instance_id,
            business_id,
            biz_ext.business_code.clone(),
            biz_ext.business_title.clone(),
            op.user_id.parse().ok(),
        )
        .await
    }

    /// 启动流程实例
    #[tracing::instrument(skip(self, ctx, flow_params, op), fields(business_id = business_id))]
    pub async fn start(
        &self,
        ctx: &DataContext<'_>,
        business_id: &str,
        flow_params: &FlowParams,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        // 1. 提取 flow_code
        let flow_code = flow_params.flow_code.as_deref().ok_or_else(|| {
            AppError::BadRequest("缺少流程编码 flow_code".to_string())
        })?;

        // 2. 查找已发布定义(纯读守卫,在事务外)
        let definitions =
            DefinitionRepository::select_published_by_flow_code(ctx, flow_code)
                .await?;
        let def = definitions
            .into_iter()
            .find(|d| d.is_publish == PublishStatus::Published as i8)
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "未找到已发布的流程定义: {}",
                    flow_code
                ))
            })?;

        let def_id = def.id.expect("id 由雪花生成或查询回填,必有值");

        // 6. 检查定义激活状态
        if def.activity_status != ActivityStatus::Active as i8 {
            return Err(AppError::BadRequest("流程定义已挂起".to_string()));
        }

        // 3. 加载节点和跳转(纯读)
        let all_nodes =
            NodeRepository::select_by_definition_id(ctx, def_id).await?;
        let all_skips =
            SkipRepository::select_by_definition_id(ctx, def_id).await?;

        // 5. 查找开始节点
        let start_node = all_nodes
            .iter()
            .find(|n| n.node_type == NodeType::Start as i8)
            .ok_or_else(|| {
                AppError::BadRequest("流程定义缺少开始节点".to_string())
            })?;

        // 准备公共值
        let now = op.now();
        let instance_id = snowflake::next_id();
        let variable_str = serde_json::to_string(&flow_params.variable)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        // 7-9. 从开始节点导航
        let mut path_way = PathWayData {
            def_id,
            ins_id: Some(instance_id),
            skip_type: Some(SkipType::Pass.as_str().to_string()),
            ..Default::default()
        };

        let next_node = graph::get_next_node(
            start_node,
            None,
            SkipType::Pass.as_str(),
            &mut path_way,
            &all_skips,
            &all_nodes,
        )
        .ok_or_else(|| {
            AppError::BadRequest("开始节点后无有效跳转".to_string())
        })?;

        // 预热 hasRole 角色快照(条件里有 hasRole 时才现查;事务外纯读)
        let spel_ctx = prefetch_spel_context(
            &*self.resolver,
            &all_skips,
            &flow_params.variable,
        )
        .await;
        let next_nodes = graph::get_next_by_check_gateway(
            &next_node,
            &flow_params.variable,
            &mut path_way,
            &all_skips,
            &all_nodes,
            &spel_ctx,
        )?;

        if next_nodes.is_empty() {
            return Err(AppError::BadRequest("无法确定下一节点".to_string()));
        }

        // 设置目标节点（图表生成前）
        path_way.target_nodes = next_nodes.clone();

        // 12. 创建实例
        let mut instance = Instance {
            id: Some(instance_id),
            definition_id: def_id,
            business_id: business_id.to_string(),
            node_type: next_nodes[0].node_type,
            node_code: next_nodes[0].node_code.clone(),
            node_name: next_nodes[0].node_name.clone(),
            variable: Some(variable_str.clone()),
            flow_status: flow_params
                .flow_status
                .clone()
                .unwrap_or_else(|| FlowStatus::ToBeSubmit.as_str().to_string()),
            activity_status: ActivityStatus::Active as i8,
            def_json: None,
            create_time: Some(now),
            create_by: Some(op.user_id.clone()),
            update_time: Some(now),
            update_by: Some(op.user_id.clone()),
            ext: None,
            del_flag: Some("0".to_string()),
            tenant_id: op.tenant_id.clone(),
        };

        // 13. 构建开始节点的历史记录
        let start_task = Task {
            id: Some(0),
            definition_id: def_id,
            instance_id,
            node_code: start_node.node_code.clone(),
            node_name: start_node.node_name.clone(),
            node_type: start_node.node_type,
            flow_status: FlowStatus::ToBeSubmit.as_str().to_string(),
            form_custom: start_node.form_custom.clone(),
            form_path: start_node.form_path.clone(),
            create_time: Some(now),
            create_by: Some(op.user_id.clone()),
            update_time: Some(now),
            update_by: Some(op.user_id.clone()),
            del_flag: Some("0".to_string()),
            tenant_id: op.tenant_id.clone(),
            permission_list: vec![],
        };

        let his_task = HisTaskService::build_skip_his(
            &start_task,
            &next_nodes,
            flow_params,
            &variable_str,
        );

        // 14. 为非结束节点创建任务
        let mut tasks: Vec<Task> = Vec::new();
        for node in &next_nodes {
            if NodeType::from_i8(node.node_type).is_some_and(|t| t.is_end()) {
                continue;
            }
            let resolved = resolve_handlers(
                &node.permission_flag.clone().unwrap_or_default(),
                &flow_params.variable,
            );
            let mut permission_list = apply_next_handler(
                resolved,
                &flow_params.next_handler,
                flow_params.next_handler_append,
            );
            if CooperateType::is_sequence(
                node.node_ratio.as_deref().unwrap_or(""),
            ) {
                permission_list.truncate(1);
            }
            let permission_list = expand_handlers(
                &*self.resolver,
                permission_list,
                Some(&crate::resolver::InstanceAssigneeContext {
                    instance_id,
                    target_node_code: node.node_code.clone(),
                    variable: flow_params.variable.clone(),
                    // 实例启动:无在途已办步,引用的 his 必已提交
                    current: None,
                }),
            )
            .await;

            let task = Task {
                id: Some(snowflake::next_id()),
                definition_id: def_id,
                instance_id,
                node_code: node.node_code.clone(),
                node_name: node.node_name.clone(),
                node_type: node.node_type,
                flow_status: set_flow_status(
                    node.node_type,
                    SkipType::Pass.as_str(),
                ),
                form_custom: node
                    .form_custom
                    .clone()
                    .or(def.form_custom.clone()),
                form_path: node.form_path.clone().or(def.form_path.clone()),
                create_time: Some(now),
                create_by: Some(op.user_id.clone()),
                update_time: Some(now),
                update_by: Some(op.user_id.clone()),
                del_flag: Some("0".to_string()),
                tenant_id: op.tenant_id.clone(),
                permission_list,
            };
            tasks.push(task);
        }

        // 15. 生成流程图 JSON
        let design_json = entities_to_def_json(&def, &all_nodes, &all_skips);
        let chart_json = chart::start_metadata(&design_json, &path_way);
        instance.def_json = Some(
            serde_json::to_string(&chart_json)
                .map_err(|e| AppError::Internal(e.to_string()))?,
        );

        // 16. 流程用户(办理人用户行)在监听器之后按最终 permission_list
        // 构建——监听器可能改写 next_tasks 办理人(回灌),提前建会被丢弃

        // 17. 监听器：assignment（对每个新任务节点触发）
        // 监听器对变量的修改回灌到 eff_vars(对齐参考实现监听器重置
        // 流程变量的用法),最终写回实例变量;对 lv.next_tasks 办理人
        // 的改写按 task id 回灌到 tasks(仅 permission_list 生效),
        // 监听器拿到的办理人是已展开的裸用户 ID
        let mut eff_vars = flow_params.variable.clone();
        {
            let mut lv = ListenerVariable::new(
                def.clone(),
                instance.clone(),
                eff_vars.clone(),
                flow_params.clone(),
            );
            lv.next_nodes = next_nodes.clone();
            lv.next_tasks = tasks.clone();

            trigger_listeners(
                &self.registry,
                def.listener_type.as_deref(),
                def.listener_path.as_deref(),
                LISTENER_ASSIGNMENT,
                &mut lv,
            )
            .await?;
            write_back_task_permissions(&mut tasks, &lv.next_tasks);
            eff_vars = lv.variable.clone();

            for node in &next_nodes {
                let mut node_lv = lv.clone();
                node_lv.node = Some(node.clone());
                trigger_listeners(
                    &self.registry,
                    node.listener_type.as_deref(),
                    node.listener_path.as_deref(),
                    LISTENER_ASSIGNMENT,
                    &mut node_lv,
                )
                .await?;
                write_back_task_permissions(&mut tasks, &node_lv.next_tasks);
                eff_vars = node_lv.variable;
            }
        }

        // 18. 监听器：create（每个新任务）
        // 配对按 node_code 关联(任务构建时跳过结束节点,位置索引会错位)
        {
            for i in 0..tasks.len() {
                let Some(node) = next_nodes
                    .iter()
                    .find(|n| n.node_code == tasks[i].node_code)
                    .cloned()
                else {
                    continue;
                };
                let mut lv = ListenerVariable::new(
                    def.clone(),
                    instance.clone(),
                    eff_vars.clone(),
                    flow_params.clone(),
                );
                lv.node = Some(node.clone());
                lv.task = Some(tasks[i].clone());
                lv.next_tasks = tasks.clone();

                trigger_listeners(
                    &self.registry,
                    def.listener_type.as_deref(),
                    def.listener_path.as_deref(),
                    LISTENER_CREATE,
                    &mut lv,
                )
                .await?;

                trigger_listeners(
                    &self.registry,
                    node.listener_type.as_deref(),
                    node.listener_path.as_deref(),
                    LISTENER_CREATE,
                    &mut lv,
                )
                .await?;
                // 定义级与节点级监听器共用同一个 lv,修改累积后回灌
                write_back_task_permissions(&mut tasks, &lv.next_tasks);
                eff_vars = lv.variable;
            }
        }

        // 监听器回灌落进实例变量快照(覆盖 12 步的初值),并按最终
        // permission_list 重建办理人用户行
        instance.variable = Some(
            serde_json::to_string(&eff_vars)
                .map_err(|e| AppError::Internal(e.to_string()))?,
        );
        // 新待办简报与办理人用户行同口径:均为监听器回写后的最终
        // permission_list;事务提交成功后由调用方推送(失败仅告警)
        let briefs: Vec<(String, Vec<String>)> = tasks
            .iter()
            .map(|t| {
                (
                    t.node_name.clone().unwrap_or_default(),
                    t.permission_list.clone(),
                )
            })
            .collect();
        let mut users: Vec<User> = Vec::new();
        for task in &tasks {
            for permission in &task.permission_list {
                let user = User {
                    id: Some(snowflake::next_id()),
                    type_: UserType::Approval as i8,
                    processed_by: Some(permission.clone()),
                    associated: task
                        .id
                        .expect("id 由雪花生成或查询回填,必有值"),
                    create_time: Some(now),
                    create_by: Some(op.user_id.clone()),
                    update_time: Some(now),
                    update_by: Some(op.user_id.clone()),
                    del_flag: Some("0".to_string()),
                    tenant_id: op.tenant_id.clone(),
                };
                users.push(user);
            }
        }

        // 19. 事务保存(写半边单闭包)。业务状态回写(提交即 WAITING,对齐
        // RuoYi)在同一事务内完成:任一步失败整笔回滚,流程启动与业务状态
        // 保持原子一致。owned 值先进闭包(transaction 的 HRTB 不收借用)。
        let instance_in = instance.clone();
        let business_id_in = instance.business_id.clone();
        let flow_code_in = flow_code.to_string();
        let callback = self.business_callback.clone();
        ctx.transaction(|tx| {
            Box::pin(async move {
                InstanceRepository::insert_in_txn(tx, &instance_in).await?;
                TaskRepository::insert_batch_in_txn(tx, &tasks).await?;
                UserRepository::insert_batch_in_txn(tx, &users).await?;
                HisTaskRepository::insert_in_txn(tx, &his_task).await?;
                if !business_id_in.is_empty() {
                    callback
                        .on_started(tx, &flow_code_in, &business_id_in)
                        .await
                        .map_err(|e| {
                            DataError::Unsupported(format!("业务回写失败:{e}"))
                        })?;
                }
                Ok(())
            })
        })
        .await?;

        // 事务提交成功后才通知新待办(尽力而为,失败仅 warn,见 shared.rs)
        notify_new_tasks(&self.notifier, &briefs).await;

        Ok(instance)
    }

    /// 启动业务流程。
    ///
    /// 编排四步:注入发起人变量 → 启动实例 → 保存业务扩展 → 取回首个任务。
    /// 对齐 RuoYi `FlwTaskServiceImpl#startWorkFlow`(不含"已存在实例走续提交"
    /// 分支,见计划文档"后续待办 #1")。
    pub async fn start_work_flow(
        &self,
        ctx: &DataContext<'_>,
        bo: &crate::model::dto::StartProcessBo,
        initiator: i64,
        initiator_dept_id: Option<i64>,
        op: &OperatorContext,
    ) -> AppResult<crate::model::dto::StartProcessReturnDTO> {
        let params = FlowParams {
            flow_code: Some(bo.flow_code.clone()),
            variable: build_start_variables(
                bo.variables.clone(),
                initiator,
                initiator_dept_id,
                &bo.business_id,
            ),
            ..Default::default()
        };

        let instance = self.start(ctx, &bo.business_id, &params, op).await?;
        let instance_id = instance.id.unwrap_or(0);

        self.save_biz_ext(ctx, instance_id, &bo.business_id, &bo.biz_ext, op)
            .await?;

        let tasks =
            TaskRepository::select_by_instance_id(ctx, instance_id).await?;
        let task_id = tasks.first().and_then(|t| t.id).unwrap_or(0);

        Ok(crate::model::dto::StartProcessReturnDTO {
            process_instance_id: instance_id,
            task_id,
        })
    }

    /// 查询单个实例
    pub async fn get_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<Option<Instance>> {
        Ok(InstanceRepository::select_by_id(ctx, id).await?)
    }

    /// 查询实例列表分页
    pub async fn list_page(
        &self,
        ctx: &DataContext<'_>,
        flow_code: Option<&str>,
        business_id: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<Instance>> {
        Ok(InstanceRepository::select_page(
            ctx,
            flow_code,
            business_id,
            page_no,
            page_size,
        )
        .await?)
    }

    /// 撤销：发起人将流程回退到开始节点后第一个任务节点
    #[tracing::instrument(skip(self, ctx, flow_params, op), fields(instance_id = instance_id))]
    pub async fn revoke(
        &self,
        ctx: &DataContext<'_>,
        instance_id: i64,
        flow_params: &FlowParams,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        // 纯读 + 守卫全部在事务外(D1 先例,见文件头);写半边收进单闭包。
        let mut instance = InstanceRepository::select_by_id(ctx, instance_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("实例不存在 (id={})", instance_id))
            })?;

        if !flow_params.ignore
            && instance.create_by.as_deref() != Some(op.user_id.as_str())
        {
            return Err(AppError::Forbidden(
                "只有流程发起人可以撤销".to_string(),
            ));
        }

        if NodeType::from_i8(instance.node_type).is_some_and(|t| t.is_end()) {
            return Err(AppError::BadRequest(
                "流程已结束，不能撤销".to_string(),
            ));
        }

        let all_nodes = NodeRepository::select_by_definition_id(
            ctx,
            instance.definition_id,
        )
        .await?;
        let all_skips = SkipRepository::select_by_definition_id(
            ctx,
            instance.definition_id,
        )
        .await?;

        let start_node = all_nodes
            .iter()
            .find(|n| {
                NodeType::from_i8(n.node_type).is_some_and(|t| t.is_start())
            })
            .ok_or_else(|| AppError::Internal("找不到开始节点".to_string()))?;

        let first_skip = all_skips
            .iter()
            .find(|s| s.now_node_code == start_node.node_code)
            .ok_or_else(|| {
                AppError::BadRequest("开始节点无出边".to_string())
            })?;
        let first_node = all_nodes
            .iter()
            .find(|n| n.node_code == first_skip.next_node_code)
            .ok_or_else(|| {
                AppError::Internal("找不到首个任务节点".to_string())
            })?;

        let now = op.now();

        let resolved = resolve_handlers(
            &first_node.permission_flag.clone().unwrap_or_default(),
            &flow_params.variable,
        );
        let permission_list = apply_next_handler(
            resolved,
            &flow_params.next_handler,
            flow_params.next_handler_append,
        );
        let permission_list = expand_handlers(
            &*self.resolver,
            permission_list,
            Some(&crate::resolver::InstanceAssigneeContext {
                instance_id,
                target_node_code: first_node.node_code.clone(),
                variable: flow_params.variable.clone(),
                // 实例启动:无在途已办步,引用的 his 必已提交
                current: None,
            }),
        )
        .await;

        let new_task = Task {
            id: Some(snowflake::next_id()),
            definition_id: instance.definition_id,
            instance_id,
            node_code: first_node.node_code.clone(),
            node_name: first_node.node_name.clone(),
            node_type: first_node.node_type,
            flow_status: FlowStatus::Approval.as_str().to_string(),
            form_custom: first_node.form_custom.clone(),
            form_path: first_node.form_path.clone(),
            create_time: Some(now),
            create_by: Some(op.user_id.clone()),
            update_time: Some(now),
            update_by: Some(op.user_id.clone()),
            del_flag: Some("0".to_string()),
            tenant_id: op.tenant_id.clone(),
            permission_list: permission_list.clone(),
        };

        let fake_revoke_task = Task {
            id: Some(0),
            definition_id: instance.definition_id,
            instance_id,
            node_code: instance.node_code.clone(),
            node_name: instance.node_name.clone(),
            node_type: instance.node_type,
            flow_status: FlowStatus::Retrieve.as_str().to_string(),
            form_custom: None,
            form_path: None,
            create_time: Some(now),
            create_by: Some(op.user_id.clone()),
            update_time: Some(now),
            update_by: Some(op.user_id.clone()),
            del_flag: Some("0".to_string()),
            tenant_id: op.tenant_id.clone(),
            permission_list: vec![],
        };

        let his_task = HisTaskService::build_skip_his(
            &fake_revoke_task,
            std::slice::from_ref(first_node),
            flow_params,
            &serde_json::to_string(&flow_params.variable).unwrap_or_default(),
        );

        let new_users: Vec<User> = permission_list
            .iter()
            .map(|p| User {
                id: Some(snowflake::next_id()),
                type_: UserType::Approval as i8,
                processed_by: Some(p.clone()),
                associated: new_task
                    .id
                    .expect("id 由雪花生成或查询回填,必有值"),
                create_time: Some(now),
                create_by: Some(op.user_id.clone()),
                update_time: Some(now),
                update_by: Some(op.user_id.clone()),
                del_flag: Some("0".to_string()),
                tenant_id: op.tenant_id.clone(),
            })
            .collect();

        instance.node_code = first_node.node_code.clone();
        instance.node_name = first_node.node_name.clone();
        instance.node_type = first_node.node_type;
        instance.flow_status = FlowStatus::Retrieve.as_str().to_string();
        instance.update_time = Some(now);
        instance.update_by = Some(op.user_id.clone());

        // 撤销后回写业务状态 cancel(对齐 Java FlwInstanceServiceImpl
        // .cancelProcessApply,其 FlowParams 显式带 BusinessStatusEnum.CANCEL)。
        // revoke 恒置 Retrieve → final_status_label 恒为 Some("cancel"),
        // 定义读取(取 flow_code)相应恒触发,可提升到事务外。
        let status = final_status_label(&instance.flow_status);
        let flow_code = if status.is_some() {
            let def =
                DefinitionRepository::select_by_id(ctx, instance.definition_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::NotFound(format!(
                            "流程定义不存在 (id={})",
                            instance.definition_id
                        ))
                    })?;
            Some(def.flow_code)
        } else {
            None
        };

        // 写半边单闭包:一票否决 → 清理剩余任务(逻辑删,@TableLogic 平价,
        // 见文件头「0 行语义与删除形态」) → 新任务/用户/历史落库 → 实例回写 → 业务状态回写。
        let first_node_code = first_node.node_code.clone();
        let status = status.map(str::to_string);
        let callback = self.business_callback.clone();
        let returned = instance.clone();
        ctx.transaction(|tx| {
            Box::pin(async move {
                one_vote_veto(tx, instance_id, &first_node_code, &all_skips)
                    .await?;
                let remaining = TaskRepository::select_by_instance_id_in_txn(
                    tx,
                    instance_id,
                )
                .await?;
                for t in remaining {
                    let tid = t.id.unwrap_or(0);
                    UserRepository::soft_delete_by_task_id_in_txn(tx, tid)
                        .await?;
                    TaskRepository::soft_delete_by_id_in_txn(tx, tid).await?;
                }

                TaskRepository::insert_batch_in_txn(tx, &[new_task]).await?;
                UserRepository::insert_batch_in_txn(tx, &new_users).await?;
                InstanceRepository::update_by_id_in_txn(tx, &instance).await?;
                HisTaskRepository::insert_in_txn(tx, &his_task).await?;

                if let (Some(flow_code), Some(status)) = (flow_code, status) {
                    notify_business_status(
                        &*callback, tx, &flow_code, &instance, &status,
                    )
                    .await
                    .map_err(|e| {
                        DataError::Unsupported(format!("业务回写失败:{e}"))
                    })?;
                }
                Ok(())
            })
        })
        .await?;
        Ok(returned)
    }

    /// 终止：强制流转到结束节点，清理所有待办任务
    #[tracing::instrument(skip(self, ctx, flow_params, op), fields(instance_id = instance_id))]
    pub async fn termination(
        &self,
        ctx: &DataContext<'_>,
        instance_id: i64,
        flow_params: &FlowParams,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        // 纯读 + 守卫全部在事务外(D1 先例,见文件头);写半边收进单闭包。
        let mut instance = InstanceRepository::select_by_id(ctx, instance_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("实例不存在 (id={})", instance_id))
            })?;

        if NodeType::from_i8(instance.node_type).is_some_and(|t| t.is_end()) {
            return Err(AppError::BadRequest("流程已结束".to_string()));
        }

        let all_nodes = NodeRepository::select_by_definition_id(
            ctx,
            instance.definition_id,
        )
        .await?;

        let end_node = all_nodes
            .iter()
            .find(|n| {
                NodeType::from_i8(n.node_type).is_some_and(|t| t.is_end())
            })
            .ok_or_else(|| AppError::Internal("找不到结束节点".to_string()))?
            .clone();

        let now = op.now();

        instance.node_code = end_node.node_code.clone();
        instance.node_name = end_node.node_name.clone();
        instance.node_type = end_node.node_type;
        instance.flow_status = FlowStatus::Terminate.as_str().to_string();
        instance.update_time = Some(now);
        instance.update_by = Some(op.user_id.clone());

        // termination 恒置 Terminate → final_status_label 恒为
        // Some("termination"),定义读取(取 flow_code)恒触发,提升事务外。
        let status = final_status_label(&instance.flow_status);
        let flow_code = if status.is_some() {
            let def =
                DefinitionRepository::select_by_id(ctx, instance.definition_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::NotFound(format!(
                            "流程定义不存在 (id={})",
                            instance.definition_id
                        ))
                    })?;
            Some(def.flow_code)
        } else {
            None
        };

        // 写半边单闭包。his 的 source_task 取自事务内读到的活跃任务
        // (读删同闭包,快照一致),故 his 的拼装也在闭包内完成。
        let status = status.map(str::to_string);
        let callback = self.business_callback.clone();
        let returned = instance.clone();
        let flow_params = flow_params.clone();
        let variable_str =
            serde_json::to_string(&flow_params.variable).unwrap_or_default();
        let user_id = op.user_id.clone();
        let tenant_id = op.tenant_id.clone();
        ctx.transaction(|tx| {
            Box::pin(async move {
                let active_tasks =
                    TaskRepository::select_by_instance_id_in_txn(
                        tx,
                        instance_id,
                    )
                    .await?;
                for t in &active_tasks {
                    let tid = t.id.unwrap_or(0);
                    UserRepository::soft_delete_by_task_id_in_txn(tx, tid)
                        .await?;
                    TaskRepository::soft_delete_by_id_in_txn(tx, tid).await?;
                }

                let source_task =
                    active_tasks.into_iter().next().unwrap_or_else(|| Task {
                        id: Some(0),
                        definition_id: instance.definition_id,
                        instance_id,
                        node_code: instance.node_code.clone(),
                        node_name: instance.node_name.clone(),
                        node_type: instance.node_type,
                        flow_status: FlowStatus::Terminate.as_str().to_string(),
                        form_custom: None,
                        form_path: None,
                        create_time: Some(now),
                        create_by: Some(user_id.clone()),
                        update_time: Some(now),
                        update_by: Some(user_id.clone()),
                        del_flag: Some("0".to_string()),
                        tenant_id: tenant_id.clone(),
                        permission_list: vec![],
                    });

                let mut his = HisTaskService::build_skip_his(
                    &source_task,
                    std::slice::from_ref(&end_node),
                    &flow_params,
                    &variable_str,
                );
                his.flow_status = FlowStatus::Terminate.as_str().to_string();

                InstanceRepository::update_by_id_in_txn(tx, &instance).await?;
                HisTaskRepository::insert_in_txn(tx, &his).await?;

                if let (Some(flow_code), Some(status)) = (flow_code, status) {
                    notify_business_status(
                        &*callback, tx, &flow_code, &instance, &status,
                    )
                    .await
                    .map_err(|e| {
                        DataError::Unsupported(format!("业务回写失败:{e}"))
                    })?;
                }
                Ok(())
            })
        })
        .await?;
        Ok(returned)
    }

    /// 查询单个实例（按业务 ID）
    pub async fn get_by_business_id(
        &self,
        ctx: &DataContext<'_>,
        business_id: &str,
    ) -> AppResult<Option<Instance>> {
        Ok(InstanceRepository::select_by_business_id(ctx, business_id).await?)
    }

    /// 运行中实例分页
    pub async fn page_running(
        &self,
        ctx: &DataContext<'_>,
        filter: &InstancePageFilter,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<Instance>> {
        Ok(InstanceRepository::select_running_page(
            ctx, filter, page_no, page_size,
        )
        .await?)
    }

    /// 已结束实例分页
    pub async fn page_finish(
        &self,
        ctx: &DataContext<'_>,
        filter: &InstancePageFilter,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<Instance>> {
        Ok(InstanceRepository::select_finish_page(
            ctx, filter, page_no, page_size,
        )
        .await?)
    }

    /// 当前用户相关实例分页（发起人或参与者）
    pub async fn page_current(
        &self,
        ctx: &DataContext<'_>,
        user_id: &str,
        filter: &InstancePageFilter,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<Instance>> {
        Ok(InstanceRepository::select_current_page(
            ctx, user_id, filter, page_no, page_size,
        )
        .await?)
    }

    /// 按业务 ID 删除实例（软删除,`@TableLogic` 平价 —— 口径说明见
    /// instance_repo 文件头;重删已删实例 rows_affected=0,批量语义静默 Ok)
    pub async fn delete_by_business_ids(
        &self,
        ctx: &DataContext<'_>,
        business_ids: &[String],
    ) -> AppResult<()> {
        InstanceRepository::soft_delete_by_business_ids(ctx, business_ids)
            .await?;
        Ok(())
    }

    /// 按实例 ID 删除实例（软删除,语义同 [`Self::delete_by_business_ids`]）
    pub async fn delete_by_instance_ids(
        &self,
        ctx: &DataContext<'_>,
        ids: &[i64],
    ) -> AppResult<()> {
        InstanceRepository::soft_delete_by_ids(ctx, ids).await?;
        Ok(())
    }

    /// 按实例 ID 删除历史任务(`@TableLogic` 平价 —— **逻辑删**,理由见
    /// his_task_repo 文件头;0 行 → Ok(0) 被忽略)
    pub async fn delete_his_by_instance_ids(
        &self,
        ctx: &DataContext<'_>,
        instance_ids: &[i64],
    ) -> AppResult<()> {
        HisTaskRepository::soft_delete_by_instance_ids(ctx, instance_ids)
            .await?;
        Ok(())
    }

    /// 激活/挂起实例
    pub async fn set_active(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        active: bool,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        let mut instance = InstanceRepository::select_by_id(ctx, id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("实例不存在 (id={})", id))
            })?;
        instance.activity_status = if active {
            ActivityStatus::Active as i8
        } else {
            ActivityStatus::Suspended as i8
        };
        instance.update_time = Some(op.now());
        instance.update_by = Some(op.user_id.clone());
        InstanceRepository::update_by_id(ctx, &instance).await?;
        Ok(instance)
    }

    /// 流程历史任务列表（含流程图）
    pub async fn flow_his_task_list(
        &self,
        ctx: &DataContext<'_>,
        business_id: &str,
    ) -> AppResult<serde_json::Value> {
        let instance =
            InstanceRepository::select_by_business_id(ctx, business_id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!(
                        "实例不存在 (business_id={})",
                        business_id
                    ))
                })?;
        let instance_id = instance.id.unwrap_or(0);

        let his_tasks =
            HisTaskRepository::select_by_instance_id(ctx, instance_id).await?;
        let list: Vec<crate::model::dto::HisTaskListVo> = his_tasks
            .iter()
            .map(crate::model::dto::HisTaskListVo::from)
            .collect();

        // 对齐 Java FlwInstanceServiceImpl#flowHisTaskList:仅返回 {list, instanceId}。
        // 流程图由 /warm-flow/query-flow-chart/{instanceId} 提供,不在此接口。
        Ok(serde_json::json!({
            "list": list,
            "instanceId": instance_id,
        }))
    }

    /// 实例流程图数据(对齐 Java WarmFlowService.queryFlowChart)。
    /// 返回实例 def_json(DefJson,节点/跳转已带 status 高亮)并补齐状态三原色,
    /// 供 warm-flow 设计器 FlowChart 模式渲染。
    pub async fn flow_chart(
        &self,
        ctx: &DataContext<'_>,
        instance_id: i64,
    ) -> AppResult<crate::model::def_json::DefJson> {
        let instance = InstanceRepository::select_by_id(ctx, instance_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("实例不存在 (id={instance_id})"))
            })?;

        let mut def_json: crate::model::def_json::DefJson =
            match instance.def_json.as_deref() {
                Some(s) if !s.is_empty() => {
                    serde_json::from_str(s).map_err(|e| {
                        AppError::Internal(format!("解析流程图 JSON 失败: {e}"))
                    })?
                }
                _ => {
                    return Err(AppError::NotFound(
                        "流程图数据为空(def_json 缺失)".to_string(),
                    ));
                }
            };

        // 按权威运行态重算节点/跳转高亮,确保 complete/reject/back/terminate/invalid
        // 后流程图都正确:待办=当前活动任务节点,已办=历史任务节点(+到达的结束节点),
        // 其余未办。避免依赖各流转分支是否回写了 def_json 状态。
        use std::collections::HashSet;

        use crate::engine::chart::{DONE, NOT_DONE, TO_DO};

        let his_tasks =
            HisTaskRepository::select_by_instance_id(ctx, instance_id).await?;
        let active_tasks =
            TaskRepository::select_by_instance_id(ctx, instance_id).await?;

        let pending: HashSet<String> =
            active_tasks.iter().map(|t| t.node_code.clone()).collect();
        let done: HashSet<String> = his_tasks
            .iter()
            .filter_map(|h| h.node_code.clone())
            .collect();
        // 已流转到达的边 (now -> next) 及到达的目标节点。
        let traversed_edges: HashSet<(String, String)> = his_tasks
            .iter()
            .filter_map(|h| {
                Some((h.node_code.clone()?, h.target_node_code.clone()?))
            })
            .collect();
        let reached_targets: HashSet<String> = his_tasks
            .iter()
            .filter_map(|h| h.target_node_code.clone())
            .collect();

        for node in &mut def_json.node_list {
            let code = &node.node_code;
            let is_end =
                NodeType::from_i8(node.node_type).is_some_and(|t| t.is_end());
            node.status = Some(if pending.contains(code) {
                TO_DO
            } else if done.contains(code)
                || (is_end && reached_targets.contains(code))
            {
                DONE
            } else {
                NOT_DONE
            });

            for skip in &mut node.skip_list {
                let edge =
                    (skip.now_node_code.clone(), skip.next_node_code.clone());
                skip.status = Some(if traversed_edges.contains(&edge) {
                    // 边状态跟随目标节点:目标待办则待办,否则已办。
                    if pending.contains(&skip.next_node_code) {
                        TO_DO
                    } else {
                        DONE
                    }
                } else {
                    NOT_DONE
                });
            }
        }

        // 状态三原色,顺序与 warm-flow ChartStatus 一致:已办/待办/未办。
        def_json.chart_status_color = Some(vec![
            "56,161,105".to_string(),
            "245,158,11".to_string(),
            "107,114,128".to_string(),
        ]);

        Ok(def_json)
    }

    /// 读取实例变量
    pub async fn get_variable(
        &self,
        ctx: &DataContext<'_>,
        instance_id: i64,
    ) -> AppResult<serde_json::Value> {
        let instance = InstanceRepository::select_by_id(ctx, instance_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("实例不存在 (id={})", instance_id))
            })?;
        match instance.variable.as_deref() {
            Some(s) if !s.is_empty() => serde_json::from_str(s).map_err(|e| {
                AppError::Internal(format!("解析变量失败: {}", e))
            }),
            _ => Ok(serde_json::Value::Object(serde_json::Map::new())),
        }
    }

    /// 更新实例变量
    pub async fn update_variable(
        &self,
        ctx: &DataContext<'_>,
        instance_id: i64,
        variable: &serde_json::Value,
        op: &OperatorContext,
    ) -> AppResult<()> {
        let mut instance = InstanceRepository::select_by_id(ctx, instance_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("实例不存在 (id={})", instance_id))
            })?;
        let variable_str = serde_json::to_string(variable).map_err(|e| {
            AppError::Internal(format!("序列化变量失败: {}", e))
        })?;
        instance.variable = Some(variable_str);
        instance.update_time = Some(op.now());
        instance.update_by = Some(op.user_id.clone());
        InstanceRepository::update_by_id(ctx, &instance).await?;
        Ok(())
    }

    /// 作废：强制置 Invalid 终态，清理未办任务并回写业务状态 invalid
    #[tracing::instrument(skip(self, ctx, op), fields(business_id = business_id, instance_id = instance_id))]
    pub async fn invalid(
        &self,
        ctx: &DataContext<'_>,
        business_id: Option<&str>,
        instance_id: Option<i64>,
        comment: Option<&str>,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        // 纯读 + 守卫全部在事务外(D1 先例,见文件头)。定义读取(取
        // flow_code)同 revoke/termination 提升事务外:invalid 恒回写
        // "invalid",读取恒触发。
        let mut instance = match (business_id, instance_id) {
            (Some(bid), _) => {
                InstanceRepository::select_by_business_id(ctx, bid).await?
            }
            (None, Some(id)) => {
                InstanceRepository::select_by_id(ctx, id).await?
            }
            (None, None) => {
                return Err(AppError::BadRequest(
                    "缺少业务ID或实例ID".to_string(),
                ));
            }
        }
        .ok_or_else(|| AppError::NotFound("实例不存在".to_string()))?;

        instance.flow_status = FlowStatus::Invalid.as_str().to_string();
        instance.update_time = Some(op.now());
        instance.update_by = Some(op.user_id.clone());

        let def =
            DefinitionRepository::select_by_id(ctx, instance.definition_id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!(
                        "流程定义不存在 (id={})",
                        instance.definition_id
                    ))
                })?;

        // 作废原因写入历史任务 message,与撤销等操作共用同一落库位置(PIN5)。
        // fake task / his 不依赖事务内读到的数据,闭包外拼装(同 revoke)。
        let now = op.now();
        let fake_invalid_task = Task {
            id: Some(0),
            definition_id: instance.definition_id,
            instance_id: instance.id.unwrap_or(0),
            node_code: instance.node_code.clone(),
            node_name: instance.node_name.clone(),
            node_type: instance.node_type,
            flow_status: FlowStatus::Invalid.as_str().to_string(),
            form_custom: None,
            form_path: None,
            create_time: Some(now),
            create_by: Some(op.user_id.clone()),
            update_time: Some(now),
            update_by: Some(op.user_id.clone()),
            del_flag: Some("0".to_string()),
            tenant_id: op.tenant_id.clone(),
            permission_list: vec![],
        };
        let flow_params = FlowParams {
            handler: Some(op.user_id.clone()),
            skip_type: Some(SkipType::None.as_str().to_string()),
            message: comment.map(String::from),
            flow_status: Some(FlowStatus::Invalid.as_str().to_string()),
            ..Default::default()
        };
        let his_task = HisTaskService::build_skip_his(
            &fake_invalid_task,
            &[],
            &flow_params,
            "{}",
        );

        // 写半边单闭包:清理未办任务(对齐终止,逻辑删 @TableLogic 平价,
        // 见文件头「0 行语义与删除形态」)→ 实例回写 → 历史 → 业务状态回写,原子一致。
        let callback = self.business_callback.clone();
        let returned = instance.clone();
        let flow_code = def.flow_code;
        ctx.transaction(|tx| {
            Box::pin(async move {
                let active_tasks =
                    TaskRepository::select_by_instance_id_in_txn(
                        tx,
                        instance.id.unwrap_or(0),
                    )
                    .await?;
                for t in &active_tasks {
                    let tid = t.id.unwrap_or(0);
                    UserRepository::soft_delete_by_task_id_in_txn(tx, tid)
                        .await?;
                    TaskRepository::soft_delete_by_id_in_txn(tx, tid).await?;
                }
                InstanceRepository::update_by_id_in_txn(tx, &instance).await?;
                HisTaskRepository::insert_in_txn(tx, &his_task).await?;
                notify_business_status(
                    &*callback, tx, &flow_code, &instance, "invalid",
                )
                .await
                .map_err(|e| {
                    DataError::Unsupported(format!("业务回写失败:{e}"))
                })?;
                Ok(())
            })
        })
        .await?;
        Ok(returned)
    }

    /// 填充实例 VO 的关联字段(定义快照/分类名/发起人名/业务扩展)
    pub async fn fill_instance_vo(
        &self,
        ctx: &DataContext<'_>,
        vo: &mut crate::model::dto::FlowInstanceVo,
    ) -> AppResult<()> {
        let def =
            DefinitionRepository::select_by_id(ctx, vo.definition_id).await?;
        if let Some(d) = &def {
            vo.flow_name = Some(d.flow_name.clone());
            vo.flow_code = Some(d.flow_code.clone());
            vo.form_custom = d.form_custom.clone();
            vo.form_path = d.form_path.clone();
            vo.version = Some(d.version.clone());
            vo.category = d.category.clone();
            // flow_definition.category 存的是 categoryId（Java 语义），按 id 解析后查 name
            if let Some(ref cat_id_str) = d.category
                && let Ok(cat_id) = cat_id_str.parse::<i64>()
            {
                let cat =
                    crate::repository::category_repo::select_by_id(ctx, cat_id)
                        .await?;
                if let Some(c) = cat {
                    vo.category_name = Some(c.category_name.clone());
                }
            }
        }
        if let Some(ref creator) = vo.create_by
            && let Ok(uid) = creator.parse::<i64>()
        {
            let names = self.resolver.user_names(&[uid]).await;
            vo.create_by_name = names.get(&uid).cloned();
        }
        let (code, title) =
            crate::service::business_fields::resolve_business_fields(
                ctx,
                &self.business_callback,
                vo.id,
                &vo.business_id,
            )
            .await?;
        vo.business_code = code;
        vo.business_title = title;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::Value;

    use super::build_start_variables;

    #[test]
    fn injects_initiator_dept_and_business_id() {
        let mut base = HashMap::new();
        base.insert("amount".to_string(), Value::from(100));

        let vars = build_start_variables(base, 42, Some(103), "BIZ-1");

        assert_eq!(vars["amount"], Value::from(100));
        assert_eq!(vars["initiator"], Value::String("42".to_string()));
        assert_eq!(vars["initiatorDeptId"], Value::Number(103.into()));
        assert_eq!(vars["businessId"], Value::String("BIZ-1".to_string()));
    }

    #[test]
    fn omits_dept_id_when_absent() {
        let vars = build_start_variables(HashMap::new(), 42, None, "BIZ-1");

        assert!(!vars.contains_key("initiatorDeptId"));
        assert_eq!(vars["initiator"], Value::String("42".to_string()));
    }

    /// 发起人不可被请求体伪造:服务端注入的三个变量覆盖调用方同名值。
    #[test]
    fn server_injected_vars_override_caller_supplied() {
        let mut base = HashMap::new();
        base.insert("initiator".to_string(), Value::String("999".to_string()));
        base.insert(
            "businessId".to_string(),
            Value::String("HACK".to_string()),
        );

        let vars = build_start_variables(base, 42, None, "BIZ-1");

        assert_eq!(vars["initiator"], Value::String("42".to_string()));
        assert_eq!(vars["businessId"], Value::String("BIZ-1".to_string()));
    }
}
