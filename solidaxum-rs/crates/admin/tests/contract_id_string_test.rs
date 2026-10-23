//! 出站 ID 字段必须是 JSON 字符串(雪花 ID 精度根治)。
//!
//! 运行: `cargo test -p admin --features test-util --test contract_id_string_test`
//! (cargo 的位置参数过滤的是**测试函数名**而非文件名,必须用 `--test` 指定目标)
//!
//! 锁定三件事:
//! 1. 走 R<T> 信封的端点,ID 字段(含小 id)恒为字符串;
//! 2. job/jobLog 不再用外层 axum::Json 绕过信封;
//! 3. 非 ID 的数值字段(total/pageNum)不被误伤。

#![cfg(feature = "test-util")]

use admin::testkit::{TestApp, super_admin};
use serde_json::Value;

/// 断言 `v` 是字符串且等于 `expected`。
fn assert_id_string(v: &Value, expected: &str, ctx: &str) {
    assert!(v.is_string(), "{ctx} 应为 JSON 字符串,实得 {v}");
    assert_eq!(v.as_str().unwrap(), expected, "{ctx} 值不对");
}

#[tokio::test]
async fn job_list_ids_are_strings() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (status, body) = app.get("/job/list", Some(&admin)).await;
    assert_eq!(status.as_u16(), 200, "GET /job/list 应 200: {body}");

    let rows = body["data"]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("data.rows 应为数组: {body}"));
    assert_eq!(rows.len(), 2, "种子应有 2 条 job: {body}");

    let ids: Vec<&Value> = rows.iter().map(|r| &r["job_id"]).collect();
    for id in &ids {
        assert!(
            id.is_string(),
            "job_id 应为字符串,实得 {id}(外层 axum::Json 会绕过信封)"
        );
    }
    let id_strs: Vec<&str> = ids.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(
        id_strs.contains(&"1761300000000000001"),
        "19 位雪花 id 应原样保真: {id_strs:?}"
    );
    assert!(id_strs.contains(&"42"), "小 id 也必须字符串化: {id_strs:?}");
    assert!(body["data"]["total"].is_number(), "total 不是 ID: {body}");
}

#[tokio::test]
async fn job_log_list_ids_are_strings() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (status, body) = app.get("/monitor/jobLog/list", Some(&admin)).await;
    assert_eq!(
        status.as_u16(),
        200,
        "GET /monitor/jobLog/list 应 200: {body}"
    );

    let rows = body["data"]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("data.rows 应为数组: {body}"));
    assert_eq!(rows.len(), 1, "种子应有 1 条 jobLog: {body}");
    assert_id_string(&rows[0]["job_log_id"], "7", "jobLog.job_log_id");
}

#[tokio::test]
async fn system_list_ids_are_strings() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (status, body) = app.get("/system/dept/list", Some(&admin)).await;
    assert_eq!(status.as_u16(), 200, "GET /system/dept/list 应 200: {body}");

    let rows = body["data"]
        .as_array()
        .unwrap_or_else(|| panic!("data 应为数组: {body}"));
    let root = rows
        .iter()
        .find(|r| r["deptId"].as_str() == Some("100"))
        .unwrap_or_else(|| panic!("应能以字符串 \"100\" 找到总部: {body}"));
    assert_id_string(&root["parentId"], "0", "dept.parentId");
    assert_id_string(&root["leader"], "1761100000000000001", "dept.leader");
    assert!(root["orderNum"].is_number(), "orderNum 不是 ID: {root}");
}
