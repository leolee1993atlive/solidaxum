use app_core::util::snowflake;
use app_data::{DataError, TxnContext};

use crate::{
    engine::vote::should_advance,
    enums::{CooperateType, UserType},
    expression::handler::{expand_handlers, resolve_handlers},
    model::{instance::Instance, user::User},
    repository::{
        his_task_repo::HisTaskRepository, instance_repo::InstanceRepository,
        task_repo::TaskRepository, user_repo::UserRepository,
    },
    service::{
        definition_service::OperatorContext,
        his_task_service::HisTaskService,
        skip::{SkipDeps, context::SkipContext},
    },
};

/// 会签 / 票签 / 或签 / 顺签的阈值判定。
///
/// 返回 `Some(instance)` 表示本次办理**未达阈值**,流转到此为止:已写入部分
/// his,顺签已把任务轮转给下一位办理人。调用方应让闭包直接返回该实例。
///
/// 返回 `None` 表示阈值达成(或本节点根本不是多人节点),继续正常流转。
///
/// 事务句柄为 `&TxnContext<'_>`,错误通道 `DataError`(本函数不产生
/// AppError 文案);任务/办理人清理是**逻辑删**(@TableLogic 平价,
/// task_repo/user_repo 文件头裁定)。
pub(crate) async fn try_halt(
    tx: &TxnContext<'_>,
    sctx: &mut SkipContext,
    ctx: &OperatorContext,
    deps: &SkipDeps<'_>,
) -> Result<Option<Instance>, DataError> {
    let task_id = sctx.task.id.unwrap_or(0);

    // 出口 0：委派回溯 —— 被委派人(Depute 用户行)办理后,任务回到委托人
    // 终审而非推进。委托人从委派 his 的 approver 恢复。
    let handler_is_depute = sctx.task_users.iter().any(|u| {
        u.type_ == UserType::Depute as i8
            && u.processed_by.as_deref() == Some(sctx.handler())
    });
    if handler_is_depute {
        let depute_his =
            HisTaskRepository::select_depute_his_by_task_id_in_txn(tx, task_id)
                .await?
                .ok_or_else(|| {
                    DataError::Unsupported(format!(
                        "委派记录缺失 (task_id={task_id})"
                    ))
                })?;
        let delegator = depute_his.approver.clone().ok_or_else(|| {
            DataError::Unsupported(format!(
                "委派记录缺少委托人 (task_id={task_id})"
            ))
        })?;

        // 被委派人的部分通过 his(计数不影响:回溯后委托人终审才推进)
        let partial_his = HisTaskService::build_skip_his(
            &sctx.task,
            &[],
            &sctx.params,
            &sctx.variables_json,
        );
        HisTaskRepository::insert_in_txn(tx, &partial_his).await?;

        // 删被委派人,恢复委托人
        UserRepository::soft_delete_by_task_and_handler_in_txn(
            tx,
            task_id,
            sctx.handler(),
        )
        .await?;
        let now = ctx.now();
        let delegator_user = User {
            id: Some(snowflake::next_id()),
            type_: UserType::Approval as i8,
            processed_by: Some(delegator.clone()),
            associated: task_id,
            create_time: Some(now),
            create_by: Some(ctx.user_id.clone()),
            update_time: Some(now),
            update_by: Some(ctx.user_id.clone()),
            del_flag: Some("0".to_string()),
            tenant_id: ctx.tenant_id.clone(),
        };
        UserRepository::insert_in_txn(tx, &delegator_user).await?;

        sctx.instance.update_time = Some(now);
        sctx.instance.update_by = Some(ctx.user_id.clone());
        InstanceRepository::update_by_id_in_txn(tx, &sctx.instance).await?;
        return Ok(Some(sctx.instance.clone()));
    }

    // 协同编组(任务含 Collaborate 用户行)强制会签语义,覆盖节点 ratio。
    let has_collaborate = sctx
        .task_users
        .iter()
        .any(|u| u.type_ == UserType::Collaborate as i8);
    let node_ratio = if has_collaborate {
        "100".to_string()
    } else {
        sctx.now_node.node_ratio.clone().unwrap_or_default()
    };
    if node_ratio.is_empty() {
        return Ok(None);
    }
    let remaining_count = sctx.task_users.len();
    let processed_count =
        HisTaskRepository::count_pass_by_task_id_in_txn(tx, task_id).await?
            as usize;
    let is_sequence = CooperateType::is_sequence(&node_ratio);

    let resolved_handlers = resolve_handlers(
        &sctx.now_node.permission_flag.clone().unwrap_or_default(),
        &sctx.variables,
    );
    let resolved_handlers = expand_handlers(
        deps.resolver,
        resolved_handlers,
        Some(&crate::resolver::InstanceAssigneeContext {
            instance_id: sctx.task.instance_id,
            target_node_code: sctx.now_node.node_code.clone(),
            variable: sctx.variables.clone(),
            // 会签/票签的人数计数读:此刻尚无在途 his 行写入,库查权威
            current: None,
        }),
    )
    .await;
    let total_handlers = resolved_handlers.len();
    let total = if is_sequence {
        total_handlers
    } else {
        remaining_count + processed_count
    };

    if !(total > 1 || processed_count > 0 || is_sequence) {
        return Ok(None);
    }

    let new_processed = processed_count + 1;
    let advance = should_advance(&node_ratio, total, new_processed);

    UserRepository::soft_delete_by_task_and_handler_in_txn(
        tx,
        task_id,
        sctx.handler(),
    )
    .await?;

    if advance {
        UserRepository::soft_delete_by_task_id_in_txn(tx, task_id).await?;
        return Ok(None);
    }

    let partial_his = HisTaskService::build_skip_his(
        &sctx.task,
        &[],
        &sctx.params,
        &sctx.variables_json,
    );
    HisTaskRepository::insert_in_txn(tx, &partial_his).await?;

    if is_sequence
        && let Some(next_handler) = resolved_handlers.get(new_processed)
    {
        // 顺签：保留同一 task，只为下一个未处理办理人创建用户
        UserRepository::soft_delete_by_task_id_in_txn(tx, task_id).await?;
        let now = ctx.now();
        let new_user = User {
            id: Some(snowflake::next_id()),
            type_: UserType::Approval as i8,
            processed_by: Some(next_handler.clone()),
            associated: task_id,
            create_time: Some(now),
            create_by: Some(ctx.user_id.clone()),
            update_time: Some(now),
            update_by: Some(ctx.user_id.clone()),
            del_flag: Some("0".to_string()),
            tenant_id: ctx.tenant_id.clone(),
        };
        UserRepository::insert_in_txn(tx, &new_user).await?;

        let mut updated_task = sctx.task.clone();
        updated_task.permission_list = vec![next_handler.clone()];
        updated_task.update_time = Some(now);
        updated_task.update_by = Some(ctx.user_id.clone());
        TaskRepository::update_by_id_in_txn(tx, &updated_task).await?;
    }

    sctx.instance.update_time = Some(ctx.now());
    sctx.instance.update_by = Some(ctx.user_id.clone());
    InstanceRepository::update_by_id_in_txn(tx, &sctx.instance).await?;

    Ok(Some(sctx.instance.clone()))
}
