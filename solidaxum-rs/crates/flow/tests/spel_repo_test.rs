//! flow_spel 仓储判例(exemption_test 同款脚手架:
//! `sqlite_memory_empty()` 单连接内存库 + `DataSource::create_table`)。
use app_data::{DataSource, testing::sqlite_memory_empty};
use flow::repository::{entity::flow_spel, spel_repo};

async fn setup_ds() -> DataSource {
    let ds = sqlite_memory_empty().await;
    ds.create_table(flow_spel::Entity).await.unwrap();
    ds
}

fn sample_spel(id: i64, method_name: &str) -> flow_spel::Model {
    flow_spel::Model {
        id,
        component_name: Some("testComponent".to_string()),
        method_name: Some(method_name.to_string()),
        method_params: Some("arg1".to_string()),
        view_spel: Some("#{}".to_string()),
        remark: None,
        status: Some("0".to_string()),
        del_flag: Some("0".to_string()),
        create_dept: None,
        create_by: Some(1),
        create_time: Some(chrono::Local::now().naive_local()),
        update_by: Some(1),
        update_time: Some(chrono::Local::now().naive_local()),
    }
}

#[tokio::test]
async fn test_select_page_filters_by_method_name() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();

    spel_repo::insert(&ctx, &sample_spel(9001, "selectDeptLeaderById"))
        .await
        .unwrap();
    spel_repo::insert(&ctx, &sample_spel(9002, "evalLeaveDays"))
        .await
        .unwrap();

    let all = spel_repo::select_page(&ctx, None, None, None, 1, 10)
        .await
        .unwrap();
    assert_eq!(all.total, 2, "全量应返回 2 条");
    assert_eq!(all.rows.len(), 2);

    let filtered =
        spel_repo::select_page(&ctx, None, Some("evalLeave"), None, 1, 10)
            .await
            .unwrap();
    assert_eq!(filtered.total, 1, "methodName 模糊过滤应只剩 1 条");
    assert_eq!(
        filtered.rows[0].method_name.as_deref(),
        Some("evalLeaveDays")
    );
}
