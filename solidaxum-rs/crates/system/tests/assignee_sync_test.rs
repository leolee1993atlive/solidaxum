//! flow_assignee_sync(outbox)enqueue 的 TDD 证据与行为钉。
//!
//! 核心不变量:outbox 行必须与业务变更**同事务**写入 ——
//! 「业务变更失败 ⟹ 补偿行不存在;补偿行存在 ⟹ 业务变更已提交」。
//! 测试钉三件事:
//! 1. enqueue_tx 写的行在**同一事务内**即可回读(钩子挂进业务事务后
//!    无跨连接可见性问题);
//! 2. 字段落库正确,user_ids 序列化为 JSON 数组字符串;
//! 3. 事务回滚时补偿行不落库(可靠性的一半:变更没了,补偿也不能留)。

use app_data::DataError;
use system::assignee_sync::{
    repository::{
        assignee_sync_repo::AssigneeSyncRepository, entity::flow_assignee_sync,
    },
    service::assignee_sync_service::AssigneeSyncService,
};

#[tokio::test]
async fn enqueue_tx_writes_row_visible_in_same_txn() {
    let ds = app_data::testing::sqlite_memory_empty().await;
    ds.create_table(flow_assignee_sync::Entity)
        .await
        .expect("建 flow_assignee_sync 表");
    let ctx = ds.system_context();

    ctx.transaction(|tx| {
        Box::pin(async move {
            AssigneeSyncService::enqueue_tx(
                tx,
                "role",
                "add",
                Some("common_admin"),
                &[201, 305],
            )
            .await?;

            // 同事务回读:行必须立即可见,字段逐项钉死
            let rows = AssigneeSyncRepository::select_all_in_txn(tx).await?;
            assert_eq!(rows.len(), 1, "同事务必须能读到刚 enqueue 的行");
            let row = &rows[0];
            assert_eq!(row.auth_type, "role");
            assert_eq!(row.oper_type, "add");
            assert_eq!(row.auth_code.as_deref(), Some("common_admin"));
            assert_eq!(
                row.user_ids.as_deref(),
                Some("[201,305]"),
                "user_ids 必须序列化为 JSON 数组字符串"
            );
            assert_eq!(row.status, "0", "新行必须为待处理");
            assert_eq!(row.retry_count, 0);
            assert!(row.error_message.is_none());
            assert!(row.create_time.is_some());
            assert!(row.update_time.is_some());
            Ok(())
        })
    })
    .await
    .expect("enqueue 同事务读必须成功");

    // 提交后连接路径同样可见(worker 的消费面)
    let pending = AssigneeSyncRepository::select_pending(&ctx, 5)
        .await
        .expect("连接路径读 pending");
    assert_eq!(pending.len(), 1);
}

#[tokio::test]
async fn enqueue_tx_empty_user_ids_serializes_empty_json_array() {
    let ds = app_data::testing::sqlite_memory_empty().await;
    ds.create_table(flow_assignee_sync::Entity)
        .await
        .expect("建 flow_assignee_sync 表");
    let ctx = ds.system_context();

    ctx.transaction(|tx| {
        Box::pin(async move {
            AssigneeSyncService::enqueue_tx(tx, "user", "update", None, &[])
                .await
        })
    })
    .await
    .expect("enqueue 空 user_ids 必须成功");

    let pending = AssigneeSyncRepository::select_pending(&ctx, 5)
        .await
        .expect("读 pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].user_ids.as_deref(), Some("[]"));
    assert!(pending[0].auth_code.is_none());
}

#[tokio::test]
async fn rollback_discards_enqueued_row_with_business_change() {
    let ds = app_data::testing::sqlite_memory_empty().await;
    ds.create_table(flow_assignee_sync::Entity)
        .await
        .expect("建 flow_assignee_sync 表");
    let ctx = ds.system_context();

    let result: Result<(), DataError> = ctx
        .transaction(|tx| {
            Box::pin(async move {
                AssigneeSyncService::enqueue_tx(
                    tx,
                    "user",
                    "add",
                    Some("u1"),
                    &[1],
                )
                .await?;
                // 模拟业务变更失败:补偿行必须随事务回滚
                Err(DataError::NotFound)
            })
        })
        .await;
    assert!(result.is_err());

    let pending = AssigneeSyncRepository::select_pending(&ctx, 5)
        .await
        .expect("读 pending");
    assert!(
        pending.is_empty(),
        "事务回滚后补偿行不得落库(outbox 可靠性不变量)"
    );
}

#[tokio::test]
async fn mark_done_and_mark_failed_flip_status() {
    let ds = app_data::testing::sqlite_memory_empty().await;
    ds.create_table(flow_assignee_sync::Entity)
        .await
        .expect("建 flow_assignee_sync 表");
    let ctx = ds.system_context();

    ctx.transaction(|tx| {
        Box::pin(async move {
            AssigneeSyncService::enqueue_tx(
                tx,
                "role",
                "delete",
                Some("r"),
                &[7],
            )
            .await
        })
    })
    .await
    .expect("enqueue");
    let id = AssigneeSyncRepository::select_pending(&ctx, 5)
        .await
        .unwrap()[0]
        .id;

    // 失败 → status='2' + retry_count=1 + 截断原因
    let long_err = "x".repeat(2100);
    ctx.transaction(|tx| {
        Box::pin(async move {
            AssigneeSyncRepository::mark_failed_in_txn(
                tx,
                id,
                1,
                &long_err,
                chrono::Utc::now().naive_utc(),
            )
            .await
        })
    })
    .await
    .expect("mark_failed");
    let rows = AssigneeSyncRepository::select_pending(&ctx, 5)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "retry_count(1) < max_retry(5) 仍会被重拉");
    assert_eq!(rows[0].status, "2");
    assert_eq!(rows[0].retry_count, 1);
    assert_eq!(rows[0].error_message.as_deref().unwrap().len(), 2000);

    // 成功 → status='1',不再被拉取
    ctx.transaction(|tx| {
        Box::pin(async move {
            AssigneeSyncRepository::mark_done_in_txn(
                tx,
                id,
                chrono::Utc::now().naive_utc(),
            )
            .await
        })
    })
    .await
    .expect("mark_done");
    let rows = AssigneeSyncRepository::select_pending(&ctx, 5)
        .await
        .unwrap();
    assert!(rows.is_empty(), "已处理行不得再被拉取");
}
