//! 任务服务 —— task 链全部经 `ctx.transaction` 单闭包执行。
//!
//! # 事务形状:守卫提升事务外,写半边单闭包
//!
//! 统一形状:**守卫(纯读/前置校验)提升事务外,写半边单闭包**。闭包错误
//! 通道固定 `DataError`,AppError 文案(Conflict/BadRequest/NotFound/
//! Forbidden)塞不进闭包 —— 全部守卫(load_skip_context 的存在性/挂起/
//! 已结束、权限校验、退回路径解析、目标节点查找)在闭包外以原文案抛出;
//! 闭包内只剩 repo 调用的 DataError 与两处显式映射(见「错误通道桥」)。
//! READ COMMITTED 是语句级快照,守卫读提升不改变任何结果(definition/
//! instance_service 文件头同款论证);每方法的全部写语句仍收在同一闭包,
//! 闭包 Err ⇒ 整体回滚。
//!
//! # 0 行语义与删除形态
//!
//! - **任务/办理人清理:逻辑删**(`soft_delete_*`,`@TableLogic` 平价,
//!   理由见 task_repo/user_repo 文件头):0 行 → Ok(0) 被忽略;读侧全滤
//!   del_flag='0',同事务重读与事务外读均看不见墓碑行,与硬删等价
//!   (仅不再物理清行)。
//! - `update_by_id_in_txn`(任务/实例):漏斗 update 的 WHERE = 主键 + 软删
//!   存活,0 行 → `DataError::NotFound`(404;触发点都是「读后被并发删除」
//!   的竞争窗口)。顺签轮转的后至者并发推进据此 Err 不脏写(并发判例
//!   `later_skip_on_advanced_task_errors`,见 engine_test)。
//! - **双转办并发窗口**(`user_repo::update_processed_by_in_txn` 文件头
//!   同注):`tx.update_where` 单语句条件 UPDATE,**先写者保(Java 平价)**
//!   —— 两个并发转办 A→X / A→Y 经行锁串行,后至者条件失配(processed_by
//!   已被先写者改走)→ 0 行 → Ok(0),不覆盖先写者结果。办理人恒为
//!   单一值,不产生"两人并存"的脏写。
//! - **skip 守卫次序**:畸形流程(当前节点无 PASS 出边)且会签/顺签
//!   阈值未达时,resolve_next 作纯读守卫先行 → BadRequest(阈值判定在
//!   闭包内,判定结果本身不变,仅错误呈现次序变化)。
//! - `update_processed_by_in_txn` 的命中行被并发软删 → 单语句的软删存活
//!   过滤 0 行 → Ok(0)。
//!
//! # 错误通道桥(闭包内仅有的两处 AppError 映射)
//!
//! - 监听器 Err → `DataError::Unsupported("监听器执行失败:{e}")`
//!   (persist::listener_failed):闭包内错误统一走 DataError,回滚触发不变;
//! - 业务回写 Err → `DataError::Unsupported("业务回写失败:{e}")`
//!   (shared.rs 判例同款):回滚触发不变,错误呈现不变(同为 500)。
//!
//! # 审计盖章
//!
//! A 组实体审计列类型与漏斗盖章签名不匹配(policy.rs 双组裁定)→
//! `OperatorContext` 保留,审计字段由本服务手工盖。
//! `DataContext` 只负责执行面(连接/事务/软删接管)。

use std::{collections::HashMap, sync::Arc};

use app_core::{
    error::{AppError, AppResult},
    util::snowflake,
};
use app_data::{DataContext, DataError, TxnContext};
use serde_json::Value;

use crate::{
    dto::flow_params::FlowParams,
    engine::{
        chart, graph, runtime::one_vote_veto, transition::PathWayData,
        vote::reject_advance,
    },
    enums::{
        ActivityStatus, CooperateType, FlowStatus, NodeType, SkipType, UserType,
    },
    expression::handler::{
        apply_next_handler, expand_handlers, resolve_handlers,
    },
    listener::{
        trigger::trigger_listeners,
        types::{LISTENER_FINISH, LISTENER_START, ListenerVariable},
    },
    model::{
        definition::Definition,
        dto::{
            BackNodeVo, CurrentTaskUserVo, FlowCopyBo, FlowHisTaskVo,
            FlowInstanceVo, FlowNextNodeVo, FlowTaskVo, FlowUrgeTaskBo,
            TaskOperationBo, TaskPageFilter,
        },
        his_task::HisTask,
        instance::Instance,
        node::Node,
        skip::Skip,
        task::Task,
        user::User,
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
            apply_node_code, final_status_label, notify_business_status,
            notify_new_tasks,
        },
        skip::{
            SkipDeps, advance, context::SkipContext,
            listeners::fire_def_and_node, multi_approval, persist,
        },
    },
};

/// 执行面全部经请求期 `DataContext`(handler 按登录用户构造);
/// 本服务只持有流程编排依赖,不再持有数据库连接。
#[derive(Clone)]
pub struct TaskService {
    registry: Arc<crate::listener::ListenerRegistry>,
    permission: Arc<dyn crate::permission::PermissionHandler>,
    resolver: Arc<dyn UserResolver>,
    business_callback: Arc<dyn BusinessStatusCallback>,
    notifier: Arc<dyn FlowNotifier>,
}

impl TaskService {
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

    /// 流转（仅支持 PASS）
    #[tracing::instrument(skip(self, ctx, flow_params, op), fields(task_id = task_id))]
    pub async fn skip(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
        flow_params: &FlowParams,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        // 守卫提升:load(读+存在性/挂起/结束校验)→ 权限 → start 监听 →
        // 去向判定(纯读)。见文件头论证。
        let mut sctx = SkipContext::load(ctx, task_id, flow_params, op).await?;

        self.check_permission(
            &sctx.task_users,
            sctx.handler(),
            sctx.params.ignore,
        )
        .await?;

        // 监听器：start（当前任务开始处理)
        {
            let mut lv = ListenerVariable::new(
                sctx.definition.clone(),
                sctx.instance.clone(),
                sctx.variables.clone(),
                sctx.params.clone(),
            );
            lv.node = Some(sctx.now_node.clone());
            lv.task = Some(sctx.task.clone());
            fire_def_and_node(
                &self.registry,
                &sctx.definition,
                &sctx.now_node,
                LISTENER_START,
                &mut lv,
            )
            .await?;
        }

        // 去向判定(纯读守卫:汇聚网关 AND-join 检测的活跃任务快照已随
        // SkipContext::load 预读,READ COMMITTED 下提升不改变结果)
        let advance = advance::resolve_next(&sctx, &*self.resolver).await?;

        // 写半边单闭包:多人审批阈值 → 汇聚挂起/正常推进的持久化。
        // owned 值先进闭包(transaction 的 HRTB 不收借用:self 字段的 Arc
        // 与 OperatorContext 均克隆后移入,SkipDeps 在闭包内组装)。
        let registry = self.registry.clone();
        let resolver = self.resolver.clone();
        let business_callback = self.business_callback.clone();
        let op_in = op.clone();

        let (instance, briefs) = ctx
            .transaction(|tx| {
                Box::pin(async move {
                    let deps = SkipDeps {
                        registry: &registry,
                        resolver: &*resolver,
                        business_callback: &*business_callback,
                    };
                    // 出口 1：会签/顺签阈值未达成(不建新任务,无通知)
                    if let Some(instance) =
                        multi_approval::try_halt(tx, &mut sctx, &op_in, &deps)
                            .await?
                    {
                        return Ok((instance, Vec::new()));
                    }
                    let result = match advance {
                        // 出口 2：汇聚网关仍有前驱分支未到达(不建新任务)
                        advance::Advance::JoinPending => {
                            persist::commit_join_pending(tx, &mut sctx, &op_in)
                                .await
                        }
                        // 出口 3：正常推进
                        advance::Advance::To {
                            path_way,
                            next_nodes,
                        } => {
                            persist::commit_advance(
                                tx,
                                &mut sctx,
                                &op_in,
                                &deps,
                                &path_way,
                                &next_nodes,
                            )
                            .await
                        }
                    };
                    // 正常推进才采集了新任务简报;随闭包返回值带出,
                    // 提交成功后由调用方推送(失败仅告警)
                    result.map(|instance| {
                        (instance, sctx.created_task_briefs.clone())
                    })
                })
            })
            .await?;
        // 事务提交成功后才通知新待办(尽力而为,失败仅 warn,见 shared.rs)
        notify_new_tasks(&self.notifier, &briefs).await;
        Ok(instance)
    }

    /// 查询待办任务分页
    pub async fn select_todo_page(
        &self,
        ctx: &DataContext<'_>,
        handler: &str,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<Task>> {
        Ok(TaskRepository::select_todo_by_handler(
            ctx,
            handler,
            &TaskPageFilter::default(),
            page_no,
            page_size,
        )
        .await?)
    }

    /// 查询已办任务分页
    pub async fn select_done_page(
        &self,
        ctx: &DataContext<'_>,
        handler: &str,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<crate::model::his_task::HisTask>> {
        Ok(HisTaskRepository::select_done_page(
            ctx,
            handler,
            &TaskPageFilter::default(),
            page_no,
            page_size,
        )
        .await?)
    }

    /// 查询单个任务
    pub async fn get_task(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
    ) -> AppResult<Option<Task>> {
        Ok(TaskRepository::select_by_id(ctx, task_id).await?)
    }

    /// 校验当前用户是否有权限办理指定任务
    /// task_users: task 的 flow_user 列表（Approval 类型）
    /// handler: 当前操作人唯一标识
    async fn check_permission(
        &self,
        task_users: &[User],
        handler: &str,
        ignore: bool,
    ) -> AppResult<()> {
        if ignore {
            return Ok(());
        }
        let mut user_perms = self.permission.permissions().await?;
        user_perms.push(handler.to_string());

        let has_perm = task_users.iter().any(|u| {
            u.processed_by
                .as_deref()
                .map(|p| user_perms.iter().any(|up| up == p))
                .unwrap_or(false)
        });
        if !has_perm {
            return Err(AppError::Forbidden("当前用户无办理权限".to_string()));
        }
        Ok(())
    }

    /// 加载流转所需基础数据并做状态校验(纯读守卫,事务外)
    async fn load_skip_context(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
    ) -> AppResult<(Task, Instance, Definition, Vec<Node>, Vec<Skip>)> {
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
            return Err(AppError::BadRequest("流程已结束".to_string()));
        }

        let all_nodes =
            NodeRepository::select_by_definition_id(ctx, task.definition_id)
                .await?;
        let all_skips =
            SkipRepository::select_by_definition_id(ctx, task.definition_id)
                .await?;

        Ok((task, instance, definition, all_nodes, all_skips))
    }

    /// 跳转到指定节点（用于 reject_last / task_back）
    /// 执行一票否决并创建目标节点任务。
    ///
    /// 事务句柄为 `&TxnContext<'_>`,错误通道 `DataError`;
    /// 定义行不再闭包内重读 —— 调用方把 load 时读到的 `flow_code` 传入
    /// (READ COMMITTED 下重读与传值等价,消掉闭包内 NotFound 文案)。
    /// 办理人解析(resolver 展开)与任务写入保持同事务。
    /// 关联函数(无 `&self`):闭包 HRTB 不收 self 借用,调用方克隆
    /// resolver/callback 的 Arc 后移入闭包。
    #[allow(clippy::too_many_arguments)]
    async fn jump_to_node(
        tx: &TxnContext<'_>,
        resolver: &dyn UserResolver,
        business_callback: &dyn BusinessStatusCallback,
        task: &Task,
        mut instance: Instance,
        target_node: &Node,
        merged_var_str: &str,
        var_map: &HashMap<String, Value>,
        params: &FlowParams,
        ctx: &OperatorContext,
        his_flow_status: &str,
        all_nodes: &[Node],
        all_skips: &[Skip],
        flow_code: &str,
    ) -> Result<Instance, DataError> {
        one_vote_veto(tx, task.instance_id, &target_node.node_code, all_skips)
            .await?;

        let now = ctx.now();
        let resolved = resolve_handlers(
            &target_node.permission_flag.clone().unwrap_or_default(),
            var_map,
        );
        let permission_list = apply_next_handler(
            resolved,
            &params.next_handler,
            params.next_handler_append,
        );
        let permission_list = expand_handlers(
            resolver,
            permission_list,
            Some(&crate::resolver::InstanceAssigneeContext {
                instance_id: task.instance_id,
                target_node_code: target_node.node_code.clone(),
                variable: var_map.clone(),
                // jump_to_node 同为退回族(REJECT),不携带 current(见下处注释)
                current: None,
            }),
        )
        .await;
        // 退回复用弹窗选人(back: 变量,对齐 Java processTaskPermission;
        // back: 保留不删,供后续重复退回)
        let permission_list = match crate::expression::handler::status_assignee(
            var_map,
            &target_node.node_code,
            "back",
        ) {
            Some(ids) => ids,
            None => permission_list,
        };

        let new_task = Task {
            id: Some(snowflake::next_id()),
            definition_id: task.definition_id,
            instance_id: task.instance_id,
            node_code: target_node.node_code.clone(),
            node_name: target_node.node_name.clone(),
            node_type: target_node.node_type,
            flow_status: FlowStatus::Reject.as_str().to_string(),
            form_custom: target_node.form_custom.clone(),
            form_path: target_node.form_path.clone(),
            create_time: Some(now),
            create_by: Some(ctx.user_id.clone()),
            update_time: Some(now),
            update_by: Some(ctx.user_id.clone()),
            del_flag: Some("0".to_string()),
            tenant_id: ctx.tenant_id.clone(),
            permission_list: permission_list.clone(),
        };

        let mut his = HisTaskService::build_skip_his(
            task,
            std::slice::from_ref(target_node),
            params,
            merged_var_str,
        );
        his.flow_status = his_flow_status.to_string();

        instance.node_code = target_node.node_code.clone();
        instance.node_name = target_node.node_name.clone();
        instance.node_type = target_node.node_type;
        instance.flow_status = FlowStatus::TaskBack.as_str().to_string();
        instance.variable = Some(merged_var_str.to_string());
        instance.update_time = Some(now);
        instance.update_by = Some(ctx.user_id.clone());

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
                create_by: Some(ctx.user_id.clone()),
                update_time: Some(now),
                update_by: Some(ctx.user_id.clone()),
                del_flag: Some("0".to_string()),
                tenant_id: ctx.tenant_id.clone(),
            })
            .collect();

        TaskRepository::insert_batch_in_txn(tx, &[new_task]).await?;
        UserRepository::insert_batch_in_txn(tx, &new_users).await?;
        InstanceRepository::update_by_id_in_txn(tx, &instance).await?;
        HisTaskRepository::insert_in_txn(tx, &his).await?;

        // 退回到**申请人节点**时才回写业务状态 back —— 对齐 Java
        // WorkflowGlobalListener:209-215「驳回的节点为申请人节点则修改流程状态为退回」。
        // 退到中间节点时流程仍在审批中,业务状态保持 waiting 不动。
        if apply_node_code(all_nodes, all_skips).as_deref()
            == Some(target_node.node_code.as_str())
            && let Some(status) = final_status_label(&instance.flow_status)
        {
            notify_business_status(
                business_callback,
                tx,
                flow_code,
                &instance,
                status,
            )
            .await
            .map_err(|e| DataError::Unsupported(format!("业务回写失败:{e}")))?;
        }

        Ok(instance)
    }

    /// 退回（按 REJECT 出边退回）
    #[tracing::instrument(skip(self, ctx, flow_params, op), fields(task_id = task_id))]
    pub async fn reject(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
        flow_params: &FlowParams,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        // ---- 守卫与构建提升(纯读,见文件头) ----
        let (task, mut instance, def, all_nodes, all_skips) =
            self.load_skip_context(ctx, task_id).await?;

        let now_node = all_nodes
            .iter()
            .find(|n| n.node_code == task.node_code)
            .cloned()
            .ok_or_else(|| {
                AppError::NotFound(format!("节点不存在: {}", task.node_code))
            })?;

        // 权限校验
        let task_users =
            UserRepository::select_by_task_id(ctx, task_id).await?;
        let mut params = flow_params.clone();
        if params.handler.is_none() {
            params.handler = Some(op.user_id.clone());
        }
        self.check_permission(
            &task_users,
            params.handler.as_deref().unwrap_or(""),
            params.ignore,
        )
        .await?;

        // 合并变量
        let mut var_map: HashMap<String, Value> = match &instance.variable {
            Some(v) if !v.is_empty() => {
                serde_json::from_str(v).unwrap_or_default()
            }
            _ => HashMap::new(),
        };
        for (k, v) in &params.variable {
            var_map.insert(k.clone(), v.clone());
        }
        let merged_var_str = serde_json::to_string(&var_map)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        params.skip_type = Some(SkipType::Reject.as_str().to_string());

        // start 监听器
        {
            let mut lv = ListenerVariable::new(
                def.clone(),
                instance.clone(),
                var_map.clone(),
                params.clone(),
            );
            lv.node = Some(now_node.clone());
            lv.task = Some(task.clone());
            trigger_listeners(
                &self.registry,
                def.listener_type.as_deref(),
                def.listener_path.as_deref(),
                LISTENER_START,
                &mut lv,
            )
            .await?;
            trigger_listeners(
                &self.registry,
                now_node.listener_type.as_deref(),
                now_node.listener_path.as_deref(),
                LISTENER_START,
                &mut lv,
            )
            .await?;
        }

        // 找 REJECT 出边目标节点
        let mut path_way = PathWayData {
            def_id: task.definition_id,
            ins_id: Some(task.instance_id),
            skip_type: Some(SkipType::Reject.as_str().to_string()),
            ..Default::default()
        };
        let any_node_allowed = now_node
            .any_node_skip
            .as_deref()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("Y"))
            .unwrap_or(false);
        let any_node_code = if any_node_allowed {
            params.node_code.as_deref()
        } else {
            None
        };
        let target_node = graph::get_next_node(
            &now_node,
            any_node_code,
            SkipType::Reject.as_str(),
            &mut path_way,
            &all_skips,
            &all_nodes,
        )
        .ok_or_else(|| {
            AppError::BadRequest("当前节点未配置退回路径".to_string())
        })?;
        path_way.target_nodes = vec![target_node.clone()];

        // 多人审批阈值检查的输入(纯计数守卫,事务外预读)
        let node_ratio = now_node.node_ratio.clone().unwrap_or_default();

        // 历史记录
        let his_task = HisTaskService::build_skip_his(
            &task,
            std::slice::from_ref(&target_node),
            &params,
            &merged_var_str,
        );

        // 新任务(办理人解析在闭包外完成,纯读 resolver)
        let now = op.now();
        let resolved = resolve_handlers(
            &target_node.permission_flag.clone().unwrap_or_default(),
            &var_map,
        );
        let permission_list = apply_next_handler(
            resolved,
            &params.next_handler,
            params.next_handler_append,
        );
        let permission_list = expand_handlers(
            &*self.resolver,
            permission_list,
            Some(&crate::resolver::InstanceAssigneeContext {
                instance_id: task.instance_id,
                target_node_code: target_node.node_code.clone(),
                variable: var_map.clone(),
                // 退回写 REJECT 行:preStep 匹配器只认 PASS,在途 REJECT 行
                // 本就不该被引用 —— 不携带 current(None),语义同旧
                current: None,
            }),
        )
        .await;
        let new_task = Task {
            id: Some(snowflake::next_id()),
            definition_id: task.definition_id,
            instance_id: task.instance_id,
            node_code: target_node.node_code.clone(),
            node_name: target_node.node_name.clone(),
            node_type: target_node.node_type,
            flow_status: FlowStatus::Reject.as_str().to_string(),
            form_custom: target_node.form_custom.clone(),
            form_path: target_node.form_path.clone(),
            create_time: Some(now),
            create_by: Some(op.user_id.clone()),
            update_time: Some(now),
            update_by: Some(op.user_id.clone()),
            del_flag: Some("0".to_string()),
            tenant_id: op.tenant_id.clone(),
            permission_list: permission_list.clone(),
        };

        // 流程图更新(纯计算提升;实例字段的改写留在闭包内比率块早退之后 ——
        // 未达阈值的早退分支不改写节点/状态)
        let def_json_str = instance.def_json.as_deref().ok_or_else(|| {
            AppError::Internal("实例缺少流程图数据".to_string())
        })?;
        let def_json: crate::model::def_json::DefJson =
            serde_json::from_str(def_json_str).map_err(|e| {
                AppError::Internal(format!("解析流程图数据失败: {}", e))
            })?;
        let updated_json = chart::skip_metadata(&def_json, &path_way);
        let updated_def_json = serde_json::to_string(&updated_json)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        // 新任务的 flow_user
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

        // ---- 写半边单闭包 ----
        // 比率块(计数读保持在事务内,未达阈值早退)→ finish 监听 → 一票
        // 否决 → 写库 → 业务回写。
        let registry = self.registry.clone();
        let callback = self.business_callback.clone();
        let handler = params.handler.clone().unwrap_or_default();
        let op_in = op.clone();
        let (instance, briefs) = ctx
            .transaction(|tx| {
                Box::pin(async move {
                    // 多人审批阈值检查（驳回）
                    if !node_ratio.is_empty() {
                        let remaining_count = task_users.len();
                        let rejected_count =
                            HisTaskRepository::count_reject_by_task_id_in_txn(
                                tx, task_id,
                            )
                            .await? as usize;
                        let passed_count =
                            HisTaskRepository::count_pass_by_task_id_in_txn(
                                tx, task_id,
                            )
                            .await? as usize;
                        let total =
                            remaining_count + rejected_count + passed_count;

                        if total > 1 || rejected_count > 0 {
                            let new_rejected = rejected_count + 1;
                            let advance =
                                reject_advance(&node_ratio, total, new_rejected);

                            UserRepository::soft_delete_by_task_and_handler_in_txn(
                                tx,
                                task_id,
                                &handler,
                            )
                            .await?;

                            if !advance {
                                let partial_his =
                                    HisTaskService::build_skip_his(
                                        &task,
                                        &[],
                                        &params,
                                        &merged_var_str,
                                    );
                                HisTaskRepository::insert_in_txn(
                                    tx,
                                    &partial_his,
                                )
                                .await?;

                                instance.update_time = Some(op_in.now());
                                instance.update_by = Some(op_in.user_id.clone());
                                InstanceRepository::update_by_id_in_txn(
                                    tx,
                                    &instance,
                                )
                                .await?;
                                // 比率块早退:不建新任务,无通知
                                return Ok((instance, Vec::new()));
                            }
                        }
                    }

                    // finish 监听器
                    {
                        let mut lv = ListenerVariable::new(
                            def.clone(),
                            instance.clone(),
                            var_map.clone(),
                            params.clone(),
                        );
                        lv.node = Some(now_node.clone());
                        lv.task = Some(task.clone());
                        trigger_listeners(
                            &registry,
                            def.listener_type.as_deref(),
                            def.listener_path.as_deref(),
                            LISTENER_FINISH,
                            &mut lv,
                        )
                        .await
                        .map_err(listener_failed)?;
                        trigger_listeners(
                            &registry,
                            now_node.listener_type.as_deref(),
                            now_node.listener_path.as_deref(),
                            LISTENER_FINISH,
                            &mut lv,
                        )
                        .await
                        .map_err(listener_failed)?;
                    }

                    // 一票否决：删除目标节点后继集内的活跃任务(逻辑删)
                    one_vote_veto(
                        tx,
                        task.instance_id,
                        &target_node.node_code,
                        &all_skips,
                    )
                    .await?;

                    // 更新实例(比率块早退之后才改写)
                    instance.def_json = Some(updated_def_json);
                    instance.node_code = target_node.node_code.clone();
                    instance.node_name = target_node.node_name.clone();
                    instance.node_type = target_node.node_type;
                    instance.flow_status = FlowStatus::Reject.as_str().to_string();
                    instance.variable = Some(merged_var_str.clone());
                    instance.update_time = Some(now);
                    instance.update_by = Some(op_in.user_id.clone());

                    // 持久化（旧任务已被 one_vote_veto 删除）
                    // 新待办简报随闭包返回值带出(提交成功后由调用方推送,
                    // 失败仅告警);办理人用户行与简报同取本闭包前的
                    // permission_list
                    let back_brief = (
                        new_task.node_name.clone().unwrap_or_default(),
                        new_task.permission_list.clone(),
                    );
                    TaskRepository::insert_batch_in_txn(tx, &[new_task])
                        .await?;
                    UserRepository::insert_batch_in_txn(tx, &new_users)
                        .await?;
                    InstanceRepository::update_by_id_in_txn(tx, &instance)
                        .await?;
                    HisTaskRepository::insert_in_txn(tx, &his_task).await?;

                    if let Some(status) =
                        final_status_label(&instance.flow_status)
                    {
                        notify_business_status(
                            &*callback,
                            tx,
                            &def.flow_code,
                            &instance,
                            status,
                        )
                        .await
                        .map_err(|e| {
                            DataError::Unsupported(format!(
                                "业务回写失败:{e}"
                            ))
                        })?;
                    }
                    Ok((instance, vec![back_brief]))
                })
            })
            .await?;
        // 事务提交成功后才通知新待办(尽力而为,失败仅 warn,见 shared.rs)
        notify_new_tasks(&self.notifier, &briefs).await;
        Ok(instance)
    }

    /// 退回到上一步（历史中最近一次 PASS 的前序节点）
    #[tracing::instrument(skip(self, ctx, flow_params, op), fields(task_id = task_id))]
    pub async fn reject_last(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
        flow_params: &FlowParams,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        // 守卫提升
        let (task, instance, def, all_nodes, all_skips) =
            self.load_skip_context(ctx, task_id).await?;

        // 权限校验
        let task_users =
            UserRepository::select_by_task_id(ctx, task_id).await?;
        let mut params = flow_params.clone();
        if params.handler.is_none() {
            params.handler = Some(op.user_id.clone());
        }
        self.check_permission(
            &task_users,
            params.handler.as_deref().unwrap_or(""),
            params.ignore,
        )
        .await?;

        // 查找上一个节点
        let prev_code = HisTaskRepository::select_prev_node_code(
            ctx,
            task.instance_id,
            &task.node_code,
        )
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("无历史审批记录，无法退回上一步".to_string())
        })?;

        let target_node = all_nodes
            .iter()
            .find(|n| n.node_code == prev_code)
            .ok_or_else(|| {
                AppError::NotFound(format!("目标节点不存在: {}", prev_code))
            })?
            .clone();

        // 合并变量
        let var_map: HashMap<String, Value> = match &instance.variable {
            Some(v) if !v.is_empty() => {
                serde_json::from_str(v).unwrap_or_default()
            }
            _ => HashMap::new(),
        };
        let merged_var_str = serde_json::to_string(&var_map)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        params.skip_type = Some(SkipType::Reject.as_str().to_string());

        // 写半边单闭包(owned 值先进闭包,HRTB 不收借用)
        let resolver = self.resolver.clone();
        let business_callback = self.business_callback.clone();
        let op_in = op.clone();
        let result = ctx
            .transaction(|tx| {
                Box::pin(async move {
                    Self::jump_to_node(
                        tx,
                        &*resolver,
                        &*business_callback,
                        &task,
                        instance,
                        &target_node,
                        &merged_var_str,
                        &var_map,
                        &params,
                        &op_in,
                        FlowStatus::TaskBack.as_str(),
                        &all_nodes,
                        &all_skips,
                        &def.flow_code,
                    )
                    .await
                })
            })
            .await?;
        Ok(result)
    }

    /// 退回到当前办理人最近一次审批过的节点（从已办退回）
    #[tracing::instrument(skip(self, ctx, flow_params, op), fields(task_id = task_id))]
    pub async fn task_back(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
        flow_params: &FlowParams,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        // 守卫提升
        let (task, instance, def, all_nodes, all_skips) =
            self.load_skip_context(ctx, task_id).await?;

        let mut params = flow_params.clone();
        let handler = params
            .handler
            .get_or_insert_with(|| op.user_id.clone())
            .clone();

        // 权限校验
        if !params.ignore {
            let task_users =
                UserRepository::select_by_task_id(ctx, task_id).await?;
            self.check_permission(&task_users, handler.as_str(), params.ignore)
                .await?;
        }

        // 查找该 handler 最近一次审批的节点
        let target_code = HisTaskRepository::select_handler_last_pass_node(
            ctx,
            task.instance_id,
            &handler,
        )
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("无审批历史，无法退回".to_string())
        })?;

        let target_node = all_nodes
            .iter()
            .find(|n| n.node_code == target_code)
            .ok_or_else(|| {
                AppError::NotFound(format!("目标节点不存在: {}", target_code))
            })?
            .clone();

        let var_map: HashMap<String, Value> = match &instance.variable {
            Some(v) if !v.is_empty() => {
                serde_json::from_str(v).unwrap_or_default()
            }
            _ => HashMap::new(),
        };
        let merged_var_str = serde_json::to_string(&var_map)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        params.skip_type = Some(SkipType::Reject.as_str().to_string());

        // 写半边单闭包(owned 值先进闭包,HRTB 不收借用)
        let resolver = self.resolver.clone();
        let business_callback = self.business_callback.clone();
        let op_in = op.clone();
        let result = ctx
            .transaction(|tx| {
                Box::pin(async move {
                    Self::jump_to_node(
                        tx,
                        &*resolver,
                        &*business_callback,
                        &task,
                        instance,
                        &target_node,
                        &merged_var_str,
                        &var_map,
                        &params,
                        &op_in,
                        FlowStatus::TaskBack.as_str(),
                        &all_nodes,
                        &all_skips,
                        &def.flow_code,
                    )
                    .await
                })
            })
            .await?;
        Ok(result)
    }

    /// 转办：将任务的办理人更换为新办理人
    #[tracing::instrument(skip(self, ctx, flow_params, op), fields(task_id = task_id))]
    pub async fn transfer(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
        flow_params: &FlowParams,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        let new_handler = flow_params
            .next_handler
            .as_ref()
            .and_then(|v| v.first())
            .cloned()
            .ok_or_else(|| {
                AppError::BadRequest("缺少目标办理人 next_handler".to_string())
            })?;

        // 守卫提升
        let (task, mut instance, _def, _nodes, _skips) =
            self.load_skip_context(ctx, task_id).await?;

        let task_users =
            UserRepository::select_by_task_id(ctx, task_id).await?;
        let current_handler = flow_params
            .handler
            .clone()
            .unwrap_or_else(|| op.user_id.clone());

        if !flow_params.ignore {
            let has_perm = task_users.iter().any(|u| {
                u.processed_by.as_deref() == Some(current_handler.as_str())
            });
            if !has_perm {
                return Err(AppError::Forbidden(
                    "当前用户无办理权限".to_string(),
                ));
            }
        }

        let now = op.now();
        let mut his =
            HisTaskService::build_skip_his(&task, &[], flow_params, "{}");
        his.cooperate_type = crate::enums::CooperateType::Transfer as i8;
        his.approver = Some(current_handler.clone());
        his.collaborator = Some(new_handler.clone());
        his.flow_status = FlowStatus::Approval.as_str().to_string();

        instance.update_time = Some(now);
        instance.update_by = Some(op.user_id.clone());

        // 写半边单闭包。update_processed_by 的双转办并发窗口裁定见文件头。
        let instance = ctx
            .transaction(|tx| {
                Box::pin(async move {
                    UserRepository::update_processed_by_in_txn(
                        tx,
                        task_id,
                        &current_handler,
                        &new_handler,
                    )
                    .await?;
                    InstanceRepository::update_by_id_in_txn(tx, &instance)
                        .await?;
                    HisTaskRepository::insert_in_txn(tx, &his).await?;
                    Ok(instance)
                })
            })
            .await?;
        Ok(instance)
    }

    /// 委派：将任务临时委托给他人处理
    /// MVP 语义：委托人处理后流程推进（不返回原办理人）
    #[tracing::instrument(skip(self, ctx, flow_params, op), fields(task_id = task_id))]
    pub async fn depute(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
        flow_params: &FlowParams,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        let delegate = flow_params
            .next_handler
            .as_ref()
            .and_then(|v| v.first())
            .cloned()
            .ok_or_else(|| {
                AppError::BadRequest("缺少委托人 next_handler".to_string())
            })?;

        // 守卫提升
        let (task, mut instance, _def, _nodes, _skips) =
            self.load_skip_context(ctx, task_id).await?;

        let task_users =
            UserRepository::select_by_task_id(ctx, task_id).await?;
        let original = flow_params
            .handler
            .clone()
            .unwrap_or_else(|| op.user_id.clone());

        if !flow_params.ignore {
            let has_perm = task_users
                .iter()
                .any(|u| u.processed_by.as_deref() == Some(original.as_str()));
            if !has_perm {
                return Err(AppError::Forbidden(
                    "当前用户无办理权限".to_string(),
                ));
            }
        }

        let now = op.now();
        let depute_user = User {
            id: Some(snowflake::next_id()),
            type_: UserType::Depute as i8,
            processed_by: Some(delegate.clone()),
            associated: task_id,
            create_time: Some(now),
            create_by: Some(op.user_id.clone()),
            update_time: Some(now),
            update_by: Some(op.user_id.clone()),
            del_flag: Some("0".to_string()),
            tenant_id: op.tenant_id.clone(),
        };

        let mut his =
            HisTaskService::build_skip_his(&task, &[], flow_params, "{}");
        his.cooperate_type = crate::enums::CooperateType::Depute as i8;
        his.approver = Some(original.clone());
        his.collaborator = Some(delegate);
        his.flow_status = FlowStatus::Approval.as_str().to_string();

        instance.update_time = Some(now);
        instance.update_by = Some(op.user_id.clone());

        // 写半边单闭包(原办理人逻辑删,@TableLogic 平价)
        let instance = ctx
            .transaction(|tx| {
                Box::pin(async move {
                    UserRepository::soft_delete_by_task_and_handler_in_txn(
                        tx, task_id, &original,
                    )
                    .await?;
                    UserRepository::insert_in_txn(tx, &depute_user).await?;

                    InstanceRepository::update_by_id_in_txn(tx, &instance)
                        .await?;
                    HisTaskRepository::insert_in_txn(tx, &his).await?;
                    Ok(instance)
                })
            })
            .await?;
        Ok(instance)
    }

    /// 协同：当前办理人退出，把任务转给一组协同人；协同人全部通过后
    /// 流程直接推进（会签语义，由 try_halt 按 Collaborate 用户行触发）。
    ///
    /// 引擎原语，对齐参考实现 needCoor/coorUserIds 业务模式——流程图上
    /// 无需复制节点与互斥网关。协同人列表取自 `flow_params.add_handlers`。
    #[tracing::instrument(skip(self, ctx, flow_params, op), fields(task_id = task_id))]
    pub async fn collaborate(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
        flow_params: &FlowParams,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        let coor_handlers = flow_params
            .add_handlers
            .as_ref()
            .filter(|v| !v.is_empty())
            .cloned()
            .ok_or_else(|| {
                AppError::BadRequest("缺少协同人 add_handlers".to_string())
            })?;

        // 守卫提升
        let (task, mut instance, _def, _nodes, _skips) =
            self.load_skip_context(ctx, task_id).await?;

        let task_users =
            UserRepository::select_by_task_id(ctx, task_id).await?;
        let current_handler = flow_params
            .handler
            .clone()
            .unwrap_or_else(|| op.user_id.clone());

        if !flow_params.ignore {
            let has_perm = task_users.iter().any(|u| {
                u.processed_by.as_deref() == Some(current_handler.as_str())
            });
            if !has_perm {
                return Err(AppError::Forbidden(
                    "当前用户无办理权限".to_string(),
                ));
            }
        }

        let now = op.now();
        let coor_users: Vec<User> = coor_handlers
            .iter()
            .map(|handler| User {
                id: Some(snowflake::next_id()),
                type_: UserType::Collaborate as i8,
                processed_by: Some(handler.clone()),
                associated: task_id,
                create_time: Some(now),
                create_by: Some(op.user_id.clone()),
                update_time: Some(now),
                update_by: Some(op.user_id.clone()),
                del_flag: Some("0".to_string()),
                tenant_id: op.tenant_id.clone(),
            })
            .collect();

        let mut his =
            HisTaskService::build_skip_his(&task, &[], flow_params, "{}");
        his.cooperate_type = crate::enums::CooperateType::Collaborate as i8;
        his.approver = Some(current_handler.clone());
        his.collaborator = Some(coor_handlers.join(","));
        his.flow_status = FlowStatus::Approval.as_str().to_string();

        instance.update_time = Some(now);
        instance.update_by = Some(op.user_id.clone());

        // 写半边单闭包:发起人逻辑删 + 协同人入行(@TableLogic 平价)
        let instance = ctx
            .transaction(|tx| {
                Box::pin(async move {
                    UserRepository::soft_delete_by_task_and_handler_in_txn(
                        tx,
                        task_id,
                        &current_handler,
                    )
                    .await?;
                    UserRepository::insert_batch_in_txn(tx, &coor_users)
                        .await?;
                    InstanceRepository::update_by_id_in_txn(tx, &instance)
                        .await?;
                    HisTaskRepository::insert_in_txn(tx, &his).await?;
                    Ok(instance)
                })
            })
            .await?;
        Ok(instance)
    }

    /// 加签：动态增加任务办理人
    #[tracing::instrument(skip(self, ctx, flow_params, op), fields(task_id = task_id))]
    pub async fn add_signature(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
        flow_params: &FlowParams,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        let add_handlers = flow_params
            .add_handlers
            .as_ref()
            .filter(|v| !v.is_empty())
            .cloned()
            .ok_or_else(|| {
                AppError::BadRequest("缺少 add_handlers".to_string())
            })?;

        // 守卫提升;用户行闭包外构建(owned 进闭包)
        let (_task, mut instance, _def, _nodes, _skips) =
            self.load_skip_context(ctx, task_id).await?;

        let now = op.now();
        let users: Vec<User> = add_handlers
            .iter()
            .map(|handler| User {
                id: Some(snowflake::next_id()),
                type_: UserType::Approval as i8,
                processed_by: Some(handler.clone()),
                associated: task_id,
                create_time: Some(now),
                create_by: Some(op.user_id.clone()),
                update_time: Some(now),
                update_by: Some(op.user_id.clone()),
                del_flag: Some("0".to_string()),
                tenant_id: op.tenant_id.clone(),
            })
            .collect();

        instance.update_time = Some(now);
        instance.update_by = Some(op.user_id.clone());

        // 写半边单闭包
        let instance = ctx
            .transaction(|tx| {
                Box::pin(async move {
                    UserRepository::insert_batch_in_txn(tx, &users).await?;
                    InstanceRepository::update_by_id_in_txn(tx, &instance)
                        .await?;
                    Ok(instance)
                })
            })
            .await?;
        Ok(instance)
    }

    /// 减签：动态移除指定任务办理人
    #[tracing::instrument(skip(self, ctx, flow_params, op), fields(task_id = task_id))]
    pub async fn reduction_signature(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
        flow_params: &FlowParams,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        let remove_handlers = flow_params
            .reduction_handlers
            .as_ref()
            .filter(|v| !v.is_empty())
            .cloned()
            .ok_or_else(|| {
                AppError::BadRequest("缺少 reduction_handlers".to_string())
            })?;

        // 守卫提升
        let (_task, mut instance, _def, _nodes, _skips) =
            self.load_skip_context(ctx, task_id).await?;

        let now = op.now();
        instance.update_time = Some(now);
        instance.update_by = Some(op.user_id.clone());

        // 写半边单闭包(逻辑删,@TableLogic 平价)
        let instance = ctx
            .transaction(|tx| {
                Box::pin(async move {
                    for handler in &remove_handlers {
                        UserRepository::soft_delete_by_task_and_handler_in_txn(
                            tx, task_id, handler,
                        )
                        .await?;
                    }
                    InstanceRepository::update_by_id_in_txn(tx, &instance)
                        .await?;
                    Ok(instance)
                })
            })
            .await?;
        Ok(instance)
    }

    /// 根据实例 ID 查询任务列表
    pub async fn get_tasks_by_instance_id(
        &self,
        ctx: &DataContext<'_>,
        instance_id: i64,
    ) -> AppResult<Vec<Task>> {
        Ok(TaskRepository::select_by_instance_id(ctx, instance_id).await?)
    }

    /// 完成任务（PASS）
    // 参数较多:assignee_map 对齐 Java CompleteTaskBo.assigneeMap(弹窗
    // 选人),形态同 jump_to_node 的既有裁定
    #[allow(clippy::too_many_arguments)]
    pub async fn complete(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
        message: Option<String>,
        variable: HashMap<String, Value>,
        flow_copy_list: &[FlowCopyBo],
        assignee_map: Option<HashMap<String, Value>>,
        op: &OperatorContext,
    ) -> AppResult<()> {
        let params = FlowParams {
            skip_type: Some(SkipType::Pass.as_str().to_string()),
            handler: Some(op.user_id.clone()),
            message,
            variable,
            assignee_map,
            ..Default::default()
        };
        self.skip(ctx, task_id, &params, op).await?;
        if !flow_copy_list.is_empty() {
            self.save_copy(ctx, task_id, flow_copy_list, op).await?;
        }
        Ok(())
    }

    /// 查询单个任务视图
    pub async fn get_task_vo(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
    ) -> AppResult<Option<FlowTaskVo>> {
        let task = match TaskRepository::select_by_id(ctx, task_id).await? {
            Some(t) => t,
            None => return Ok(None),
        };
        Ok(Some(self.to_flow_task_vo(ctx, &task).await?))
    }

    /// 当前任务所有办理人
    pub async fn current_task_all_user(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
    ) -> AppResult<Vec<CurrentTaskUserVo>> {
        let users = UserRepository::select_all_by_task_id(ctx, task_id).await?;
        let ids: Vec<i64> = users
            .iter()
            .filter_map(|u| {
                u.processed_by.as_deref().and_then(|s| s.parse().ok())
            })
            .collect();
        let names_map = self.resolver.user_names(&ids).await;
        let result = ids
            .into_iter()
            .map(|id| CurrentTaskUserVo {
                user_id: id,
                nick_name: names_map.get(&id).cloned().unwrap_or_default(),
            })
            .collect();
        Ok(result)
    }

    /// 待办分页（当前用户）
    pub async fn page_wait(
        &self,
        ctx: &DataContext<'_>,
        handler: &str,
        filter: &TaskPageFilter,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<FlowTaskVo>> {
        let page = TaskRepository::select_todo_by_handler(
            ctx, handler, filter, page_no, page_size,
        )
        .await?;
        let rows = self.to_flow_task_vos(ctx, &page.rows).await?;
        Ok(page.with_rows(rows))
    }

    /// 已办分页（当前用户）
    pub async fn page_finish(
        &self,
        ctx: &DataContext<'_>,
        handler: &str,
        filter: &TaskPageFilter,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<FlowHisTaskVo>> {
        let page = HisTaskRepository::select_done_page(
            ctx, handler, filter, page_no, page_size,
        )
        .await?;
        let rows = self.to_flow_his_task_vos(ctx, &page.rows).await?;
        Ok(page.with_rows(rows))
    }

    /// 所有待办分页（管理员视角）
    pub async fn page_all_wait(
        &self,
        ctx: &DataContext<'_>,
        filter: &TaskPageFilter,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<FlowTaskVo>> {
        let page = TaskRepository::select_page(ctx, filter, page_no, page_size)
            .await?;
        let rows = self.to_flow_task_vos(ctx, &page.rows).await?;
        Ok(page.with_rows(rows))
    }

    /// 所有已办分页（管理员视角）
    pub async fn page_all_finish(
        &self,
        ctx: &DataContext<'_>,
        filter: &TaskPageFilter,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<FlowHisTaskVo>> {
        let page = HisTaskRepository::select_all_done_page(
            ctx, filter, page_no, page_size,
        )
        .await?;
        let rows = self.to_flow_his_task_vos(ctx, &page.rows).await?;
        Ok(page.with_rows(rows))
    }

    /// 抄送分页
    pub async fn page_copy(
        &self,
        ctx: &DataContext<'_>,
        handler: &str,
        filter: &TaskPageFilter,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<FlowTaskVo>> {
        let page = UserRepository::select_copy_task_ids_by_handler(
            ctx, handler, filter, page_no, page_size,
        )
        .await?;
        if page.rows.is_empty() {
            return Ok(app_data::Page::exact(Vec::new(), 0));
        }
        let mut rows = Vec::new();
        for task_id in &page.rows {
            if let Some(vo) = self.get_task_vo(ctx, *task_id).await? {
                rows.push(vo);
            }
        }
        Ok(page.with_rows(rows))
    }

    /// 保存抄送记录
    async fn save_copy(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
        flow_copy_list: &[FlowCopyBo],
        op: &OperatorContext,
    ) -> AppResult<()> {
        let now = op.now();
        let copies: Vec<User> = flow_copy_list
            .iter()
            .map(|c| User {
                id: Some(snowflake::next_id()),
                type_: UserType::Copy as i8,
                processed_by: Some(c.user_id.to_string()),
                associated: task_id,
                create_time: Some(now),
                create_by: Some(op.user_id.clone()),
                update_time: Some(now),
                update_by: Some(op.user_id.clone()),
                del_flag: Some("0".to_string()),
                tenant_id: op.tenant_id.clone(),
            })
            .collect();
        UserRepository::insert_batch(ctx, &copies).await?;
        Ok(())
    }

    /// 任务操作分发：transferTask / delegateTask / addSignature / reductionSignature / collaborateTask
    pub async fn task_operation(
        &self,
        ctx: &DataContext<'_>,
        op: &str,
        bo: &TaskOperationBo,
        op_ctx: &OperatorContext,
    ) -> AppResult<Instance> {
        let handlers = bo.handlers();
        if handlers.is_empty() {
            return Err(AppError::BadRequest("办理人不能为空".to_string()));
        }
        match op {
            "collaborateTask" | "collaborate" => {
                let params = FlowParams {
                    handler: Some(op_ctx.user_id.clone()),
                    add_handlers: Some(handlers),
                    message: bo.message.clone(),
                    ..Default::default()
                };
                self.collaborate(ctx, bo.task_id, &params, op_ctx).await
            }
            "transferTask" | "transfer" => {
                let params = FlowParams {
                    handler: Some(op_ctx.user_id.clone()),
                    next_handler: Some(handlers),
                    message: bo.message.clone(),
                    ..Default::default()
                };
                self.transfer(ctx, bo.task_id, &params, op_ctx).await
            }
            "delegateTask" | "delegate" | "depute" => {
                let params = FlowParams {
                    handler: Some(op_ctx.user_id.clone()),
                    next_handler: Some(handlers),
                    message: bo.message.clone(),
                    ..Default::default()
                };
                self.depute(ctx, bo.task_id, &params, op_ctx).await
            }
            "addSignature" => {
                let params = FlowParams {
                    add_handlers: Some(handlers),
                    message: bo.message.clone(),
                    ..Default::default()
                };
                self.add_signature(ctx, bo.task_id, &params, op_ctx).await
            }
            "reductionSignature" => {
                let params = FlowParams {
                    reduction_handlers: Some(handlers),
                    message: bo.message.clone(),
                    ..Default::default()
                };
                self.reduction_signature(ctx, bo.task_id, &params, op_ctx)
                    .await
            }
            _ => Err(AppError::BadRequest(format!("不支持的操作类型: {}", op))),
        }
    }

    /// 批量修改任务办理人
    pub async fn update_assignee(
        &self,
        ctx: &DataContext<'_>,
        task_ids: &[i64],
        user_id: i64,
        op: &OperatorContext,
    ) -> AppResult<()> {
        let now = op.now();
        let new_handler = user_id.to_string();
        // 用户行闭包外构建(owned 进闭包)
        let assignments: Vec<(i64, User)> = task_ids
            .iter()
            .map(|&task_id| {
                let user = User {
                    id: Some(snowflake::next_id()),
                    type_: UserType::Approval as i8,
                    processed_by: Some(new_handler.clone()),
                    associated: task_id,
                    create_time: Some(now),
                    create_by: Some(op.user_id.clone()),
                    update_time: Some(now),
                    update_by: Some(op.user_id.clone()),
                    del_flag: Some("0".to_string()),
                    tenant_id: op.tenant_id.clone(),
                };
                (task_id, user)
            })
            .collect();

        // 写半边单闭包(旧办理人逻辑删,@TableLogic 平价)
        ctx.transaction(|tx| {
            Box::pin(async move {
                for (task_id, user) in assignments {
                    UserRepository::soft_delete_by_task_id_in_txn(tx, task_id)
                        .await?;
                    UserRepository::insert_in_txn(tx, &user).await?;
                }
                Ok(())
            })
        })
        .await?;
        Ok(())
    }

    /// 退回到指定节点
    pub async fn back_process(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
        node_code: &str,
        message: Option<String>,
        op: &OperatorContext,
    ) -> AppResult<Instance> {
        // 守卫提升
        let (task, instance, def, all_nodes, all_skips) =
            self.load_skip_context(ctx, task_id).await?;

        let task_users =
            UserRepository::select_by_task_id(ctx, task_id).await?;
        let params = FlowParams {
            handler: Some(op.user_id.clone()),
            message,
            ..Default::default()
        };
        self.check_permission(
            &task_users,
            params.handler.as_deref().unwrap_or(""),
            params.ignore,
        )
        .await?;

        let target_node = all_nodes
            .iter()
            .find(|n| n.node_code == node_code)
            .cloned()
            .ok_or_else(|| {
                AppError::NotFound(format!("目标节点不存在: {}", node_code))
            })?;

        let var_map: HashMap<String, Value> = match &instance.variable {
            Some(v) if !v.is_empty() => {
                serde_json::from_str(v).unwrap_or_default()
            }
            _ => HashMap::new(),
        };
        let merged_var_str = serde_json::to_string(&var_map)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let params = FlowParams {
            skip_type: Some(SkipType::Reject.as_str().to_string()),
            ..params
        };

        // 写半边单闭包(owned 值先进闭包,HRTB 不收借用)
        let resolver = self.resolver.clone();
        let business_callback = self.business_callback.clone();
        let op_in = op.clone();
        let result = ctx
            .transaction(|tx| {
                Box::pin(async move {
                    Self::jump_to_node(
                        tx,
                        &*resolver,
                        &*business_callback,
                        &task,
                        instance,
                        &target_node,
                        &merged_var_str,
                        &var_map,
                        &params,
                        &op_in,
                        FlowStatus::TaskBack.as_str(),
                        &all_nodes,
                        &all_skips,
                        &def.flow_code,
                    )
                    .await
                })
            })
            .await?;
        Ok(result)
    }

    /// 查询可退回节点列表
    pub async fn get_back_nodes(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
        now_node_code: &str,
    ) -> AppResult<Vec<BackNodeVo>> {
        let task = TaskRepository::select_by_id(ctx, task_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("任务不存在 (id={})", task_id))
            })?;

        let his_tasks = HisTaskRepository::select_by_target_node_code(
            ctx,
            task.instance_id,
            now_node_code,
        )
        .await?;

        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for h in his_tasks {
            let code = h.node_code.clone();
            if seen.insert(code.clone()) {
                result.push(BackNodeVo {
                    node_code: code,
                    node_name: h.node_name.clone(),
                });
            }
        }
        Ok(result)
    }

    /// 预览下一节点(纯读,无事务)
    pub async fn get_next_nodes(
        &self,
        ctx: &DataContext<'_>,
        task_id: i64,
    ) -> AppResult<Vec<FlowNextNodeVo>> {
        let (task, instance, _def, all_nodes, all_skips) =
            self.load_skip_context(ctx, task_id).await?;

        let now_node = all_nodes
            .iter()
            .find(|n| n.node_code == task.node_code)
            .ok_or_else(|| {
                AppError::NotFound(format!("节点不存在: {}", task.node_code))
            })?;

        let var_map: HashMap<String, Value> = match &instance.variable {
            Some(v) if !v.is_empty() => {
                serde_json::from_str(v).unwrap_or_default()
            }
            _ => HashMap::new(),
        };

        let mut path_way = PathWayData {
            def_id: task.definition_id,
            ins_id: Some(task.instance_id),
            skip_type: Some(SkipType::Pass.as_str().to_string()),
            ..Default::default()
        };

        let next_node = graph::get_next_node(
            now_node,
            None,
            SkipType::Pass.as_str(),
            &mut path_way,
            &all_skips,
            &all_nodes,
        )
        .ok_or_else(|| AppError::BadRequest("无法找到下一节点".to_string()))?;

        // 预热 hasRole 角色快照(条件里有 hasRole 时才现查;纯读预览)
        let spel_ctx = crate::expression::condition::prefetch_spel_context(
            &*self.resolver,
            &all_skips,
            &var_map,
        )
        .await;
        let next_nodes = graph::get_next_by_check_gateway(
            &next_node,
            &var_map,
            &mut path_way,
            &all_skips,
            &all_nodes,
            &spel_ctx,
        )?;

        Ok(next_nodes
            .into_iter()
            .map(|n| FlowNextNodeVo {
                node_code: n.node_code,
                node_name: n.node_name,
            })
            .collect())
    }

    /// 催办任务：查找任务办理人并记录催办通知日志
    #[tracing::instrument(skip(self, ctx, bo, op), fields(task_id = bo.task_id))]
    pub async fn urge_task(
        &self,
        ctx: &DataContext<'_>,
        bo: &FlowUrgeTaskBo,
        op: &OperatorContext,
    ) -> AppResult<()> {
        // 查找任务办理人
        let users = UserRepository::select_by_task_id(ctx, bo.task_id).await?;
        let handlers: Vec<String> =
            users.into_iter().filter_map(|u| u.processed_by).collect();
        let user_ids: Vec<i64> =
            handlers.iter().filter_map(|s| s.parse().ok()).collect();
        let msg = bo.message.as_deref().unwrap_or("请尽快处理");
        if !user_ids.is_empty() {
            let _ = self
                .notifier
                .notify_users(&user_ids, msg, Some("/task/taskWaiting"))
                .await;
        }
        tracing::info!(
            task_id = bo.task_id,
            handlers = ?handlers,
            message = ?bo.message,
            operator = %op.user_id,
            "催办任务通知"
        );
        Ok(())
    }

    async fn to_flow_task_vos(
        &self,
        ctx: &DataContext<'_>,
        tasks: &[Task],
    ) -> AppResult<Vec<FlowTaskVo>> {
        let mut rows = Vec::with_capacity(tasks.len());
        for task in tasks {
            rows.push(self.to_flow_task_vo(ctx, task).await?);
        }
        Ok(rows)
    }

    async fn to_flow_his_task_vos(
        &self,
        ctx: &DataContext<'_>,
        his_tasks: &[HisTask],
    ) -> AppResult<Vec<FlowHisTaskVo>> {
        let mut rows = Vec::with_capacity(his_tasks.len());
        for his in his_tasks {
            rows.push(self.to_flow_his_task_vo(ctx, his).await?);
        }
        Ok(rows)
    }

    async fn to_flow_task_vo(
        &self,
        ctx: &DataContext<'_>,
        task: &Task,
    ) -> AppResult<FlowTaskVo> {
        let users =
            UserRepository::select_by_task_id(ctx, task.id.unwrap_or(0))
                .await?;
        let assignee_ids: Vec<i64> = users
            .iter()
            .filter_map(|u| {
                u.processed_by.as_deref().and_then(|s| s.parse().ok())
            })
            .collect();
        let names_map = self.resolver.user_names(&assignee_ids).await;
        let assignee_names = if assignee_ids.is_empty() {
            None
        } else {
            Some(
                assignee_ids
                    .iter()
                    .filter_map(|id| {
                        names_map
                            .get(id)
                            .cloned()
                            .or_else(|| Some(id.to_string()))
                    })
                    .collect::<Vec<_>>()
                    .join(","),
            )
        };

        let instance =
            InstanceRepository::select_by_id(ctx, task.instance_id).await?;
        let business_id = instance.as_ref().map(|i| i.business_id.clone());

        let def =
            DefinitionRepository::select_by_id(ctx, task.definition_id).await?;
        let flow_name = def.as_ref().map(|d| d.flow_name.clone());
        let flow_code = def.as_ref().map(|d| d.flow_code.clone());
        let form_custom = def.as_ref().and_then(|d| d.form_custom.clone());
        let form_path = def.as_ref().and_then(|d| d.form_path.clone());
        let version = def.as_ref().map(|d| d.version.clone());
        let category = def.as_ref().and_then(|d| d.category.clone());
        let category_name = if let Some(ref cat_id_str) = category
            && let Ok(cat_id) = cat_id_str.parse::<i64>()
        {
            crate::repository::category_repo::select_by_id(ctx, cat_id)
                .await
                .ok()
                .flatten()
                .map(|c| c.category_name.clone())
        } else {
            None
        };

        // 「申请人」取实例发起人,对齐 Java FlwTaskMapper 的
        // `.select("i", .., FlowInstance::getCreateBy)`。
        // task.create_by 是创建该任务行的操作者(首节点之后即上一个审批人),
        // 用它会让列表「申请人」列与 createByIds 筛选口径都错位。
        let applicant = instance.as_ref().and_then(|i| i.create_by.clone());
        let creator_name = if let Some(ref creator) = applicant {
            if let Ok(uid) = creator.parse::<i64>() {
                let names = self.resolver.user_names(&[uid]).await;
                names.get(&uid).cloned()
            } else {
                None
            }
        } else {
            None
        };

        let assignee_ids_str = if assignee_ids.is_empty() {
            None
        } else {
            Some(
                assignee_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            )
        };

        let apply_node =
            NodeRepository::select_by_definition_id(ctx, task.definition_id)
                .await
                .ok()
                .and_then(|nodes| {
                    nodes.iter().find(|n| n.node_code == task.node_code).map(
                        |n| {
                            NodeType::from_i8(n.node_type)
                                .is_some_and(|t| t.is_start())
                        },
                    )
                });

        // 根据节点类型和跳转配置计算可用操作按钮
        let button_list: Vec<crate::model::dto::ButtonPermissionVo> =
            if NodeType::from_i8(task.node_type).is_some_and(|t| t.is_end()) {
                vec![]
            } else {
                let mut buttons: Vec<(&str, Option<&str>)> = vec![
                    ("complete", None),
                    ("back", None),
                    ("transfer", None),
                    ("delegate", None),
                ];
                let skips =
                    SkipRepository::select_by_definition_id_and_now_node_code(
                        ctx,
                        task.definition_id,
                        &task.node_code,
                    )
                    .await
                    .unwrap_or_default();
                if skips.iter().any(|s| {
                    s.skip_type.as_deref() == Some(SkipType::Reject.as_str())
                }) {
                    buttons.push(("reject", None));
                }
                buttons
                    .into_iter()
                    .map(|(code, value)| {
                        crate::model::dto::ButtonPermissionVo {
                            code: code.to_string(),
                            value: value.map(|s| s.to_string()),
                            show: true,
                        }
                    })
                    .collect()
            };
        let button_permission =
            serde_json::to_value(&button_list).unwrap_or_default();

        let permission_list = task.permission_list.clone();

        let user_list: Vec<crate::model::dto::FlowUserVo> = users
            .iter()
            .filter_map(|u| {
                let uid = u.processed_by.as_deref()?.parse::<i64>().ok()?;
                Some(crate::model::dto::FlowUserVo {
                    user_id: uid,
                    nick_name: names_map.get(&uid).cloned(),
                })
            })
            .collect();

        let (business_code, business_title) =
            crate::service::business_fields::resolve_business_fields(
                ctx,
                &self.business_callback,
                task.instance_id,
                business_id.as_deref().unwrap_or_default(),
            )
            .await?;

        Ok(FlowTaskVo {
            id: task.id.unwrap_or(0),
            definition_id: task.definition_id,
            instance_id: task.instance_id,
            node_code: task.node_code.clone(),
            node_name: task.node_name.clone(),
            node_type: task.node_type,
            flow_name,
            flow_code,
            flow_status: task.flow_status.clone(),
            flow_status_label: FlowInstanceVo::status_label(&task.flow_status),
            category,
            category_name,
            form_custom,
            form_path,
            version,
            assignee_ids: assignee_ids_str,
            assignee_names,
            create_by_name: creator_name,
            processed_by: None,
            processed_by_name: None,
            node_ratio: None,
            type_: None,
            apply_node,
            button_permission,
            business_id,
            business_code,
            business_title,
            create_time: task.create_time.map(|t| t.to_string()),
            create_by: applicant,
            update_time: task.update_time,
            del_flag: task.del_flag.clone(),
            permission_list,
            user_list,
            button_list,
            copy_list: vec![],
            var_list: None,
        })
    }

    async fn to_flow_his_task_vo(
        &self,
        ctx: &DataContext<'_>,
        his: &HisTask,
    ) -> AppResult<FlowHisTaskVo> {
        let instance =
            InstanceRepository::select_by_id(ctx, his.instance_id).await?;
        let business_id = instance.as_ref().map(|i| i.business_id.clone());

        let def =
            DefinitionRepository::select_by_id(ctx, his.definition_id).await?;
        let flow_name = def.as_ref().map(|d| d.flow_name.clone());
        let flow_code = def.as_ref().map(|d| d.flow_code.clone());
        let form_custom = def.as_ref().and_then(|d| d.form_custom.clone());
        let form_path = def.as_ref().and_then(|d| d.form_path.clone());
        let version = def.as_ref().map(|d| d.version.clone());
        let category = def.as_ref().and_then(|d| d.category.clone());
        let category_name = if let Some(ref cat_id_str) = category
            && let Ok(cat_id) = cat_id_str.parse::<i64>()
        {
            crate::repository::category_repo::select_by_id(ctx, cat_id)
                .await
                .ok()
                .flatten()
                .map(|c| c.category_name.clone())
        } else {
            None
        };

        // HisTask 本身无 create_by 列;「申请人」取实例发起人,与待办页同口径
        // (对齐 Java FlwHisTaskMapper 的 .select("c"/"b", .., FlowInstance::getCreateBy))。
        let applicant = instance.as_ref().and_then(|i| i.create_by.clone());
        let creator_name = if let Some(ref creator) = applicant {
            if let Ok(uid) = creator.parse::<i64>() {
                let names = self.resolver.user_names(&[uid]).await;
                names.get(&uid).cloned()
            } else {
                None
            }
        } else {
            None
        };

        let assignee_ids: Vec<i64> = his
            .approver
            .as_deref()
            .map(|s| s.parse().ok())
            .into_iter()
            .flatten()
            .collect();
        let names_map = self.resolver.user_names(&assignee_ids).await;
        let assignee_names = names_map
            .get(assignee_ids.first().unwrap_or(&0))
            .cloned()
            .or_else(|| his.approver.clone());

        let assignee_ids_str = if assignee_ids.is_empty() {
            None
        } else {
            Some(
                assignee_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            )
        };

        let (business_code, business_title) =
            crate::service::business_fields::resolve_business_fields(
                ctx,
                &self.business_callback,
                his.instance_id,
                business_id.as_deref().unwrap_or_default(),
            )
            .await?;

        // 运行时长:create_time→update_time(对齐 Java runDuration)。
        let run_duration = match (his.create_time, his.update_time) {
            (Some(c), Some(u)) => Some(format_between_by_second(c, u)),
            _ => None,
        };
        let skip_type =
            (!his.skip_type.is_empty()).then(|| his.skip_type.clone());

        Ok(FlowHisTaskVo {
            id: his.id.unwrap_or(0),
            definition_id: his.definition_id,
            instance_id: his.instance_id,
            task_id: his.task_id,
            node_code: his.node_code.clone(),
            node_name: his.node_name.clone(),
            node_type: his.node_type,
            target_node_code: his.target_node_code.clone(),
            target_node_name: his.target_node_name.clone(),
            flow_name,
            flow_code,
            flow_status: his.flow_status.clone(),
            flow_status_label: FlowInstanceVo::status_label(&his.flow_status),
            flow_task_status: FlowHisTaskVo::derive_flow_task_status(
                his.cooperate_type,
                skip_type.as_deref(),
                &his.flow_status,
            ),
            cooperate_type: his.cooperate_type,
            cooperate_type_name: CooperateType::label(his.cooperate_type)
                .map(String::from),
            approver: his.approver.clone(),
            approver_name: assignee_names.clone(),
            collaborator: his.collaborator.clone(),
            skip_type,
            message: his.message.clone(),
            run_duration,
            ext: his.ext.clone(),
            category,
            category_name,
            form_custom,
            form_path,
            version,
            assignee_ids: assignee_ids_str,
            assignee_names,
            create_by_name: creator_name,
            button_permission: serde_json::Value::Object(serde_json::Map::new()),
            business_id,
            business_code,
            business_title,
            create_time: his.create_time.map(|t| t.to_string()),
            create_by: applicant,
            update_time: his.update_time,
            del_flag: his.del_flag.clone(),
            permission_list: vec![],
            user_list: vec![],
            button_list: vec![],
            copy_list: vec![],
            var_list: None,
        })
    }
}

/// 闭包内监听器 Err 的通道桥(同 persist::listener_failed,AppError 文案
/// 经 Unsupported 透传,500 ↔ 500,回滚触发不变)。
fn listener_failed(e: AppError) -> DataError {
    DataError::Unsupported(format!("监听器执行失败:{e}"))
}

/// 计算两个时间点之间的时长,精确到秒,中文单位(对齐 Java
/// `DateUtils.formatBetweenBySecond` / hutool `BetweenFormatter` Level.SECOND)。
/// 省略为零的高位单位;完全相等时返回 "0秒"。
fn format_between_by_second(
    create_time: chrono::NaiveDateTime,
    update_time: chrono::NaiveDateTime,
) -> String {
    let secs = (update_time - create_time).num_seconds().abs();
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let mins = (secs % 3_600) / 60;
    let s = secs % 60;
    let mut out = String::new();
    if days > 0 {
        out.push_str(&format!("{days}天"));
    }
    if hours > 0 {
        out.push_str(&format!("{hours}小时"));
    }
    if mins > 0 {
        out.push_str(&format!("{mins}分钟"));
    }
    if s > 0 {
        out.push_str(&format!("{s}秒"));
    }
    if out.is_empty() {
        out.push_str("0秒");
    }
    out
}
