//! test_leave 仓储判例(exemption_test 同款脚手架:
//! `sqlite_memory_empty()` 单连接内存库 + `DataSource::create_table`)。
use app_data::{DataError, DataSource, testing::sqlite_memory_empty};
use flow::repository::{entity::test_leave, leave_repo};

async fn setup_ds() -> DataSource {
    let ds = sqlite_memory_empty().await;
    ds.create_table(test_leave::Entity).await.unwrap();
    ds
}

fn sample_leave(id: i64, leave_days: i32) -> test_leave::Model {
    let now = chrono::Local::now().naive_local();
    test_leave::Model {
        id,
        apply_code: Some(format!("LEAVE{id}")),
        leave_type: "事假".to_string(),
        start_date: Some(now),
        end_date: Some(now + chrono::Duration::days(i64::from(leave_days))),
        leave_days: Some(leave_days),
        remark: None,
        status: Some("draft".to_string()),
        create_dept: None,
        create_by: Some(1),
        create_time: Some(now),
        update_by: Some(1),
        update_time: Some(now),
    }
}

#[tokio::test]
async fn test_select_page_filters_by_leave_days_range() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();

    // 种子(owned 值进闭包)
    let m1 = sample_leave(9001, 2);
    let m2 = sample_leave(9002, 5);
    let seeded: Result<(), DataError> = ctx
        .transaction(|tx| {
            Box::pin(async move {
                leave_repo::insert_in_txn(tx, &m1).await?;
                leave_repo::insert_in_txn(tx, &m2).await?;
                Ok(())
            })
        })
        .await;
    seeded.unwrap();

    let all = leave_repo::select_page(&ctx, None, None, None, None, 1, 10)
        .await
        .unwrap();
    assert_eq!(all.total, 2, "全量应返回 2 条");
    assert_eq!(all.rows.len(), 2);

    let filtered =
        leave_repo::select_page(&ctx, None, None, Some(4), Some(6), 1, 10)
            .await
            .unwrap();
    assert_eq!(filtered.total, 1, "按请假天数范围过滤应只剩 1 条");
    assert_eq!(filtered.rows[0].leave_days, Some(5));
}

/// 导出路径不得再走分页。
///
/// `Paging` 把 page_size 钳在 1..=200(app-data/src/page.rs 不变式 1),
/// 所以 `select_page(.., 1, EXPORT_ROW_LIMIT)` 无论上限写多大都只取 200 行 ——
/// 导出因此静默少给数据,不报错、不告警。这条用 250 行种子把两边的差别钉死:
/// 分页拿到 200,`select_list` 拿到 250。
#[tokio::test]
async fn select_list_returns_everything_while_paging_clamps_at_200() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();

    let seeded: Result<(), DataError> = ctx
        .transaction(|tx| {
            Box::pin(async move {
                for i in 0..250i64 {
                    leave_repo::insert_in_txn(tx, &sample_leave(9100 + i, 3))
                        .await?;
                }
                Ok(())
            })
        })
        .await;
    seeded.unwrap();

    // 分页:page_size 被钳到 200,这正是导出此前拿到的行数
    let paged =
        leave_repo::select_page(&ctx, None, None, None, None, 1, 50_000)
            .await
            .unwrap();
    assert_eq!(
        paged.rows.len(),
        200,
        "page_size 被 Paging 钳到 200 —— 导出走分页就是这么少给的"
    );

    // 导出路径:全取,一条不少
    let all = leave_repo::select_list(&ctx, None, None, None, None)
        .await
        .unwrap();
    assert_eq!(all.len(), 250, "导出必须拿到全部行");

    // 闸门用的计数与导出同一套过滤,且是精确值(不受分页上限计数影响)
    let total = leave_repo::count_by_filters(&ctx, None, None, None, None)
        .await
        .unwrap();
    assert_eq!(total, 250);

    // 过滤条件在两条路径上一致
    let filtered = leave_repo::select_list(&ctx, None, None, Some(4), None)
        .await
        .unwrap();
    assert!(filtered.is_empty(), "种子都是 3 天,>=4 天应为空");
}
