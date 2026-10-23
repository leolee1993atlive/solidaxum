//!参考实现 契约测试:重解析管理端点 + 补偿记录查询。
//!
//! - `POST /workflow/task/reResolveAssignee`:管理端兜底运维手段,对运行中
//!   实例重解析办理人(权限点 `workflow:task:reResolve`)。
//! - `GET /system/assigneeSync/list`:outbox 补偿任务分页查询
//!   (权限点 `system:assigneeSync:list`)。
//!
//! 运行: `cargo test -p admin --features test-util --test contract_assignee_sync_test`

#![cfg(feature = "test-util")]

use admin::testkit::{TestApp, assert_ok, code, super_admin, user_with_perms};

#[tokio::test]
async fn re_resolve_assignee_requires_permission() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:dept:list"]); // 无目标权限
    let (status, body) = app
        .post(
            "/workflow/task/reResolveAssignee",
            serde_json::json!({}),
            Some(&user),
        )
        .await;
    assert_eq!(status.as_u16(), 403, "无权限应 403: {body}");
    assert_eq!(code(&body), 403, "无权限应 code=403: {body}");
}

#[tokio::test]
async fn re_resolve_assignee_runs_full_resync_as_super_admin() {
    let app = TestApp::new().await;
    let (status, body) = app
        .post(
            "/workflow/task/reResolveAssignee",
            serde_json::json!({}),
            Some(&super_admin()),
        )
        .await;
    assert_ok(status, &body);
    // 报告五字段齐备(instances/tasksTouched/usersAdded/usersRemoved/
    // tasksSkippedManual,camelCase)
    for key in [
        "instances",
        "tasksTouched",
        "usersAdded",
        "usersRemoved",
        "tasksSkippedManual",
    ] {
        assert!(
            body["data"][key].is_number(),
            "报告缺数字字段 {key}: {body}"
        );
    }
}

#[tokio::test]
async fn re_resolve_assignee_with_instance_id_ok() {
    let app = TestApp::new().await;
    // 指定实例分支:空库上 instanceId 大概率扫不到运行中实例,只钉形状
    let (status, body) = app
        .post(
            "/workflow/task/reResolveAssignee",
            serde_json::json!({ "instanceId": 999999 }),
            Some(&super_admin()),
        )
        .await;
    assert_ok(status, &body);
    assert_eq!(body["data"]["instances"], 0);
}

#[tokio::test]
async fn assignee_sync_list_requires_permission() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:dept:list"]); // 无目标权限
    let (status, body) =
        app.get("/system/assigneeSync/list", Some(&user)).await;
    assert_eq!(status.as_u16(), 403, "无权限应 403: {body}");
    assert_eq!(code(&body), 403, "无权限应 code=403: {body}");
}

#[tokio::test]
async fn assignee_sync_list_returns_rows() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["system:assigneeSync:list"]);
    let (status, body) =
        app.get("/system/assigneeSync/list", Some(&user)).await;
    assert_ok(status, &body);
    // R 信封 data = TableDataInfo{rows,total,...}
    assert!(body["data"]["rows"].is_array(), "缺 rows: {body}");
    assert!(body["data"]["total"].is_i64(), "缺 total: {body}");
}
