//! 流程终态 → 业务表状态回写的 contract 测试。
//!
//! 背景:交付后复核发现两件事 ——
//! 1. `final_status_label` 产出过字典外的值("reject"),前端 dict-tag 翻译不到;
//! 2. 撤销(Retrieve)与退回(TaskBack)路径**根本没调用** `notify_business_status`,
//!    业务表状态会一直停在 waiting。
//!
//! 这里从 HTTP 入口一路验到 `test_leave.status` 真实落库值,断言写进去的
//! 必须是 `wf_business_status` 字典里的值。
//!
//! 运行: `cargo test -p admin --features test-util --test contract_business_status_test`

#![cfg(feature = "test-util")]

use std::sync::atomic::{AtomicU64, Ordering};

use admin::testkit::{TestApp, assert_ok, user_with_perms};
use app_core::model::login_user::LoginUser;

static SEQ: AtomicU64 = AtomicU64::new(0);

/// `wf_business_status` 字典取值(sql/seed_dict_wf.sql,对齐 Java BusinessStatusEnum)。
const WF_BUSINESS_STATUS: &[&str] = &[
    "cancel",
    "draft",
    "waiting",
    "finish",
    "invalid",
    "back",
    "termination",
];

fn as_id(v: &serde_json::Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

fn flow_user() -> LoginUser {
    let mut u = user_with_perms(&[
        "workflow:definition:add",
        "workflow:definition:publish",
        "workflow:instance:list",
        "workflow:instance:query",
        "workflow:instance:cancel",
    ]);
    // 种子 leave1 的 approve 节点 permission_flag 是 '301',办理人须为 301。
    u.user_id = 301;
    u
}

/// 插入一条请假单,返回其 id(即流程的 businessId)。
async fn seed_leave(app: &TestApp) -> i64 {
    let id =
        7_700_000_000_000_000_i64 + SEQ.fetch_add(1, Ordering::SeqCst) as i64;
    let ds = app.state.infra.data_source();
    app_data::testkit::execute_raw(
        &ds,
        &format!(
            "INSERT INTO test_leave (id, leave_type, status, leave_days) \
             VALUES ({id}, '事假', 'waiting', 1)"
        ),
    )
    .await
    .expect("插入请假单失败");
    id
}

/// 读回请假单当前 status(走 `ds.system_context()` + `select_by_id`)。
async fn leave_status(app: &TestApp, id: i64) -> Option<String> {
    let ds = app.state.infra.data_source();
    flow::repository::leave_repo::select_by_id(&ds.system_context(), id)
        .await
        .expect("查请假单失败")
        .and_then(|m| m.status)
}

/// 用种子 leave1(start → approve → end)起一个流程,返回 (businessId, taskId)。
async fn start_leave_flow(app: &TestApp, user: &LoginUser) -> (i64, i64) {
    let business_id = seed_leave(app).await;
    let (s, b) = app
        .post(
            "/workflow/task/startWorkFlow",
            serde_json::json!({
                "businessId": business_id.to_string(),
                "flowCode": "leave1",
                "variables": {}
            }),
            Some(user),
        )
        .await;
    assert_ok(s, &b);
    let task_id = b
        .pointer("/data/taskId")
        .and_then(as_id)
        .expect("应返回 taskId");
    (business_id, task_id)
}

#[tokio::test]
async fn start_flow_writes_waiting() {
    // 基线:提交申请后业务状态应为 waiting(此路径本就已接,作为对照组)。
    let app = TestApp::new().await;
    let user = flow_user();
    let (business_id, _task_id) = start_leave_flow(&app, &user).await;

    assert_eq!(
        leave_status(&app, business_id).await.as_deref(),
        Some("waiting"),
        "提交申请后业务状态应为 waiting"
    );
}

#[tokio::test]
async fn cancel_writes_cancel_not_stuck_at_waiting() {
    // 撤销路径此前不回调 notify_business_status,业务状态会一直停在 waiting。
    // 对齐 Java FlwInstanceServiceImpl.cancelProcessApply(显式带 BusinessStatusEnum.CANCEL)。
    let app = TestApp::new().await;
    let user = flow_user();
    let (business_id, _task_id) = start_leave_flow(&app, &user).await;
    assert_eq!(
        leave_status(&app, business_id).await.as_deref(),
        Some("waiting"),
        "撤销前应是 waiting"
    );

    let (s, b) = app
        .put(
            "/workflow/instance/cancelProcessApply",
            serde_json::json!({
                "businessId": business_id.to_string(),
                "message": "填错了,撤回重填"
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    let status = leave_status(&app, business_id).await;
    assert_eq!(
        status.as_deref(),
        Some("cancel"),
        "撤销后业务状态应为 cancel,而不是停在 waiting"
    );
    assert!(
        WF_BUSINESS_STATUS.contains(&status.as_deref().unwrap_or_default()),
        "写入的业务状态必须在 wf_business_status 字典内,否则前端显示原码值"
    );
}

/// 建一个多审批节点的请假流程并发布,返回定义 id。
/// flowCode 必须以 "leave" 开头,否则 LeaveStatusCallback 的 flowCode 守卫会跳过回写。
async fn publish_leave_flow(
    app: &TestApp,
    user: &LoginUser,
    flow_code: &str,
    middles: &[&str],
) -> i64 {
    let mut node_list = vec![serde_json::json!({
        "nodeType": 0,
        "nodeCode": "start",
        "nodeName": "开始",
        "skipList": [{
            "nowNodeCode": "start",
            "nextNodeCode": middles[0],
            "skipType": "PASS"
        }]
    })];
    for (i, code) in middles.iter().enumerate() {
        let next = middles.get(i + 1).copied().unwrap_or("end");
        node_list.push(serde_json::json!({
            "nodeType": 1,
            "nodeCode": code,
            "nodeName": code,
            // 全部节点都由 301 办理,便于同一用户推进整条流程
            "permissionFlag": "301",
            "skipList": [{
                "nowNodeCode": code,
                "nextNodeCode": next,
                "skipType": "PASS"
            }]
        }));
    }
    node_list.push(serde_json::json!({
        "nodeType": 2,
        "nodeCode": "end",
        "nodeName": "结束",
        "skipList": []
    }));

    let (s, b) = app
        .post(
            "/workflow/definition",
            serde_json::json!({
                "flowCode": flow_code,
                "flowName": "请假多节点测试流程",
                "version": "1",
                "category": "test",
                "nodeList": node_list
            }),
            Some(user),
        )
        .await;
    assert_ok(s, &b);
    let def_id = b
        .pointer("/data/id")
        .and_then(as_id)
        .expect("应返回定义 id");
    let (s, _) = app
        .put(
            &format!("/workflow/definition/publish/{def_id}"),
            serde_json::json!({}),
            Some(user),
        )
        .await;
    assert_eq!(s, 200, "发布应 200");
    def_id
}

#[tokio::test]
async fn back_to_apply_node_writes_back() {
    // 退回到**申请人节点**时业务状态应变 back
    // (对齐 Java WorkflowGlobalListener:209-215)。该路径此前完全不回调。
    let app = TestApp::new().await;
    let user = flow_user();
    publish_leave_flow(&app, &user, "leave_back_apply", &["apply", "audit"])
        .await;

    let business_id = seed_leave(&app).await;
    let (s, b) = app
        .post(
            "/workflow/task/startWorkFlow",
            serde_json::json!({
                "businessId": business_id.to_string(),
                "flowCode": "leave_back_apply",
                "variables": {}
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    let apply_task = b.pointer("/data/taskId").and_then(as_id).expect("taskId");

    // 申请人节点办完 → 流转到 audit
    let (s, b) = app
        .post(
            "/workflow/task/completeTask",
            serde_json::json!({ "taskId": apply_task.to_string(), "variables": {} }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    let (s, b) = app
        .get(
            "/workflow/task/pageByTaskWait?pageNum=1&pageSize=10",
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    let audit_task = b["data"]["rows"]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|r| r["nodeCode"] == "audit")
                .and_then(|r| as_id(&r["id"]))
        })
        .expect("应有 audit 节点待办");

    // 从 audit 退回申请人节点
    let (s, b) = app
        .post(
            "/workflow/task/backProcess",
            serde_json::json!({
                "taskId": audit_task.to_string(),
                "nodeCode": "apply",
                "message": "材料不全,退回补充"
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    assert_eq!(
        leave_status(&app, business_id).await.as_deref(),
        Some("back"),
        "退回到申请人节点后业务状态应为 back"
    );
}

#[tokio::test]
async fn back_to_middle_node_keeps_waiting() {
    // 退回到**中间审批节点**时流程仍在审批中,业务状态不应变 back。
    // Java 仅在「驳回的节点为申请人节点」时才置 BACK。
    let app = TestApp::new().await;
    let user = flow_user();
    publish_leave_flow(
        &app,
        &user,
        "leave_back_middle",
        &["apply", "audit1", "audit2"],
    )
    .await;

    let business_id = seed_leave(&app).await;
    let (s, b) = app
        .post(
            "/workflow/task/startWorkFlow",
            serde_json::json!({
                "businessId": business_id.to_string(),
                "flowCode": "leave_back_middle",
                "variables": {}
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    let mut task_id =
        b.pointer("/data/taskId").and_then(as_id).expect("taskId");

    // 推进两步:apply → audit1 → audit2
    for _ in 0..2 {
        let (s, b) = app
            .post(
                "/workflow/task/completeTask",
                serde_json::json!({ "taskId": task_id.to_string(), "variables": {} }),
                Some(&user),
            )
            .await;
        assert_ok(s, &b);
        let (s, b) = app
            .get(
                "/workflow/task/pageByTaskWait?pageNum=1&pageSize=10",
                Some(&user),
            )
            .await;
        assert_ok(s, &b);
        task_id = b["data"]["rows"]
            .as_array()
            .and_then(|rows| {
                rows.iter()
                    .find(|r| r["flowCode"] == "leave_back_middle")
                    .and_then(|r| as_id(&r["id"]))
            })
            .expect("应有下一个待办");
    }

    // 从 audit2 退回 audit1(中间节点,不是申请人节点)
    let (s, b) = app
        .post(
            "/workflow/task/backProcess",
            serde_json::json!({
                "taskId": task_id.to_string(),
                "nodeCode": "audit1",
                "message": "退回上一审批环节"
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    assert_eq!(
        leave_status(&app, business_id).await.as_deref(),
        Some("waiting"),
        "退回到中间审批节点时流程仍在审批中,业务状态不应变成 back"
    );
}

#[tokio::test]
async fn every_written_business_status_is_in_dict() {
    // 兜底:凡是被写进业务表的状态,都必须能被 wf_business_status 翻译。
    // 历史缺陷正是写入了字典外的 "reject"。
    let app = TestApp::new().await;
    let user = flow_user();

    let (started, _) = start_leave_flow(&app, &user).await;
    let (cancelled, _) = start_leave_flow(&app, &user).await;
    let (s, b) = app
        .put(
            "/workflow/instance/cancelProcessApply",
            serde_json::json!({
                "businessId": cancelled.to_string(),
                "message": "撤销"
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    for id in [started, cancelled] {
        let status = leave_status(&app, id).await.expect("应有 status");
        assert!(
            WF_BUSINESS_STATUS.contains(&status.as_str()),
            "业务状态 {status:?} 不在 wf_business_status 字典内"
        );
    }
}
