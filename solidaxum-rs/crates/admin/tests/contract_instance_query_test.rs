//! InstancePageQuery 参数面 contract 测试(审计 PIN2/PIN4/MDO1)。
//!
//! 运行: `cargo test -p admin --features test-util --test contract_instance_query_test`

#![cfg(feature = "test-util")]

use admin::testkit::{TestApp, assert_ok, user_with_perms};
use app_core::model::login_user::LoginUser;

/// 专用于实例查询参数面测试的种子用户(user_id=901)。
/// 该用户在 testkit FLOW_SEED 中拥有一批固定运行中/已完成实例。
fn flow_instance_user() -> LoginUser {
    let mut u = user_with_perms(&[
        "workflow:instance:list",
        "workflow:instance:currentList",
    ]);
    u.user_id = 901;
    u.username = "flow_instance_user".to_string();
    u.nick_name = Some("flow_instance".to_string());
    u
}

fn admin() -> LoginUser {
    admin::testkit::super_admin()
}

async fn get_total(app: &TestApp, uri: &str, user: &LoginUser) -> i64 {
    let (status, body) = app.get(uri, Some(user)).await;
    assert_ok(status, &body);
    body["data"]["total"].as_i64().unwrap_or(0)
}

fn assert_reduced(all: i64, filtered: i64, msg: &str) {
    assert!(filtered < all, "{msg}: 过滤后 {filtered} 应小于全量 {all}");
}

#[tokio::test]
async fn contract_instance_query_running_filters_by_category() {
    let app = TestApp::new().await;
    let admin = admin();
    let all = get_total(
        &app,
        "/workflow/instance/pageByRunning?pageNum=1&pageSize=50",
        &admin,
    )
    .await;
    assert!(all >= 2, "种子数据需要至少 2 条不同分类的运行中实例");
    let filtered = get_total(
        &app,
        "/workflow/instance/pageByRunning?pageNum=1&pageSize=50&category=1",
        &admin,
    )
    .await;
    assert_reduced(all, filtered, "pageByRunning category 过滤未生效(PIN2)");
}

#[tokio::test]
async fn contract_instance_query_running_filters_by_create_by_ids() {
    let app = TestApp::new().await;
    let admin = admin();
    let all = get_total(
        &app,
        "/workflow/instance/pageByRunning?pageNum=1&pageSize=50",
        &admin,
    )
    .await;
    let filtered = get_total(
        &app,
        "/workflow/instance/pageByRunning?pageNum=1&pageSize=50&createByIds=302",
        &admin,
    )
    .await;
    assert_reduced(all, filtered, "pageByRunning createByIds 过滤未生效(PIN4)");
}

#[tokio::test]
async fn contract_instance_query_running_filters_by_node_name() {
    let app = TestApp::new().await;
    let admin = admin();
    let all = get_total(
        &app,
        "/workflow/instance/pageByRunning?pageNum=1&pageSize=50",
        &admin,
    )
    .await;
    let filtered = get_total(
        &app,
        "/workflow/instance/pageByRunning?pageNum=1&pageSize=50&nodeName=审计",
        &admin,
    )
    .await;
    assert_reduced(all, filtered, "pageByRunning nodeName 过滤未生效(PIN4)");
}

#[tokio::test]
async fn contract_instance_query_running_filters_by_flow_name() {
    let app = TestApp::new().await;
    let admin = admin();
    let all = get_total(
        &app,
        "/workflow/instance/pageByRunning?pageNum=1&pageSize=50",
        &admin,
    )
    .await;
    let filtered = get_total(
        &app,
        "/workflow/instance/pageByRunning?pageNum=1&pageSize=50&flowName=采购",
        &admin,
    )
    .await;
    assert_reduced(all, filtered, "pageByRunning flowName 过滤未生效(PIN4)");
}

#[tokio::test]
async fn contract_instance_query_finish_filters_by_category() {
    let app = TestApp::new().await;
    let admin = admin();
    let all = get_total(
        &app,
        "/workflow/instance/pageByFinish?pageNum=1&pageSize=50",
        &admin,
    )
    .await;
    assert!(all >= 2, "种子数据需要至少 2 条不同分类的已完成实例");
    let filtered = get_total(
        &app,
        "/workflow/instance/pageByFinish?pageNum=1&pageSize=50&category=1",
        &admin,
    )
    .await;
    assert_reduced(all, filtered, "pageByFinish category 过滤未生效(PIN2)");
}

#[tokio::test]
async fn contract_instance_query_finish_filters_by_create_by_ids() {
    let app = TestApp::new().await;
    let admin = admin();
    let all = get_total(
        &app,
        "/workflow/instance/pageByFinish?pageNum=1&pageSize=50",
        &admin,
    )
    .await;
    let filtered = get_total(
        &app,
        "/workflow/instance/pageByFinish?pageNum=1&pageSize=50&createByIds=302",
        &admin,
    )
    .await;
    assert_reduced(all, filtered, "pageByFinish createByIds 过滤未生效(PIN4)");
}

#[tokio::test]
async fn contract_instance_query_finish_filters_by_node_name() {
    let app = TestApp::new().await;
    let admin = admin();
    let all = get_total(
        &app,
        "/workflow/instance/pageByFinish?pageNum=1&pageSize=50",
        &admin,
    )
    .await;
    let filtered = get_total(
        &app,
        "/workflow/instance/pageByFinish?pageNum=1&pageSize=50&nodeName=归档",
        &admin,
    )
    .await;
    assert_reduced(all, filtered, "pageByFinish nodeName 过滤未生效(PIN4)");
}

#[tokio::test]
async fn contract_instance_query_finish_filters_by_flow_name() {
    let app = TestApp::new().await;
    let admin = admin();
    let all = get_total(
        &app,
        "/workflow/instance/pageByFinish?pageNum=1&pageSize=50",
        &admin,
    )
    .await;
    let filtered = get_total(
        &app,
        "/workflow/instance/pageByFinish?pageNum=1&pageSize=50&flowName=采购",
        &admin,
    )
    .await;
    assert_reduced(all, filtered, "pageByFinish flowName 过滤未生效(PIN4)");
}

#[tokio::test]
async fn contract_instance_query_my_document_filters_by_category() {
    // MDO1: 我发起页走 pageByCurrent,同样必须支持 category,否则只修一半。
    let app = TestApp::new().await;
    let user = flow_instance_user();
    let all = get_total(
        &app,
        "/workflow/instance/pageByCurrent?pageNum=1&pageSize=50",
        &user,
    )
    .await;
    assert!(all >= 2, "901 发起实例应至少 2 条且属于不同分类");
    let filtered = get_total(
        &app,
        "/workflow/instance/pageByCurrent?pageNum=1&pageSize=50&category=1",
        &user,
    )
    .await;
    assert_reduced(all, filtered, "pageByCurrent category 过滤未生效(MDO1)");
}

#[tokio::test]
async fn contract_instance_query_my_document_filters_by_create_by_ids() {
    let app = TestApp::new().await;
    let user = flow_instance_user();
    let all = get_total(
        &app,
        "/workflow/instance/pageByCurrent?pageNum=1&pageSize=50",
        &user,
    )
    .await;
    // 当前用户是 901,按 302 过滤应直接清空,证明参数生效
    let filtered = get_total(
        &app,
        "/workflow/instance/pageByCurrent?pageNum=1&pageSize=50&createByIds=302",
        &user,
    )
    .await;
    assert_reduced(all, filtered, "pageByCurrent createByIds 过滤未生效(PIN4)");
}

#[tokio::test]
async fn contract_instance_query_my_document_filters_by_node_name() {
    let app = TestApp::new().await;
    let user = flow_instance_user();
    let all = get_total(
        &app,
        "/workflow/instance/pageByCurrent?pageNum=1&pageSize=50",
        &user,
    )
    .await;
    assert!(all >= 2, "901 发起实例应至少 2 条且节点名不同");
    let filtered = get_total(
        &app,
        "/workflow/instance/pageByCurrent?pageNum=1&pageSize=50&nodeName=审批",
        &user,
    )
    .await;
    assert_reduced(all, filtered, "pageByCurrent nodeName 过滤未生效(PIN4)");
}

#[tokio::test]
async fn contract_instance_query_my_document_filters_by_flow_name() {
    let app = TestApp::new().await;
    let user = flow_instance_user();
    let all = get_total(
        &app,
        "/workflow/instance/pageByCurrent?pageNum=1&pageSize=50",
        &user,
    )
    .await;
    assert!(all >= 2, "901 发起实例应至少 2 条且流程名不同");
    let filtered = get_total(
        &app,
        "/workflow/instance/pageByCurrent?pageNum=1&pageSize=50&flowName=采购",
        &user,
    )
    .await;
    assert_reduced(all, filtered, "pageByCurrent flowName 过滤未生效(PIN4)");
}
