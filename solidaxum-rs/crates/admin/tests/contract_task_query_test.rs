//! TaskPageQuery 参数面 contract 测试。
//! 这些参数此前完全不在结构体里 → 被 serde 静默丢弃,前端传了没有任何反应。
//! 故测试必须断言「加了参数结果集真的变少」,而非只断 HTTP 200。
//!
//! 运行: `cargo test -p admin --features test-util --test contract_task_query_test`

#![cfg(feature = "test-util")]

use admin::testkit::{TestApp, assert_ok, super_admin, user_with_perms};
use app_core::model::login_user::LoginUser;

/// 专用于任务查询参数面测试的种子用户(user_id=901)。
/// 该用户在 testkit FLOW_SEED 中拥有一批固定的待办/已办/抄送任务。
fn flow_query_user() -> LoginUser {
    let mut u = user_with_perms(&["workflow:task:list"]);
    u.user_id = 901;
    u.username = "flow_query_user".to_string();
    u.nick_name = Some("flow_query".to_string());
    u
}

#[tokio::test]
async fn contract_task_query_all_task_wait_filters_by_node_name() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByAllTaskWait?pageNum=1&pageSize=50",
            Some(&admin),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(all_total >= 2, "种子数据需要至少 2 条不同节点的待办");

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByAllTaskWait?pageNum=1&pageSize=50&nodeName=审计",
            Some(&admin),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "nodeName 过滤未生效: 过滤后 {filtered_total} 应小于全量 {all_total}"
    );
    for row in filtered["data"]["rows"].as_array().unwrap() {
        assert_eq!(row["nodeName"].as_str().unwrap(), "审计");
    }
}

#[tokio::test]
async fn contract_task_query_all_task_wait_filters_by_create_by_ids() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByAllTaskWait?pageNum=1&pageSize=50",
            Some(&admin),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByAllTaskWait?pageNum=1&pageSize=50&createByIds=302",
            Some(&admin),
        )
        .await;
    assert_ok(s_f, &filtered);
    assert!(
        filtered["data"]["total"].as_i64().unwrap() < all_total,
        "createByIds 过滤未生效"
    );

    // 「申请人」= 实例发起人。种子里只有实例 202(business_id=biz_wait_2)由 302 发起,
    // 而 flow_task.create_by='302' 的是 task 301/303(实例 201/203,发起人 901)。
    // 若过滤走成 flow_task.create_by,这里会返回 biz_wait_1 / biz_wait_3。
    let rows = filtered["data"]["rows"].as_array().unwrap();
    let business_ids: Vec<&str> = rows
        .iter()
        .map(|r| r["businessId"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        business_ids,
        vec!["biz_wait_2"],
        "createByIds 应按 flow_instance.create_by(申请人)过滤,而非 flow_task.create_by(建行操作者)"
    );
}

#[tokio::test]
async fn contract_task_query_all_task_wait_filters_by_create_by_ids_multi() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByAllTaskWait?pageNum=1&pageSize=50",
            Some(&admin),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(
        all_total >= 2,
        "种子数据需要至少 2 条不同申请人的待办才能验证多 id 过滤"
    );

    // 逗号分隔、含空格、含不存在 id: 验证 split(',') / trim() / is_in 路径
    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByAllTaskWait?pageNum=1&pageSize=50&createByIds=302,%20999",
            Some(&admin),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "createByIds 多 id 过滤未生效: 过滤后 {filtered_total} 应小于全量 {all_total}"
    );
    // createBy 暴露的是实例发起人(对齐 Java FlowTaskVo 取 FlowInstance::getCreateBy)。
    // 种子刻意让 flow_task.create_by 与之错开,所以这条断言能同时钉住
    // 「过滤列正确」与「VO 字段口径正确」两件事。
    for row in filtered["data"]["rows"].as_array().unwrap() {
        let create_by = row["createBy"].as_str().unwrap();
        assert!(
            create_by == "302" || create_by == "999",
            "createBy 应匹配 302 或 999 之一: {row}"
        );
    }
}

#[tokio::test]
async fn contract_task_query_all_task_wait_filters_by_flow_code() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByAllTaskWait?pageNum=1&pageSize=50",
            Some(&admin),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByAllTaskWait?pageNum=1&pageSize=50&flowCode=purchase1",
            Some(&admin),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "flowCode 过滤未生效: 过滤后 {filtered_total} 应小于全量 {all_total}"
    );
    for row in filtered["data"]["rows"].as_array().unwrap() {
        assert_eq!(row["flowCode"].as_str().unwrap(), "purchase1");
    }
}

#[tokio::test]
async fn contract_task_query_all_task_wait_filters_by_flow_name() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByAllTaskWait?pageNum=1&pageSize=50",
            Some(&admin),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByAllTaskWait?pageNum=1&pageSize=50&flowName=采购",
            Some(&admin),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "flowName 过滤未生效: 过滤后 {filtered_total} 应小于全量 {all_total}"
    );
    for row in filtered["data"]["rows"].as_array().unwrap() {
        assert!(
            row["flowName"].as_str().unwrap().contains("采购"),
            "flowName 应包含'采购': {row}"
        );
    }
}

#[tokio::test]
async fn contract_task_query_my_task_wait_filters_by_node_name() {
    let app = TestApp::new().await;
    let user = flow_query_user();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByTaskWait?pageNum=1&pageSize=50",
            Some(&user),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(all_total >= 2, "901 种子待办应至少 2 条");

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByTaskWait?pageNum=1&pageSize=50&nodeName=审批",
            Some(&user),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "我的待办 nodeName 过滤未生效: {filtered_total} 应小于 {all_total}"
    );
}

#[tokio::test]
async fn contract_task_query_my_task_wait_filters_by_create_by_ids() {
    let app = TestApp::new().await;
    let user = flow_query_user();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByTaskWait?pageNum=1&pageSize=50",
            Some(&user),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByTaskWait?pageNum=1&pageSize=50&createByIds=302",
            Some(&user),
        )
        .await;
    assert_ok(s_f, &filtered);
    assert!(
        filtered["data"]["total"].as_i64().unwrap() < all_total,
        "我的待办 createByIds 过滤未生效"
    );

    // 901 的三条待办里,只有 task 302 所属实例(biz_wait_2)由 302 发起;
    // flow_task.create_by='302' 的却是另外两条 —— 走错列会返回相反的集合。
    let rows = filtered["data"]["rows"].as_array().unwrap();
    let business_ids: Vec<&str> = rows
        .iter()
        .map(|r| r["businessId"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        business_ids,
        vec!["biz_wait_2"],
        "我的待办 createByIds 应按申请人(flow_instance.create_by)过滤"
    );
}

#[tokio::test]
async fn contract_task_query_my_task_finish_filters_by_node_name() {
    let app = TestApp::new().await;
    let user = flow_query_user();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByTaskFinish?pageNum=1&pageSize=50",
            Some(&user),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(all_total >= 2, "901 种子已办应至少 2 条");

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByTaskFinish?pageNum=1&pageSize=50&nodeName=开始",
            Some(&user),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "我的已办 nodeName 过滤未生效: {filtered_total} 应小于 {all_total}"
    );
}

#[tokio::test]
async fn contract_task_query_my_task_finish_filters_by_create_by_ids() {
    let app = TestApp::new().await;
    let user = flow_query_user();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByTaskFinish?pageNum=1&pageSize=50",
            Some(&user),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByTaskFinish?pageNum=1&pageSize=50&createByIds=302",
            Some(&user),
        )
        .await;
    assert_ok(s_f, &filtered);
    assert!(
        filtered["data"]["total"].as_i64().unwrap() < all_total,
        "我的已办 createByIds 过滤未生效"
    );
}

#[tokio::test]
async fn contract_task_query_all_task_finish_filters_by_node_name() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByAllTaskFinish?pageNum=1&pageSize=50",
            Some(&admin),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(all_total >= 2, "种子已办应至少 2 条");

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByAllTaskFinish?pageNum=1&pageSize=50&nodeName=开始",
            Some(&admin),
        )
        .await;
    assert_ok(s_f, &filtered);
    assert!(
        filtered["data"]["total"].as_i64().unwrap() < all_total,
        "全部已办 nodeName 过滤未生效"
    );
}

#[tokio::test]
async fn contract_task_query_all_task_finish_filters_by_create_by_ids() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByAllTaskFinish?pageNum=1&pageSize=50",
            Some(&admin),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByAllTaskFinish?pageNum=1&pageSize=50&createByIds=302",
            Some(&admin),
        )
        .await;
    assert_ok(s_f, &filtered);
    assert!(
        filtered["data"]["total"].as_i64().unwrap() < all_total,
        "全部已办 createByIds 过滤未生效"
    );
}

#[tokio::test]
async fn contract_task_query_task_copy_filters_by_node_name() {
    let app = TestApp::new().await;
    let user = flow_query_user();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByTaskCopy?pageNum=1&pageSize=50",
            Some(&user),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(all_total >= 2, "901 种子抄送应至少 2 条");

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByTaskCopy?pageNum=1&pageSize=50&nodeName=审计",
            Some(&user),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "抄送我的 nodeName 过滤未生效: {filtered_total} 应小于 {all_total}"
    );
}

#[tokio::test]
async fn contract_task_query_my_task_wait_filters_by_flow_code() {
    let app = TestApp::new().await;
    let user = flow_query_user();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByTaskWait?pageNum=1&pageSize=50",
            Some(&user),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(all_total >= 2, "901 种子待办应至少 2 条");

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByTaskWait?pageNum=1&pageSize=50&flowCode=purchase1",
            Some(&user),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "我的待办 flowCode 过滤未生效: {filtered_total} 应小于 {all_total}"
    );
    for row in filtered["data"]["rows"].as_array().unwrap() {
        assert_eq!(row["flowCode"].as_str().unwrap(), "purchase1");
    }
}

#[tokio::test]
async fn contract_task_query_my_task_wait_filters_by_flow_name() {
    let app = TestApp::new().await;
    let user = flow_query_user();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByTaskWait?pageNum=1&pageSize=50",
            Some(&user),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(all_total >= 2, "901 种子待办应至少 2 条");

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByTaskWait?pageNum=1&pageSize=50&flowName=采购",
            Some(&user),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "我的待办 flowName 过滤未生效: {filtered_total} 应小于 {all_total}"
    );
    for row in filtered["data"]["rows"].as_array().unwrap() {
        assert!(
            row["flowName"].as_str().unwrap().contains("采购"),
            "flowName 应包含'采购': {row}"
        );
    }
}

#[tokio::test]
async fn contract_task_query_my_task_finish_filters_by_flow_code() {
    let app = TestApp::new().await;
    let user = flow_query_user();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByTaskFinish?pageNum=1&pageSize=50",
            Some(&user),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(all_total >= 2, "901 种子已办应至少 2 条");

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByTaskFinish?pageNum=1&pageSize=50&flowCode=purchase1",
            Some(&user),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "我的已办 flowCode 过滤未生效: {filtered_total} 应小于 {all_total}"
    );
    for row in filtered["data"]["rows"].as_array().unwrap() {
        assert_eq!(row["flowCode"].as_str().unwrap(), "purchase1");
    }
}

#[tokio::test]
async fn contract_task_query_my_task_finish_filters_by_flow_name() {
    let app = TestApp::new().await;
    let user = flow_query_user();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByTaskFinish?pageNum=1&pageSize=50",
            Some(&user),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(all_total >= 2, "901 种子已办应至少 2 条");

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByTaskFinish?pageNum=1&pageSize=50&flowName=采购",
            Some(&user),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "我的已办 flowName 过滤未生效: {filtered_total} 应小于 {all_total}"
    );
    for row in filtered["data"]["rows"].as_array().unwrap() {
        assert!(
            row["flowName"].as_str().unwrap().contains("采购"),
            "flowName 应包含'采购': {row}"
        );
    }
}

#[tokio::test]
async fn contract_task_query_all_task_finish_filters_by_flow_code() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByAllTaskFinish?pageNum=1&pageSize=50",
            Some(&admin),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(all_total >= 2, "种子已办应至少 2 条");

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByAllTaskFinish?pageNum=1&pageSize=50&flowCode=purchase1",
            Some(&admin),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "全部已办 flowCode 过滤未生效: {filtered_total} 应小于 {all_total}"
    );
    for row in filtered["data"]["rows"].as_array().unwrap() {
        assert_eq!(row["flowCode"].as_str().unwrap(), "purchase1");
    }
}

#[tokio::test]
async fn contract_task_query_all_task_finish_filters_by_flow_name() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByAllTaskFinish?pageNum=1&pageSize=50",
            Some(&admin),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(all_total >= 2, "种子已办应至少 2 条");

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByAllTaskFinish?pageNum=1&pageSize=50&flowName=采购",
            Some(&admin),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "全部已办 flowName 过滤未生效: {filtered_total} 应小于 {all_total}"
    );
    for row in filtered["data"]["rows"].as_array().unwrap() {
        assert!(
            row["flowName"].as_str().unwrap().contains("采购"),
            "flowName 应包含'采购': {row}"
        );
    }
}

#[tokio::test]
async fn contract_task_query_task_copy_filters_by_create_by_ids() {
    let app = TestApp::new().await;
    let user = flow_query_user();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByTaskCopy?pageNum=1&pageSize=50",
            Some(&user),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(all_total >= 2, "901 种子抄送应至少 2 条");

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByTaskCopy?pageNum=1&pageSize=50&createByIds=302",
            Some(&user),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "抄送我的 createByIds 过滤未生效: {filtered_total} 应小于 {all_total}"
    );
    // 901 收到抄送的三条里,只有 task 302 所属实例由 302 发起;
    // flow_task.create_by='302' 的是另外两条 —— 走错列会返回相反集合。
    let rows = filtered["data"]["rows"].as_array().unwrap();
    let business_ids: Vec<&str> = rows
        .iter()
        .map(|r| r["businessId"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        business_ids,
        vec!["biz_wait_2"],
        "抄送我的 createByIds 应按申请人(flow_instance.create_by)过滤"
    );
    for row in rows {
        assert_eq!(row["createBy"].as_str().unwrap(), "302");
    }
}

#[tokio::test]
async fn contract_task_query_task_copy_filters_by_flow_code() {
    let app = TestApp::new().await;
    let user = flow_query_user();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByTaskCopy?pageNum=1&pageSize=50",
            Some(&user),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(all_total >= 2, "901 种子抄送应至少 2 条");

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByTaskCopy?pageNum=1&pageSize=50&flowCode=purchase1",
            Some(&user),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "抄送我的 flowCode 过滤未生效: {filtered_total} 应小于 {all_total}"
    );
    for row in filtered["data"]["rows"].as_array().unwrap() {
        assert_eq!(row["flowCode"].as_str().unwrap(), "purchase1");
    }
}

#[tokio::test]
async fn contract_task_query_task_copy_filters_by_flow_name() {
    let app = TestApp::new().await;
    let user = flow_query_user();

    let (s_all, all) = app
        .get(
            "/workflow/task/pageByTaskCopy?pageNum=1&pageSize=50",
            Some(&user),
        )
        .await;
    assert_ok(s_all, &all);
    let all_total = all["data"]["total"].as_i64().unwrap();
    assert!(all_total >= 2, "901 种子抄送应至少 2 条");

    let (s_f, filtered) = app
        .get(
            "/workflow/task/pageByTaskCopy?pageNum=1&pageSize=50&flowName=采购",
            Some(&user),
        )
        .await;
    assert_ok(s_f, &filtered);
    let filtered_total = filtered["data"]["total"].as_i64().unwrap();
    assert!(
        filtered_total < all_total,
        "抄送我的 flowName 过滤未生效: {filtered_total} 应小于 {all_total}"
    );
    for row in filtered["data"]["rows"].as_array().unwrap() {
        assert!(
            row["flowName"].as_str().unwrap().contains("采购"),
            "flowName 应包含'采购': {row}"
        );
    }
}
