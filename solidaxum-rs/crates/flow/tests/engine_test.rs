use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use flow::{
    dto::flow_params::FlowParams,
    enums::NodeType,
    listener::{Listener, ListenerRegistry},
    model::def_json::{DefJson, NodeJson, SkipJson},
    notifier::NoopFlowNotifier,
    permission::{DefaultPermissionHandler, PermissionHandler},
    repository::{
        entity::{
            flow_definition, flow_his_task, flow_instance, flow_node,
            flow_skip, flow_task, flow_user,
        },
        his_task_repo::HisTaskRepository,
        task_repo::TaskRepository,
        user_repo::UserRepository,
    },
    resolver::{
        BusinessStatusCallback, NoopBusinessCallback, NoopResolver,
        UserResolver,
    },
    service::{
        definition_service::{DefinitionService, OperatorContext},
        instance_service::InstanceService,
        task_service::TaskService,
    },
};

fn default_registry() -> Arc<ListenerRegistry> {
    Arc::new(ListenerRegistry::new())
}

fn default_permission() -> Arc<dyn PermissionHandler> {
    Arc::new(DefaultPermissionHandler)
}

fn default_resolver() -> Arc<dyn UserResolver> {
    Arc::new(NoopResolver)
}

fn default_callback() -> Arc<dyn BusinessStatusCallback> {
    Arc::new(NoopBusinessCallback)
}

fn default_notifier() -> Arc<dyn flow::notifier::FlowNotifier> {
    Arc::new(NoopFlowNotifier)
}

/// 一次通知记录:(用户 id 集, 消息文本)。
type NotifyCall = (Vec<i64>, String);

/// 记录调用的新待办通知桩(RecordingNotifier):每收到一次推送记一条
/// (用户 id 集, 消息文本)。
#[derive(Default, Clone)]
struct RecordingNotifier {
    calls: Arc<Mutex<Vec<NotifyCall>>>,
}

#[async_trait]
impl flow::notifier::FlowNotifier for RecordingNotifier {
    async fn notify_users(
        &self,
        user_ids: &[i64],
        message: &str,
        _path: Option<&str>,
    ) -> app_core::error::AppResult<()> {
        self.calls
            .lock()
            .unwrap()
            .push((user_ids.to_vec(), message.to_string()));
        Ok(())
    }
}

async fn setup_ds() -> app_data::DataSource {
    // 脚手架只留 DataSource(单连接内存库,max=min=1 同库可见 + 实体衍生
    // DDL)。
    let ds = app_data::testing::sqlite_memory_empty().await;
    ds.create_table(flow_definition::Entity).await.unwrap();
    ds.create_table(flow_node::Entity).await.unwrap();
    ds.create_table(flow_skip::Entity).await.unwrap();
    ds.create_table(flow_instance::Entity).await.unwrap();
    ds.create_table(flow_task::Entity).await.unwrap();
    ds.create_table(flow_his_task::Entity).await.unwrap();
    ds.create_table(flow_user::Entity).await.unwrap();
    ds
}

fn test_ctx() -> OperatorContext {
    OperatorContext {
        user_id: "1".to_string(),
        tenant_id: None,
    }
}

fn make_skip(now: &str, next: &str, name: &str) -> SkipJson {
    SkipJson {
        now_node_code: now.to_string(),
        next_node_code: next.to_string(),
        skip_name: Some(name.to_string()),
        skip_type: Some("PASS".to_string()),
        skip_condition: None,
        coordinate: None,
        create_by: None,
        update_by: None,
        status: None,
    }
}

fn make_skip_cond(now: &str, next: &str, name: &str, cond: &str) -> SkipJson {
    SkipJson {
        now_node_code: now.to_string(),
        next_node_code: next.to_string(),
        skip_name: Some(name.to_string()),
        skip_type: Some("PASS".to_string()),
        skip_condition: if cond.is_empty() {
            None
        } else {
            Some(cond.to_string())
        },
        coordinate: None,
        create_by: None,
        update_by: None,
        status: None,
    }
}

fn make_node(
    node_type: NodeType,
    code: &str,
    name: &str,
    perm: Option<&str>,
    skips: Vec<SkipJson>,
) -> NodeJson {
    NodeJson {
        node_type: node_type as i8,
        node_code: code.to_string(),
        node_name: Some(name.to_string()),
        version: None,
        permission_flag: perm.map(|s| s.to_string()),
        node_ratio: None,
        coordinate: None,
        any_node_skip: None,
        listener_type: None,
        listener_path: None,
        form_custom: None,
        form_path: None,
        ext: None,
        create_by: None,
        update_by: None,
        status: None,
        skip_list: skips,
    }
}

fn make_node_ratio(
    node_type: NodeType,
    code: &str,
    name: &str,
    perm: Option<&str>,
    ratio: &str,
    skips: Vec<SkipJson>,
) -> NodeJson {
    let mut n = make_node(node_type, code, name, perm, skips);
    n.node_ratio = Some(ratio.to_string());
    n
}

fn make_reject_skip(now: &str, next: &str, name: &str) -> SkipJson {
    SkipJson {
        now_node_code: now.to_string(),
        next_node_code: next.to_string(),
        skip_name: Some(name.to_string()),
        skip_type: Some("REJECT".to_string()),
        skip_condition: None,
        coordinate: None,
        create_by: None,
        update_by: None,
        status: None,
    }
}

fn make_def_json(code: &str, name: &str, nodes: Vec<NodeJson>) -> DefJson {
    DefJson {
        id: None,
        flow_code: code.to_string(),
        flow_name: name.to_string(),
        model_value: "CLASSICS".to_string(),
        category: None,
        version: "1".to_string(),
        is_publish: None,
        form_custom: None,
        form_path: None,
        listener_type: None,
        listener_path: None,
        ext: None,
        create_by: None,
        update_by: None,
        node_list: nodes,
        chart_status_color: None,
        top_text: None,
        top_text_show: false,
    }
}

// ---------------------------------------------------------------------------
// Test 1: Simple linear flow Start -> Task -> End
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_simple_start_pass_end() {
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    let def_json = make_def_json(
        "test_linear",
        "线性流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "node1", "提交")],
            ),
            make_node(
                NodeType::Between,
                "node1",
                "审批",
                Some("user1"),
                vec![make_skip("node1", "end", "通过")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let flow_params = FlowParams {
        flow_code: Some("test_linear".to_string()),
        handler: Some("user0".to_string()),
        skip_type: Some("PASS".to_string()),
        ..Default::default()
    };
    let instance = ins_service
        .start(&dctx, "biz_001", &flow_params, &test_ctx())
        .await
        .unwrap();

    assert_eq!(instance.node_code, "node1");

    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks.len(), 1);
    let task = &tasks[0];
    assert_eq!(task.node_code, "node1");

    let users = UserRepository::select_by_task_id(&dctx, task.id.unwrap())
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].processed_by.as_deref(), Some("user1"));
    assert_eq!(users[0].type_, 1);

    let his_tasks =
        HisTaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(his_tasks.len(), 1);

    let pass_params = FlowParams {
        handler: Some("user1".to_string()),
        skip_type: Some("PASS".to_string()),
        message: Some("同意".to_string()),
        ..Default::default()
    };
    let updated_instance = task_service
        .skip(&dctx, task.id.unwrap(), &pass_params, &test_ctx())
        .await
        .unwrap();

    assert_eq!(updated_instance.node_code, "end");
    assert_eq!(updated_instance.flow_status, "8");

    let remaining = TaskRepository::select_by_instance_id(
        &dctx,
        updated_instance.id.unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(remaining.len(), 0);

    let his = HisTaskRepository::select_by_instance_id(
        &dctx,
        updated_instance.id.unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(his.len(), 2);
}

// ---------------------------------------------------------------------------
// Test 2a: Serial gateway routes to director when amount > 1000
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_serial_gateway_condition_high_amount() {
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    // Flow: start -> node1 -> gw(serial) -> director / manager -> end
    let def_json = make_def_json(
        "test_serial",
        "串行网关流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "node1", "提交")],
            ),
            make_node(
                NodeType::Between,
                "node1",
                "审批",
                Some("user1"),
                vec![make_skip("node1", "gw", "通过")],
            ),
            make_node(
                NodeType::Serial,
                "gw",
                "金额判断",
                None,
                vec![
                    make_skip_cond(
                        "gw",
                        "director",
                        "大于1000",
                        "gt@@amount|1000",
                    ),
                    make_skip_cond("gw", "manager", "默认", ""),
                ],
            ),
            make_node(
                NodeType::Between,
                "director",
                "主管审批",
                Some("user2"),
                vec![make_skip("director", "end", "通过")],
            ),
            make_node(
                NodeType::Between,
                "manager",
                "经理审批",
                Some("user3"),
                vec![make_skip("manager", "end", "通过")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let mut variable = HashMap::new();
    variable.insert("amount".to_string(), serde_json::json!(1500));

    let flow_params = FlowParams {
        flow_code: Some("test_serial".to_string()),
        handler: Some("user0".to_string()),
        variable,
        ..Default::default()
    };
    let instance = ins_service
        .start(&dctx, "biz_002", &flow_params, &test_ctx())
        .await
        .unwrap();

    assert_eq!(instance.node_code, "node1");

    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks.len(), 1);
    let task = &tasks[0];
    assert_eq!(task.node_code, "node1");

    let pass_params = FlowParams {
        handler: Some("user1".to_string()),
        skip_type: Some("PASS".to_string()),
        message: Some("同意".to_string()),
        ..Default::default()
    };
    let updated_instance = task_service
        .skip(&dctx, task.id.unwrap(), &pass_params, &test_ctx())
        .await
        .unwrap();

    assert_eq!(updated_instance.node_code, "director");

    let new_tasks = TaskRepository::select_by_instance_id(
        &dctx,
        updated_instance.id.unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(new_tasks.len(), 1);
    assert_eq!(new_tasks[0].node_code, "director");

    let users =
        UserRepository::select_by_task_id(&dctx, new_tasks[0].id.unwrap())
            .await
            .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].processed_by.as_deref(), Some("user2"));
}

// ---------------------------------------------------------------------------
// Test 2b: Serial gateway routes to manager when amount <= 1000 (default)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_serial_gateway_condition_low_amount() {
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    let def_json = make_def_json(
        "test_serial",
        "串行网关流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "node1", "提交")],
            ),
            make_node(
                NodeType::Between,
                "node1",
                "审批",
                Some("user1"),
                vec![make_skip("node1", "gw", "通过")],
            ),
            make_node(
                NodeType::Serial,
                "gw",
                "金额判断",
                None,
                vec![
                    make_skip_cond(
                        "gw",
                        "director",
                        "大于1000",
                        "gt@@amount|1000",
                    ),
                    make_skip_cond("gw", "manager", "默认", ""),
                ],
            ),
            make_node(
                NodeType::Between,
                "director",
                "主管审批",
                Some("user2"),
                vec![make_skip("director", "end", "通过")],
            ),
            make_node(
                NodeType::Between,
                "manager",
                "经理审批",
                Some("user3"),
                vec![make_skip("manager", "end", "通过")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let mut variable = HashMap::new();
    variable.insert("amount".to_string(), serde_json::json!(500));

    let flow_params = FlowParams {
        flow_code: Some("test_serial".to_string()),
        handler: Some("user0".to_string()),
        variable,
        ..Default::default()
    };
    let instance = ins_service
        .start(&dctx, "biz_003", &flow_params, &test_ctx())
        .await
        .unwrap();

    assert_eq!(instance.node_code, "node1");

    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    let task = &tasks[0];

    let pass_params = FlowParams {
        handler: Some("user1".to_string()),
        skip_type: Some("PASS".to_string()),
        message: Some("同意".to_string()),
        ..Default::default()
    };
    let updated_instance = task_service
        .skip(&dctx, task.id.unwrap(), &pass_params, &test_ctx())
        .await
        .unwrap();

    assert_eq!(updated_instance.node_code, "manager");

    let new_tasks = TaskRepository::select_by_instance_id(
        &dctx,
        updated_instance.id.unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(new_tasks.len(), 1);
    assert_eq!(new_tasks[0].node_code, "manager");

    let users =
        UserRepository::select_by_task_id(&dctx, new_tasks[0].id.unwrap())
            .await
            .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].processed_by.as_deref(), Some("user3"));
}

// ---------------------------------------------------------------------------
// Test 3: Parallel gateway split (P2 behavior — each branch independently
// flows to END)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_parallel_gateway() {
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    // Flow: start -> gw(parallel) -> taskA + taskB -> end
    let def_json = make_def_json(
        "test_parallel",
        "并行网关流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "gw", "提交")],
            ),
            make_node(
                NodeType::Parallel,
                "gw",
                "并行分支",
                None,
                vec![
                    make_skip("gw", "taskA", "分支A"),
                    make_skip("gw", "taskB", "分支B"),
                ],
            ),
            make_node(
                NodeType::Between,
                "taskA",
                "任务A",
                Some("userA"),
                vec![make_skip("taskA", "end", "完成")],
            ),
            make_node(
                NodeType::Between,
                "taskB",
                "任务B",
                Some("userB"),
                vec![make_skip("taskB", "end", "完成")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let flow_params = FlowParams {
        flow_code: Some("test_parallel".to_string()),
        handler: Some("user0".to_string()),
        ..Default::default()
    };
    let instance = ins_service
        .start(&dctx, "biz_004", &flow_params, &test_ctx())
        .await
        .unwrap();

    // After start: parallel gateway splits into taskA + taskB
    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks.len(), 2);

    let task_a = tasks
        .iter()
        .find(|t| t.node_code == "taskA")
        .expect("taskA should exist");
    let task_b = tasks
        .iter()
        .find(|t| t.node_code == "taskB")
        .expect("taskB should exist");

    // Verify both tasks have correct users
    let users_a = UserRepository::select_by_task_id(&dctx, task_a.id.unwrap())
        .await
        .unwrap();
    assert_eq!(users_a.len(), 1);
    assert_eq!(users_a[0].processed_by.as_deref(), Some("userA"));

    let users_b = UserRepository::select_by_task_id(&dctx, task_b.id.unwrap())
        .await
        .unwrap();
    assert_eq!(users_b.len(), 1);
    assert_eq!(users_b[0].processed_by.as_deref(), Some("userB"));

    // Pass taskA -> taskA reaches END, instance finishes, taskB cleaned up
    let pass_params = FlowParams {
        handler: Some("userA".to_string()),
        skip_type: Some("PASS".to_string()),
        message: Some("完成A".to_string()),
        ..Default::default()
    };
    let updated_instance = task_service
        .skip(&dctx, task_a.id.unwrap(), &pass_params, &test_ctx())
        .await
        .unwrap();

    // P2 behavior: passing one branch reaches END
    assert_eq!(updated_instance.node_code, "end");
    assert_eq!(updated_instance.flow_status, "8");

    // All tasks cleaned up (hand_undone_task removes taskB)
    let remaining = TaskRepository::select_by_instance_id(
        &dctx,
        updated_instance.id.unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(remaining.len(), 0);

    // History: start + taskA pass = 2 records
    let his = HisTaskRepository::select_by_instance_id(
        &dctx,
        updated_instance.id.unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(his.len(), 2);
}

// ---------------------------------------------------------------------------
// Test 3b: Parallel gateway join — downstream task created only after all
// branches arrive
// ---------------------------------------------------------------------------

#[tokio::test]
async fn parallel_gateway_joins_before_downstream() {
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    // Flow: start -> split(parallel) -> [B, C] -> join(parallel) -> D -> end
    let def_json = make_def_json(
        "test_parallel_join",
        "并行汇聚网关流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "split", "提交")],
            ),
            make_node(
                NodeType::Parallel,
                "split",
                "并行分支",
                None,
                vec![
                    make_skip("split", "taskB", "分支B"),
                    make_skip("split", "taskC", "分支C"),
                ],
            ),
            make_node(
                NodeType::Between,
                "taskB",
                "任务B",
                Some("userB"),
                vec![make_skip("taskB", "join", "完成")],
            ),
            make_node(
                NodeType::Between,
                "taskC",
                "任务C",
                Some("userC"),
                vec![make_skip("taskC", "join", "完成")],
            ),
            make_node(
                NodeType::Parallel,
                "join",
                "并行汇聚",
                None,
                vec![make_skip("join", "taskD", "通过")],
            ),
            make_node(
                NodeType::Between,
                "taskD",
                "任务D",
                Some("userD"),
                vec![make_skip("taskD", "end", "完成")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let flow_params = FlowParams {
        flow_code: Some("test_parallel_join".to_string()),
        handler: Some("user0".to_string()),
        ..Default::default()
    };
    let instance = ins_service
        .start(&dctx, "biz_004b", &flow_params, &test_ctx())
        .await
        .unwrap();

    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks.len(), 2);

    let task_b = tasks
        .iter()
        .find(|t| t.node_code == "taskB")
        .expect("taskB should exist");
    let task_c = tasks
        .iter()
        .find(|t| t.node_code == "taskC")
        .expect("taskC should exist");

    // Pass taskB -> taskC still active, join should block, no taskD yet
    let pass_b = FlowParams {
        handler: Some("userB".to_string()),
        skip_type: Some("PASS".to_string()),
        message: Some("完成B".to_string()),
        ..Default::default()
    };
    task_service
        .skip(&dctx, task_b.id.unwrap(), &pass_b, &test_ctx())
        .await
        .unwrap();

    let after_b =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(after_b.len(), 1);
    assert_eq!(after_b[0].node_code, "taskC");

    // Pass taskC -> last branch arrives, taskD generated (only one)
    let pass_c = FlowParams {
        handler: Some("userC".to_string()),
        skip_type: Some("PASS".to_string()),
        message: Some("完成C".to_string()),
        ..Default::default()
    };
    let updated_instance = task_service
        .skip(&dctx, task_c.id.unwrap(), &pass_c, &test_ctx())
        .await
        .unwrap();

    let after_c = TaskRepository::select_by_instance_id(
        &dctx,
        updated_instance.id.unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(after_c.len(), 1);
    assert_eq!(after_c[0].node_code, "taskD");
}

// ---------------------------------------------------------------------------
// Test 4a: Inclusive gateway produces multiple branches when both conditions
// match
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_inclusive_gateway_multi_match() {
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    let def_json = make_def_json(
        "test_inclusive",
        "包容网关流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "node1", "提交")],
            ),
            make_node(
                NodeType::Between,
                "node1",
                "审批",
                Some("user1"),
                vec![make_skip("node1", "gw", "通过")],
            ),
            make_node(
                NodeType::Inclusive,
                "gw",
                "包容分支",
                None,
                vec![
                    make_skip_cond("gw", "taskA", "条件A", "gt@@amount|1000"),
                    make_skip_cond("gw", "taskB", "条件B", "eq@@type|urgent"),
                    make_skip_cond("gw", "manager", "默认", ""),
                ],
            ),
            make_node(
                NodeType::Between,
                "taskA",
                "高额审批",
                Some("userA"),
                vec![make_skip("taskA", "end", "通过")],
            ),
            make_node(
                NodeType::Between,
                "taskB",
                "加急审批",
                Some("userB"),
                vec![make_skip("taskB", "end", "通过")],
            ),
            make_node(
                NodeType::Between,
                "manager",
                "经理审批",
                Some("userM"),
                vec![make_skip("manager", "end", "通过")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let mut variable = HashMap::new();
    variable.insert("amount".to_string(), serde_json::json!(1500));
    variable.insert("type".to_string(), serde_json::json!("urgent"));

    let flow_params = FlowParams {
        flow_code: Some("test_inclusive".to_string()),
        handler: Some("user0".to_string()),
        variable,
        ..Default::default()
    };
    let instance = ins_service
        .start(&dctx, "biz_005", &flow_params, &test_ctx())
        .await
        .unwrap();

    assert_eq!(instance.node_code, "node1");

    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    let task = &tasks[0];

    let pass_params = FlowParams {
        handler: Some("user1".to_string()),
        skip_type: Some("PASS".to_string()),
        message: Some("同意".to_string()),
        ..Default::default()
    };
    let updated_instance = task_service
        .skip(&dctx, task.id.unwrap(), &pass_params, &test_ctx())
        .await
        .unwrap();

    let new_tasks = TaskRepository::select_by_instance_id(
        &dctx,
        updated_instance.id.unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(new_tasks.len(), 3);
    let codes: Vec<&str> =
        new_tasks.iter().map(|t| t.node_code.as_str()).collect();
    assert!(codes.contains(&"taskA"));
    assert!(codes.contains(&"taskB"));
    assert!(codes.contains(&"manager"));

    for t in &new_tasks {
        let expected_handler = match t.node_code.as_str() {
            "taskA" => Some("userA"),
            "taskB" => Some("userB"),
            "manager" => Some("userM"),
            _ => None,
        };
        let users = UserRepository::select_by_task_id(&dctx, t.id.unwrap())
            .await
            .unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].processed_by.as_deref(), expected_handler);
    }
}

// ---------------------------------------------------------------------------
// Test 4b: Inclusive gateway falls back to default branch when no condition
// matches
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_inclusive_gateway_default_only() {
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    let def_json = make_def_json(
        "test_inclusive",
        "包容网关流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "node1", "提交")],
            ),
            make_node(
                NodeType::Between,
                "node1",
                "审批",
                Some("user1"),
                vec![make_skip("node1", "gw", "通过")],
            ),
            make_node(
                NodeType::Inclusive,
                "gw",
                "包容分支",
                None,
                vec![
                    make_skip_cond("gw", "taskA", "条件A", "gt@@amount|1000"),
                    make_skip_cond("gw", "taskB", "条件B", "eq@@type|urgent"),
                    make_skip_cond("gw", "manager", "默认", ""),
                ],
            ),
            make_node(
                NodeType::Between,
                "taskA",
                "高额审批",
                Some("userA"),
                vec![make_skip("taskA", "end", "通过")],
            ),
            make_node(
                NodeType::Between,
                "taskB",
                "加急审批",
                Some("userB"),
                vec![make_skip("taskB", "end", "通过")],
            ),
            make_node(
                NodeType::Between,
                "manager",
                "经理审批",
                Some("userM"),
                vec![make_skip("manager", "end", "通过")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let mut variable = HashMap::new();
    variable.insert("amount".to_string(), serde_json::json!(500));
    variable.insert("type".to_string(), serde_json::json!("normal"));

    let flow_params = FlowParams {
        flow_code: Some("test_inclusive".to_string()),
        handler: Some("user0".to_string()),
        variable,
        ..Default::default()
    };
    let instance = ins_service
        .start(&dctx, "biz_006", &flow_params, &test_ctx())
        .await
        .unwrap();

    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    let task = &tasks[0];

    let pass_params = FlowParams {
        handler: Some("user1".to_string()),
        skip_type: Some("PASS".to_string()),
        message: Some("同意".to_string()),
        ..Default::default()
    };
    let updated_instance = task_service
        .skip(&dctx, task.id.unwrap(), &pass_params, &test_ctx())
        .await
        .unwrap();

    let new_tasks = TaskRepository::select_by_instance_id(
        &dctx,
        updated_instance.id.unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(new_tasks.len(), 1);
    assert_eq!(new_tasks[0].node_code, "manager");

    let users =
        UserRepository::select_by_task_id(&dctx, new_tasks[0].id.unwrap())
            .await
            .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].processed_by.as_deref(), Some("userM"));
}

// ---------------------------------------------------------------------------
// Test 4c: Inclusive gateway always keeps the default (empty-condition) branch
// alongside matched branches
// ---------------------------------------------------------------------------

#[tokio::test]
async fn inclusive_gateway_keeps_default_branch() {
    // start -> node1 -> incl -> [A(amount>1000), B(default), C(amount>5000)]
    // amount=1500 -> A matches, C does not, B default -> active tasks {A, B}
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    let def_json = make_def_json(
        "test_inclusive_default_kept",
        "包容网关默认分支",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "node1", "提交")],
            ),
            make_node(
                NodeType::Between,
                "node1",
                "审批",
                Some("user1"),
                vec![make_skip("node1", "gw", "通过")],
            ),
            make_node(
                NodeType::Inclusive,
                "gw",
                "包容分支",
                None,
                vec![
                    make_skip_cond("gw", "taskA", "高额", "gt@@amount|1000"),
                    make_skip_cond("gw", "taskB", "默认", ""),
                    make_skip_cond("gw", "taskC", "超高额", "gt@@amount|5000"),
                ],
            ),
            make_node(
                NodeType::Between,
                "taskA",
                "高额审批",
                Some("userA"),
                vec![make_skip("taskA", "end", "通过")],
            ),
            make_node(
                NodeType::Between,
                "taskB",
                "默认审批",
                Some("userB"),
                vec![make_skip("taskB", "end", "通过")],
            ),
            make_node(
                NodeType::Between,
                "taskC",
                "超高额审批",
                Some("userC"),
                vec![make_skip("taskC", "end", "通过")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let mut variable = HashMap::new();
    variable.insert("amount".to_string(), serde_json::json!(1500));

    let flow_params = FlowParams {
        flow_code: Some("test_inclusive_default_kept".to_string()),
        handler: Some("user0".to_string()),
        variable,
        ..Default::default()
    };
    let instance = ins_service
        .start(&dctx, "biz_005b", &flow_params, &test_ctx())
        .await
        .unwrap();

    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    let task = &tasks[0];

    let pass_params = FlowParams {
        handler: Some("user1".to_string()),
        skip_type: Some("PASS".to_string()),
        message: Some("同意".to_string()),
        ..Default::default()
    };
    let updated_instance = task_service
        .skip(&dctx, task.id.unwrap(), &pass_params, &test_ctx())
        .await
        .unwrap();

    let new_tasks = TaskRepository::select_by_instance_id(
        &dctx,
        updated_instance.id.unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(new_tasks.len(), 2);
    let codes: Vec<&str> =
        new_tasks.iter().map(|t| t.node_code.as_str()).collect();
    assert!(codes.contains(&"taskA"));
    assert!(codes.contains(&"taskB"));
    assert!(!codes.contains(&"taskC"));
}

// ---------------------------------------------------------------------------
// Test 5: Countersign (会签) — node_ratio = "100", all 3 users must pass
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_countersign_all_must_pass() {
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    // Flow: start → countersign (user1@@user2@@user3, ratio=100) → end
    let def_json = make_def_json(
        "test_countersign",
        "会签流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "cs_node", "提交")],
            ),
            make_node_ratio(
                NodeType::Between,
                "cs_node",
                "会签审批",
                Some("user1@@user2@@user3"),
                "100",
                vec![make_skip("cs_node", "end", "通过")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let instance = ins_service
        .start(
            &dctx,
            "biz_cs_001",
            &FlowParams {
                flow_code: Some("test_countersign".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    assert_eq!(instance.node_code, "cs_node");

    // 会签任务有 3 个 flow_user
    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks.len(), 1);
    let task_id = tasks[0].id.unwrap();

    let users = UserRepository::select_by_task_id(&dctx, task_id)
        .await
        .unwrap();
    assert_eq!(users.len(), 3);
    let handler_set: Vec<&str> = users
        .iter()
        .filter_map(|u| u.processed_by.as_deref())
        .collect();
    assert!(handler_set.contains(&"user1"));
    assert!(handler_set.contains(&"user2"));
    assert!(handler_set.contains(&"user3"));

    // --- user1 通过：尚未达到阈值，不推进 ---
    let pass = |h: &str| FlowParams {
        handler: Some(h.to_string()),
        skip_type: Some("PASS".to_string()),
        ..Default::default()
    };

    let after_user1 = task_service
        .skip(&dctx, task_id, &pass("user1"), &test_ctx())
        .await
        .unwrap();

    assert_eq!(after_user1.node_code, "cs_node", "user1 通过后不应推进");
    assert_ne!(after_user1.flow_status, "8", "流程状态不应已完成");

    // user1 已从 flow_user 移除，剩余 2 人
    let remaining = UserRepository::select_by_task_id(&dctx, task_id)
        .await
        .unwrap();
    assert_eq!(remaining.len(), 2);
    let remaining_handlers: Vec<&str> = remaining
        .iter()
        .filter_map(|u| u.processed_by.as_deref())
        .collect();
    assert!(!remaining_handlers.contains(&"user1"));

    // his_task：start 写 1 条，user1 部分通过再写 1 条，共 2 条
    let his =
        HisTaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(his.len(), 2);

    // --- user2 通过：仍未达到阈值 ---
    let after_user2 = task_service
        .skip(&dctx, task_id, &pass("user2"), &test_ctx())
        .await
        .unwrap();

    assert_eq!(after_user2.node_code, "cs_node", "user2 通过后不应推进");

    let remaining2 = UserRepository::select_by_task_id(&dctx, task_id)
        .await
        .unwrap();
    assert_eq!(remaining2.len(), 1);
    assert_eq!(remaining2[0].processed_by.as_deref(), Some("user3"));

    let his2 =
        HisTaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(his2.len(), 3);

    // --- user3 通过：达到 100%，推进到 end ---
    let final_instance = task_service
        .skip(&dctx, task_id, &pass("user3"), &test_ctx())
        .await
        .unwrap();

    assert_eq!(
        final_instance.node_code, "end",
        "全员通过后应推进到结束节点"
    );
    assert_eq!(final_instance.flow_status, "8", "流程状态应为已完成");

    // 无活跃任务
    let final_tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(final_tasks.len(), 0);
}

// ---------------------------------------------------------------------------
// Test 6: Reject (退回) — reject back to node1, re-approve through to end
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_reject_and_reapprove() {
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    // Flow: start → node1(user1, PASS→node2) → node2(user2, PASS→end, REJECT→node1) → end
    let def_json = make_def_json(
        "test_reject",
        "退回流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "node1", "提交")],
            ),
            make_node(
                NodeType::Between,
                "node1",
                "一级审批",
                Some("user1"),
                vec![make_skip("node1", "node2", "通过")],
            ),
            make_node(
                NodeType::Between,
                "node2",
                "二级审批",
                Some("user2"),
                vec![
                    make_skip("node2", "end", "通过"),
                    make_reject_skip("node2", "node1", "退回"),
                ],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let instance = ins_service
        .start(
            &dctx,
            "biz_rej_001",
            &FlowParams {
                flow_code: Some("test_reject".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    assert_eq!(instance.node_code, "node1");

    // --- user1 通过 node1 → 到达 node2 ---
    let tasks1 =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    let task1_id = tasks1[0].id.unwrap();

    let at_node2 = task_service
        .skip(
            &dctx,
            task1_id,
            &FlowParams {
                handler: Some("user1".to_string()),
                skip_type: Some("PASS".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    assert_eq!(at_node2.node_code, "node2");

    // --- user2 退回 node2 → node1，流程状态为 Reject ---
    let tasks2 =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks2.len(), 1);
    assert_eq!(tasks2[0].node_code, "node2");
    let task2_id = tasks2[0].id.unwrap();

    let rejected = task_service
        .reject(
            &dctx,
            task2_id,
            &FlowParams {
                handler: Some("user2".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    assert_eq!(rejected.node_code, "node1", "退回后实例应回到 node1");
    assert_eq!(rejected.flow_status, "9", "流程状态应为 Reject");

    // 退回后 node1 有新任务，且 user1 仍是办理人
    let tasks_after_reject =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks_after_reject.len(), 1);
    assert_eq!(tasks_after_reject[0].node_code, "node1");

    let users_after_reject = UserRepository::select_by_task_id(
        &dctx,
        tasks_after_reject[0].id.unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(users_after_reject.len(), 1);
    assert_eq!(users_after_reject[0].processed_by.as_deref(), Some("user1"));

    // --- user1 再次通过 node1 → node2 ---
    let task3_id = tasks_after_reject[0].id.unwrap();

    let at_node2_again = task_service
        .skip(
            &dctx,
            task3_id,
            &FlowParams {
                handler: Some("user1".to_string()),
                skip_type: Some("PASS".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    assert_eq!(at_node2_again.node_code, "node2");

    // --- user2 通过 node2 → 流程结束 ---
    let tasks4 =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    let task4_id = tasks4[0].id.unwrap();

    let finished = task_service
        .skip(
            &dctx,
            task4_id,
            &FlowParams {
                handler: Some("user2".to_string()),
                skip_type: Some("PASS".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    assert_eq!(finished.node_code, "end", "最终应到达结束节点");
    assert_eq!(finished.flow_status, "8", "流程状态应为已完成");

    let final_tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(final_tasks.len(), 0, "结束后不应有活跃任务");
}

// ---------------------------------------------------------------------------
// Test 8: Listener fire order is start -> assignment -> finish -> create
// ---------------------------------------------------------------------------

struct RecordingListener {
    events: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl Listener for RecordingListener {
    async fn notify(
        &self,
        listener_type: &str,
        _params: Option<&serde_json::Value>,
        _var: &mut flow::listener::ListenerVariable,
    ) -> app_core::error::AppResult<()> {
        self.events.lock().unwrap().push(listener_type.to_string());
        Ok(())
    }
}

#[tokio::test]
async fn listener_fire_order_is_start_assignment_finish_create() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut registry = ListenerRegistry::new();
    registry.register(
        "recorder",
        Arc::new(RecordingListener {
            events: events.clone(),
        }),
    );
    let registry = Arc::new(registry);

    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        registry.clone(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        registry.clone(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    // node1 and node2 both trigger all four events via the recorder; we clear
    // the log before skip so that only the skip-phase order is asserted.
    let make_recorder_node = |code: &str, perm: &str, next: &str| {
        let mut node = make_node(
            NodeType::Between,
            code,
            "审批",
            Some(perm),
            vec![make_skip(code, next, "通过")],
        );
        node.listener_type = Some("start,assignment,finish,create".to_string());
        node.listener_path =
            Some("recorder@@recorder@@recorder@@recorder".to_string());
        node
    };

    let def_json = make_def_json(
        "test_listener_order",
        "监听器顺序流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "node1", "提交")],
            ),
            make_recorder_node("node1", "user1", "node2"),
            make_recorder_node("node2", "user2", "end"),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let instance = ins_service
        .start(
            &dctx,
            "biz_listener_order",
            &FlowParams {
                flow_code: Some("test_listener_order".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    let task_id = tasks[0].id.unwrap();

    // Ignore assignment/create events emitted during instance start.
    events.lock().unwrap().clear();

    task_service
        .skip(
            &dctx,
            task_id,
            &FlowParams {
                handler: Some("user1".to_string()),
                skip_type: Some("PASS".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let captured: Vec<String> = events.lock().unwrap().clone();
    assert_eq!(captured, vec!["start", "assignment", "finish", "create"]);
}

// ---------------------------------------------------------------------------
// Test 7: Countersign reject respects threshold (rejectCount / reject-ratio)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn countersign_reject_needs_threshold() {
    // Flow: start -> cs(countersign u1/u2/u3, ratio=60, PASS->end, REJECT->back)
    // ratio=60 -> reject threshold = ceil(3 * 40%) = 2 votes
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    let def_json = make_def_json(
        "test_reject_threshold",
        "驳回阈值流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "cs", "提交")],
            ),
            make_node_ratio(
                NodeType::Between,
                "cs",
                "会签审批",
                Some("u1@@u2@@u3"),
                "60",
                vec![
                    make_skip("cs", "end", "通过"),
                    make_reject_skip("cs", "back", "驳回"),
                ],
            ),
            make_node(
                NodeType::Between,
                "back",
                "驳回回退",
                Some("userBack"),
                vec![make_skip("back", "cs", "重新提交")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let instance = ins_service
        .start(
            &dctx,
            "biz_rej_thr_001",
            &FlowParams {
                flow_code: Some("test_reject_threshold".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    // After start: one countersign task with 3 users
    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks.len(), 1);
    let cs_task_id = tasks[0].id.unwrap();
    let cs_users = UserRepository::select_by_task_id(&dctx, cs_task_id)
        .await
        .unwrap();
    assert_eq!(cs_users.len(), 3);

    // u1 rejects -> 1 reject, below threshold, cs task stays
    let after_u1 = task_service
        .reject(
            &dctx,
            cs_task_id,
            &FlowParams {
                handler: Some("u1".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();
    assert_eq!(after_u1.node_code, "cs");

    let tasks_after_u1 =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks_after_u1.len(), 1);
    assert_eq!(tasks_after_u1[0].node_code, "cs");

    // u2 rejects -> 2 rejects, reaches threshold, route to back
    let after_u2 = task_service
        .reject(
            &dctx,
            cs_task_id,
            &FlowParams {
                handler: Some("u2".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();
    assert_eq!(after_u2.node_code, "back");
    assert_eq!(after_u2.flow_status, "9");

    let tasks_after_u2 =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks_after_u2.len(), 1);
    assert_eq!(tasks_after_u2[0].node_code, "back");
}

// ---------------------------------------------------------------------------
// Test 9: one_vote_veto only deletes tasks in the target's successor set
// ---------------------------------------------------------------------------

#[tokio::test]
async fn veto_keeps_unrelated_parallel_branch() {
    // start -> split(parallel) -> [taskB, taskX]
    // taskB has REJECT -> back; taskX has PASS -> end
    // Rejecting taskB should NOT delete taskX.
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    let def_json = make_def_json(
        "test_veto_scope",
        "veto 范围流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "split", "提交")],
            ),
            make_node(
                NodeType::Parallel,
                "split",
                "并行分支",
                None,
                vec![
                    make_skip("split", "taskB", "分支B"),
                    make_skip("split", "taskX", "分支X"),
                ],
            ),
            make_node(
                NodeType::Between,
                "taskB",
                "任务B",
                Some("userB"),
                vec![make_reject_skip("taskB", "back", "驳回")],
            ),
            make_node(
                NodeType::Between,
                "taskX",
                "任务X",
                Some("userX"),
                vec![make_skip("taskX", "end", "完成")],
            ),
            make_node(
                NodeType::Between,
                "back",
                "回退",
                Some("userBack"),
                vec![make_skip("back", "taskB", "重新提交")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let instance = ins_service
        .start(
            &dctx,
            "biz_veto_scope_001",
            &FlowParams {
                flow_code: Some("test_veto_scope".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks.len(), 2);
    let task_b = tasks
        .iter()
        .find(|t| t.node_code == "taskB")
        .expect("taskB should exist");

    let rejected = task_service
        .reject(
            &dctx,
            task_b.id.unwrap(),
            &FlowParams {
                handler: Some("userB".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    assert_eq!(rejected.node_code, "back");

    let remaining =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(remaining.len(), 2);
    let codes: Vec<&str> =
        remaining.iter().map(|t| t.node_code.as_str()).collect();
    assert!(codes.contains(&"back"));
    assert!(codes.contains(&"taskX"));
}

// ---------------------------------------------------------------------------
// Test 10: anyNodeSkip allows rejecting to an arbitrary upstream node
// ---------------------------------------------------------------------------

#[tokio::test]
async fn reject_to_any_node_when_allowed() {
    // start -> node1 -> node2. node2 has anyNodeSkip=1 and REJECT->back.
    // Reject with node_code="node1" should jump to node1 instead of back.
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    let mut node2 = make_node(
        NodeType::Between,
        "node2",
        "二级审批",
        Some("user2"),
        vec![make_reject_skip("node2", "back", "驳回")],
    );
    node2.any_node_skip = Some("1".to_string());

    let def_json = make_def_json(
        "test_any_node_skip",
        "任意驳回流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "node1", "提交")],
            ),
            make_node(
                NodeType::Between,
                "node1",
                "一级审批",
                Some("user1"),
                vec![make_skip("node1", "node2", "通过")],
            ),
            node2,
            make_node(
                NodeType::Between,
                "back",
                "默认回退",
                Some("userBack"),
                vec![make_skip("back", "node2", "重新提交")],
            ),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let instance = ins_service
        .start(
            &dctx,
            "biz_any_skip_001",
            &FlowParams {
                flow_code: Some("test_any_node_skip".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    // Pass node1 to reach node2
    let tasks1 =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    task_service
        .skip(
            &dctx,
            tasks1[0].id.unwrap(),
            &FlowParams {
                handler: Some("user1".to_string()),
                skip_type: Some("PASS".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let tasks2 =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks2.len(), 1);
    assert_eq!(tasks2[0].node_code, "node2");

    // Reject node2 to arbitrary node1
    let rejected = task_service
        .reject(
            &dctx,
            tasks2[0].id.unwrap(),
            &FlowParams {
                handler: Some("user2".to_string()),
                node_code: Some("node1".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    assert_eq!(rejected.node_code, "node1");
    assert_eq!(rejected.flow_status, "9");

    let remaining =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].node_code, "node1");
}

// ---------------------------------------------------------------------------
// Test 11: Sequential sign (@@sequence) creates one task at a time
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sequential_sign_creates_one_task_at_a_time() {
    // start -> cs (u1@@u2@@u3, ratio=100@@sequence, PASS->end) -> end
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    let def_json = make_def_json(
        "test_sequence",
        "顺签流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "cs", "提交")],
            ),
            make_node_ratio(
                NodeType::Between,
                "cs",
                "顺签审批",
                Some("u1@@u2@@u3"),
                "100@@sequence",
                vec![make_skip("cs", "end", "通过")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let instance = ins_service
        .start(
            &dctx,
            "biz_seq_001",
            &FlowParams {
                flow_code: Some("test_sequence".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    // After start: exactly one task for u1
    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks.len(), 1);
    let mut current_task_id = tasks[0].id.unwrap();
    let users = UserRepository::select_by_task_id(&dctx, current_task_id)
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].processed_by.as_deref(), Some("u1"));

    // u1 passes -> task for u2
    for (handler, expected_next) in [("u1", Some("u2")), ("u2", Some("u3"))] {
        let after = task_service
            .skip(
                &dctx,
                current_task_id,
                &FlowParams {
                    handler: Some(handler.to_string()),
                    skip_type: Some("PASS".to_string()),
                    ..Default::default()
                },
                &test_ctx(),
            )
            .await
            .unwrap();

        let next_tasks =
            TaskRepository::select_by_instance_id(&dctx, after.id.unwrap())
                .await
                .unwrap();
        assert_eq!(next_tasks.len(), 1, "{handler} 通过后应只剩 1 个任务");
        current_task_id = next_tasks[0].id.unwrap();
        let next_users =
            UserRepository::select_by_task_id(&dctx, current_task_id)
                .await
                .unwrap();
        assert_eq!(next_users.len(), 1);
        assert_eq!(
            next_users[0].processed_by.as_deref(),
            expected_next,
            "{handler} 通过后下一位应为 {expected_next:?}"
        );
    }

    // u3 passes -> node completes, reach end
    let finished = task_service
        .skip(
            &dctx,
            current_task_id,
            &FlowParams {
                handler: Some("u3".to_string()),
                skip_type: Some("PASS".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    assert_eq!(finished.node_code, "end");
}

// ---------------------------------------------------------------------------
// Test 12: Each simple skip writes exactly one history record
// ---------------------------------------------------------------------------

#[tokio::test]
async fn skip_writes_single_his_record_per_step() {
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    let def_json = make_def_json(
        "test_single_his",
        "单步历史流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "node1", "提交")],
            ),
            make_node(
                NodeType::Between,
                "node1",
                "审批",
                Some("user1"),
                vec![make_skip("node1", "end", "通过")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let instance = ins_service
        .start(
            &dctx,
            "biz_single_his_001",
            &FlowParams {
                flow_code: Some("test_single_his".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let his_before =
        HisTaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    let count_before = his_before.len();

    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    task_service
        .skip(
            &dctx,
            tasks[0].id.unwrap(),
            &FlowParams {
                handler: Some("user1".to_string()),
                skip_type: Some("PASS".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let his_after =
        HisTaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(his_after.len(), count_before + 1);
}

// ---------------------------------------------------------------------------
// Test 13: Mixed pass/reject — reject total must include already-passed users
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mixed_vote_total() {
    // 票签节点 ratio=60%, 3 个办理人 u1/u2/u3
    // 通过阈值 = ceil(3 * 60%) = 2 票
    // 驳回阈值 = ceil(3 * 40%) = 2 票
    // 场景：u1 通过后，u2 驳回。驳回票数=1 < 2，不应触发退回。
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    let def_json = make_def_json(
        "test_mixed_vote",
        "混合投票流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "cs", "提交")],
            ),
            make_node_ratio(
                NodeType::Between,
                "cs",
                "票签审批",
                Some("u1@@u2@@u3"),
                "60",
                vec![
                    make_skip("cs", "end", "通过"),
                    make_reject_skip("cs", "back", "驳回"),
                ],
            ),
            make_node(
                NodeType::Between,
                "back",
                "驳回回退",
                Some("userBack"),
                vec![make_skip("back", "cs", "重新提交")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );

    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let instance = ins_service
        .start(
            &dctx,
            "biz_mixed_vote_001",
            &FlowParams {
                flow_code: Some("test_mixed_vote".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    assert_eq!(instance.node_code, "cs");

    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks.len(), 1);
    let cs_task_id = tasks[0].id.unwrap();

    // u1 通过：1 票通过，未达通过阈值 2，留在 cs
    let after_u1_pass = task_service
        .skip(
            &dctx,
            cs_task_id,
            &FlowParams {
                handler: Some("u1".to_string()),
                skip_type: Some("PASS".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();
    assert_eq!(after_u1_pass.node_code, "cs", "u1 通过后不应推进");

    // u2 驳回：total 应为 3（1 通过 + 0 驳回 + 2 剩余），驳回阈值 2
    // 仅 1 票驳回，不应触发退回
    let after_u2_reject = task_service
        .reject(
            &dctx,
            cs_task_id,
            &FlowParams {
                handler: Some("u2".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();
    assert_eq!(
        after_u2_reject.node_code, "cs",
        "1 票驳回不应触发退回（需 2 票，total 应含已通过人数）"
    );

    // cs 任务仍然存在
    let remaining_tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(remaining_tasks.len(), 1);
    assert_eq!(remaining_tasks[0].node_code, "cs");
}

// ---------------------------------------------------------------------------
// 并发判例
// ---------------------------------------------------------------------------

/// 判例:**后至者并发推进必须报错不脏写**。
/// 两个语境推进同一 task —— 先至者提交后,任务行已逻辑删,后至者的
/// load 守卫(select_by_id,漏斗软删接管)读不见任务行 → NotFound Err,
/// 写闭包不进入:任务状态未被二次改写,历史不追加,下一节点任务不多生。
/// (序列化执行模拟行锁串行化的竞争窗口。)
#[tokio::test]
async fn later_skip_on_advanced_task_errors() {
    let ds = setup_ds().await;
    let dctx = ds.system_context();
    let def_service = DefinitionService::new();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    let def_json = make_def_json(
        "test_later_skip",
        "后至者判例",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "apply", "提交")],
            ),
            make_node(
                NodeType::Between,
                "apply",
                "申请",
                Some("user1"),
                vec![make_skip("apply", "audit", "通过")],
            ),
            make_node(
                NodeType::Between,
                "audit",
                "审批",
                Some("user2"),
                vec![make_skip("audit", "end", "通过")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );
    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let instance = ins_service
        .start(
            &dctx,
            "biz_later_skip",
            &FlowParams {
                flow_code: Some("test_later_skip".to_string()),
                handler: Some("user0".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();
    assert_eq!(instance.node_code, "apply");

    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    let apply_task = tasks[0].id.unwrap();

    let pass = |h: &str| FlowParams {
        handler: Some(h.to_string()),
        skip_type: Some("PASS".to_string()),
        ..Default::default()
    };

    // 先至者推进 apply → audit,成功
    let advanced = task_service
        .skip(&dctx, apply_task, &pass("user1"), &test_ctx())
        .await
        .unwrap();
    assert_eq!(advanced.node_code, "audit");

    // 后至者对同一 task 再推进:任务已被逻辑删,必须 Err,不得脏写
    let later = task_service
        .skip(&dctx, apply_task, &pass("user1"), &test_ctx())
        .await;
    assert!(later.is_err(), "后至者推进同一任务必须报错,而不是二次改写");

    // 不脏写:任务集合仍是先至者产生的单个 audit 任务
    let tasks_after =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks_after.len(), 1, "后至者不得复活/多生任务");
    assert_eq!(tasks_after[0].node_code, "audit");
    // 历史不追加(发起 + 先至者推进各一条)
    let his =
        HisTaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(his.len(), 2, "后至者不得追加历史");
}

/// 判例:**办理人解析与任务写入同事务(回滚一致)**。
/// 生产形态里 skip 链的 flow_user(办理人解析产物)与 flow_task 在同一
/// `ctx.transaction` 闭包内落库 —— 任一写失败整笔回滚。这里以第二写的主键
/// 冲突模拟失败,断言先写入的任务行一并回滚、不落库。
#[tokio::test]
async fn resolver_output_and_task_write_rollback_together() {
    use app_data::{DataError, sea_orm::ActiveValue::Set};

    let ds = setup_ds().await;
    let ctx = ds.system_context();

    // FK 父行:definition 1 + 实例 1(task_user_repo_test 同款,SQLite
    // foreign_keys=ON,缺父行 insert 报 787)
    ctx.insert(flow_definition::ActiveModel {
        id: Set(1),
        flow_code: Set("fk".to_string()),
        flow_name: Set("fk".to_string()),
        model_value: Set("CLASSICS".to_string()),
        version: Set("1".to_string()),
        is_publish: Set(1),
        activity_status: Set(1),
        del_flag: Set(Some("0".to_string())),
        ..Default::default()
    })
    .await
    .unwrap();
    ctx.insert(flow_instance::ActiveModel {
        id: Set(1),
        definition_id: Set(1),
        business_id: Set("biz_fk".to_string()),
        node_type: Set(1),
        node_code: Set("seed".to_string()),
        flow_status: Set("1".to_string()),
        activity_status: Set(1),
        del_flag: Set(Some("0".to_string())),
        ..Default::default()
    })
    .await
    .unwrap();

    // 预置:任务 S + 办理人行 U(占用 U 的主键,制造第二写冲突)
    ctx.insert(flow_task::ActiveModel {
        id: Set(900_001),
        definition_id: Set(1),
        instance_id: Set(1),
        node_code: Set("seed".to_string()),
        node_name: Set(Some("seed".to_string())),
        node_type: Set(1),
        flow_status: Set("1".to_string()),
        form_custom: Set(None),
        form_path: Set(None),
        create_time: Set(None),
        create_by: Set(None),
        update_time: Set(None),
        update_by: Set(None),
        del_flag: Set(Some("0".to_string())),
        tenant_id: Set(None),
    })
    .await
    .unwrap();
    ctx.insert(flow_user::ActiveModel {
        id: Set(800_001),
        type_: Set("1".to_string()),
        processed_by: Set(Some("seed_user".to_string())),
        associated: Set(900_001),
        create_time: Set(None),
        create_by: Set(None),
        update_time: Set(None),
        update_by: Set(None),
        del_flag: Set(Some("0".to_string())),
        tenant_id: Set(None),
    })
    .await
    .unwrap();

    // 同事务:先写任务 T(成功),再写办理人行(主键冲突)→ 整笔回滚
    let result: Result<(), DataError> = ctx
        .transaction(|tx| {
            Box::pin(async move {
                tx.insert(flow_task::ActiveModel {
                    id: Set(900_002),
                    definition_id: Set(1),
                    instance_id: Set(1),
                    node_code: Set("victim".to_string()),
                    node_name: Set(Some("victim".to_string())),
                    node_type: Set(1),
                    flow_status: Set("1".to_string()),
                    form_custom: Set(None),
                    form_path: Set(None),
                    create_time: Set(None),
                    create_by: Set(None),
                    update_time: Set(None),
                    update_by: Set(None),
                    del_flag: Set(Some("0".to_string())),
                    tenant_id: Set(None),
                })
                .await?;
                // 办理人行主键撞预置行 → Backend(唯一冲突)→ 闭包 Err
                tx.insert(flow_user::ActiveModel {
                    id: Set(800_001),
                    type_: Set("1".to_string()),
                    processed_by: Set(Some("dup".to_string())),
                    associated: Set(900_002),
                    create_time: Set(None),
                    create_by: Set(None),
                    update_time: Set(None),
                    update_by: Set(None),
                    del_flag: Set(Some("0".to_string())),
                    tenant_id: Set(None),
                })
                .await?;
                Ok(())
            })
        })
        .await;

    assert!(result.is_err(), "办理人行写入失败必须使整笔事务失败");
    // 回滚一致:先写的任务行未落库
    let victim = TaskRepository::select_by_id(&ctx, 900_002).await.unwrap();
    assert!(victim.is_none(), "任务行必须随办理人行失败一并回滚");
}

// ---------------------------------------------------------------------------
// 协同(collaborate):发起人把任务转给协同人编组,全员通过才推进(会签语义)
// 对齐参考实现 needCoor/coorUserIds 业务模式的引擎原语
// ---------------------------------------------------------------------------

#[tokio::test]
async fn collaborate_group_must_all_pass_then_advance() {
    let ds = setup_ds().await;
    let dctx = ds.system_context();
    let def_service = DefinitionService::new();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    // Flow: start → approve(user1,单办理人无 ratio) → end
    let def_json = make_def_json(
        "test_collab",
        "协同流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "approve", "提交")],
            ),
            make_node(
                NodeType::Between,
                "approve",
                "审批",
                Some("user1"),
                vec![make_skip("approve", "end", "通过")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );
    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();

    let instance = ins_service
        .start(
            &dctx,
            "biz_collab_001",
            &FlowParams {
                flow_code: Some("test_collab".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();
    assert_eq!(instance.node_code, "approve");

    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    let task_id = tasks[0].id.unwrap();

    // user1 发起协同:协同人 c1, c2
    task_service
        .collaborate(
            &dctx,
            task_id,
            &FlowParams {
                handler: Some("user1".to_string()),
                add_handlers: Some(vec!["c1".to_string(), "c2".to_string()]),
                message: Some("请协同审核".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    // 办理人换成 c1/c2(UserType::Collaborate),user1 退出
    let users = UserRepository::select_by_task_id(&dctx, task_id)
        .await
        .unwrap();
    assert_eq!(users.len(), 2);
    assert!(
        users
            .iter()
            .all(|u| u.type_ == flow::enums::UserType::Collaborate as i8),
        "协同人用户行类型应为 Collaborate"
    );
    let handlers: Vec<&str> = users
        .iter()
        .filter_map(|u| u.processed_by.as_deref())
        .collect();
    assert!(handlers.contains(&"c1"));
    assert!(handlers.contains(&"c2"));
    assert!(!handlers.contains(&"user1"), "发起人应退出办理人列表");

    // his 有一条协同记录(cooperate_type = Collaborate)
    let his =
        HisTaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert!(
        his.iter().any(|h| h.cooperate_type
            == flow::enums::CooperateType::Collaborate as i8),
        "应有协同协作类型的历史记录"
    );

    let pass = |h: &str| FlowParams {
        handler: Some(h.to_string()),
        skip_type: Some("PASS".to_string()),
        ..Default::default()
    };

    // c1 通过:未全员通过,不推进
    let after_c1 = task_service
        .skip(&dctx, task_id, &pass("c1"), &test_ctx())
        .await
        .unwrap();
    assert_eq!(after_c1.node_code, "approve", "c1 通过后不应推进");
    let remaining = UserRepository::select_by_task_id(&dctx, task_id)
        .await
        .unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].processed_by.as_deref(), Some("c2"));

    // c2 通过:全员通过,推进到 end
    let fin = task_service
        .skip(&dctx, task_id, &pass("c2"), &test_ctx())
        .await
        .unwrap();
    assert_eq!(fin.node_code, "end", "协同人全员通过后应推进");
    assert_eq!(fin.flow_status, "8");
}

/// 非办理人发起协同 → Forbidden
#[tokio::test]
async fn collaborate_requires_current_handler() {
    let ds = setup_ds().await;
    let dctx = ds.system_context();
    let def_service = DefinitionService::new();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    let def_json = make_def_json(
        "test_collab_perm",
        "协同权限流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "approve", "提交")],
            ),
            make_node(
                NodeType::Between,
                "approve",
                "审批",
                Some("user1"),
                vec![make_skip("approve", "end", "通过")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );
    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();
    let instance = ins_service
        .start(
            &dctx,
            "biz_collab_002",
            &FlowParams {
                flow_code: Some("test_collab_perm".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();
    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    let task_id = tasks[0].id.unwrap();

    let result = task_service
        .collaborate(
            &dctx,
            task_id,
            &FlowParams {
                handler: Some("outsider".to_string()),
                add_handlers: Some(vec!["c1".to_string()]),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await;
    assert!(result.is_err(), "非办理人发起协同应被拒绝");
}

// ---------------------------------------------------------------------------
// 委派回溯:被委派人通过后,任务回到委托人终审(而非直接推进)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn depute_returns_to_delegator_for_final_review() {
    let ds = setup_ds().await;
    let dctx = ds.system_context();
    let def_service = DefinitionService::new();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    // Flow: start → approve(user1) → end
    let def_json = make_def_json(
        "test_depute_back",
        "委派回溯流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "approve", "提交")],
            ),
            make_node(
                NodeType::Between,
                "approve",
                "审批",
                Some("user1"),
                vec![make_skip("approve", "end", "通过")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );
    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();
    let instance = ins_service
        .start(
            &dctx,
            "biz_depute_001",
            &FlowParams {
                flow_code: Some("test_depute_back".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();
    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    let task_id = tasks[0].id.unwrap();

    // user1 委派给 user2
    task_service
        .depute(
            &dctx,
            task_id,
            &FlowParams {
                handler: Some("user1".to_string()),
                next_handler: Some(vec!["user2".to_string()]),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let users = UserRepository::select_by_task_id(&dctx, task_id)
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].processed_by.as_deref(), Some("user2"));
    assert_eq!(users[0].type_, flow::enums::UserType::Depute as i8);

    // user2(被委派人)通过 → 不推进,任务回到 user1 终审
    let pass = |h: &str| FlowParams {
        handler: Some(h.to_string()),
        skip_type: Some("PASS".to_string()),
        ..Default::default()
    };
    let after_delegatee = task_service
        .skip(&dctx, task_id, &pass("user2"), &test_ctx())
        .await
        .unwrap();
    assert_eq!(
        after_delegatee.node_code, "approve",
        "被委派人通过后应回到委托人终审,而非推进"
    );
    let users = UserRepository::select_by_task_id(&dctx, task_id)
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(
        users[0].processed_by.as_deref(),
        Some("user1"),
        "办理人应恢复为委托人"
    );
    assert_eq!(users[0].type_, flow::enums::UserType::Approval as i8);

    // user1 终审通过 → 推进到 end
    let fin = task_service
        .skip(&dctx, task_id, &pass("user1"), &test_ctx())
        .await
        .unwrap();
    assert_eq!(fin.node_code, "end");
    assert_eq!(fin.flow_status, "8");
}

// ---------------------------------------------------------------------------
// 弹窗选人(assigneeMap,对齐 Java 基线 CompleteTaskBo.assigneeMap):
// 办理时传 {下一节点编码: 用户ID},写 pass:{code}/back:{code} 变量,
// 建任务时覆盖办理人;pass: 消费一次即删,back: 保留供退回复用
// ---------------------------------------------------------------------------

/// 搭一个三段流程 start → A(user1) → B(user2) → C(user3) → end,
/// 返回 (dctx, task_service, instance_id, A 的 task_id)
async fn setup_abc_flow(
    flow_code: &str,
    business_id: &str,
) -> (app_data::DataSource, TaskService, i64, i64) {
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    let def_json = make_def_json(
        flow_code,
        "选人审批流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "A", "提交")],
            ),
            make_node(
                NodeType::Between,
                "A",
                "节点A",
                Some("user1"),
                vec![make_skip("A", "B", "通过")],
            ),
            make_node(
                NodeType::Between,
                "B",
                "节点B",
                Some("user2"),
                vec![make_skip("B", "C", "通过")],
            ),
            make_node(
                NodeType::Between,
                "C",
                "节点C",
                Some("user3"),
                vec![make_skip("C", "end", "通过")],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );
    let dctx = ds.system_context();
    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();
    let instance = ins_service
        .start(
            &dctx,
            business_id,
            &FlowParams {
                flow_code: Some(flow_code.to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();
    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    (ds, task_service, instance.id.unwrap(), tasks[0].id.unwrap())
}

fn pass_params(
    handler: &str,
    assignee_map: Option<HashMap<String, serde_json::Value>>,
) -> FlowParams {
    FlowParams {
        handler: Some(handler.to_string()),
        skip_type: Some("PASS".to_string()),
        assignee_map,
        ..Default::default()
    }
}

#[tokio::test]
async fn assignee_map_overrides_next_node_handlers() {
    let (ds, task_service, ins_id, task_a) =
        setup_abc_flow("test_assignee", "biz_am_001").await;
    let dctx = ds.system_context();

    // user1 办理 A,弹窗选 B 的办理人为 u7,u8
    let mut assignee_map = HashMap::new();
    assignee_map.insert("B".to_string(), serde_json::json!("u7,u8"));
    task_service
        .skip(
            &dctx,
            task_a,
            &pass_params("user1", Some(assignee_map)),
            &test_ctx(),
        )
        .await
        .unwrap();

    // B 任务办理人被覆盖为 u7,u8(而非节点预设的 user2)
    let tasks = TaskRepository::select_by_instance_id(&dctx, ins_id)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].node_code, "B");
    let users = UserRepository::select_by_task_id(&dctx, tasks[0].id.unwrap())
        .await
        .unwrap();
    let handlers: Vec<&str> = users
        .iter()
        .filter_map(|u| u.processed_by.as_deref())
        .collect();
    assert_eq!(handlers, vec!["u7", "u8"], "弹窗选人应覆盖节点预设办理人");

    // pass:B 消费一次即删;back:B 保留(供退回复用)
    let instance =
        flow::repository::instance_repo::InstanceRepository::select_by_id(
            &dctx, ins_id,
        )
        .await
        .unwrap()
        .unwrap();
    let var = instance.variable.unwrap_or_default();
    assert!(!var.contains("pass:B"), "pass: 变量应消费即删,实际 {var}");
    assert!(var.contains("back:B"), "back: 变量应保留,实际 {var}");
}

#[tokio::test]
async fn assignee_map_unions_with_existing_bare_variable() {
    let (ds, task_service, ins_id, task_a) =
        setup_abc_flow("test_assignee_union", "biz_am_002").await;
    let dctx = ds.system_context();

    // 实例存量变量含裸键 B="u9"(对齐 Java 与存量并集的语义)
    let mut params = pass_params("user1", None);
    params
        .variable
        .insert("B".to_string(), serde_json::json!("u9"));
    params.assignee_map = Some(HashMap::from([(
        "B".to_string(),
        serde_json::json!(["u7"]),
    )]));
    task_service
        .skip(&dctx, task_a, &params, &test_ctx())
        .await
        .unwrap();

    let tasks = TaskRepository::select_by_instance_id(&dctx, ins_id)
        .await
        .unwrap();
    let users = UserRepository::select_by_task_id(&dctx, tasks[0].id.unwrap())
        .await
        .unwrap();
    let mut handlers: Vec<&str> = users
        .iter()
        .filter_map(|u| u.processed_by.as_deref())
        .collect();
    handlers.sort_unstable();
    assert_eq!(handlers, vec!["u7", "u9"], "应与存量变量并集");
}

#[tokio::test]
async fn back_assignee_variable_applies_on_return() {
    let (ds, task_service, ins_id, task_a) =
        setup_abc_flow("test_assignee_back", "biz_am_003").await;
    let dctx = ds.system_context();

    // A 办理时指定 B 的办理人 u7
    let mut assignee_map = HashMap::new();
    assignee_map.insert("B".to_string(), serde_json::json!("u7"));
    task_service
        .skip(
            &dctx,
            task_a,
            &pass_params("user1", Some(assignee_map)),
            &test_ctx(),
        )
        .await
        .unwrap();

    // u7 办理 B → C
    let tasks = TaskRepository::select_by_instance_id(&dctx, ins_id)
        .await
        .unwrap();
    let task_b = tasks[0].id.unwrap();
    task_service
        .skip(&dctx, task_b, &pass_params("u7", None), &test_ctx())
        .await
        .unwrap();
    let tasks = TaskRepository::select_by_instance_id(&dctx, ins_id)
        .await
        .unwrap();
    assert_eq!(tasks[0].node_code, "C");

    // C 退回到 B:back:B 变量生效,办理人仍是 u7 而非预设 user2
    let ctx3 = OperatorContext {
        user_id: "user3".to_string(),
        tenant_id: None,
    };
    task_service
        .back_process(&dctx, tasks[0].id.unwrap(), "B", None, &ctx3)
        .await
        .unwrap();
    let tasks = TaskRepository::select_by_instance_id(&dctx, ins_id)
        .await
        .unwrap();
    assert_eq!(tasks[0].node_code, "B");
    let users = UserRepository::select_by_task_id(&dctx, tasks[0].id.unwrap())
        .await
        .unwrap();
    let handlers: Vec<&str> = users
        .iter()
        .filter_map(|u| u.processed_by.as_deref())
        .collect();
    assert_eq!(handlers, vec!["u7"], "退回应复用 back: 变量指定的人");
}

// ---------------------------------------------------------------------------
// 监听器声明式参数(name({json}))+ 变量回灌:
// 监听器对 ListenerVariable.variable 的修改必须写回实例
// (修复克隆回传被丢弃的缺口;对齐参考实现 AeRepNodeStartListener 的
// resetVariables 声明)
// ---------------------------------------------------------------------------

/// create 事件时按 params.resetVariables 列出的 key 删除流程变量
struct VarResetListener;

#[async_trait]
impl Listener for VarResetListener {
    async fn notify(
        &self,
        _listener_type: &str,
        params: Option<&serde_json::Value>,
        var: &mut flow::listener::types::ListenerVariable,
    ) -> app_core::error::AppResult<()> {
        if let Some(keys) = params
            .and_then(|p| p.get("resetVariables"))
            .and_then(|v| v.as_array())
        {
            for k in keys.iter().filter_map(|v| v.as_str()) {
                var.variable.remove(k);
            }
        }
        Ok(())
    }
}

#[tokio::test]
async fn listener_variable_mutation_persists_to_instance() {
    let ds = setup_ds().await;
    let dctx = ds.system_context();
    let def_service = DefinitionService::new();

    let mut registry = ListenerRegistry::new();
    registry.register("var_reset", Arc::new(VarResetListener));
    let registry = Arc::new(registry);

    let ins_service = InstanceService::new(
        registry.clone(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        registry,
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    // start → A(user1) → B(user2, create 监听 var_reset 带参) → end
    let mut node_b = make_node(
        NodeType::Between,
        "B",
        "节点B",
        Some("user2"),
        vec![make_skip("B", "end", "通过")],
    );
    node_b.listener_type = Some("create".to_string());
    node_b.listener_path =
        Some(r#"var_reset({"resetVariables":["needCoor"]})"#.to_string());

    let def_json = make_def_json(
        "test_var_reset",
        "变量重置流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "A", "提交")],
            ),
            make_node(
                NodeType::Between,
                "A",
                "节点A",
                Some("user1"),
                vec![make_skip("A", "B", "通过")],
            ),
            node_b,
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );
    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();
    let instance = ins_service
        .start(
            &dctx,
            "biz_var_reset",
            &FlowParams {
                flow_code: Some("test_var_reset".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    // 办理时注入 needCoor=Y;create 监听器应把它删掉并回灌到实例
    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    task_service
        .skip(
            &dctx,
            tasks[0].id.unwrap(),
            &FlowParams {
                handler: Some("user1".to_string()),
                skip_type: Some("PASS".to_string()),
                variable: HashMap::from([(
                    "needCoor".to_string(),
                    serde_json::json!("Y"),
                )]),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let instance =
        flow::repository::instance_repo::InstanceRepository::select_by_id(
            &dctx,
            instance.id.unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    let var = instance.variable.unwrap_or_default();
    assert!(
        !var.contains("needCoor"),
        "监听器删除的变量应回灌到实例,实际 {var}"
    );
}

// ---------------------------------------------------------------------------
// 监听器改写下一任务办理人(next_tasks 回灌):
// assignment/create 监听器对 lv.next_tasks[i].permission_list 的修改
// 必须落到实际创建的 flow_user(修复克隆回传被丢弃的缺口)
// ---------------------------------------------------------------------------

/// assignment 事件时把下一任务办理人改写为 u9
struct ReassignListener;

#[async_trait]
impl Listener for ReassignListener {
    async fn notify(
        &self,
        listener_type: &str,
        _params: Option<&serde_json::Value>,
        var: &mut flow::listener::types::ListenerVariable,
    ) -> app_core::error::AppResult<()> {
        if listener_type == "assignment" {
            for t in &mut var.next_tasks {
                t.permission_list = vec!["u9".to_string()];
            }
        }
        Ok(())
    }
}

#[tokio::test]
async fn listener_next_tasks_permission_override_persists() {
    let ds = setup_ds().await;
    let dctx = ds.system_context();
    let def_service = DefinitionService::new();

    let mut registry = ListenerRegistry::new();
    registry.register("reassign", Arc::new(ReassignListener));
    let registry = Arc::new(registry);

    let ins_service = InstanceService::new(
        registry.clone(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        registry,
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    // start → A(user1) → B(user2, assignment 监听 reassign) → end
    let mut node_b = make_node(
        NodeType::Between,
        "B",
        "节点B",
        Some("user2"),
        vec![make_skip("B", "end", "通过")],
    );
    node_b.listener_type = Some("assignment".to_string());
    node_b.listener_path = Some("reassign".to_string());

    let def_json = make_def_json(
        "test_reassign",
        "监听器改办理人流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "A", "提交")],
            ),
            make_node(
                NodeType::Between,
                "A",
                "节点A",
                Some("user1"),
                vec![make_skip("A", "B", "通过")],
            ),
            node_b,
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );
    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();
    let instance = ins_service
        .start(
            &dctx,
            "biz_reassign",
            &FlowParams {
                flow_code: Some("test_reassign".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();
    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks[0].node_code, "A");

    // user1 办理 A → 推进到 B;B 的 assignment 监听器把办理人改写为 u9
    task_service
        .skip(
            &dctx,
            tasks[0].id.unwrap(),
            &FlowParams {
                handler: Some("user1".to_string()),
                skip_type: Some("PASS".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].node_code, "B");
    let users = UserRepository::select_by_task_id(&dctx, tasks[0].id.unwrap())
        .await
        .unwrap();
    let handlers: Vec<&str> = users
        .iter()
        .filter_map(|u| u.processed_by.as_deref())
        .collect();
    assert_eq!(
        handlers,
        vec!["u9"],
        "监听器对 next_tasks 办理人的改写应落库(而非仍用节点预设 user2)"
    );
}

// ---------------------------------------------------------------------------
// TODO(#3) 复现:包容网关出边中结束节点不在末尾时,监听器 node/task 配对错位
// (add_tasks 移除结束节点任务后变短,按位置 zip/索引会把任务配给错误的节点)
// ---------------------------------------------------------------------------

/// 记录 (node_code, task.node_code) 配对的监听器
struct PairRecorder {
    hits: Arc<Mutex<Vec<(String, String)>>>,
}

#[async_trait]
impl Listener for PairRecorder {
    async fn notify(
        &self,
        _listener_type: &str,
        _params: Option<&serde_json::Value>,
        var: &mut flow::listener::types::ListenerVariable,
    ) -> app_core::error::AppResult<()> {
        if let (Some(n), Some(t)) = (&var.node, &var.task) {
            self.hits
                .lock()
                .unwrap()
                .push((n.node_code.clone(), t.node_code.clone()));
        }
        Ok(())
    }
}

/// start → A(user1) → G(包容网关) → [end(skip 存储在前), B(user2)]
/// B 挂 create 监听器;修复前 add_tasks=[taskB] 与 next_nodes=[end,B]
/// 按位置配对成 (taskB, end),B 的监听器永不触发
#[tokio::test]
async fn listener_pairs_correctly_when_end_node_not_last() {
    let ds = setup_ds().await;
    let dctx = ds.system_context();
    let def_service = DefinitionService::new();

    let hits = Arc::new(Mutex::new(Vec::new()));
    let mut registry = ListenerRegistry::new();
    registry
        .register("pair_rec", Arc::new(PairRecorder { hits: hits.clone() }));
    let registry = Arc::new(registry);

    let ins_service = InstanceService::new(
        registry.clone(),
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );
    let task_service = TaskService::new(
        registry,
        default_permission(),
        default_resolver(),
        default_callback(),
        default_notifier(),
    );

    // 包容网关 G 的两条出边:end 在前,B 在后(错位触发条件)
    let mut node_g =
        make_node(NodeType::Inclusive, "G", "包容网关", None, vec![]);
    node_g.skip_list = vec![
        make_skip("G", "end", "结束分支"),
        make_skip("G", "B", "复审分支"),
    ];
    let mut node_b = make_node(
        NodeType::Between,
        "B",
        "复审",
        Some("user2"),
        vec![make_skip("B", "end", "通过")],
    );
    node_b.listener_type = Some("create".to_string());
    node_b.listener_path = Some("pair_rec".to_string());

    let def_json = make_def_json(
        "test_zip_misalign",
        "错位配对流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "A", "提交")],
            ),
            make_node(
                NodeType::Between,
                "A",
                "节点A",
                Some("user1"),
                vec![make_skip("A", "G", "通过")],
            ),
            node_g,
            node_b,
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    );
    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();
    let instance = ins_service
        .start(
            &dctx,
            "biz_zip_001",
            &FlowParams {
                flow_code: Some("test_zip_misalign".to_string()),
                handler: Some("submitter".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();
    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks[0].node_code, "A");
    hits.lock().unwrap().clear();

    // user1 办理 A → 包容网关展开 [end, B],只为 B 建任务
    task_service
        .skip(
            &dctx,
            tasks[0].id.unwrap(),
            &FlowParams {
                handler: Some("user1".to_string()),
                skip_type: Some("PASS".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let recorded = hits.lock().unwrap().clone();
    assert!(
        recorded.contains(&("B".to_string(), "B".to_string())),
        "B 的 create 监听器应拿到 B 自己的任务,实际记录 {recorded:?}"
    );

    // 注:包容网关含结束分支时实例直接判完成,B 的任务随后被
    // hand_undone_task 清理(对齐 Java setInsFinishInfo 结束节点优先
    // 语义)——本测试只锚监听器配对,不锚任务存活
    let instance =
        flow::repository::instance_repo::InstanceRepository::select_by_id(
            &dctx,
            instance.id.unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(instance.flow_status, "8", "含结束分支应判流程完成");
}

// ---------------------------------------------------------------------------
// D5: 新待办触发 FlowNotifier(启动/推进/退回,事务提交成功后尽力而为推送)
// ---------------------------------------------------------------------------

/// 通知流程:start -> n1(101) -> n2(102) -> end,n2 带退回边。
fn notify_def_json() -> DefJson {
    make_def_json(
        "test_notify",
        "通知流程",
        vec![
            make_node(
                NodeType::Start,
                "start",
                "开始",
                None,
                vec![make_skip("start", "n1", "提交")],
            ),
            make_node(
                NodeType::Between,
                "n1",
                "部门审批",
                Some("101"),
                vec![make_skip("n1", "n2", "通过")],
            ),
            make_node(
                NodeType::Between,
                "n2",
                "总经理审批",
                Some("102"),
                vec![
                    make_skip("n2", "end", "通过"),
                    make_reject_skip("n2", "n1", "退回"),
                ],
            ),
            make_node(NodeType::End, "end", "结束", None, vec![]),
        ],
    )
}

async fn start_notify_flow(
    ds: &app_data::DataSource,
    def_service: &DefinitionService,
    ins_service: &InstanceService,
) -> flow::model::instance::Instance {
    let dctx = ds.system_context();
    let def_json = notify_def_json();
    let def = def_service
        .save_def(&dctx, &def_json, &test_ctx())
        .await
        .unwrap();
    def_service
        .publish(&dctx, def.id.unwrap(), &test_ctx())
        .await
        .unwrap();
    ins_service
        .start(
            &dctx,
            "biz_notify_001",
            &FlowParams {
                flow_code: Some("test_notify".to_string()),
                handler: Some("100".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn new_todo_triggers_flow_notifier() {
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let notifier = RecordingNotifier::default();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        Arc::new(notifier.clone()),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        Arc::new(notifier.clone()),
    );

    let instance = start_notify_flow(&ds, &def_service, &ins_service).await;

    // 启动:首节点办理人收到一次通知,消息含节点名
    {
        let calls = notifier.calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "启动后首节点办理人应收到一次新待办通知");
        assert_eq!(calls[0].0, vec![101]);
        assert!(
            calls[0].1.contains("部门审批"),
            "消息应含节点名:{}",
            calls[0].1
        );
    }

    // complete 推进:下一节点办理人收到通知
    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(tasks.len(), 1);
    task_service
        .skip(
            &dctx,
            tasks[0].id.unwrap(),
            &FlowParams {
                handler: Some("101".to_string()),
                skip_type: Some("PASS".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let calls = notifier.calls.lock().unwrap();
    assert_eq!(calls.len(), 2, "推进后下一节点办理人应再收到一次通知");
    assert_eq!(calls[1].0, vec![102]);
    assert!(
        calls[1].1.contains("总经理审批"),
        "消息应含节点名:{}",
        calls[1].1
    );
}

#[tokio::test]
async fn reject_new_todo_triggers_flow_notifier() {
    let ds = setup_ds().await;
    let def_service = DefinitionService::new();
    let dctx = ds.system_context();
    let notifier = RecordingNotifier::default();
    let ins_service = InstanceService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        Arc::new(notifier.clone()),
    );
    let task_service = TaskService::new(
        default_registry(),
        default_permission(),
        default_resolver(),
        default_callback(),
        Arc::new(notifier.clone()),
    );

    let instance = start_notify_flow(&ds, &def_service, &ins_service).await;

    // 推进到 n2
    let tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    task_service
        .skip(
            &dctx,
            tasks[0].id.unwrap(),
            &FlowParams {
                handler: Some("101".to_string()),
                skip_type: Some("PASS".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();
    assert_eq!(notifier.calls.lock().unwrap().len(), 2);

    // 从 n2 退回到 n1:退回目标节点办理人收到通知
    let n2_tasks =
        TaskRepository::select_by_instance_id(&dctx, instance.id.unwrap())
            .await
            .unwrap();
    assert_eq!(n2_tasks.len(), 1);
    assert_eq!(n2_tasks[0].node_code, "n2");
    task_service
        .reject(
            &dctx,
            n2_tasks[0].id.unwrap(),
            &FlowParams {
                handler: Some("102".to_string()),
                skip_type: Some("REJECT".to_string()),
                ..Default::default()
            },
            &test_ctx(),
        )
        .await
        .unwrap();

    let calls = notifier.calls.lock().unwrap();
    assert_eq!(calls.len(), 3, "退回后目标节点办理人应收到一次通知");
    assert_eq!(calls[2].0, vec![101]);
    assert!(
        calls[2].1.contains("部门审批"),
        "消息应含节点名:{}",
        calls[2].1
    );
}
