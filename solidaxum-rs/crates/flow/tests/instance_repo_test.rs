//! instance_repo 软删判例(task_user_repo_test 同款脚手架)—— 钉住
//! `soft_delete_by_ids` / `soft_delete_by_business_ids` 的 alive 口径:
//! **WHERE = cond AND 软删存活**,已删行不重复计数、不被重写
//! (Java `@TableLogic(value="0", delval="1")` 平价;口径说明见
//! instance_repo 文件头)。
use app_data::{
    DataSource,
    sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter},
    testing::sqlite_memory_empty,
};
use flow::repository::{
    entity::{flow_definition, flow_instance},
    instance_repo::InstanceRepository,
};

async fn setup_ds() -> DataSource {
    let ds = sqlite_memory_empty().await;
    // FK 目标表也要建(engine_test 同款:flow_instance.definition_id 外键)
    ds.create_table(flow_definition::Entity).await.unwrap();
    ds.create_table(flow_instance::Entity).await.unwrap();
    // FK 父行:definition 1(flow_instance.definition_id 外键目标)
    ds.system_context()
        .insert(flow_definition::ActiveModel {
            id: Set(1),
            flow_code: Set("test".to_string()),
            flow_name: Set("测试".to_string()),
            model_value: Set("CLASSICS".to_string()),
            version: Set("1".to_string()),
            is_publish: Set(1),
            activity_status: Set(1),
            del_flag: Set(Some("0".to_string())),
            ..Default::default()
        })
        .await
        .unwrap();
    ds
}

/// 免盖章种一行实例:del_flag 由调用方显式给值("0" 存活 / "1" 已删)。
/// 已删行必须经 `insert_unstamped` 种 —— 它是 test-util 的种子原语,
/// 表达「这行本来就带着 del_flag='1' 落库」的历史状态。
async fn seed_instance(
    ds: &DataSource,
    id: i64,
    business_id: &str,
    del_flag: &str,
) {
    ds.system_context()
        .insert_unstamped(flow_instance::ActiveModel {
            id: Set(id),
            definition_id: Set(1),
            business_id: Set(business_id.to_string()),
            node_type: Set(1),
            node_code: Set("node".to_string()),
            flow_status: Set("1".to_string()),
            activity_status: Set(1),
            del_flag: Set(Some(del_flag.to_string())),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// 存活行计数(include_deleted 之外的常规逃生舱读 = 软删过滤照施加)。
async fn alive_count(ds: &DataSource) -> usize {
    ds.system_context()
        .select(
            flow_instance::Entity::find()
                .filter(flow_instance::Column::DelFlag.eq("0")),
        )
        .without_row_scope()
        .all()
        .await
        .unwrap()
        .len()
}

/// 判例:重删已删实例 rows_affected=0 且存活行计数不变(幂等重删)。
/// 种法:一行存活(del_flag='0')+ 一行已软删(del_flag='1',经
/// `insert_unstamped` 免盖章落库);对已删行同 id 再 `soft_delete_by_ids`,
/// 断言 0 行命中、存活计数不动 —— @TableLogic 的 alive 过滤由
/// `soft_delete_where` 施加。
#[tokio::test]
async fn soft_delete_by_ids_is_idempotent_on_already_deleted_rows() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    seed_instance(&ds, 1, "biz-alive", "0").await;
    seed_instance(&ds, 2, "biz-dead", "1").await;

    // 重删已删行:0 行,存活计数不变
    let n = InstanceRepository::soft_delete_by_ids(&ctx, &[2])
        .await
        .unwrap();
    assert_eq!(n, 0, "已删行不重复计数、不被新 del 值重写");
    assert_eq!(alive_count(&ds).await, 1, "幂等重删不动存活行");

    // 删存活行:命中 1 行,读侧随即滤掉(@TableLogic 平价)
    let n = InstanceRepository::soft_delete_by_ids(&ctx, &[1])
        .await
        .unwrap();
    assert_eq!(n, 1, "存活行命中 1 行");
    assert!(
        InstanceRepository::select_by_id(&ctx, 1)
            .await
            .unwrap()
            .is_none(),
        "逻辑删后读不见"
    );
    assert_eq!(alive_count(&ds).await, 0);

    // 墓碑行物理仍在(软删而非硬删,his_task/task 同款语义差)
    let all = ds
        .system_context()
        .select(flow_instance::Entity::find())
        .without_row_scope()
        .include_deleted()
        .all()
        .await
        .unwrap();
    assert!(
        all.iter()
            .any(|m| m.id == 1 && m.del_flag.as_deref() == Some("1")),
        "实例 1 应为 del_flag='1' 的墓碑行,而非物理消失"
    );

    // 幂等回环:再删刚删过的行,依旧 0
    let n = InstanceRepository::soft_delete_by_ids(&ctx, &[1])
        .await
        .unwrap();
    assert_eq!(n, 0, "二次删除同为 0(批量语义无 NotFound 约定)");
}

/// 判例:business_id 路径同 alive 口径(已删行不重复计数)。
#[tokio::test]
async fn soft_delete_by_business_ids_keeps_alive_semantics() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    seed_instance(&ds, 11, "BIZ-A", "1").await; // 已删
    seed_instance(&ds, 12, "BIZ-B", "0").await; // 存活

    let n = InstanceRepository::soft_delete_by_business_ids(
        &ctx,
        &["BIZ-A".to_string()],
    )
    .await
    .unwrap();
    assert_eq!(n, 0, "按 business_id 重删已删实例同样 0 行");
    assert_eq!(alive_count(&ds).await, 1);

    let n = InstanceRepository::soft_delete_by_business_ids(
        &ctx,
        &["BIZ-B".to_string()],
    )
    .await
    .unwrap();
    assert_eq!(n, 1, "存活实例按 business_id 删命中 1 行");
    assert!(
        InstanceRepository::select_by_business_id(&ctx, "BIZ-B")
            .await
            .unwrap()
            .is_none()
    );
}
