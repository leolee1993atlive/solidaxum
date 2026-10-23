//! 节点配置单点更新判例(对齐参考实现 FlowNodeConfServiceImpl):
//! 显式 set 单字段,支持置空(null → 清空);节点/连线不存在 → 404。
//!
//! 脚手架同 engine_test:`sqlite_memory_empty()` + 实体衍生 DDL 建
//! flow_node / flow_skip 表。更新作用于已发布定义的存量行,不动定义版本。
use app_data::{sea_orm::ActiveValue::Set, testing::sqlite_memory_empty};
use flow::{
    repository::{
        entity::{flow_node, flow_skip},
        node_repo::NodeRepository,
        skip_repo::SkipRepository,
    },
    service::definition_service::DefinitionService,
};

async fn setup_ds() -> app_data::DataSource {
    let ds = sqlite_memory_empty().await;
    ds.create_table(flow_node::Entity).await.unwrap();
    ds.create_table(flow_skip::Entity).await.unwrap();
    ds
}

fn node_am(
    id: i64,
    definition_id: i64,
    node_code: &str,
) -> flow_node::ActiveModel {
    flow_node::ActiveModel {
        id: Set(id),
        node_type: Set(1),
        definition_id: Set(definition_id),
        node_code: Set(node_code.to_string()),
        node_name: Set(Some("审批节点".to_string())),
        permission_flag: Set(Some("user:1".to_string())),
        node_ratio: Set(Some("100".to_string())),
        version: Set("1".to_string()),
        del_flag: Set(Some("0".to_string())),
        ..Default::default()
    }
}

fn skip_am(
    id: i64,
    definition_id: i64,
    now: &str,
    next: &str,
) -> flow_skip::ActiveModel {
    flow_skip::ActiveModel {
        id: Set(id),
        definition_id: Set(definition_id),
        now_node_code: Set(now.to_string()),
        next_node_code: Set(next.to_string()),
        skip_name: Set(Some("同意".to_string())),
        skip_type: Set(Some("PASS".to_string())),
        skip_condition: Set(Some("#days > 3".to_string())),
        del_flag: Set(Some("0".to_string())),
        ..Default::default()
    }
}

#[tokio::test]
async fn test_update_permission_flag_set_and_clear() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    ctx.insert(node_am(9201, 8001, "n1")).await.unwrap();

    // 单点更新生效
    let affected = NodeRepository::update_permission_flag(
        &ctx,
        8001,
        "n1",
        Some("role:2,user:3".to_string()),
    )
    .await
    .unwrap();
    assert_eq!(affected, 1, "应命中 1 行");
    let node =
        NodeRepository::select_by_definition_id_and_node_code(&ctx, 8001, "n1")
            .await
            .unwrap()
            .unwrap();
    assert_eq!(node.permission_flag.as_deref(), Some("role:2,user:3"));

    // 置空生效(对齐 Java 显式 set,null 也进 SET 子句)
    let affected =
        NodeRepository::update_permission_flag(&ctx, 8001, "n1", None)
            .await
            .unwrap();
    assert_eq!(affected, 1);
    let node =
        NodeRepository::select_by_definition_id_and_node_code(&ctx, 8001, "n1")
            .await
            .unwrap()
            .unwrap();
    assert_eq!(node.permission_flag, None, "permission_flag 应被清空");
}

#[tokio::test]
async fn test_update_permission_flag_missing_node_returns_zero() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();

    let affected = NodeRepository::update_permission_flag(
        &ctx,
        9999,
        "ghost",
        Some("x".into()),
    )
    .await
    .unwrap();
    assert_eq!(affected, 0, "节点不存在应 0 行,由 service 层判 404");
}

#[tokio::test]
async fn test_service_update_permission_flag_not_found() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();

    let err = DefinitionService::new()
        .update_permission_flag(&ctx, 9999, "ghost", Some("x".into()))
        .await
        .unwrap_err();
    assert!(
        matches!(err, app_core::error::AppError::NotFound(_)),
        "节点不存在应 404,实际: {err}"
    );
}

#[tokio::test]
async fn test_update_skip_condition_set_and_clear() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    ctx.insert(skip_am(9301, 8001, "n1", "n2")).await.unwrap();

    let affected = SkipRepository::update_skip_condition(
        &ctx,
        9301,
        Some("#amount >= 1000".to_string()),
    )
    .await
    .unwrap();
    assert_eq!(affected, 1);
    let skip = SkipRepository::select_by_id(&ctx, 9301)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(skip.skip_condition.as_deref(), Some("#amount >= 1000"));

    // 置空 = 无条件通过(对齐 Java 注释)
    let affected = SkipRepository::update_skip_condition(&ctx, 9301, None)
        .await
        .unwrap();
    assert_eq!(affected, 1);
    let skip = SkipRepository::select_by_id(&ctx, 9301)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(skip.skip_condition, None, "skip_condition 应被清空");
}

#[tokio::test]
async fn test_service_update_skip_condition_not_found() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();

    let err = DefinitionService::new()
        .update_skip_condition(&ctx, 9999, Some("x".into()))
        .await
        .unwrap_err();
    assert!(
        matches!(err, app_core::error::AppError::NotFound(_)),
        "连线不存在应 404,实际: {err}"
    );
}

#[tokio::test]
async fn test_service_update_skip_condition_ok() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    ctx.insert(skip_am(9302, 8001, "n1", "n2")).await.unwrap();

    DefinitionService::new()
        .update_skip_condition(&ctx, 9302, Some("#days > 1".to_string()))
        .await
        .unwrap();
    let skip = SkipRepository::select_by_id(&ctx, 9302)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(skip.skip_condition.as_deref(), Some("#days > 1"));
}
