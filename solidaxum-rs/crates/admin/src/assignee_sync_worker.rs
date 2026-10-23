//! 办理人补偿 worker(参考实现 消费侧)。
//!
//! 形态:`Arc<AppState>` move
//! 进闭包 + `interval` + 首拍跳过 + fire-and-forget。**刻意不走
//! `sys_job` 调度器** —— 那张表是用户可编辑的,把补偿消费放进去
//! 等于给了「后台点一下就停止补偿」的开关。
//!
//! 每拍:
//! 1. 拉取 `status='0'` 或 `(status='2' 且 retry_count < max_retry)`
//!    的 flow_assignee_sync 行,按 id 升序逐条消费;
//! 2. 解析 `user_ids` JSON → 事件过滤的 `event_user_ids`;
//! 3. `AssigneeResyncService::resync`(运行中实例重解析 + diff 应用,
//!    每实例一笔事务,见 flow::service::assignee_resync);
//! 4. 成功 → status='1';失败 → status='2'、retry_count+1、
//!    `tracing::error!` 告警(补偿失败必须在日志里可见,
//!    超过 max_retry 后行停留 '2' 不再被拉取,等待人工介入)。
//!
//! dctx 用 `system_context`(无请求身份;flow 表全豁免不受辖,
//! flow_assignee_sync 同为全豁免 outbox 表);resolver 与
//! `router.rs` 装配的 `SystemUserResolver` 同款。

use std::{sync::Arc, time::Duration};

use flow::{
    resolver::UserResolver,
    service::assignee_resync::{AssigneeResyncService, ResyncFilter},
};
use system::{
    app::AppState,
    assignee_sync::repository::assignee_sync_repo::AssigneeSyncRepository,
};

use crate::flow_resolver::SystemUserResolver;

/// 启动办理人补偿 worker,返回 JoinHandle。`interval_secs` 下限 5 秒。
pub fn spawn_assignee_sync_worker(
    app_state: Arc<AppState>,
    interval_secs: u64,
    max_retry: u32,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker =
            tokio::time::interval(Duration::from_secs(interval_secs.max(5)));
        // 启动后立刻会有一拍,tick 掉它 —— 补偿不抢启动窗口
        ticker.tick().await;

        loop {
            ticker.tick().await;

            let ds = app_state.infra.data_source();
            let dctx = ds.system_context();
            let pending =
                match AssigneeSyncRepository::select_pending(&dctx, max_retry)
                    .await
                {
                    Ok(rows) => rows,
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            "办理人补偿: 拉取待处理任务失败,本拍跳过"
                        );
                        continue;
                    }
                };
            if pending.is_empty() {
                continue;
            }

            let resolver: Arc<dyn UserResolver> = Arc::new(
                SystemUserResolver::new(app_state.infra.data_source()),
            );
            let service = AssigneeResyncService::new(resolver);

            for row in pending {
                let row_id = row.id;
                // user_ids 是坏 JSON 时按空集合处理(空事件集 =
                // 不过滤,等价全量重解析 —— 补偿优先于丢弃)。
                let event_user_ids: Vec<i64> = row
                    .user_ids
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();

                let result = service
                    .resync(
                        &dctx,
                        ResyncFilter {
                            instance_id: None,
                            event_user_ids: Some(event_user_ids),
                        },
                    )
                    .await;

                match result {
                    Ok(report) => {
                        tracing::info!(
                            row_id,
                            instances = report.instances,
                            tasks_touched = report.tasks_touched,
                            users_added = report.users_added,
                            users_removed = report.users_removed,
                            tasks_skipped_manual = report.tasks_skipped_manual,
                            "办理人补偿: 消费成功"
                        );
                        let now = chrono::Utc::now().naive_utc();
                        let mark = dctx
                            .transaction(|tx| {
                                Box::pin(async move {
                                    AssigneeSyncRepository::mark_done_in_txn(
                                        tx, row_id, now,
                                    )
                                    .await
                                })
                            })
                            .await;
                        if let Err(e) = mark {
                            tracing::error!(
                                row_id,
                                error = %e,
                                "办理人补偿: 置成功态失败(下拍幂等重试)"
                            );
                        }
                    }
                    Err(e) => {
                        let retry_count = row.retry_count + 1;
                        tracing::error!(
                            row_id,
                            retry_count,
                            error = %e,
                            "办理人补偿: 消费失败"
                        );
                        let now = chrono::Utc::now().naive_utc();
                        let err_text = e.to_string();
                        let mark = dctx
                            .transaction(|tx| {
                                Box::pin(async move {
                                    AssigneeSyncRepository::mark_failed_in_txn(
                                        tx,
                                        row_id,
                                        retry_count,
                                        &err_text,
                                        now,
                                    )
                                    .await
                                })
                            })
                            .await;
                        if let Err(e) = mark {
                            tracing::error!(
                                row_id,
                                error = %e,
                                "办理人补偿: 置失败态失败(下拍幂等重试)"
                            );
                        }
                        if retry_count >= max_retry as i32 {
                            tracing::error!(
                                row_id,
                                max_retry,
                                "办理人补偿: 已达最大重试次数,停留失败态等待人工介入"
                            );
                        }
                    }
                }
            }
        }
    })
}
