use app_core::{error::AppError, util::snowflake};
use app_data::{DataError, TxnContext};

use crate::{
    dto::flow_params::FlowParams,
    engine::{chart, transition::PathWayData},
    enums::{NodeType, SkipType, UserType},
    expression::handler::{
        apply_next_handler, expand_handlers, resolve_handlers,
    },
    listener::types::{
        LISTENER_ASSIGNMENT, LISTENER_CREATE, LISTENER_FINISH, ListenerVariable,
    },
    model::{
        def_json::DefJson, instance::Instance, node::Node, task::Task,
        user::User,
    },
    repository::{
        his_task_repo::HisTaskRepository, instance_repo::InstanceRepository,
        task_repo::TaskRepository, user_repo::UserRepository,
    },
    service::{
        definition_service::OperatorContext,
        his_task_service::HisTaskService,
        shared::{final_status_label, notify_business_status, set_flow_status},
        skip::{SkipDeps, context::SkipContext, listeners::fire_def_and_node},
    },
};

/// 汇聚网关仍有前驱分支未到达：只写 his、删掉本分支任务,不展开下游。
pub(crate) async fn commit_join_pending(
    tx: &TxnContext<'_>,
    sctx: &mut SkipContext,
    ctx: &OperatorContext,
) -> Result<Instance, DataError> {
    let task_id = sctx.task.id.unwrap_or(0);

    let his_task = HisTaskService::build_skip_his(
        &sctx.task,
        &[],
        &sctx.params,
        &sctx.variables_json,
    );
    UserRepository::soft_delete_by_task_id_in_txn(tx, task_id).await?;
    TaskRepository::soft_delete_by_id_in_txn(tx, task_id).await?;
    HisTaskRepository::insert_in_txn(tx, &his_task).await?;

    sctx.instance.update_time = Some(ctx.now());
    sctx.instance.update_by = Some(ctx.user_id.clone());
    InstanceRepository::update_by_id_in_txn(tx, &sctx.instance).await?;

    Ok(sctx.instance.clone())
}

/// 正常推进：建新任务与办理人、更新流程图与实例、写 his、触发三组监听器、
/// 流程终态时清理未完成任务并回写业务状态。
///
/// 事务句柄为 `&TxnContext<'_>`,错误通道 `DataError` —— def_json 缺失/
/// 解析失败经 `DataError::Unsupported` 往返(Unsupported → Internal)无损;
/// 监听器与业务回写的 Err 映射为 `Unsupported`(同 shared.rs 判例的
/// 「业务回写失败」前缀形态,500 ↔ 500,回滚触发不变)。
pub(crate) async fn commit_advance(
    tx: &TxnContext<'_>,
    sctx: &mut SkipContext,
    ctx: &OperatorContext,
    deps: &SkipDeps<'_>,
    path_way: &PathWayData,
    next_nodes: &[Node],
) -> Result<Instance, DataError> {
    let task_id = sctx.task.id.unwrap_or(0);
    let now = ctx.now();

    // 构建新任务
    let mut add_tasks =
        build_next_tasks(sctx, ctx, deps, next_nodes, now).await;

    // pass: 选人变量在 build_next_tasks 里消费即删,回写实例/his 前重算快照
    sctx.variables_json = serde_json::to_string(&sctx.variables)
        .map_err(|e| DataError::Unsupported(e.to_string()))?;

    // 更新流程图
    let def_json_str = sctx.instance.def_json.as_deref().ok_or_else(|| {
        DataError::Unsupported("实例缺少流程图数据".to_string())
    })?;
    let def_json: DefJson =
        serde_json::from_str(def_json_str).map_err(|e| {
            DataError::Unsupported(format!("解析流程图数据失败: {}", e))
        })?;
    let updated_json = chart::skip_metadata(&def_json, path_way);
    sctx.instance.def_json = Some(
        serde_json::to_string(&updated_json)
            .map_err(|e| DataError::Unsupported(e.to_string()))?,
    );

    let his_task = HisTaskService::build_skip_his(
        &sctx.task,
        next_nodes,
        &sctx.params,
        &sctx.variables_json,
    );

    // 注意：会从 add_tasks 中移除结束节点任务
    set_ins_finish_info(
        &mut sctx.instance,
        &mut add_tasks,
        &sctx.params,
        &sctx.variables_json,
    );
    sctx.instance.update_time = Some(now);
    sctx.instance.update_by = Some(ctx.user_id.clone());

    // 监听器：assignment → finish → create（顺序是契约）
    //
    // 监听器回灌语义:
    // - lv.variable 修改 → 回灌 sctx.variables(实例变量持久化)
    // - lv.next_tasks[i].permission_list 修改 → 回灌 add_tasks(按 task id
    //   匹配),监听器可动态改写下一任务办理人;其余字段与结构性增删
    //   不支持(会破坏汇聚/流程图不变量)
    // 注意:监听器拿到的是已展开的裸用户 ID 列表(expand 已完成),
    // 改写应写裸用户 ID,写 role:/dept: 等设计符不会再被展开。
    //
    // 配对按 node_code 关联而非位置索引(set_ins_finish_info 会移除
    // 结束节点任务,结束节点不在末尾时位置配对会错位——TODO(#3) 已修,
    // engine_test::listener_pairs_correctly_when_end_node_not_last 守着)。
    for i in 0..add_tasks.len() {
        let Some(node) = next_nodes
            .iter()
            .find(|n| n.node_code == add_tasks[i].node_code)
            .cloned()
        else {
            continue;
        };
        let mut lv = new_lv(sctx);
        lv.node = Some(node.clone());
        lv.task = Some(add_tasks[i].clone());
        lv.next_tasks = add_tasks.clone();
        fire_def_and_node(
            deps.registry,
            &sctx.definition,
            &node,
            LISTENER_ASSIGNMENT,
            &mut lv,
        )
        .await
        .map_err(listener_failed)?;
        write_back_task_permissions(&mut add_tasks, &lv.next_tasks);
        sctx.variables = lv.variable;
    }

    {
        let mut lv = new_lv(sctx);
        lv.node = Some(sctx.now_node.clone());
        lv.task = Some(sctx.task.clone());
        lv.next_nodes = next_nodes.to_vec();
        lv.next_tasks = add_tasks.clone();
        fire_def_and_node(
            deps.registry,
            &sctx.definition,
            &sctx.now_node,
            LISTENER_FINISH,
            &mut lv,
        )
        .await
        .map_err(listener_failed)?;
        write_back_task_permissions(&mut add_tasks, &lv.next_tasks);
        sctx.variables = lv.variable;
    }

    for i in 0..add_tasks.len() {
        let Some(node) = next_nodes
            .iter()
            .find(|n| n.node_code == add_tasks[i].node_code)
            .cloned()
        else {
            continue;
        };
        let mut lv = new_lv(sctx);
        lv.node = Some(node.clone());
        lv.task = Some(add_tasks[i].clone());
        lv.next_tasks = add_tasks.clone();
        fire_def_and_node(
            deps.registry,
            &sctx.definition,
            &node,
            LISTENER_CREATE,
            &mut lv,
        )
        .await
        .map_err(listener_failed)?;
        write_back_task_permissions(&mut add_tasks, &lv.next_tasks);
        sctx.variables = lv.variable;
    }

    // 监听器可能改了变量与办理人(回灌),落库前重算实例变量快照并按
    // 最终 permission_list 重建办理人用户行
    sctx.variables_json = serde_json::to_string(&sctx.variables)
        .map_err(|e| DataError::Unsupported(e.to_string()))?;
    sctx.instance.variable = Some(sctx.variables_json.clone());
    // 新待办简报与办理人用户行同口径:均为监听器回写后的最终
    // permission_list(结束节点任务已被 set_ins_finish_info 移除,
    // 不会误通知)
    sctx.created_task_briefs = add_tasks
        .iter()
        .map(|t| {
            (
                t.node_name.clone().unwrap_or_default(),
                t.permission_list.clone(),
            )
        })
        .collect();
    let new_users = build_task_users(&add_tasks, ctx, now);

    // 删旧、写新(逻辑删,@TableLogic 平价 —— task_repo/user_repo 文件头
    // 裁定)
    UserRepository::soft_delete_by_task_id_in_txn(tx, task_id).await?;
    TaskRepository::soft_delete_by_id_in_txn(tx, task_id).await?;
    TaskRepository::insert_batch_in_txn(tx, &add_tasks).await?;
    UserRepository::insert_batch_in_txn(tx, &new_users).await?;
    InstanceRepository::update_by_id_in_txn(tx, &sctx.instance).await?;
    HisTaskRepository::insert_in_txn(tx, &his_task).await?;

    // 流程结束时清理未完成任务
    if NodeType::from_i8(sctx.instance.node_type).is_some_and(|t| t.is_end()) {
        hand_undone_task(
            tx,
            sctx.instance.id.expect("id 由雪花生成或查询回填,必有值"),
        )
        .await?;
    }

    // 业务终态回写与本次办理同事务
    if let Some(status) = final_status_label(&sctx.instance.flow_status) {
        notify_business_status(
            deps.business_callback,
            tx,
            &sctx.definition.flow_code,
            &sctx.instance,
            status,
        )
        .await
        .map_err(|e| DataError::Unsupported(format!("业务回写失败:{e}")))?;
    }

    Ok(sctx.instance.clone())
}

/// 闭包内监听器 Err 的通道桥:AppError 文案塞不进 DataError(形态偏离,
/// 见 task_service 文件头),经 Unsupported 透传(500 ↔ 500,回滚触发不变)。
fn listener_failed(e: AppError) -> DataError {
    DataError::Unsupported(format!("监听器执行失败:{e}"))
}

fn new_lv(sctx: &SkipContext) -> ListenerVariable {
    ListenerVariable::new(
        sctx.definition.clone(),
        sctx.instance.clone(),
        sctx.variables.clone(),
        sctx.params.clone(),
    )
}

/// 监听器对 next_tasks 办理人改写的回灌:按 task id 匹配,仅
/// permission_list 生效;结构性增删与其余字段不支持(会破坏汇聚/
/// 流程图不变量)。
pub(crate) fn write_back_task_permissions(
    add_tasks: &mut [Task],
    from: &[Task],
) {
    for modified in from {
        if let Some(t) = add_tasks.iter_mut().find(|t| t.id == modified.id) {
            t.permission_list = modified.permission_list.clone();
        }
    }
}

async fn build_next_tasks(
    sctx: &mut SkipContext,
    ctx: &OperatorContext,
    deps: &SkipDeps<'_>,
    next_nodes: &[Node],
    now: chrono::NaiveDateTime,
) -> Vec<Task> {
    // 弹窗选人变量的消费方向(对齐 Java hisStatus):退回走 back:,其余 pass:
    let status = if sctx.params.skip_type.as_deref()
        == Some(crate::enums::SkipType::Reject.as_str())
    {
        "back"
    } else {
        "pass"
    };
    let mut add_tasks = Vec::with_capacity(next_nodes.len());
    for node in next_nodes {
        let resolved = resolve_handlers(
            &node.permission_flag.clone().unwrap_or_default(),
            &sctx.variables,
        );
        let permission_list = apply_next_handler(
            resolved,
            &sctx.params.next_handler,
            sctx.params.next_handler_append,
        );
        let permission_list = expand_handlers(
            deps.resolver,
            permission_list,
            Some(&crate::resolver::InstanceAssigneeContext {
                instance_id: sctx.task.instance_id,
                target_node_code: node.node_code.clone(),
                variable: sctx.variables.clone(),
                // 在途当前步:his 行在本事务内未提交,resolver 独立连接
                // 查不到(D14)—— 把内存里现成的当前步带进上下文;
                // 命中键 = 在途行的 target = 本次推进的目标节点
                current: Some(crate::resolver::CurrentStepContext {
                    node_code: sctx.task.node_code.clone(),
                    skip_target_node_code: node.node_code.clone(),
                    task_id: sctx.task.id.unwrap_or_default(),
                    handlers: sctx
                        .task_users
                        .iter()
                        .filter_map(|u| {
                            u.processed_by
                                .as_deref()
                                .and_then(|p| p.parse().ok())
                        })
                        .collect(),
                    approver: sctx
                        .params
                        .handler
                        .as_deref()
                        .and_then(|h| h.parse().ok()),
                }),
            }),
        )
        .await;
        // 弹窗选人覆盖(对齐 Java processTaskPermission):命中即整体覆盖
        // 办理人;pass: 消费一次即删,back: 保留
        let permission_list =
            match crate::expression::handler::take_status_assignee(
                &mut sctx.variables,
                &node.node_code,
                status,
            ) {
                Some(ids) => ids,
                None => permission_list,
            };

        add_tasks.push(Task {
            id: Some(snowflake::next_id()),
            definition_id: sctx.task.definition_id,
            instance_id: sctx.task.instance_id,
            node_code: node.node_code.clone(),
            node_name: node.node_name.clone(),
            node_type: node.node_type,
            flow_status: sctx.params.flow_status.clone().unwrap_or_else(|| {
                set_flow_status(node.node_type, SkipType::Pass.as_str())
            }),
            form_custom: node
                .form_custom
                .clone()
                .or(sctx.definition.form_custom.clone()),
            form_path: node
                .form_path
                .clone()
                .or(sctx.definition.form_path.clone()),
            create_time: Some(now),
            create_by: Some(ctx.user_id.clone()),
            update_time: Some(now),
            update_by: Some(ctx.user_id.clone()),
            del_flag: Some("0".to_string()),
            tenant_id: ctx.tenant_id.clone(),
            permission_list,
        });
    }
    add_tasks
}

fn build_task_users(
    add_tasks: &[Task],
    ctx: &OperatorContext,
    now: chrono::NaiveDateTime,
) -> Vec<User> {
    let mut users = Vec::new();
    for new_task in add_tasks {
        for permission in &new_task.permission_list {
            users.push(User {
                id: Some(snowflake::next_id()),
                type_: UserType::Approval as i8,
                processed_by: Some(permission.clone()),
                associated: new_task
                    .id
                    .expect("id 由雪花生成或查询回填,必有值"),
                create_time: Some(now),
                create_by: Some(ctx.user_id.clone()),
                update_time: Some(now),
                update_by: Some(ctx.user_id.clone()),
                del_flag: Some("0".to_string()),
                tenant_id: ctx.tenant_id.clone(),
            });
        }
    }
    users
}

// ---- 以下三个函数从 task_service.rs 原样剪切过来（Step 1）----

/// 从任务列表中获取下一任务（单个取之，多个取 ID 最大）
fn get_next_task(tasks: &[Task]) -> Option<&Task> {
    if tasks.is_empty() {
        return None;
    }
    if tasks.len() == 1 {
        return Some(&tasks[0]);
    }
    tasks.iter().max_by_key(|t| t.id.unwrap_or(0))
}

/// 设置实例完成信息：更新节点、流程状态，并从任务列表中移除结束节点任务
fn set_ins_finish_info(
    instance: &mut Instance,
    add_tasks: &mut Vec<Task>,
    flow_params: &FlowParams,
    merged_variable: &str,
) {
    instance.variable = Some(merged_variable.to_string());

    let end_task = add_tasks
        .iter()
        .find(|t| NodeType::from_i8(t.node_type).is_some_and(|nt| nt.is_end()))
        .cloned();

    add_tasks.retain(|t| {
        !NodeType::from_i8(t.node_type).is_some_and(|nt| nt.is_end())
    });

    let finally_task = if let Some(et) = end_task {
        Some(et)
    } else {
        get_next_task(add_tasks).cloned()
    };

    if let Some(ft) = finally_task {
        instance.node_code = ft.node_code.clone();
        instance.node_name = ft.node_name.clone();
        instance.node_type = ft.node_type;
        instance.flow_status =
            flow_params.flow_status.clone().unwrap_or(ft.flow_status);
    }
}

/// 处理未完成任务（流程结束时清理,逻辑删 @TableLogic 平价）
async fn hand_undone_task(
    tx: &TxnContext<'_>,
    instance_id: i64,
) -> Result<(), DataError> {
    let tasks =
        TaskRepository::select_by_instance_id_in_txn(tx, instance_id).await?;
    for task in tasks {
        let tid = task.id.unwrap_or(0);
        UserRepository::soft_delete_by_task_id_in_txn(tx, tid).await?;
        TaskRepository::soft_delete_by_id_in_txn(tx, tid).await?;
    }
    Ok(())
}
