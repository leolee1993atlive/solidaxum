//! 办理人重解析补偿服务(参考实现 消费侧)。
//!
//! worker(`admin::assignee_sync_worker`)消费 flow_assignee_sync 行后,
//! 本服务对**运行中实例**重解析办理人并 diff 增删 flow_user ——
//! 替代旧 Java 系统脆弱的 @Async 事件链(事件丢了无人知晓;
//! outbox 行丢了 = 业务事务本身失败,天然可靠)。
//!
//! 语义锚(与 Java ruoyi-workflow `assigneeHandler` 同口径):
//! - 运行中实例:`node_type BETWEEN 0 AND 1` 且
//!   `flow_status IN ('1','13')`(Approval/Pending,Java runningStatus);
//! - 任务级跳过:任务下 flow_user 存在 type IN ('2','3')(转办/委派)
//!   即整任务不动(对齐 Java assigneeHandler:1401-1406 —— 人工介入后
//!   办理人以人工操作为准,机器重解析不得覆盖);
//! - 事件过滤:`event_user_ids` 与 diff(adds ∪ removes)无交集 → 不应用;
//!   有交集 → 应用**完整** diff(对齐 Java 语义,不做部分应用);
//! - 软删死值:removes 走
//!   [`UserRepository::soft_delete_by_task_and_handler_in_txn`]
//!   (del_flag='1',`@TableLogic(value="0", delval="1")` 平价,
//!   死值照抄不发明);adds 形态与引擎 `build_task_users` 一致
//!   (type='1' 审批,时间戳/操作人手工盖)。
//!
//! 事务边界:**每实例一笔事务** —— 读与解析全走连接路径(resolver
//! 可能读库,不参与写事务),diff 应用收敛进 `dctx.transaction` 单笔
//! 提交/回滚。两实例之间失败互不影响(补偿语义:能补多少补多少,
//! 失败实例留给 worker 重试)。

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use app_core::util::snowflake;
use app_data::{DataContext, DataError};
use serde::Serialize;
use serde_json::Value;

use crate::{
    enums::UserType,
    expression::handler::{expand_handlers, resolve_handlers},
    model::user::User,
    repository::{
        instance_repo::InstanceRepository, node_repo::NodeRepository,
        task_repo::TaskRepository, user_repo::UserRepository,
    },
    resolver::{InstanceAssigneeContext, UserResolver},
};

/// 补偿操作人标识(无请求身份的 worker 路径;对齐 system 审计
/// 「无身份路径审计人 = 0(系统)」的约定)。
const SYSTEM_OPERATOR: &str = "0";

/// 重解析过滤条件。
#[derive(Clone, Debug, Default)]
pub struct ResyncFilter {
    /// 只处理该实例(空 = 全部运行中实例)。
    pub instance_id: Option<i64>,
    /// 事件波及用户(空 = 不过滤)。与 diff 无交集的任务跳过。
    pub event_user_ids: Option<Vec<i64>>,
}

/// 重解析结果报告(查询端点直接回传,故 Serialize)。
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResyncReport {
    /// 扫描到的运行中实例数
    pub instances: usize,
    /// 发生了 diff 应用的任务数
    pub tasks_touched: usize,
    /// 新增办理人行数
    pub users_added: usize,
    /// 软删办理人行数
    pub users_removed: usize,
    /// 因转办/委派跳过(人工介入)的任务数
    pub tasks_skipped_manual: usize,
}

/// 单个任务的待应用 diff(连接路径解析产物,事务内只做写)。
struct PendingTaskDiff {
    task_id: i64,
    tenant_id: Option<String>,
    adds: Vec<String>,
    removes: Vec<String>,
}

/// 办理人重解析补偿服务(无状态,持有 resolver 即可)。
pub struct AssigneeResyncService {
    resolver: Arc<dyn UserResolver>,
}

impl AssigneeResyncService {
    pub fn new(resolver: Arc<dyn UserResolver>) -> Self {
        Self { resolver }
    }

    /// 对运行中实例重解析办理人并应用 diff。全程 dctx 直传
    /// (worker 以 `system_context` 调用,flow 表全豁免不受辖)。
    pub async fn resync(
        &self,
        dctx: &DataContext<'_>,
        filter: ResyncFilter,
    ) -> Result<ResyncReport, DataError> {
        let instances = InstanceRepository::select_running_for_resync(
            dctx,
            filter.instance_id,
        )
        .await?;

        let mut report = ResyncReport::default();
        for inst in &instances {
            report.instances += 1;
            let instance_id = inst.id.unwrap_or_default();
            let vars = parse_vars(inst.variable.as_deref());
            let tasks =
                TaskRepository::select_by_instance_id(dctx, instance_id)
                    .await?;

            // 1) 连接路径逐任务解析 + diff(读与 resolver 不入事务)
            let mut pending: Vec<PendingTaskDiff> = Vec::new();
            for task in &tasks {
                let Some(task_id) = task.id else { continue };
                let Some(node) =
                    NodeRepository::select_by_definition_id_and_node_code(
                        dctx,
                        inst.definition_id,
                        &task.node_code,
                    )
                    .await?
                else {
                    continue;
                };
                let Some(perm) =
                    node.permission_flag.filter(|p| !p.trim().is_empty())
                else {
                    continue;
                };
                let users =
                    UserRepository::select_all_by_task_id(dctx, task_id)
                        .await?;
                // 跳过规则:存在转办(2)/委派(3)即整任务跳过(人工介入优先)
                if users.iter().any(|u| {
                    u.type_ == UserType::Transfer as i8
                        || u.type_ == UserType::Depute as i8
                }) {
                    report.tasks_skipped_manual += 1;
                    continue;
                }
                let old_set: HashSet<String> = users
                    .iter()
                    .filter(|u| u.type_ == UserType::Approval as i8)
                    .filter_map(|u| u.processed_by.clone())
                    .collect();
                let resolved = resolve_handlers(&perm, &vars);
                let expanded = expand_handlers(
                    self.resolver.as_ref(),
                    resolved,
                    Some(&InstanceAssigneeContext {
                        instance_id,
                        target_node_code: task.node_code.clone(),
                        variable: vars.clone(),
                        // 补偿 worker 跑在提交后:引用的 his 必已提交
                        current: None,
                    }),
                )
                .await;
                let new_set: HashSet<String> = expanded.into_iter().collect();
                let adds: Vec<String> =
                    new_set.difference(&old_set).cloned().collect();
                let removes: Vec<String> =
                    old_set.difference(&new_set).cloned().collect();
                if adds.is_empty() && removes.is_empty() {
                    continue;
                }
                // 事件过滤:波及用户与 diff 无交集 → 不应用;有交集 →
                // 应用完整 diff(对齐 Java 语义,不做部分应用)
                if let Some(event) =
                    filter.event_user_ids.as_ref().filter(|e| !e.is_empty())
                {
                    let intersects =
                        adds.iter().chain(removes.iter()).any(|handler| {
                            event.iter().any(|id| handler == &id.to_string())
                        });
                    if !intersects {
                        continue;
                    }
                }
                pending.push(PendingTaskDiff {
                    task_id,
                    tenant_id: task.tenant_id.clone(),
                    adds,
                    removes,
                });
            }
            if pending.is_empty() {
                continue;
            }

            // 2) 每实例一笔事务应用 diff(removes 软删 + adds 插入);
            //    报告计数先取出(pending 随即 move 进闭包)
            report.tasks_touched += pending.len();
            report.users_added +=
                pending.iter().map(|d| d.adds.len()).sum::<usize>();
            report.users_removed +=
                pending.iter().map(|d| d.removes.len()).sum::<usize>();
            let now = chrono::Utc::now().naive_utc();
            dctx.transaction(|tx| {
                Box::pin(async move {
                    for diff in &pending {
                        for handler in &diff.removes {
                            UserRepository::soft_delete_by_task_and_handler_in_txn(
                                tx, diff.task_id, handler,
                            )
                            .await?;
                        }
                        for handler in &diff.adds {
                            UserRepository::insert_in_txn(
                                tx,
                                &User {
                                    id: Some(snowflake::next_id()),
                                    type_: UserType::Approval as i8,
                                    processed_by: Some(handler.clone()),
                                    associated: diff.task_id,
                                    create_time: Some(now),
                                    create_by: Some(SYSTEM_OPERATOR.to_string()),
                                    update_time: Some(now),
                                    update_by: Some(SYSTEM_OPERATOR.to_string()),
                                    del_flag: Some("0".to_string()),
                                    tenant_id: diff.tenant_id.clone(),
                                },
                            )
                            .await?;
                        }
                    }
                    Ok(())
                })
            })
            .await?;
        }
        Ok(report)
    }
}

/// 实例变量 JSON 解析为 map;空串/坏 JSON 按空 map(与引擎
/// 变量缺失丢弃的语义兼容 —— resolve_handlers 对缺失变量本就丢弃)。
fn parse_vars(variable: Option<&str>) -> HashMap<String, Value> {
    variable
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default()
}
