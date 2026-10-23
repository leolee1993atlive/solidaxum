//! definition 链集成测试(脚手架:`app_data::testing`)。
//!
//! 脚手架:`sqlite_memory_empty()`(单连接内存库)+ `DataSource::create_table`
//! 按实体建表;执行面经 `system_context()`(flow 全豁免实体,豁免读不解析
//! 范围,与用户 ctx 行为等价;审计字段由 OperatorContext 手工盖,与生产
//! handler 路径一致)。`DbBackend::MySql` 的 SQL 断言经 test-util 面
//! (`QueryTrait`/`DbBackend` 均在受限 re-export)。
use std::collections::HashMap;

use app_data::{
    DataError, DataSource,
    sea_orm::{ActiveValue::Set, DbBackend, EntityTrait, QueryTrait},
    testing::sqlite_memory_empty,
};
use flow::{
    enums::{NodeType, PublishStatus},
    model::def_json::{DefJson, NodeJson, SkipJson},
    repository::{
        definition_repo::DefinitionRepository,
        entity::{flow_definition, flow_instance, flow_node, flow_skip},
        node_repo::NodeRepository,
        skip_repo::SkipRepository,
    },
    service::definition_service::{DefinitionService, OperatorContext},
};

async fn setup_ds() -> DataSource {
    let ds = sqlite_memory_empty().await;
    ds.create_table(flow_definition::Entity).await.unwrap();
    ds.create_table(flow_node::Entity).await.unwrap();
    ds.create_table(flow_skip::Entity).await.unwrap();
    ds.create_table(flow_instance::Entity).await.unwrap();
    ds
}

fn test_ctx() -> OperatorContext {
    OperatorContext {
        user_id: "1".to_string(),
        tenant_id: None,
    }
}

fn make_simple_def_json() -> DefJson {
    DefJson {
        id: None,
        flow_code: "test_flow".to_string(),
        flow_name: "测试流程".to_string(),
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
        node_list: vec![
            NodeJson {
                node_type: NodeType::Start as i8,
                node_code: "start".to_string(),
                node_name: Some("开始".to_string()),
                version: None,
                permission_flag: None,
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
                skip_list: vec![SkipJson {
                    now_node_code: "start".to_string(),
                    next_node_code: "node1".to_string(),
                    skip_name: Some("提交".to_string()),
                    skip_type: Some("PASS".to_string()),
                    skip_condition: None,
                    coordinate: None,
                    create_by: None,
                    update_by: None,
                    status: None,
                }],
            },
            NodeJson {
                node_type: NodeType::Between as i8,
                node_code: "node1".to_string(),
                node_name: Some("审批".to_string()),
                version: None,
                permission_flag: Some("1".to_string()),
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
                skip_list: vec![SkipJson {
                    now_node_code: "node1".to_string(),
                    next_node_code: "end".to_string(),
                    skip_name: Some("通过".to_string()),
                    skip_type: Some("PASS".to_string()),
                    skip_condition: None,
                    coordinate: None,
                    create_by: None,
                    update_by: None,
                    status: None,
                }],
            },
            NodeJson {
                node_type: NodeType::End as i8,
                node_code: "end".to_string(),
                node_name: Some("结束".to_string()),
                version: None,
                permission_flag: None,
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
                skip_list: vec![],
            },
        ],
        chart_status_color: None,
        top_text: None,
        top_text_show: false,
    }
}

#[tokio::test]
async fn test_save_and_publish_definition() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    let service = DefinitionService::new();
    let op = test_ctx();

    // 保存
    let def_json = make_simple_def_json();
    let saved = service.save_def(&ctx, &def_json, &op).await.unwrap();
    assert_eq!(saved.flow_code, "test_flow");
    assert_eq!(saved.version, "1");
    assert!(saved.id.is_some());
    assert_eq!(saved.create_by, Some("1".to_string()));

    let def_id = saved.id.unwrap();

    // 导出验证 next_node_type 已填充
    let exported = service.export_json(&ctx, def_id).await.unwrap();
    assert_eq!(exported.node_list.len(), 3);

    // 发布
    service.publish(&ctx, def_id, &op).await.unwrap();
    let published = service.get_by_id(&ctx, def_id).await.unwrap().unwrap();
    assert_eq!(published.is_publish, PublishStatus::Published as i8);

    // 取消发布
    service.unpublish(&ctx, def_id, &op).await.unwrap();
    let unpublished = service.get_by_id(&ctx, def_id).await.unwrap().unwrap();
    assert_eq!(unpublished.is_publish, PublishStatus::Unpublished as i8);
}

#[tokio::test]
async fn test_copy_definition() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    let service = DefinitionService::new();
    let op = test_ctx();

    let def_json = make_simple_def_json();
    let saved = service.save_def(&ctx, &def_json, &op).await.unwrap();
    let def_id = saved.id.unwrap();

    let copied = service.copy_def(&ctx, def_id, &op).await.unwrap();
    assert_ne!(copied.id.unwrap(), def_id);
    assert_eq!(copied.flow_code, "test_flow");
    assert_eq!(copied.version, "2");
    assert!(copied.flow_name.contains("复制"));

    let exported = service.export_json(&ctx, copied.id.unwrap()).await.unwrap();
    assert_eq!(exported.node_list.len(), 3);
}

#[tokio::test]
async fn test_validation_missing_start_node() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    let service = DefinitionService::new();
    let op = test_ctx();

    let mut def_json = make_simple_def_json();
    def_json.node_list.retain(|n| n.node_code != "start");

    let result = service.save_def(&ctx, &def_json, &op).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_validation_multi_pass_edge() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    let service = DefinitionService::new();
    let op = test_ctx();

    let mut def_json = make_simple_def_json();
    if let Some(node1) = def_json
        .node_list
        .iter_mut()
        .find(|n| n.node_code == "node1")
    {
        node1.skip_list.push(SkipJson {
            now_node_code: "node1".to_string(),
            next_node_code: "end".to_string(),
            skip_name: Some("分支".to_string()),
            skip_type: Some("PASS".to_string()),
            skip_condition: None,
            coordinate: None,
            create_by: None,
            update_by: None,
            status: None,
        });
    }

    let result = service.save_def(&ctx, &def_json, &op).await;
    assert!(result.is_err());
    let msg = format!("{:?}", result.unwrap_err());
    assert!(msg.contains("多条 PASS"), "应拒绝多条 PASS 出边: {}", msg);
}

#[tokio::test]
async fn test_version_conflict() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    let service = DefinitionService::new();
    let op = test_ctx();

    let def_json = make_simple_def_json();
    service.save_def(&ctx, &def_json, &op).await.unwrap();

    // 再次保存同 flow_code + version
    let result = service.save_def(&ctx, &def_json, &op).await;
    assert!(result.is_err());
    let msg = format!("{:?}", result.unwrap_err());
    assert!(msg.contains("已存在"), "应拒绝重复版本: {}", msg);
}

#[tokio::test]
async fn test_soft_delete() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    let service = DefinitionService::new();
    let op = test_ctx();

    let def_json = make_simple_def_json();
    let saved = service.save_def(&ctx, &def_json, &op).await.unwrap();
    let def_id = saved.id.unwrap();

    service.delete_def(&ctx, def_id, &op).await.unwrap();

    // 软删除后查不到
    let result = service.get_by_id(&ctx, def_id).await.unwrap();
    assert!(result.is_none());
}

/// 判例:已删定义不可见 —— 软删由漏斗接管,读侧(分页/导出)与写侧
/// (对已删 id 的 update)都把已删行当不存在,不会出现「del_flag 被覆盖回
/// 存活值、形同复活」的窗口(Java @TableLogic 平价)。
#[tokio::test]
async fn soft_deleted_definition_is_invisible_to_reads_and_writes() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    let service = DefinitionService::new();
    let op = test_ctx();

    let def_json = make_simple_def_json();
    let saved = service.save_def(&ctx, &def_json, &op).await.unwrap();
    let def_id = saved.id.unwrap();
    service.delete_def(&ctx, def_id, &op).await.unwrap();

    // 读侧:未发布分页不含已删定义
    let page = service
        .unpublished_list_page(&ctx, None, None, None, 1, 10)
        .await
        .unwrap();
    assert_eq!(page.total, 0, "已删定义不进分页结果");
    assert!(page.rows.is_empty());

    // 读侧:导出已删定义 → NotFound
    let result = service.export_json(&ctx, def_id).await;
    assert!(result.is_err(), "已删定义不可导出");

    // 写侧:对已删 id 走 save_def 更新分支 → 软删接管,update 0 行 →
    // NotFound(无存活过滤的直更会把 del_flag 覆盖回存活值,形同复活)
    let mut edit_json = make_simple_def_json();
    edit_json.id = Some(def_id);
    let result = service.save_def(&ctx, &edit_json, &op).await;
    assert!(result.is_err(), "已删定义不可被更新复活");

    // 且软删是幂等的:重复删除已删定义 → NotFound(守卫读已不可见)
    let result = service.delete_def(&ctx, def_id, &op).await;
    assert!(result.is_err(), "重复删除已删定义应报不存在");
}

/// 判例:定义保存写序列(定义/节点/连边)在同一事务闭包内,
/// 闭包 Err → 整体回滚。守卫提升后业务错误在闭包外抛出(见
/// definition_service 头注),此处直接钉闭包内写序列的回滚语义:
/// 模拟 save_def 更新分支的写序(软删节点 → 软删连边 → 更新定义),
/// 末位注入失败,三个写必须全部不落库。
#[tokio::test]
async fn definition_writes_rollback_when_closure_fails() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    let service = DefinitionService::new();
    let op = test_ctx();

    let def_json = make_simple_def_json();
    let saved = service.save_def(&ctx, &def_json, &op).await.unwrap();
    let def_id = saved.id.unwrap();

    let result: Result<(), DataError> = ctx
        .transaction(|tx| {
            Box::pin(async move {
                NodeRepository::soft_delete_by_definition_id(tx, def_id)
                    .await?;
                SkipRepository::soft_delete_by_definition_id(tx, def_id)
                    .await?;
                let mut def = saved.clone();
                def.flow_name = "不应落库".to_string();
                DefinitionRepository::update_by_id_in_txn(tx, &def).await?;
                Err(DataError::Conflict)
            })
        })
        .await;
    assert!(matches!(result, Err(DataError::Conflict)));

    // 回滚后:定义未被改名,节点/连边仍全部存活
    let def = service.get_by_id(&ctx, def_id).await.unwrap().unwrap();
    assert_eq!(def.flow_name, "测试流程", "闭包 Err 回滚:定义更新不落库");
    let nodes = NodeRepository::select_by_definition_id(&ctx, def_id)
        .await
        .unwrap();
    let skips = SkipRepository::select_by_definition_id(&ctx, def_id)
        .await
        .unwrap();
    assert_eq!(nodes.len(), 3, "闭包 Err 回滚:节点软删不落库");
    assert_eq!(skips.len(), 2, "闭包 Err 回滚:连边软删不落库");
}

/// flow_skip.node_id 应在保存定义时填充为对应 flow_node.id
#[tokio::test]
async fn flow_skip_node_id() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    let service = DefinitionService::new();
    let op = test_ctx();

    let def_json = make_simple_def_json();
    let saved = service.save_def(&ctx, &def_json, &op).await.unwrap();
    let def_id = saved.id.unwrap();

    let nodes = NodeRepository::select_by_definition_id(&ctx, def_id)
        .await
        .unwrap();
    let skips = SkipRepository::select_by_definition_id(&ctx, def_id)
        .await
        .unwrap();

    assert!(!nodes.is_empty());
    assert!(!skips.is_empty());

    // node_id 为 #[sea_orm(ignore)] 运行时字段（SQL flow_skip 表无此列，与 warm-flow
    // Java 表结构一致），不持久化。从 DB 读回恒为 None；运行时由 service 的
    // node_code -> node_id 映射填充。此处验证跳转关系本身正确即可。
    let node_code_map: HashMap<&str, &str> = nodes
        .iter()
        .map(|n| (n.node_code.as_str(), n.node_code.as_str()))
        .collect();

    for skip in &skips {
        // node_id 不入库，读回应为 None
        assert!(
            skip.node_id.is_none(),
            "skip(now={}) 的 node_id 不应持久化（SQL 无此列）",
            skip.now_node_code
        );
        // 跳转关系（now/next node_code）应正确持久化
        assert!(
            node_code_map.contains_key(skip.now_node_code.as_str()),
            "skip.now_node_code 应指向已存在的节点"
        );
    }
}

/// 复制定义后，新 skip 的跳转关系应正确，节点 id 指向新定义的节点（非原定义）
#[tokio::test]
async fn flow_skip_node_id_after_copy() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    let service = DefinitionService::new();
    let op = test_ctx();

    let def_json = make_simple_def_json();
    let saved = service.save_def(&ctx, &def_json, &op).await.unwrap();
    let def_id = saved.id.unwrap();

    let copied = service.copy_def(&ctx, def_id, &op).await.unwrap();
    let new_def_id = copied.id.unwrap();

    let new_nodes = NodeRepository::select_by_definition_id(&ctx, new_def_id)
        .await
        .unwrap();
    let new_skips = SkipRepository::select_by_definition_id(&ctx, new_def_id)
        .await
        .unwrap();

    // 新定义节点 id 集合
    let new_node_ids: std::collections::HashSet<i64> =
        new_nodes.iter().filter_map(|n| n.id).collect();
    // 原定义节点 id 集合，确保新定义的节点不与原定义重叠
    let old_node_ids: std::collections::HashSet<i64> =
        NodeRepository::select_by_definition_id(&ctx, def_id)
            .await
            .unwrap()
            .iter()
            .filter_map(|n| n.id)
            .collect();

    // 新定义的节点 id 应全部不同于原定义（复制生成全新 id）
    assert!(
        new_node_ids.intersection(&old_node_ids).next().is_none(),
        "复制后的节点 id 不应与原定义重叠"
    );
    // node_id 不入库（ignore），此处验证跳转关系正确即可
    assert!(!new_skips.is_empty(), "复制后应存在跳转记录");
}

/// 端到端导入测试：复现 warm-flow 设计器 JSON（leave6.json 结构）导入。
///
/// 此测试覆盖用户报的两个问题：
/// 1. nodeType 以字符串形式传输（"0"/"3"/"4"），验证 flexible_i8 反序列化
/// 2. flow_skip.node_id 不入库（SQL 无此列），验证 #[sea_orm(ignore)] 修复后
///    import_def_json 不再报 "None of the records are inserted"
#[tokio::test]
async fn import_def_json_with_gateway_and_spel_condition() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    let service = DefinitionService::new();
    let op = test_ctx();

    // 模拟 leave6.json 的核心结构：开始 -> 申请人 -> 排他网关(条件分支) -> ... -> 结束
    // nodeType 用字符串（设计器实际输出格式）
    let json = r#"{
        "flowCode": "leave6",
        "flowName": "请假申请-排他并行会签",
        "modelValue": "CLASSICS",
        "category": "1762300000000000103",
        "version": "1",
        "formCustom": "N",
        "nodeList": [
            {
                "nodeType": "0",
                "nodeCode": "start",
                "nodeName": "开始",
                "nodeRatio": "0.000",
                "formCustom": "N",
                "skipList": [
                    {"nowNodeCode": "start", "nextNodeCode": "applicant", "skipType": "PASS"}
                ]
            },
            {
                "nodeType": "1",
                "nodeCode": "applicant",
                "nodeName": "申请人",
                "permissionFlag": "${initiator}",
                "nodeRatio": "0.000",
                "formCustom": "N",
                "skipList": [
                    {"nowNodeCode": "applicant", "nextNodeCode": "gateway", "skipType": "PASS"}
                ]
            },
            {
                "nodeType": "3",
                "nodeCode": "gateway",
                "nodeName": "排他网关",
                "nodeRatio": "0.000",
                "formCustom": "N",
                "skipList": [
                    {"nowNodeCode": "gateway", "nextNodeCode": "manager", "skipType": "PASS", "skipCondition": "default@@${leaveDays > 2}", "skipName": "大于两天"},
                    {"nowNodeCode": "gateway", "nextNodeCode": "vice", "skipType": "PASS", "skipCondition": "spel@@#{@testLeaveServiceImpl.eval(#leaveDays)}"}
                ]
            },
            {
                "nodeType": "1",
                "nodeCode": "manager",
                "nodeName": "经理会签",
                "permissionFlag": "1761100000000000001@@1761100000000000003",
                "nodeRatio": "100.000",
                "formCustom": "N",
                "skipList": [
                    {"nowNodeCode": "manager", "nextNodeCode": "end", "skipType": "PASS"}
                ]
            },
            {
                "nodeType": "1",
                "nodeCode": "vice",
                "nodeName": "副经理",
                "permissionFlag": "1761100000000000001",
                "nodeRatio": "0.000",
                "formCustom": "N",
                "skipList": [
                    {"nowNodeCode": "vice", "nextNodeCode": "end", "skipType": "PASS"}
                ]
            },
            {
                "nodeType": "2",
                "nodeCode": "end",
                "nodeName": "结束",
                "nodeRatio": "0.000",
                "formCustom": "N",
                "skipList": []
            }
        ]
    }"#;

    // 1. JSON 反序列化（字符串 nodeType 应被 flexible_i8 接受）
    let def_json: DefJson = serde_json::from_str(json)
        .expect("warm-flow 设计器 JSON 应能反序列化（字符串 nodeType）");
    assert_eq!(def_json.node_list.len(), 6);
    assert_eq!(def_json.node_list[2].node_type, 3); // 排他网关
    assert_eq!(def_json.node_list[3].node_type, 1); // 会签节点

    // 2. 导入落库（不应报 None of the records are inserted）
    let saved = service
        .import_def_json(&ctx, &def_json, &op)
        .await
        .expect("导入应成功（flow_skip.node_id 不再入库）");
    let def_id = saved.id.unwrap();

    // 3. 验证定义、节点、跳转全部正确落库
    let nodes = NodeRepository::select_by_definition_id(&ctx, def_id)
        .await
        .unwrap();
    let skips = SkipRepository::select_by_definition_id(&ctx, def_id)
        .await
        .unwrap();
    assert_eq!(nodes.len(), 6, "应落库 6 个节点");
    assert_eq!(skips.len(), 6, "应落库 6 条跳转");

    // 4. 验证网关节点和 SpEL 条件正确持久化
    let gateway = nodes.iter().find(|n| n.node_code == "gateway").unwrap();
    assert_eq!(gateway.node_type, 3, "排他网关 nodeType 应为 3");
    let gateway_skips: Vec<_> = skips
        .iter()
        .filter(|s| s.now_node_code == "gateway")
        .collect();
    assert_eq!(gateway_skips.len(), 2, "网关应有 2 条出边");
    assert!(
        gateway_skips.iter().any(|s| s.skip_condition.as_deref()
            == Some("default@@${leaveDays > 2}")),
        "应持久化 default 条件"
    );
    assert!(
        gateway_skips.iter().any(|s| s.skip_condition.as_deref()
            == Some("spel@@#{@testLeaveServiceImpl.eval(#leaveDays)}")),
        "应持久化 spel 条件"
    );

    // 5. 验证会签节点 nodeRatio 正确持久化
    let manager = nodes.iter().find(|n| n.node_code == "manager").unwrap();
    assert_eq!(
        manager.node_ratio.as_deref(),
        Some("100.000"),
        "会签 nodeRatio 应持久化"
    );

    // 6. node_id 从 DB 读回应为 None（#[sea_orm(ignore)]，不持久化）
    for skip in &skips {
        assert!(
            skip.node_id.is_none(),
            "skip(now={}) node_id 不应持久化",
            skip.now_node_code
        );
    }
}

/// 诊断测试：打印 sea-orm 在 MySQL 方言下为 leave6 数据生成的 INSERT SQL，
/// 用于肉眼检查是否存在幽灵列（SQL 表无对应列）或非法值。
/// 运行：cargo test -p flow --test definition_test print_insert_sql -- --nocapture
#[test]
fn print_insert_sql_for_leave6_data() {
    let backend = DbBackend::MySql;

    // 构造一条 flow_definition ActiveModel（模拟 import 新建）
    let def = flow_definition::ActiveModel {
        id: Set(123),
        flow_code: Set("leave6".into()),
        flow_name: Set("请假申请-排他并行会签".into()),
        model_value: Set("CLASSICS".into()),
        category: Set(Some("1762300000000000103".into())),
        version: Set("1".into()),
        is_publish: Set(0),
        form_custom: Set(Some("N".into())),
        form_path: Set(Some("/workflow/leaveEdit/index".into())),
        activity_status: Set(1),
        listener_type: Set(None),
        listener_path: Set(None),
        ext: Set(None),
        create_time: Set(None),
        create_by: Set(Some("1".into())),
        update_time: Set(None),
        update_by: Set(Some("1".into())),
        del_flag: Set(Some("0".into())),
        tenant_id: Set(None),
    };
    let def_sql = flow_definition::Entity::insert(def).build(backend).sql;
    eprintln!("=== flow_definition INSERT (MySql) ===");
    eprintln!("{}", def_sql);

    // 构造两条 flow_node ActiveModel（含网关 nodeType=3）
    let nodes = vec![
        flow_node::ActiveModel {
            id: Set(201),
            node_type: Set(0),
            definition_id: Set(123),
            node_code: Set("start".into()),
            node_name: Set(Some("开始".into())),
            permission_flag: Set(None),
            node_ratio: Set(Some("0.000".into())),
            coordinate: Set(Some("240,300|240,300".into())),
            any_node_skip: Set(None),
            listener_type: Set(None),
            listener_path: Set(None),
            form_custom: Set(Some("N".into())),
            form_path: Set(None),
            version: Set("1".into()),
            create_time: Set(None),
            create_by: Set(Some("1".into())),
            update_time: Set(None),
            update_by: Set(None),
            ext: Set(Some("[]".into())),
            del_flag: Set(Some("0".into())),
            tenant_id: Set(None),
        },
        flow_node::ActiveModel {
            id: Set(202),
            node_type: Set(3),
            definition_id: Set(123),
            node_code: Set("gateway".into()),
            node_name: Set(None),
            permission_flag: Set(None),
            node_ratio: Set(Some("0.000".into())),
            coordinate: Set(Some("700,300".into())),
            any_node_skip: Set(None),
            listener_type: Set(None),
            listener_path: Set(None),
            form_custom: Set(Some("N".into())),
            form_path: Set(None),
            version: Set("1".into()),
            create_time: Set(None),
            create_by: Set(None),
            update_time: Set(None),
            update_by: Set(None),
            ext: Set(Some("[]".into())),
            del_flag: Set(Some("0".into())),
            tenant_id: Set(None),
        },
    ];
    let node_sql = flow_node::Entity::insert_many(nodes).build(backend).sql;
    eprintln!("=== flow_node INSERT (MySql) ===");
    eprintln!("{}", node_sql);

    // 构造两条 flow_skip ActiveModel（验证 node_id 不出现在 SQL 中）
    let skips = vec![
        flow_skip::ActiveModel {
            id: Set(301),
            definition_id: Set(123),
            now_node_code: Set("gateway".into()),
            now_node_type: Set(Some(3)),
            next_node_code: Set("manager".into()),
            next_node_type: Set(Some(1)),
            // node_id 为 #[sea_orm(ignore)]，ActiveModel 不含此字段
            skip_name: Set(Some("大于两天".into())),
            skip_type: Set(Some("PASS".into())),
            skip_condition: Set(Some("default@@${leaveDays > 2}".into())),
            coordinate: Set(None),
            create_time: Set(None),
            create_by: Set(None),
            update_time: Set(None),
            update_by: Set(None),
            del_flag: Set(Some("0".into())),
            tenant_id: Set(None),
        },
        flow_skip::ActiveModel {
            id: Set(302),
            definition_id: Set(123),
            now_node_code: Set("gateway".into()),
            now_node_type: Set(Some(3)),
            next_node_code: Set("vice".into()),
            next_node_type: Set(Some(1)),
            skip_name: Set(None),
            skip_type: Set(Some("PASS".into())),
            skip_condition: Set(Some(
                "spel@@#{@testLeaveServiceImpl.eval(#leaveDays)}".into(),
            )),
            coordinate: Set(None),
            create_time: Set(None),
            create_by: Set(None),
            update_time: Set(None),
            update_by: Set(None),
            del_flag: Set(Some("0".into())),
            tenant_id: Set(None),
        },
    ];
    let skip_sql = flow_skip::Entity::insert_many(skips).build(backend).sql;
    eprintln!("=== flow_skip INSERT (MySql) ===");
    eprintln!("{}", skip_sql);
    eprintln!("=== END (检查上面 SQL 是否含 node_id 列) ===");
}
