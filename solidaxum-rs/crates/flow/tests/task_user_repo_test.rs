//! task_repo / user_repo canonical 判例(exemption_test / definition_test
//! 同款脚手架):
//!
//! 1. canonical 读变体在受限 resolver 下经 `without_row_scope()` 照见全量
//!    (warm-flow/ruoyi-workflow Mapper 零 @DataPermission,工作流数据非行级),
//!    软删存活过滤(@TableLogic)不受豁免影响;
//! 2. `insert_batch_in_txn` 批插(`tx.insert_many` 单条多值 INSERT),
//!    空集合 no-op;
//! 3. `soft_delete_by_id_in_txn` 逻辑删语义(@TableLogic 平价,node_repo
//!    先例):删后读不见、墓碑行保留;
//! 4. `update_processed_by_in_txn` 转办的单语句条件更新(`tx.update_where`)
//!    只动命中行;后至的同条件转办 0 行(先写者保)。
use std::sync::Arc;

use app_data::{
    DataError, DataSource, Principal, ScopeFilter,
    sea_orm::{ActiveValue::Set, EntityTrait},
    testing::{FixedResolver, sqlite_memory_empty},
};
use flow::{
    model::{task::Task, user::User},
    repository::{
        entity::{flow_definition, flow_instance, flow_task, flow_user},
        task_repo::TaskRepository,
        user_repo::UserRepository,
    },
};

async fn ds_seeded() -> DataSource {
    let ds = sqlite_memory_empty().await;
    // FK 目标表也要建(engine_test 同款:SQLite foreign_keys=ON,
    // 缺表时报 "no such table: main.flow_instance")
    ds.create_table(flow_definition::Entity).await.unwrap();
    ds.create_table(flow_instance::Entity).await.unwrap();
    ds.create_table(flow_task::Entity).await.unwrap();
    ds.create_table(flow_user::Entity).await.unwrap();
    let ctx = ds.system_context();
    // FK 父行:definition 1 + 实例 100/200/300(flow_task.definition_id /
    // instance_id 外键,SQLite foreign_keys=ON)
    ctx.insert(flow_definition::ActiveModel {
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
    let instance = |id: i64| flow_instance::ActiveModel {
        id: Set(id),
        definition_id: Set(1),
        business_id: Set(format!("biz{id}")),
        node_type: Set(1),
        node_code: Set("node".to_string()),
        flow_status: Set("1".to_string()),
        activity_status: Set(1),
        del_flag: Set(Some("0".to_string())),
        ..Default::default()
    };
    ctx.insert(instance(100)).await.unwrap();
    ctx.insert(instance(200)).await.unwrap();
    ctx.insert(instance(300)).await.unwrap();
    let task = |id: i64, instance_id: i64, del: &str| flow_task::ActiveModel {
        id: Set(id),
        definition_id: Set(1),
        instance_id: Set(instance_id),
        node_code: Set(format!("node{id}")),
        node_type: Set(1),
        flow_status: Set("1".to_string()),
        del_flag: Set(Some(del.to_string())),
        ..Default::default()
    };
    // 实例 100:t1/t2 存活,t3 已软删;实例 200:t4 存活
    ctx.insert(task(1, 100, "0")).await.unwrap();
    ctx.insert(task(2, 100, "0")).await.unwrap();
    ctx.insert(task(3, 100, "2")).await.unwrap();
    ctx.insert(task(4, 200, "0")).await.unwrap();

    let user = |id: i64, task_id: i64, ty: &str, handler: &str, del: &str| {
        flow_user::ActiveModel {
            id: Set(id),
            type_: Set(ty.to_string()),
            processed_by: Set(Some(handler.to_string())),
            associated: Set(task_id),
            del_flag: Set(Some(del.to_string())),
            ..Default::default()
        }
    };
    // 任务 1:u1(type=1 存活)/u2(type=2 存活)/u3(type=1 已删)
    ctx.insert(user(11, 1, "1", "alice", "0")).await.unwrap();
    ctx.insert(user(12, 1, "2", "bob", "0")).await.unwrap();
    ctx.insert(user(13, 1, "1", "carol", "2")).await.unwrap();
    ds
}

fn make_task(id: i64, instance_id: i64) -> Task {
    Task {
        id: Some(id),
        definition_id: 1,
        instance_id,
        node_code: format!("node{id}"),
        node_name: None,
        node_type: 1,
        flow_status: "1".to_string(),
        form_custom: None,
        form_path: None,
        create_time: None,
        create_by: None,
        update_time: None,
        update_by: None,
        del_flag: Some("0".to_string()),
        tenant_id: None,
        permission_list: vec![],
    }
}

/// 判例①(task):canonical 读变体在受限范围下照见全量存活行 ——
/// `without_row_scope()` 豁免行级,@TableLogic 存活过滤照施加。
#[tokio::test]
async fn task_canonical_read_sees_all_alive_rows_under_limited_scope() {
    let ds = ds_seeded()
        .await
        .with_resolver(Arc::new(FixedResolver(ScopeFilter::SelfOnly(7))));
    let ctx = ds.context(Principal {
        user_id: 8, // SelfOnly(7) 下的「别人」,受辖实体对它应不可见
        dept_id: None,
    });

    let rows = TaskRepository::select_by_instance_id(&ctx, 100)
        .await
        .unwrap();
    assert_eq!(
        rows.len(),
        2,
        "受限范围下豁免读照见全量存活行;已删行仍被 @TableLogic 过滤"
    );

    // 事务路径变体同语义
    let ctx2 = ds.context(Principal {
        user_id: 8,
        dept_id: None,
    });
    let n = ctx2
        .transaction(|tx| {
            Box::pin(async move {
                let rows =
                    TaskRepository::select_by_instance_id_in_txn(tx, 100)
                        .await?;
                Ok::<usize, DataError>(rows.len())
            })
        })
        .await
        .unwrap();
    assert_eq!(n, 2, "事务路径豁免读同语义");
}

/// 判例①(user):两跳之外的基础读 —— 受限范围下 type=1 存活办理人照见。
#[tokio::test]
async fn user_canonical_read_sees_all_alive_rows_under_limited_scope() {
    let ds = ds_seeded()
        .await
        .with_resolver(Arc::new(FixedResolver(ScopeFilter::SelfOnly(7))));
    let ctx = ds.context(Principal {
        user_id: 8,
        dept_id: None,
    });

    let rows = UserRepository::select_by_task_id(&ctx, 1).await.unwrap();
    assert_eq!(rows.len(), 1, "type=1 且存活仅 u1;已删 u3 被过滤");
    assert_eq!(rows[0].processed_by.as_deref(), Some("alice"));

    let all = UserRepository::select_all_by_task_id(&ctx, 1)
        .await
        .unwrap();
    assert_eq!(all.len(), 2, "不限类型:u1/u2 存活,u3 已删被过滤");
}

/// 判例②:批插 —— 3 行落库;空集合 no-op。
#[tokio::test]
async fn insert_batch_in_txn_loops_rows_and_empty_is_noop() {
    let ds = ds_seeded().await;
    let ctx = ds.system_context();

    ctx.transaction(|tx| {
        Box::pin(async move {
            // 空集合:零迭代,直接 Ok
            TaskRepository::insert_batch_in_txn(tx, &[]).await?;
            let batch =
                [make_task(21, 300), make_task(22, 300), make_task(23, 300)];
            TaskRepository::insert_batch_in_txn(tx, &batch).await?;
            Ok::<(), DataError>(())
        })
    })
    .await
    .unwrap();

    let rows = TaskRepository::select_by_instance_id(&ctx, 300)
        .await
        .unwrap();
    assert_eq!(rows.len(), 3, "批插 3 行全部落库");

    // 连接路径批插(user_repo 双路径消费:抄送走 &db)
    let users = vec![
        User {
            id: Some(31),
            type_: 4,
            processed_by: Some("dave".to_string()),
            associated: 1,
            create_time: None,
            create_by: None,
            update_time: None,
            update_by: None,
            del_flag: Some("0".to_string()),
            tenant_id: None,
        },
        User {
            id: Some(32),
            type_: 4,
            processed_by: Some("erin".to_string()),
            associated: 1,
            create_time: None,
            create_by: None,
            update_time: None,
            update_by: None,
            del_flag: Some("0".to_string()),
            tenant_id: None,
        },
    ];
    UserRepository::insert_batch(&ctx, &users).await.unwrap();
    let all = UserRepository::select_all_by_task_id(&ctx, 1)
        .await
        .unwrap();
    assert_eq!(all.len(), 4, "连接路径批插 2 行落库(u1/u2 + 2 抄送)");
}

/// 判例③:canonical 删除 = 逻辑删(@TableLogic 平价,node_repo 先例)。
/// 删后读不见、墓碑行保留(物理仍在)。
#[tokio::test]
async fn soft_delete_by_id_in_txn_hides_row_but_keeps_tombstone() {
    let ds = ds_seeded().await;
    let ctx = ds.system_context();

    ctx.transaction(|tx| {
        Box::pin(async move {
            let n = TaskRepository::soft_delete_by_id_in_txn(tx, 1).await?;
            assert_eq!(n, 1, "命中 1 行");
            Ok::<(), DataError>(())
        })
    })
    .await
    .unwrap();

    let rows = TaskRepository::select_by_instance_id(&ctx, 100)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "逻辑删后读不见(t2 存活)");
    assert_eq!(rows[0].id, Some(2));

    // 墓碑行物理仍在:include_deleted 直查实体(canonical repo 不开这扇门,
    // 判例直接用逃生舱验证「软删而非硬删」的语义差)
    let all = ctx
        .select(flow_task::Entity::find())
        .without_row_scope()
        .include_deleted()
        .all()
        .await
        .unwrap();
    assert!(
        all.iter()
            .any(|m| m.id == 1 && m.del_flag.as_deref() == Some("1")),
        "t1 应为 del_flag='1' 的墓碑行,而非物理消失"
    );

    // 重复删已删行:0 行 → Ok(0)(批量语义无 NotFound 约定)
    ctx.transaction(|tx| {
        Box::pin(async move {
            let n = TaskRepository::soft_delete_by_id_in_txn(tx, 1).await?;
            assert_eq!(n, 0, "已删行不重复计数");
            Ok::<(), DataError>(())
        })
    })
    .await
    .unwrap();
}

/// 判例④:转办(`update_processed_by_in_txn`)的单语句条件更新 ——
/// `tx.update_where` 只动 (task, old_handler, type=1) 命中行,0 行 → Ok(0);
/// 双转办重叠窗口(串行模拟)后至者 0 行不改写(先写者保,Java 平价)。
#[tokio::test]
async fn update_processed_by_in_txn_rewrites_only_matching_rows() {
    let ds = ds_seeded().await;
    let ctx = ds.system_context();

    ctx.transaction(|tx| {
        Box::pin(async move {
            let n = UserRepository::update_processed_by_in_txn(
                tx, 1, "alice", "zoe",
            )
            .await?;
            assert_eq!(n, 1, "仅 u1 命中(alice, type=1)");
            Ok::<(), DataError>(())
        })
    })
    .await
    .unwrap();

    let rows = UserRepository::select_all_by_task_id(&ctx, 1)
        .await
        .unwrap();
    let zoe = rows.iter().find(|u| u.id == Some(11)).unwrap();
    assert_eq!(zoe.processed_by.as_deref(), Some("zoe"), "u1 已转办");
    let bob = rows.iter().find(|u| u.id == Some(12)).unwrap();
    assert_eq!(
        bob.processed_by.as_deref(),
        Some("bob"),
        "u2(type=2)不受影响"
    );

    // 无命中 → Ok(0)
    ctx.transaction(|tx| {
        Box::pin(async move {
            let n = UserRepository::update_processed_by_in_txn(
                tx, 1, "nobody", "zoe",
            )
            .await?;
            assert_eq!(n, 0);
            Ok::<(), DataError>(())
        })
    })
    .await
    .unwrap();

    // 双转办重叠窗口(串行模拟):第一个转办 alice→zoe 已落库,后至的
    // 同条件转办 alice→dave 条件失配 → 0 行,不覆盖先写者结果(先写者保)
    ctx.transaction(|tx| {
        Box::pin(async move {
            let n = UserRepository::update_processed_by_in_txn(
                tx, 1, "alice", "dave",
            )
            .await?;
            assert_eq!(n, 0, "后至者条件失配 → Ok(0)");
            Ok::<(), DataError>(())
        })
    })
    .await
    .unwrap();
    let rows = UserRepository::select_all_by_task_id(&ctx, 1)
        .await
        .unwrap();
    let winner = rows.iter().find(|u| u.id == Some(11)).unwrap();
    assert_eq!(
        winner.processed_by.as_deref(),
        Some("zoe"),
        "先写者保:u1 保持 zoe,未被后至的 dave 覆盖"
    );
}
