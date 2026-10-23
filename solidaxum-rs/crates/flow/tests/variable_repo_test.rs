//! flow_variable 仓储判例(spel_repo_test 同款脚手架:
//! `sqlite_memory_empty()` 单连接内存库 + `DataSource::create_table`)。
//!
//! flow_variable 无 del_flag(DDL 锚参考实现 update_flow_registry.sql),
//! 删除一律物理删 `delete_where`。
use app_data::{DataSource, testing::sqlite_memory_empty};
use flow::repository::{entity::flow_variable, variable_repo};

async fn setup_ds() -> DataSource {
    let ds = sqlite_memory_empty().await;
    ds.create_table(flow_variable::Entity).await.unwrap();
    ds
}

fn sample_variable(
    id: i64,
    variable_name: &str,
    label: &str,
) -> flow_variable::Model {
    flow_variable::Model {
        id,
        variable_name: variable_name.to_string(),
        label: label.to_string(),
        data_type: "String".to_string(),
        source: Some("AeRepInfoServiceImpl.startNewWorkFlow".to_string()),
        dict_type: None,
        remark: None,
        status: "0".to_string(),
        create_dept: None,
        create_by: Some(1),
        create_time: Some(chrono::Local::now().naive_local()),
        update_by: Some(1),
        update_time: Some(chrono::Local::now().naive_local()),
    }
}

#[tokio::test]
async fn test_insert_and_select_by_id() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();

    variable_repo::insert(
        &ctx,
        &sample_variable(9101, "initiator", "流程发起人"),
    )
    .await
    .unwrap();

    let found = variable_repo::select_by_id(&ctx, 9101).await.unwrap();
    assert!(found.is_some(), "按主键应能查到");
    let m = found.unwrap();
    assert_eq!(m.variable_name, "initiator");
    assert_eq!(m.label, "流程发起人");
    assert_eq!(m.data_type, "String");
    assert_eq!(m.status, "0");
}

#[tokio::test]
async fn test_unique_constraint_on_variable_name() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();

    variable_repo::insert(
        &ctx,
        &sample_variable(9102, "initiator", "流程发起人"),
    )
    .await
    .unwrap();

    // 同名再插 → DataError::Conflict(结构化唯一键判定)
    let err = variable_repo::insert(
        &ctx,
        &sample_variable(9103, "initiator", "重复变量"),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, app_data::DataError::Conflict),
        "唯一键冲突应判 Conflict,实际: {err}"
    );

    // 不同名可插
    variable_repo::insert(
        &ctx,
        &sample_variable(9104, "initiatorDeptId", "发起人部门"),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_select_page_filters_and_paging() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();

    variable_repo::insert(
        &ctx,
        &sample_variable(9105, "initiator", "流程发起人"),
    )
    .await
    .unwrap();
    variable_repo::insert(
        &ctx,
        &sample_variable(9106, "initiatorDeptId", "发起人部门id"),
    )
    .await
    .unwrap();
    let mut stopped = sample_variable(9107, "auditResult", "审核结果");
    stopped.status = "1".to_string();
    variable_repo::insert(&ctx, &stopped).await.unwrap();

    // 全量
    let all = variable_repo::select_page(&ctx, None, None, None, 1, 10)
        .await
        .unwrap();
    assert_eq!(all.total, 3, "全量应返回 3 条");

    // variableName 模糊
    let by_name =
        variable_repo::select_page(&ctx, Some("initiator"), None, None, 1, 10)
            .await
            .unwrap();
    assert_eq!(by_name.total, 2, "variableName 模糊应命中 2 条");

    // label 模糊
    let by_label =
        variable_repo::select_page(&ctx, None, Some("发起人"), None, 1, 10)
            .await
            .unwrap();
    assert_eq!(by_label.total, 2, "label 模糊应命中 2 条");

    // status 等值
    let by_status =
        variable_repo::select_page(&ctx, None, None, Some("1"), 1, 10)
            .await
            .unwrap();
    assert_eq!(by_status.total, 1, "status=1 应只剩停用的一条");
    assert_eq!(by_status.rows[0].variable_name, "auditResult");

    // 分页
    let page1 = variable_repo::select_page(&ctx, None, None, None, 1, 2)
        .await
        .unwrap();
    assert_eq!(page1.total, 3);
    assert_eq!(page1.rows.len(), 2, "第 1 页应 2 条");
    let page2 = variable_repo::select_page(&ctx, None, None, None, 2, 2)
        .await
        .unwrap();
    assert_eq!(page2.rows.len(), 1, "第 2 页应 1 条");
}

#[tokio::test]
async fn test_update_by_id() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();

    variable_repo::insert(
        &ctx,
        &sample_variable(9108, "initiator", "流程发起人"),
    )
    .await
    .unwrap();

    let mut m = variable_repo::select_by_id(&ctx, 9108)
        .await
        .unwrap()
        .unwrap();
    m.label = "发起人".to_string();
    m.data_type = "Long".to_string();
    m.source = None;
    variable_repo::update_by_id(&ctx, &m).await.unwrap();

    let updated = variable_repo::select_by_id(&ctx, 9108)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.label, "发起人");
    assert_eq!(updated.data_type, "Long");
    assert_eq!(updated.source, None, "source 应可置空");
}

#[tokio::test]
async fn test_delete_by_ids_physical() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();

    variable_repo::insert(&ctx, &sample_variable(9109, "v1", "变量1"))
        .await
        .unwrap();
    variable_repo::insert(&ctx, &sample_variable(9110, "v2", "变量2"))
        .await
        .unwrap();

    let affected = variable_repo::delete_by_ids(&ctx, &[9109, 9110])
        .await
        .unwrap();
    assert_eq!(affected, 2, "物理删应命中 2 行");

    // 无 del_flag:行彻底消失,不是墓碑
    assert!(
        variable_repo::select_by_id(&ctx, 9109)
            .await
            .unwrap()
            .is_none()
    );
    let rest = variable_repo::select_page(&ctx, None, None, None, 1, 10)
        .await
        .unwrap();
    assert_eq!(rest.total, 0, "删后应无残留行");
}
