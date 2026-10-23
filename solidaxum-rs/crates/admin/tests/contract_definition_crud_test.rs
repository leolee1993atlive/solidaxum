//! 流程定义 新建/编辑 契约测试。
//!
//! 运行: `cargo test -p admin --features test-util --test contract_definition_crud_test`
//! (cargo 的位置参数过滤的是**测试函数名**而非文件名,必须用 `--test` 指定目标)
//!
//! 锁定两条:
//! 1. 「添加流程」产出的是**没有节点、前端不传 version** 的空定义,必须能存下
//!    (对齐 Java `FlwDefinitionController.add` → `checkAndSave`);
//! 2. 空定义**不可发布** —— 节点完整性的兜底仍在 publish,不能因放开 save 而丢失。

#![cfg(feature = "test-util")]

use admin::testkit::{TestApp, code, super_admin};
use serde_json::json;

/// 前端「添加流程」实际发出的最小 body(对齐 Vue `index.vue:389-397` 的 initFormData,
/// 其中**没有 version、没有 nodeList**)。
fn add_body(flow_code: &str) -> serde_json::Value {
    json!({
        "flowCode": flow_code,
        "flowName": "空流程",
        "category": "1",
        "modelValue": "CLASSICS",
        "formCustom": "N",
        "formPath": "",
        "ext": "{\"autoPass\":false}"
    })
}

#[tokio::test]
async fn add_definition_without_version_and_nodes_succeeds() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (status, body) = app
        .post(
            "/workflow/definition",
            add_body("pf1_empty_def"),
            Some(&admin),
        )
        .await;

    assert_eq!(status.as_u16(), 200, "新建空流程定义应 200: {body}");
    assert_eq!(code(&body), 200, "code 应为 200: {body}");
    assert_eq!(
        body["data"]["flowCode"], "pf1_empty_def",
        "回显流程编码: {body}"
    );
    assert!(
        body["data"]["id"].is_string(),
        "id 应为字符串(stringify_ids): {body}"
    );
}

#[tokio::test]
async fn publish_empty_definition_still_rejected() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (status, body) = app
        .post(
            "/workflow/definition",
            add_body("pf1_publish_guard"),
            Some(&admin),
        )
        .await;
    assert_eq!(status.as_u16(), 200, "前置新建应 200: {body}");
    let id = body["data"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("id 应为字符串: {body}"))
        .to_string();

    let (status, body) = app
        .put(
            &format!("/workflow/definition/publish/{id}"),
            json!({}),
            Some(&admin),
        )
        .await;
    assert_ne!(
        status.as_u16(),
        200,
        "无节点定义不可发布(节点校验的兜底不能丢): {body}"
    );
}
