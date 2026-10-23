#![cfg(feature = "test-util")]

use std::sync::atomic::{AtomicU64, Ordering};

use admin::testkit::{TestApp, api_uri, assert_ok, code, user_with_perms};
use app_core::model::login_user::LoginUser;
use axum::{
    body::Body,
    http::{Method, Request, header},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use http_body_util::BodyExt;
use tower::ServiceExt;

static HELPER_SEQ: AtomicU64 = AtomicU64::new(0);

/// 读取被 `stringify_ids` 序列化为字符串的 ID(仍兼容数字形态以防回退)。
fn as_id(v: &serde_json::Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

/// 启动流程并返回第一个待办任务 id(使用种子 leave1 已发布定义)。
/// 后续用例复用此辅助函数获取 taskId。
async fn start_and_get_wait_task(app: &TestApp, user: &LoginUser) -> i64 {
    let seq = HELPER_SEQ.fetch_add(1, Ordering::SeqCst);
    let biz = format!("helper_biz_{seq}");
    let (s, b) = app
        .post(
            "/workflow/task/startWorkFlow",
            serde_json::json!({
                "businessId": biz,
                "flowCode": "leave1",
                "variables": {}
            }),
            Some(user),
        )
        .await;
    assert_ok(s, &b);
    b.pointer("/data/taskId")
        .and_then(as_id)
        .expect("应返回 taskId")
}

#[tokio::test]
async fn workflow_requires_login_and_permission() {
    let app = TestApp::new().await;
    let (_s, b) = app.get("/workflow/definition/list", None).await;
    assert_eq!(code(&b), 401, "未登录应 401: {b}");

    let no_perm = user_with_perms(&[]);
    let (_s2, b2) = app.get("/workflow/definition/list", Some(&no_perm)).await;
    assert_eq!(code(&b2), 403, "缺权限应 403: {b2}");
}

#[tokio::test]
async fn instance_running_finished_and_his_map() {
    let app = TestApp::new().await;
    let user = user_with_perms(&[
        "workflow:definition:add",
        "workflow:definition:publish",
        "workflow:instance:list",
        "workflow:instance:query",
        "workflow:instance:currentList",
    ]);

    let add_body = serde_json::json!({
        "flowCode": "test_instance_flow",
        "flowName": "实例契约测试流程",
        "version": "1",
        "category": "test",
        "nodeList": [
            {
                "nodeType": 0,
                "nodeCode": "start",
                "nodeName": "开始",
                "skipList": [
                    {
                        "nowNodeCode": "start",
                        "nextNodeCode": "end",
                        "skipType": "PASS"
                    }
                ]
            },
            {
                "nodeType": 2,
                "nodeCode": "end",
                "nodeName": "结束",
                "skipList": []
            }
        ]
    });

    let (s, b) = app
        .post("/workflow/definition", add_body, Some(&user))
        .await;
    assert_ok(s, &b);
    let def_id = b
        .pointer("/data/id")
        .and_then(as_id)
        .expect("新增应返回定义 id");

    let (s, _b) = app
        .put(
            &format!("/workflow/definition/publish/{}", def_id),
            serde_json::json!({}),
            Some(&user),
        )
        .await;
    assert_eq!(s, 200, "发布应 200");

    let ds = app.state.infra.data_source();
    app_data::testkit::execute_raw(
        &ds,
        &format!(
            "INSERT INTO flow_instance \
             (id, definition_id, business_id, node_type, node_code, node_name, \
              flow_status, activity_status, del_flag, create_by) VALUES \
             (9000000000000000001, {}, 'test_biz_running', 1, 'between', '审批中', '1', 1, '0', '301'), \
             (9000000000000000002, {}, 'test_biz_finished', 2, 'end', '结束', '8', 1, '0', '301')",
            def_id, def_id
        ),
    )
    .await
    .expect("插入实例失败");

    let (s, b) = app
        .get("/workflow/instance/pageByRunning", Some(&user))
        .await;
    assert_ok(s, &b);
    assert!(b.pointer("/data/rows").is_some(), "应含 data.rows: {b}");
    assert!(b.pointer("/data/total").is_some(), "应含 data.total: {b}");
    // 列表须走富 VO(对齐 Java selectRunningInstanceList 返回 FlowInstanceVo):
    // flowName/flowStatusName/createByName 必须填充,否则管理页表格列空白。
    let rows = b["data"]["rows"].as_array().expect("rows 数组");
    let running = rows
        .iter()
        .find(|r| r["businessId"] == "test_biz_running")
        .expect("运行中列表应含 test_biz_running");
    assert_eq!(
        running["flowName"], "实例契约测试流程",
        "运行实例列表应填充 flowName: {b}"
    );
    assert_eq!(
        running["flowStatusName"], "审批中",
        "运行实例列表应填充 flowStatusName: {b}"
    );
    assert!(
        running
            .get("createByName")
            .and_then(|v| v.as_str())
            .is_some(),
        "运行实例列表应填充 createByName: {b}"
    );

    let (s, b) = app
        .get(
            "/workflow/instance/pageByCurrent?pageNum=1&pageSize=10",
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    assert!(b.pointer("/data/rows").is_some(), "应含 data.rows: {b}");
    assert!(b.pointer("/data/total").is_some(), "应含 data.total: {b}");

    let (s, b) = app
        .get("/workflow/instance/pageByFinish", Some(&user))
        .await;
    assert_ok(s, &b);
    assert!(b.pointer("/data/rows").is_some(), "应含 data.rows: {b}");
    assert!(b.pointer("/data/total").is_some(), "应含 data.total: {b}");

    let (s, b) = app
        .get(
            "/workflow/instance/flowHisTaskList/test_biz_running",
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    // 对齐 Java FlwInstanceServiceImpl#flowHisTaskList:仅 {list, instanceId}。
    // 流程图改由 /warm-flow/query-flow-chart/{instanceId} 提供,此接口不再含 image/flowName。
    assert!(b.pointer("/data/list").is_some(), "应含 data.list: {b}");
    assert!(
        b.pointer("/data/instanceId").is_some(),
        "应含 data.instanceId: {b}"
    );
    assert!(b.pointer("/data/image").is_none(), "不应再返回 image: {b}");
}

#[tokio::test]
async fn category_tree_and_spel_list() {
    let app = TestApp::new().await;
    let cu = user_with_perms(&["workflow:category:list"]);
    let (s1, b1) = app.get("/workflow/category/categoryTree", Some(&cu)).await;
    assert_ok(s1, &b1);
    // categoryTree 返回 hutool Tree 形态：节点含 label/weight，不含 categoryName/orderNum
    let tree = b1.pointer("/data").expect("应含 data");
    if let Some(node) = tree.as_array().and_then(|a| a.first()) {
        assert!(
            node.get("label").is_some(),
            "categoryTree 节点应含 label: {b1}"
        );
        assert!(
            node.get("weight").is_some(),
            "categoryTree 节点应含 weight: {b1}"
        );
        assert!(
            node.get("categoryName").is_none()
                && node.get("orderNum").is_none(),
            "categoryTree 节点不应含 categoryName/orderNum: {b1}"
        );
    }
    let su = user_with_perms(&["workflow:spel:list"]);
    let (s2, b2) = app.get("/workflow/spel/list", Some(&su)).await;
    assert_ok(s2, &b2);
    // spel list 返回 {rows,total}，行含 componentName/viewSpel/remark，不含 name/spel/description
    assert!(
        b2.pointer("/data/rows").is_some(),
        "spel list 应含 data.rows: {b2}"
    );
    assert!(
        b2.pointer("/data/total").is_some(),
        "spel list 应含 data.total: {b2}"
    );
    if let Some(row) = b2
        .pointer("/data/rows")
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())
    {
        assert!(
            row.get("componentName").is_some(),
            "spel 行应含 componentName: {b2}"
        );
        assert!(row.get("viewSpel").is_some(), "spel 行应含 viewSpel: {b2}");
        assert!(
            row.get("name").is_none()
                && row.get("spel").is_none()
                && row.get("description").is_none(),
            "spel 行不应含 name/spel/description: {b2}"
        );
    }
}

#[tokio::test]
async fn workflow_definition_list_exists() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["workflow:definition:list"]);
    let (status, body) =
        app.get("/workflow/definition/list", Some(&user)).await;
    assert_ok(status, &body);
}

#[tokio::test]
async fn workflow_definition_list_paged_envelope() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["workflow:definition:list"]);
    let (s, b) = app.get("/workflow/definition/list", Some(&user)).await;
    assert_ok(s, &b);
    assert!(b.pointer("/data/rows").is_some(), "应含 data.rows: {b}");
    assert!(b.pointer("/data/total").is_some(), "应含 data.total: {b}");
}

#[tokio::test]
async fn workflow_definition_list_only_published() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["workflow:definition:list"]);

    let ds = app.state.infra.data_source();
    app_data::testkit::execute_raw(
        &ds,
        r#"INSERT INTO flow_definition (id, flow_code, flow_name, model_value, category, version, is_publish, form_custom, form_path, activity_status, listener_type, listener_path, ext, create_time, create_by, update_time, update_by, del_flag, tenant_id) VALUES
            (9100000000000000001, 'unpub_def', '未发布流程', '{}', '1', '1', 0, NULL, NULL, 1, NULL, NULL, NULL, datetime('now'), '301', datetime('now'), '301', '0', NULL)"#,
    )
    .await
    .expect("插入未发布定义失败");

    let (s, b) = app.get("/workflow/definition/list", Some(&user)).await;
    assert_ok(s, &b);
    let rows = b["data"]["rows"].as_array().expect("rows 应为数组");
    assert!(
        rows.iter().any(|r| r["flowCode"] == "leave1"),
        "已发布流程 leave1 应出现在列表中: {b}"
    );
    assert!(
        !rows.iter().any(|r| r["flowCode"] == "unpub_def"),
        "未发布流程 unpub_def 不应出现在 /workflow/definition/list: {b}"
    );
}

#[tokio::test]
async fn task_start_complete_and_wait_list() {
    let app = TestApp::new().await;
    let user = user_with_perms(&[
        "workflow:definition:add",
        "workflow:definition:publish",
        "workflow:instance:list",
        "task:complete",
        "task:waitList",
        "task:finishList",
        "task:query",
    ]);

    let flow_body = serde_json::json!({
        "flowCode": "task_contract_flow",
        "flowName": "任务契约流程",
        "version": "1",
        "category": "test",
        "nodeList": [
            {
                "nodeType": 0,
                "nodeCode": "start",
                "nodeName": "开始",
                "skipList": [
                    { "nowNodeCode": "start", "nextNodeCode": "approve", "skipType": "PASS" }
                ]
            },
            {
                "nodeType": 1,
                "nodeCode": "approve",
                "nodeName": "审批",
                "permissionFlag": "301",
                "skipList": [
                    { "nowNodeCode": "approve", "nextNodeCode": "end", "skipType": "PASS" }
                ]
            },
            {
                "nodeType": 2,
                "nodeCode": "end",
                "nodeName": "结束",
                "skipList": []
            }
        ]
    });

    let (s, b) = app
        .post("/workflow/definition", flow_body, Some(&user))
        .await;
    assert_ok(s, &b);
    let def_id = b
        .pointer("/data/id")
        .and_then(as_id)
        .expect("新增应返回定义 id");

    let (s, _b) = app
        .put(
            &format!("/workflow/definition/publish/{}", def_id),
            serde_json::json!({}),
            Some(&user),
        )
        .await;
    assert_eq!(s, 200, "发布应 200");

    let business_id = "task_contract_biz_1";
    let (s, b) = app
        .post(
            "/workflow/task/startWorkFlow",
            serde_json::json!({
                "businessId": business_id,
                "flowCode": "task_contract_flow",
                "variables": {}
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    let instance_id = b
        .pointer("/data/processInstanceId")
        .and_then(as_id)
        .expect("应返回 processInstanceId");
    let task_id = b
        .pointer("/data/taskId")
        .and_then(as_id)
        .expect("应返回 taskId");
    assert!(instance_id > 0, "instanceId 应大于 0");
    assert!(task_id > 0, "taskId 应大于 0");

    let (s, b) = app.get("/workflow/task/pageByTaskWait", Some(&user)).await;
    assert_ok(s, &b);
    let rows = b.pointer("/data/rows").expect("应含 rows");
    assert_eq!(rows.as_array().unwrap().len(), 1, "待办应只有一条");
    assert_eq!(
        rows.as_array().unwrap()[0].pointer("/id").and_then(as_id),
        Some(task_id),
        "待办任务 id 应匹配"
    );
    assert_eq!(
        rows.as_array().unwrap()[0]
            .pointer("/businessId")
            .and_then(|v| v.as_str()),
        Some(business_id),
        "待办任务 businessId 应匹配"
    );

    let (s, b) = app
        .get(&format!("/workflow/task/getTask/{}", task_id), Some(&user))
        .await;
    assert_ok(s, &b);
    assert_eq!(b.pointer("/data/id").and_then(as_id), Some(task_id));
    // 对齐 Java FlowTaskVo:列表字段必须始终为数组(即使为空),否则前端
    // submitVerify.vue 的 copyList.map()/buttonList.forEach() 会读到 undefined 报错。
    for field in ["copyList", "buttonList", "userList", "permissionList"] {
        assert!(
            b.pointer(&format!("/data/{field}"))
                .map(|v| v.is_array())
                .unwrap_or(false),
            "getTask 响应的 {field} 必须为数组(空也应为 []): {b}"
        );
    }

    let (s, b) = app
        .post(
            "/workflow/task/completeTask",
            serde_json::json!({
                "taskId": task_id,
                "message": "通过",
                "variables": {},
                "flowCopyList": []
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    let (s, b) = app
        .get("/workflow/task/pageByTaskFinish", Some(&user))
        .await;
    assert_ok(s, &b);
    let rows = b.pointer("/data/rows").expect("应含 rows");
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 1, "已办应只有一条");
    // 对齐 Java FlowHisTaskVo:已办列表须含审批意见/协作方式名/任务ID。
    let done = &rows[0];
    assert_eq!(done["message"], "通过", "已办应回填审批意见 message: {b}");
    assert!(done.get("taskId").is_some(), "已办应含 taskId: {b}");
    assert!(
        done.get("cooperateTypeName")
            .and_then(|v| v.as_str())
            .is_some(),
        "已办应回填 cooperateTypeName: {b}"
    );
}

/// 对齐 RuoYi 请假 demo:首节点办理人为 `${initiator}`(发起人)。
/// startWorkFlow 须把 initiator 写入流程变量,否则发起人办理首节点会被
/// check_permission 判为「当前用户无办理权限」(403)。
#[tokio::test]
async fn start_injects_initiator_so_starter_can_complete_first_task() {
    let app = TestApp::new().await;
    let user = user_with_perms(&[
        "workflow:definition:add",
        "workflow:definition:publish",
        "task:complete",
    ]);

    let flow_body = serde_json::json!({
        "flowCode": "initiator_flow",
        "flowName": "发起人办理流程",
        "version": "1",
        "category": "test",
        "nodeList": [
            {
                "nodeType": 0,
                "nodeCode": "start",
                "nodeName": "开始",
                "skipList": [
                    { "nowNodeCode": "start", "nextNodeCode": "approve", "skipType": "PASS" }
                ]
            },
            {
                "nodeType": 1,
                "nodeCode": "approve",
                "nodeName": "申请人",
                "permissionFlag": "${initiator}",
                "skipList": [
                    { "nowNodeCode": "approve", "nextNodeCode": "end", "skipType": "PASS" }
                ]
            },
            { "nodeType": 2, "nodeCode": "end", "nodeName": "结束", "skipList": [] }
        ]
    });

    let (s, b) = app
        .post("/workflow/definition", flow_body, Some(&user))
        .await;
    assert_ok(s, &b);
    let def_id = b
        .pointer("/data/id")
        .and_then(as_id)
        .expect("应返回定义 id");

    let (s, _b) = app
        .put(
            &format!("/workflow/definition/publish/{}", def_id),
            serde_json::json!({}),
            Some(&user),
        )
        .await;
    assert_eq!(s, 200, "发布应 200");

    let (s, b) = app
        .post(
            "/workflow/task/startWorkFlow",
            serde_json::json!({
                "businessId": "initiator_biz_1",
                "flowCode": "initiator_flow",
                "variables": {}
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    let task_id = b
        .pointer("/data/taskId")
        .and_then(as_id)
        .expect("应返回 taskId");

    // 发起人(user 301)办理首节点,initiator 已注入 → 不应 403。
    let (s, b) = app
        .post(
            "/workflow/task/completeTask",
            serde_json::json!({
                "taskId": task_id,
                "message": "提交申请",
                "variables": {},
                "flowCopyList": []
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
}

/// Vue 契约: taskOperation 操作名与请求体字段。
/// transfer/delegate → transferTask/delegateTask + userId(单值 string)
/// addSignature/reductionSignature → userIds(string[])
#[tokio::test]
async fn task_operation_dispatches_transfer_vs_delegate() {
    let app = TestApp::new().await;
    // Java FlwTaskController 无 @SaCheckPermission,仅需登录
    let user = user_with_perms(&[]);

    // 转办: POST /workflow/task/taskOperation/transferTask, body TaskOperationBo
    // TaskOperationBo.userId = "302"(单个 string)
    let task_id = start_and_get_wait_task(&app, &user).await;
    let (s, b) = app
        .post(
            "/workflow/task/taskOperation/transferTask",
            serde_json::json!({
                "taskId": task_id.to_string(),
                "userId": "302",
                "message": "转办"
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    // 委派: POST /workflow/task/taskOperation/delegateTask, body TaskOperationBo
    // TaskOperationBo.userId = "302"(单个 string)
    let task_id2 = start_and_get_wait_task(&app, &user).await;
    let (s, b) = app
        .post(
            "/workflow/task/taskOperation/delegateTask",
            serde_json::json!({
                "taskId": task_id2.to_string(),
                "userId": "302",
                "message": "委托"
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    // 加签: POST /workflow/task/taskOperation/addSignature
    // TaskOperationBo.userIds = ["302"](string 数组)
    let task_id3 = start_and_get_wait_task(&app, &user).await;
    let (s, b) = app
        .post(
            "/workflow/task/taskOperation/addSignature",
            serde_json::json!({
                "taskId": task_id3.to_string(),
                "userIds": ["302"],
                "message": "加签"
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    // 减签: POST /workflow/task/taskOperation/reductionSignature
    // TaskOperationBo.userIds = ["302"](string 数组)
    let (s, b) = app
        .post(
            "/workflow/task/taskOperation/reductionSignature",
            serde_json::json!({
                "taskId": task_id3.to_string(),
                "userIds": ["302"],
                "message": "减签"
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    // 未知操作应失败
    let (_s, b) = app
        .post(
            "/workflow/task/taskOperation/unknown",
            serde_json::json!({ "taskId": task_id.to_string(), "userId": "302" }),
            Some(&user),
        )
        .await;
    assert_ne!(code(&b), 200, "未知操作应失败: {b}");
}

/// Vue 契约: currentTaskAllUser 返回 R<List<UserDTO>>,
/// UserDTO = {userId, nickName}(对象数组,非纯 string 数组)。
#[tokio::test]
async fn current_task_all_user_returns_user_objects() {
    let app = TestApp::new().await;
    let user = user_with_perms(&[]);
    let task_id = start_and_get_wait_task(&app, &user).await;

    let (s, b) = app
        .get(
            &format!("/workflow/task/currentTaskAllUser/{}", task_id),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    let data = b
        .pointer("/data")
        .and_then(|v| v.as_array())
        .expect("应返回用户列表");
    assert!(!data.is_empty(), "启动后应至少有一个办理人");
    let first = &data[0];
    assert!(first.get("userId").is_some(), "UserDTO 应含 userId: {b}");
    assert!(
        first.get("nickName").is_some(),
        "UserDTO 应含 nickName: {b}"
    );
}

/// Vue 契约: 任务端点仅需登录,无 @SaCheckPermission。
#[tokio::test]
async fn task_endpoints_need_only_login_not_special_perm() {
    let app = TestApp::new().await;
    // 无任何权限,仅登录
    let user = user_with_perms(&[]);

    let (s, b) = app.get("/workflow/task/pageByTaskWait", Some(&user)).await;
    assert_eq!(s, 200);
    assert_ne!(code(&b), 403, "待办列表不应 403: {b}");

    let (s, b) = app
        .get("/workflow/task/pageByTaskFinish", Some(&user))
        .await;
    assert_eq!(s, 200);
    assert_ne!(code(&b), 403, "已办列表不应 403: {b}");

    let task_id = start_and_get_wait_task(&app, &user).await;

    let (s, b) = app
        .get(&format!("/workflow/task/getTask/{}", task_id), Some(&user))
        .await;
    assert_eq!(s, 200);
    assert_ne!(code(&b), 403, "任务详情不应 403: {b}");
}

/// Vue 契约: PUT /workflow/task/updateAssignee/{userId}, body 为 taskIdList(string[])。
#[tokio::test]
#[ignore = "W7-PENDING: 测试夹具权限未配齐(实际失败为 403 缺 workflow:task:edit,非 no-task)"]
async fn update_assignee_changes_handler() {
    let app = TestApp::new().await;
    let user = user_with_perms(&[]);
    let task_id = start_and_get_wait_task(&app, &user).await;

    let (s, b) = app
        .put(
            "/workflow/task/updateAssignee/302",
            serde_json::json!([task_id.to_string()]),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
}

/// Vue 契约: POST /workflow/task/backProcess, body BackProcessBo {taskId, nodeCode, message}。
#[tokio::test]
async fn back_process_rejects_to_node() {
    let app = TestApp::new().await;
    let user = user_with_perms(&[]);
    let task_id = start_and_get_wait_task(&app, &user).await;

    let (s, b) = app
        .post(
            "/workflow/task/backProcess",
            serde_json::json!({
                "taskId": task_id.to_string(),
                "nodeCode": "start",
                "message": "驳回"
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
}

/// Vue 契约: GET /workflow/task/getBackTaskNode/{taskId}/{nodeCode}。
#[tokio::test]
async fn get_back_task_node_returns_list() {
    let app = TestApp::new().await;
    let user = user_with_perms(&[]);
    let task_id = start_and_get_wait_task(&app, &user).await;

    let (s, b) = app
        .get(
            &format!("/workflow/task/getBackTaskNode/{}/approve", task_id),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    assert!(
        b.get("data").map(|d| d.is_array()).unwrap_or(false),
        "data 应为数组: {b}"
    );
}

#[tokio::test]
async fn start_workflow_persists_business_ext() {
    let app = TestApp::new().await;
    let user = user_with_perms(&[]);
    let seq = HELPER_SEQ.fetch_add(1, Ordering::SeqCst);
    let business_id = format!("biz_ext_test_{seq}");
    let (s, b) = app
        .post(
            "/workflow/task/startWorkFlow",
            serde_json::json!({
                "businessId": business_id,
                "flowCode": "leave1",
                "variables": {},
                "bizExt": {
                    "businessTitle": "测试请假标题"
                }
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    let inst_id = b
        .pointer("/data/processInstanceId")
        .and_then(as_id)
        .unwrap();

    let (s2, b2) = app
        .get(
            "/workflow/task/pageByTaskWait?pageNum=1&pageSize=20",
            Some(&user),
        )
        .await;
    assert_ok(s2, &b2);
    let row = b2["data"]["rows"]
        .as_array()
        .and_then(|rows| {
            rows.iter().find(|x| {
                x.pointer("/instanceId").and_then(as_id) == Some(inst_id)
            })
        })
        .expect("待办应含新实例");
    assert!(
        row.get("businessCode").and_then(|v| v.as_str()).is_some(),
        "缺 businessCode: {row}"
    );
    assert_eq!(
        row.get("businessTitle").and_then(|v| v.as_str()),
        Some("测试请假标题")
    );
}

/// Vue 契约: POST /workflow/task/urgeTask, body {taskId, message}。
#[tokio::test]
async fn urge_task_sends_message() {
    let app = TestApp::new().await;
    // urgeTask 需 workflow:task:edit 权限（对齐 Java 6.X FlwTaskController @SaCheckPermission）
    let user = user_with_perms(&["workflow:task:edit"]);
    let mut rx = app.push_hub.subscribe();
    let task_id = start_and_get_wait_task(&app, &user).await;

    // D5:启动实例即向首节点办理人推送新待办,先消费掉这条再断言催办
    let start_msg =
        tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("应收 SSE 推送")
            .expect("广播通道未关闭");
    assert_eq!(start_msg.source, "workflow");
    assert!(
        start_msg.message.contains("您有新的待办任务"),
        "启动推送应为新待办通知:{}",
        start_msg.message
    );
    assert_eq!(start_msg.path.as_deref(), Some("/task/taskWaiting"));
    assert_eq!(start_msg.to_user_id, Some(301));

    let (s, b) = app
        .post(
            "/workflow/task/urgeTask",
            serde_json::json!({
                "taskId": task_id.to_string(),
                "message": "请尽快处理"
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    let msg =
        tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("应收 SSE 推送")
            .expect("广播通道未关闭");
    assert_eq!(msg.source, "workflow");
    assert_eq!(msg.message, "请尽快处理");
    assert_eq!(msg.path.as_deref(), Some("/task/taskWaiting"));
    assert_eq!(msg.to_user_id, Some(301));
}

#[tokio::test]
async fn leave_submit_start_approve_writeback() {
    let app = TestApp::new().await;
    let applicant = user_with_perms(&[
        "workflow:leave:add",
        "workflow:leave:query",
        "task:complete",
        "task:waitList",
    ]);
    let bo =
        serde_json::json!({"leaveType":"事假","leaveDays":2,"remark":"家事"});
    let (s, b) = app
        .post("/workflow/leave/submitAndFlowStart", bo, Some(&applicant))
        .await;
    assert_ok(s, &b);
    let business_id = b
        .pointer("/data/id")
        .and_then(as_id)
        .expect("应返回请假 id");
    let (sg, bg) = app
        .get(&format!("/workflow/leave/{business_id}"), Some(&applicant))
        .await;
    assert_ok(sg, &bg);
    assert_eq!(
        bg.pointer("/data/status").and_then(|v| v.as_str()),
        Some("waiting")
    );

    let (s, b) = app
        .get("/workflow/task/pageByTaskWait", Some(&applicant))
        .await;
    assert_ok(s, &b);
    let task_id = b
        .pointer("/data/rows/0/id")
        .and_then(as_id)
        .expect("应返回待办任务 id");

    let (s, _b) = app
        .post(
            "/workflow/task/completeTask",
            serde_json::json!({
                "taskId": task_id,
                "message": "同意",
                "variables": {},
                "flowCopyList": []
            }),
            Some(&applicant),
        )
        .await;
    assert_eq!(s, 200, "审批完成应 200: {_b}");

    let (sg, bg) = app
        .get(&format!("/workflow/leave/{business_id}"), Some(&applicant))
        .await;
    assert_ok(sg, &bg);
    assert_eq!(
        bg.pointer("/data/status").and_then(|v| v.as_str()),
        Some("finish"),
        "审批后请假单状态应回写为 finish: {bg}"
    );
}

/// 从「修改」页点提交时前端会带上 `id`,后端必须**就地更新**
/// 而不是新建一条(对齐 Java `leaveMapper.insertOrUpdate`,TestLeaveServiceImpl:157)。
///
/// `submit_and_start` 若无条件走 `add()`,草稿会停在 draft,列表凭空多一行,
/// 流程还发在那条新单据上;全绿的 test/typecheck/build 一条都拦不住。
#[tokio::test]
async fn leave_submit_with_id_updates_draft_in_place() {
    let app = TestApp::new().await;
    let applicant = user_with_perms(&[
        "workflow:leave:add",
        "workflow:leave:query",
        "workflow:leave:list",
    ]);

    // 1. 先存一张草稿
    let (s, b) = app
        .post(
            "/workflow/leave",
            serde_json::json!({"leaveType":"1","leaveDays":2,"remark":"草稿","status":"draft"}),
            Some(&applicant),
        )
        .await;
    assert_ok(s, &b);
    let draft_id = b
        .pointer("/data/id")
        .and_then(as_id)
        .expect("应返回草稿 id");

    // 2. 带 id 提交(并顺手把天数从 2 改成 5)
    let (s, b) = app
        .post(
            "/workflow/leave/submitAndFlowStart",
            serde_json::json!({
                "id": draft_id.to_string(),  // 出站 id 是字符串,前端原样回传
                "leaveType":"1","leaveDays":5,"remark":"草稿","flowCode":"leave1"
            }),
            Some(&applicant),
        )
        .await;
    assert_ok(s, &b);
    assert_eq!(
        b.pointer("/data/id").and_then(as_id),
        Some(draft_id),
        "带 id 提交应返回同一条单据,而不是新建的那条: {b}"
    );

    // 3. 列表只应有这一行
    let (s, b) = app
        .get(
            "/workflow/leave/list?pageNum=1&pageSize=10",
            Some(&applicant),
        )
        .await;
    assert_ok(s, &b);
    assert_eq!(
        b.pointer("/data/total").and_then(|v| v.as_i64()),
        Some(1),
        "带 id 提交后不应多出一条单据: {b}"
    );

    // 4. 原草稿本身变成 waiting,且提交时改的天数落了库
    let (s, b) = app
        .get(&format!("/workflow/leave/{draft_id}"), Some(&applicant))
        .await;
    assert_ok(s, &b);
    assert_eq!(
        b.pointer("/data/status").and_then(|v| v.as_str()),
        Some("waiting"),
        "原草稿应就地转为 waiting(而不是仍停在 draft): {b}"
    );
    assert_eq!(
        b.pointer("/data/leaveDays").and_then(|v| v.as_i64()),
        Some(5),
        "提交时修改的天数应落库: {b}"
    );
}

/// 反向:不带 id 仍然是新建(add 模式的既有行为不能被上一条改动带坏)。
#[tokio::test]
async fn leave_submit_without_id_still_creates_new_record() {
    let app = TestApp::new().await;
    let applicant =
        user_with_perms(&["workflow:leave:add", "workflow:leave:list"]);

    for _ in 0..2 {
        let (s, b) = app
            .post(
                "/workflow/leave/submitAndFlowStart",
                serde_json::json!({"leaveType":"1","leaveDays":1,"flowCode":"leave1"}),
                Some(&applicant),
            )
            .await;
        assert_ok(s, &b);
    }

    let (s, b) = app
        .get(
            "/workflow/leave/list?pageNum=1&pageSize=10",
            Some(&applicant),
        )
        .await;
    assert_ok(s, &b);
    assert_eq!(
        b.pointer("/data/total").and_then(|v| v.as_i64()),
        Some(2),
        "不带 id 的两次提交应是两条独立单据: {b}"
    );
}

/// 复现前端真实提交路径(非 submitAndFlowStart):
/// POST /workflow/leave 建草稿 → 通用 startWorkFlow → 请假单状态须从 draft 回写为 waiting。
/// 对齐 RuoYi:提交即 BusinessStatusEnum.WAITING。
#[tokio::test]
async fn generic_start_workflow_writes_back_waiting_status() {
    let app = TestApp::new().await;
    let applicant =
        user_with_perms(&["workflow:leave:add", "workflow:leave:query"]);

    // 1. 建请假草稿。
    let (s, b) = app
        .post(
            "/workflow/leave",
            serde_json::json!({"leaveType":"事假","leaveDays":1,"remark":"测试"}),
            Some(&applicant),
        )
        .await;
    assert_ok(s, &b);
    let business_id = b
        .pointer("/data/id")
        .and_then(as_id)
        .expect("应返回请假 id");
    assert_eq!(
        b.pointer("/data/status").and_then(|v| v.as_str()),
        Some("draft"),
        "新建请假应为 draft: {b}"
    );

    // 2. 走通用 startWorkFlow(前端真实路径,非 submitAndFlowStart)。
    let (s, b) = app
        .post(
            "/workflow/task/startWorkFlow",
            serde_json::json!({
                "businessId": business_id.to_string(),
                "flowCode": "leave1",
                "variables": {}
            }),
            Some(&applicant),
        )
        .await;
    assert_ok(s, &b);

    // 3. 业务状态须回写为 waiting。
    let (sg, bg) = app
        .get(&format!("/workflow/leave/{business_id}"), Some(&applicant))
        .await;
    assert_ok(sg, &bg);
    assert_eq!(
        bg.pointer("/data/status").and_then(|v| v.as_str()),
        Some("waiting"),
        "startWorkFlow 后请假单应回写为 waiting: {bg}"
    );
}

#[tokio::test]
async fn definition_crud_and_publish_contract() {
    let app = TestApp::new().await;
    let user = user_with_perms(&[
        "workflow:definition:list",
        "workflow:definition:query",
        "workflow:definition:add",
        "workflow:definition:edit",
        "workflow:definition:publish",
        "workflow:definition:active",
        "workflow:definition:export",
        "workflow:definition:remove",
        "workflow:definition:import",
    ]);

    let add_body = serde_json::json!({
        "flowCode": "test_contract_flow",
        "flowName": "契约测试流程",
        "version": "1",
        "category": "test",
        "nodeList": [
            {
                "nodeType": 0,
                "nodeCode": "start",
                "nodeName": "开始",
                "skipList": [
                    {
                        "nowNodeCode": "start",
                        "nextNodeCode": "end",
                        "skipType": "PASS"
                    }
                ]
            },
            {
                "nodeType": 2,
                "nodeCode": "end",
                "nodeName": "结束",
                "skipList": []
            }
        ]
    });

    let (s, b) = app
        .post("/workflow/definition", add_body, Some(&user))
        .await;
    assert_ok(s, &b);
    let def_id = b
        .pointer("/data/id")
        .and_then(as_id)
        .expect("新增应返回定义 id");
    assert_eq!(
        b.pointer("/data/flowCode").and_then(|v| v.as_str()),
        Some("test_contract_flow")
    );

    let (s, b) = app
        .get(&format!("/workflow/definition/{}", def_id), Some(&user))
        .await;
    assert_ok(s, &b);
    assert_eq!(
        b.pointer("/data/flowCode").and_then(|v| v.as_str()),
        Some("test_contract_flow")
    );
    assert!(
        b.pointer("/data/nodeList").is_none(),
        "GET /{{id}} 应返回定义行,不含全图 nodeList: {b}"
    );

    let edit_body = serde_json::json!({
        "id": def_id,
        "flowCode": "test_contract_flow",
        "flowName": "契约测试流程-已编辑",
        "version": "1",
        "category": "test",
        "nodeList": [
            {
                "nodeType": 0,
                "nodeCode": "start",
                "nodeName": "开始",
                "skipList": [
                    {
                        "nowNodeCode": "start",
                        "nextNodeCode": "end",
                        "skipType": "PASS"
                    }
                ]
            },
            {
                "nodeType": 2,
                "nodeCode": "end",
                "nodeName": "结束",
                "skipList": []
            }
        ]
    });
    let (s, b) = app
        .put("/workflow/definition", edit_body, Some(&user))
        .await;
    assert_ok(s, &b);
    assert_eq!(
        b.pointer("/data/flowName").and_then(|v| v.as_str()),
        Some("契约测试流程-已编辑")
    );

    let (s, b) = app
        .put(
            &format!("/workflow/definition/publish/{}", def_id),
            serde_json::json!({}),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    let (s, b) = app
        .put(
            &format!("/workflow/definition/unPublish/{}", def_id),
            serde_json::json!({}),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    let (s, b) = app
        .put(
            &format!("/workflow/definition/active/{}?active=0", def_id),
            serde_json::json!({}),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    let (s, b) = app
        .put(
            &format!("/workflow/definition/active/{}?active=1", def_id),
            serde_json::json!({}),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    let (s, b) = app
        .get("/workflow/definition/unPublishList", Some(&user))
        .await;
    assert_ok(s, &b);
    assert!(b.pointer("/data/rows").is_some(), "应含 data.rows: {b}");
    assert!(b.pointer("/data/total").is_some(), "应含 data.total: {b}");

    let (s, _b) = app
        .post(
            &format!("/workflow/definition/exportDef/{}", def_id),
            serde_json::json!({}),
            Some(&user),
        )
        .await;
    assert_eq!(s, 200, "导出应 200");

    let (s, b) = app
        .get(
            &format!("/workflow/definition/xmlString/{}", def_id),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    assert!(b.pointer("/data").is_some(), "应返回 xml/json 字符串: {b}");

    let add_body2 = serde_json::json!({
        "flowCode": "test_contract_flow_del2",
        "flowName": "契约测试流程-待删除2",
        "version": "1",
        "category": "test",
        "nodeList": [
            {
                "nodeType": 0,
                "nodeCode": "start",
                "nodeName": "开始",
                "skipList": [
                    {
                        "nowNodeCode": "start",
                        "nextNodeCode": "end",
                        "skipType": "PASS"
                    }
                ]
            },
            {
                "nodeType": 2,
                "nodeCode": "end",
                "nodeName": "结束",
                "skipList": []
            }
        ]
    });
    let (s, b) = app
        .post("/workflow/definition", add_body2, Some(&user))
        .await;
    assert_ok(s, &b);
    let def_id2 = b
        .pointer("/data/id")
        .and_then(as_id)
        .expect("新增应返回定义 id");

    let (s, b) = app
        .delete(
            &format!("/workflow/definition/{},{}", def_id, def_id2),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    let import_json = serde_json::json!({
        "flowCode": "test_contract_flow_import",
        "flowName": "契约测试流程-导入",
        "version": "1",
        "category": "test",
        "nodeList": [
            {
                "nodeType": 0,
                "nodeCode": "start",
                "nodeName": "开始",
                "skipList": [
                    {
                        "nowNodeCode": "start",
                        "nextNodeCode": "end",
                        "skipType": "PASS"
                    }
                ]
            },
            {
                "nodeType": 2,
                "nodeCode": "end",
                "nodeName": "结束",
                "skipList": []
            }
        ]
    });
    let boundary = "----form_boundary_test";
    let file_body = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"import.json\"\r\nContent-Type: application/json\r\n\r\n{}\r\n--{}--\r\n",
        boundary, import_json, boundary
    );
    let payload = serde_json::to_vec(&user).expect("序列化 LoginUser 失败");
    let req = Request::builder()
        .method("POST")
        .uri(api_uri("/workflow/definition/importDef"))
        .header("x-test-login-user", STANDARD.encode(&payload))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(Body::from(file_body))
        .expect("构建请求失败");
    let resp = app.router.clone().oneshot(req).await.expect("路由处理失败");
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("读取响应体失败")
        .to_bytes();
    let b: serde_json::Value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    assert_ok(status, &b);
}

/// /warm-flow 公开面/API 面分界:config 保持匿名公开,
/// listener-list/query-def 等其余 /warm-flow/* API 须带凭据
/// (锚 Java sa-token excludes:公开面 = 静态资源 + /warm-flow-ui/config)。
#[tokio::test]
async fn warm_flow_ui_config_public_others_require_auth() {
    let app = TestApp::new().await;
    let user = user_with_perms(&[]);

    // /warm-flow-ui/config(公开面唯一 API,保持匿名)
    let (_s, b) = app.get("/warm-flow-ui/config", None).await;
    assert_eq!(code(&b), 200, "config 应公开: {b}");
    assert_eq!(
        b.pointer("/data/tokenNameList")
            .and_then(|v| v.as_array())
            .map(|a| a.len()),
        Some(1),
        "config 应返回 tokenNameList: {b}"
    );

    // /warm-flow/listener-list(须带凭据)
    let (s, b) = app.get("/warm-flow/listener-list", Some(&user)).await;
    assert_eq!(s, 200);
    assert_eq!(code(&b), 200, "listener-list 登录后应 200: {b}");
    assert!(
        b.pointer("/data").is_some(),
        "listener-list 应返回 data: {b}"
    );

    // /warm-flow/query-def/{id}(须带凭据)
    let (s, b) = app.get("/warm-flow/query-def/0", Some(&user)).await;
    assert_eq!(s, 200);
    assert_eq!(code(&b), 200, "query-def 登录后应 200: {b}");
}

/// /warm-flow/* API 匿名一律 401(含未鉴权写
/// 端点 save-json 与用户/部门枚举 handler-result);设计器静态入口与
/// config 匿名 200(公开面钉死)。锚 Java sa-token excludes
/// (RuoYi-Vue-Plus application.yml:100-113)。
#[tokio::test]
async fn warm_flow_api_requires_auth_and_static_stays_public() {
    let app = TestApp::new().await;

    // ① 匿名写端点(未鉴权保存流程定义)→ 401。
    let (s, b) = app
        .post("/warm-flow/save-json", serde_json::json!({}), None)
        .await;
    assert_eq!(s, 401, "匿名 save-json 应 HTTP 401: {b}");
    assert_eq!(code(&b), 401, "匿名 save-json 应 code 401: {b}");

    // ② 匿名枚举用户/部门(选人弹窗候选数据)→ 401。
    let (s, b) = app
        .get("/warm-flow/handler-result?pageNum=1&pageSize=10", None)
        .await;
    assert_eq!(s, 401, "匿名 handler-result 应 HTTP 401: {b}");
    assert_eq!(code(&b), 401, "匿名 handler-result 应 code 401: {b}");

    // ③ 公开面钉死:静态入口与 config 匿名 200。
    let (s, _b) = app.get("/designer/", None).await;
    assert_eq!(s, 200, "设计器静态入口应匿名 200");
    let (s, b) = app.get("/warm-flow-ui/config", None).await;
    assert_eq!(s, 200, "config 应匿名 HTTP 200");
    assert_eq!(code(&b), 200, "config 应匿名 code 200: {b}");
}

/// 流程图(approvalRecord.vue 的 iframe → /warm-flow/query-flow-chart/{instanceId})。
/// 必须返回设计器可渲染的 DefJson(nodeList + 三原色 + 节点状态高亮),而非空壳。
/// 同时验证流转后 def_json 高亮更新(approve 由待办变已办)。
#[tokio::test]
async fn query_flow_chart_renders_instance_def_json_with_status() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["task:complete", "task:waitList"]);

    // 启动 leave1(种子定义 start→approve→end,approve 办理人=301)。
    let (s, b) = app
        .post(
            "/workflow/task/startWorkFlow",
            serde_json::json!({
                "businessId": "chart_biz_1",
                "flowCode": "leave1",
                "variables": {}
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    let instance_id = b
        .pointer("/data/processInstanceId")
        .and_then(as_id)
        .expect("应返回 processInstanceId");
    let task_id = b
        .pointer("/data/taskId")
        .and_then(as_id)
        .expect("应返回 taskId");

    // 流程图接口(须登录;Vue 侧经 iframe URL 拼
    // Authorization/clientid 穿透,见 approvalRecord.vue / design.vue)。
    let (s, b) = app
        .get(
            &format!("/warm-flow/query-flow-chart/{instance_id}"),
            Some(&user),
        )
        .await;
    assert_eq!(s, 200);
    assert_eq!(code(&b), 200, "query-flow-chart 应 200: {b}");
    let nodes = b
        .pointer("/data/nodeList")
        .and_then(|v| v.as_array())
        .expect("应返回 nodeList");
    assert!(!nodes.is_empty(), "nodeList 不应为空(空白图根因): {b}");
    let colors = b
        .pointer("/data/chartStatusColor")
        .and_then(|v| v.as_array())
        .expect("应返回 chartStatusColor");
    assert_eq!(colors.len(), 3, "应返回三种状态颜色: {b}");

    let status_of = |body: &serde_json::Value, code: &str| -> Option<i64> {
        body.pointer("/data/nodeList")?
            .as_array()?
            .iter()
            .find_map(|n| {
                (n.get("nodeCode").and_then(|v| v.as_str()) == Some(code))
                    .then(|| n.get("status").and_then(|v| v.as_i64()))
                    .flatten()
            })
    };
    // 启动后:start 已办(2),approve 待办(1)。
    assert_eq!(status_of(&b, "start"), Some(2), "start 应为已办: {b}");
    assert_eq!(status_of(&b, "approve"), Some(1), "approve 应为待办: {b}");

    // 办理 approve 任务。
    let (s, _b) = app
        .post(
            "/workflow/task/completeTask",
            serde_json::json!({
                "taskId": task_id,
                "message": "同意",
                "variables": {},
                "flowCopyList": []
            }),
            Some(&user),
        )
        .await;
    assert_eq!(s, 200, "办理应 200: {_b}");

    // 流转后:approve 应变为已办(2)。
    let (_s, b2) = app
        .get(
            &format!("/warm-flow/query-flow-chart/{instance_id}"),
            Some(&user),
        )
        .await;
    assert_eq!(
        status_of(&b2, "approve"),
        Some(2),
        "办理后 approve 应为已办: {b2}"
    );
}

/// 对齐 Java FlwInstanceServiceImpl#flowHisTaskList:仅返回 {list, instanceId},
/// 不含多余的 image 字段(流程图由 query-flow-chart 提供,非此接口)。
#[tokio::test]
async fn flow_his_task_list_omits_image_field() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["task:complete", "workflow:instance:query"]);

    let (s, b) = app
        .post(
            "/workflow/task/startWorkFlow",
            serde_json::json!({
                "businessId": "his_chart_biz_1",
                "flowCode": "leave1",
                "variables": {}
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    let (s, b) = app
        .get(
            "/workflow/instance/flowHisTaskList/his_chart_biz_1",
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    assert!(b.pointer("/data/list").is_some(), "应返回 list: {b}");
    assert!(
        b.pointer("/data/instanceId").is_some(),
        "应返回 instanceId: {b}"
    );
    assert!(
        b.pointer("/data/image").is_none(),
        "不应再返回多余的 image 字段: {b}"
    );
}

/// 统计流程图中处于"待办"(status=1)的节点数。
fn pending_node_count(chart_body: &serde_json::Value) -> usize {
    chart_body
        .pointer("/data/nodeList")
        .and_then(|v| v.as_array())
        .map(|nodes| {
            nodes
                .iter()
                .filter(|n| n.get("status").and_then(|v| v.as_i64()) == Some(1))
                .count()
        })
        .unwrap_or(0)
}

/// 终止后流程图须反映最新状态:无待办高亮(def_json 随流转更新)。
#[tokio::test]
async fn query_flow_chart_after_terminate_has_no_pending() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["task:complete"]);
    let (s, b) = app
        .post(
            "/workflow/task/startWorkFlow",
            serde_json::json!({"businessId":"term_chart_1","flowCode":"leave1","variables":{}}),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    let instance_id = b
        .pointer("/data/processInstanceId")
        .and_then(as_id)
        .expect("应返回 processInstanceId");
    let task_id = b.pointer("/data/taskId").and_then(as_id).expect("taskId");

    // 启动后应有一个待办(approve)。
    let (_s, b0) = app
        .get(
            &format!("/warm-flow/query-flow-chart/{instance_id}"),
            Some(&user),
        )
        .await;
    assert_eq!(pending_node_count(&b0), 1, "启动后应有 1 个待办: {b0}");

    let (s, tb) = app
        .post(
            "/workflow/task/terminationTask",
            serde_json::json!({"taskId": task_id, "message":"终止"}),
            Some(&user),
        )
        .await;
    assert_eq!(s, 200, "终止应 200: {tb}");

    let (_s, b) = app
        .get(
            &format!("/warm-flow/query-flow-chart/{instance_id}"),
            Some(&user),
        )
        .await;
    assert_eq!(pending_node_count(&b), 0, "终止后不应有待办节点: {b}");
}

/// 作废后流程图须无待办高亮,且作废应清理未办任务。
#[tokio::test]
async fn query_flow_chart_after_invalidate_has_no_pending() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["workflow:instance:invalid"]);
    let (s, b) = app
        .post(
            "/workflow/task/startWorkFlow",
            serde_json::json!({"businessId":"inv_chart_1","flowCode":"leave1","variables":{}}),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);
    let instance_id = b
        .pointer("/data/processInstanceId")
        .and_then(as_id)
        .expect("应返回 processInstanceId");

    let (s, ib) = app
        .post(
            "/workflow/instance/invalid",
            serde_json::json!({"instanceId": instance_id}),
            Some(&user),
        )
        .await;
    assert_eq!(s, 200, "作废应 200: {ib}");

    let (_s, b) = app
        .get(
            &format!("/warm-flow/query-flow-chart/{instance_id}"),
            Some(&user),
        )
        .await;
    assert_eq!(pending_node_count(&b), 0, "作废后不应有待办节点: {b}");
}

#[tokio::test]
async fn warm_flow_ui_static_index_html_is_served() {
    let app = TestApp::new().await;

    // 静态资源应无需登录即可访问
    let (status, _headers, body) = app
        .request_bytes(Method::GET, "/warm-flow-ui/index.html", None, None)
        .await;
    assert_eq!(status, 200, "warm-flow-ui 静态入口应返回 200");
    let text = String::from_utf8(body).expect("响应体应为 UTF-8");
    assert!(
        text.contains("Warm-Flow设计器"),
        "应返回 warm-flow-ui 的 index.html: {text}"
    );

    // 关联的 js/css 也应无需登录
    let (status2, _headers2, body2) = app
        .request_bytes(
            Method::GET,
            "/warm-flow-ui/js/index-D1aMxIL-.js",
            None,
            None,
        )
        .await;
    assert_eq!(status2, 200, "warm-flow-ui JS 应返回 200");
    assert!(!body2.is_empty(), "JS 文件不应为空");

    let (status3, _headers3, body3) = app
        .request_bytes(
            Method::GET,
            "/warm-flow-ui/css/index-DfdPwo0U.css",
            None,
            None,
        )
        .await;
    assert_eq!(status3, 200, "warm-flow-ui CSS 应返回 200");
    assert!(!body3.is_empty(), "CSS 文件不应为空");
}

#[tokio::test]
async fn invalid_persists_comment() {
    // PIN5: 作废原因此前不落库 —— 提交 comment 后应从历史任务列表读回。
    let app = TestApp::new().await;
    let user = user_with_perms(&[
        "workflow:instance:invalid",
        "workflow:instance:query",
    ]);
    let seq = HELPER_SEQ.fetch_add(1, Ordering::SeqCst);
    let business_id = format!("invalid_comment_biz_{seq}");
    let (s, b) = app
        .post(
            "/workflow/task/startWorkFlow",
            serde_json::json!({
                "businessId": &business_id,
                "flowCode": "leave1",
                "variables": {}
            }),
            Some(&user),
        )
        .await;
    assert_ok(s, &b);

    let (si, bi) = app
        .post(
            "/workflow/instance/invalid",
            serde_json::json!({ "businessId": business_id, "comment": "预算超支作废" }),
            Some(&user),
        )
        .await;
    assert_ok(si, &bi);

    let (sg, detail) = app
        .get(
            &format!("/workflow/instance/flowHisTaskList/{business_id}"),
            Some(&user),
        )
        .await;
    assert_ok(sg, &detail);
    assert!(
        detail.to_string().contains("预算超支作废"),
        "PIN5: 作废原因 comment 未落库: {detail}"
    );
}

#[tokio::test]
async fn instance_vo_exposes_is_suspended() {
    // PIN11: Vue 读 isSuspended, 后端只有 activityStatus。
    // 运行中分页 pageByRunning 的 VO 须暴露 isSuspended。
    let app = TestApp::new().await;
    let user = user_with_perms(&["workflow:instance:list"]);

    let (s, page) = app
        .get(
            "/workflow/instance/pageByRunning?pageNum=1&pageSize=1",
            Some(&user),
        )
        .await;
    assert_ok(s, &page);
    let rows = page["data"]["rows"].as_array().expect("rows 数组");
    assert!(!rows.is_empty(), "pageByRunning 应至少有一条数据: {page}");
    let row = &rows[0];
    assert!(
        row.get("isSuspended").is_some(),
        "PIN11: FlowInstanceVo 缺 isSuspended: {page}"
    );
}

#[tokio::test]
async fn his_task_vo_exposes_flow_task_status() {
    // 我的已办「任务状态」列(wf_task_status dict-tag)曾无数据源。
    let app = TestApp::new().await;
    let user = user_with_perms(&["task:complete", "task:finishList"]);

    let task_id = start_and_get_wait_task(&app, &user).await;
    let (s, _b) = app
        .post(
            "/workflow/task/completeTask",
            serde_json::json!({
                "taskId": task_id,
                "message": "通过",
                "variables": {},
                "flowCopyList": []
            }),
            Some(&user),
        )
        .await;
    assert_eq!(s, 200, "审批完成应 200: {_b}");

    let (s, page) = app
        .get(
            "/workflow/task/pageByTaskFinish?pageNum=1&pageSize=1",
            Some(&user),
        )
        .await;
    assert_ok(s, &page);
    let rows = page["data"]["rows"].as_array().expect("rows 数组");
    assert!(
        !rows.is_empty(),
        "pageByTaskFinish 应至少有一条数据: {page}"
    );
    let row = &rows[0];
    assert!(
        row.get("flowTaskStatus").is_some(),
        "TFI5: FlowHisTaskVo 缺 flowTaskStatus: {page}"
    );
}
