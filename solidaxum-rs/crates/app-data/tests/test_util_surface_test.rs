#![cfg(feature = "test-util")]
//! test-util 补齐面的对外消费契约(设计 9 节第 4 条)。
//!
//! 背景:system 测试套件(28 文件)迁 `app_data(test-util)` 前的盘点结论 ——
//! 四类直连 sea-orm 的消费里,三类既有面已覆盖(各钉一条用例防回归),
//! 唯一缺口是**免盖章种子**,由 `DataContext::insert_unstamped` 补齐。
//!
//! 与 runtime_read_test.rs / runtime_write_test.rs 同一纪律:不出现任何对
//! sea-orm crate 本身的依赖,一切经受限 re-export 与 testing 脚手架。
use app_data::{
    DataError,
    sea_orm::{
        ActiveValue::Set, ColumnTrait, DateTimeUtc, EntityTrait, QueryFilter,
    },
    testing::{self, sys_user},
};

/// 缺口①:免盖章种子。system 测试的范围交叉用例要种 create_by=7 这类
/// 显式审计值(user_repo_test.rs:56、role_repo_test.rs:91-95 的
/// create_dept + create_by),漏斗 `insert` 无条件盖章会把它顶掉 ——
/// `insert_unstamped` 必须原样落库,未 Set 的审计列不得补盖。
#[tokio::test]
async fn insert_unstamped_keeps_caller_audit_values() {
    let ds = testing::sqlite_memory().await;
    let ctx = ds.system_context();
    let fixed = DateTimeUtc::from_timestamp_millis(1_700_000_000_000).unwrap();

    let mut am = testing::sandbox_row(1, Some(10), Some(7));
    am.create_by = Set(Some(7));
    am.create_time = Set(Some(fixed));
    am.update_by = Set(Some(8));
    let m = ctx.insert_unstamped(am).await.unwrap();
    assert_eq!(m.create_by, Some(7), "显式 create_by 不得被盖章顶掉");
    assert_eq!(m.create_time, Some(fixed), "显式 create_time 原样落库");
    assert_eq!(m.update_by, Some(8));
    assert_eq!(m.update_time, None, "未 Set 的审计列保持 None,不补盖");

    // 对照组:同一形状的行走漏斗 insert,审计列被 system_context 盖成 0
    let mut stamped = testing::sandbox_row(2, Some(10), Some(7));
    stamped.create_by = Set(Some(7));
    let m2 = ctx.insert(stamped).await.unwrap();
    assert_eq!(m2.create_by, Some(0), "漏斗 insert 照盖章(系统 = 0)");
}

/// 核验①:含软删行读取**不需要新原语**。post_repo_test.rs:138-148 /
/// user_repo_test.rs:240 / role_repo_test.rs:297-302 的 del_flag 断言,
/// 现状是 select + include_deleted 的整行读(逃生舱无投影,设计 6.4):
/// 断言落在 Model 的 del_flag 字段上,与裸 SQL 单列读等价。
#[tokio::test]
async fn include_deleted_reads_soft_deleted_row() {
    let ds = testing::sqlite_memory().await;
    let ctx = ds.system_context();
    let mut am = testing::sandbox_row(3, Some(10), Some(1));
    am.del_flag = Set("2".into()); // 已删行种子(免盖章,值原样落库)
    ctx.insert_unstamped(am).await.unwrap();

    // 常规面不可见:窄 API 与逃生舱默认都过滤软删
    assert!(matches!(
        ctx.find_by_id::<testing::sandbox::Entity>(3)
            .await
            .unwrap_err(),
        DataError::NotFound
    ));
    let invisible = ctx
        .select(
            testing::sandbox::Entity::find()
                .filter(testing::sandbox::Column::Id.eq(3)),
        )
        .one()
        .await
        .unwrap();
    assert!(invisible.is_none(), "默认面不得漏出已删行");

    // include_deleted 单次放行:整行读回,del_flag 断言落在字段上
    let row = ctx
        .select(
            testing::sandbox::Entity::find()
                .filter(testing::sandbox::Column::Id.eq(3)),
        )
        .include_deleted()
        .one()
        .await
        .unwrap()
        .expect("已删行经 include_deleted 可读");
    assert_eq!(row.del_flag, "2");
}

/// 核验②:`DataSource::create_table` 覆盖手写 DDL 的最复杂表形状 ——
/// sys_user(19 字段、`column_name` 属性、create_dept、审计四列、软删列,
/// 对应 system tests common/mod.rs 的 sys_user 手写 DDL)。种一行
/// del_flag='1' + create_by=7 的已删行(user_repo_test.rs:56 同款),
/// 整行读回逐字段核。
#[tokio::test]
async fn create_table_covers_real_entity_and_seeds_deleted_row() {
    let ds = testing::sqlite_memory_empty().await;
    ds.create_table(sys_user::Entity).await.unwrap();
    let fixed = DateTimeUtc::from_timestamp_millis(1_700_000_000_000).unwrap();

    let am = sys_user::ActiveModel {
        user_id: Set(301),
        dept_id: Set(Some(101)),
        user_name: Set("u_all".into()),
        nick_name: Set("全部数据".into()),
        user_type: Set(None),
        email: Set(None),
        phone_number: Set(Some("13800000000".into())),
        gender: Set(None),
        avatar: Set(None),
        password: Set("$2a$10$placeholder".into()),
        status: Set("0".into()),
        del_flag: Set("1".into()), // 已删行种子
        login_ip: Set(None),
        login_date: Set(None),
        create_dept: Set(Some(100)),
        create_by: Set(Some(7)),
        create_time: Set(Some(fixed)),
        update_by: Set(None),
        update_time: Set(None),
        remark: Set(None),
    };
    let ctx = ds.system_context();
    ctx.insert_unstamped(am).await.unwrap();

    let row = ctx
        .select(
            sys_user::Entity::find().filter(sys_user::Column::UserId.eq(301)),
        )
        .include_deleted()
        .one()
        .await
        .unwrap()
        .expect("已删行经 include_deleted 可读");
    assert_eq!(row.del_flag, "1");
    assert_eq!(row.create_by, Some(7), "显式审计值原样落库");
    assert_eq!(row.create_dept, Some(100));
    assert_eq!(row.phone_number.as_deref(), Some("13800000000"));
    assert_eq!(row.create_time, Some(fixed));
    assert!(
        ctx.find_by_id::<sys_user::Entity>(301).await.is_err(),
        "常规面不得漏出已删行"
    );
}

/// 核验③:关联表往返**不需要新原语**。relation_entity_test.rs:19-71 的
/// `ActiveModelTrait::insert(&db)` + `Entity::find().one(&db)`:关联实体
/// 无审计列(无可盖)、无软删列、CONSTRAINED=false,漏斗 `insert` 与
/// select + without_row_scope 已逐字等价(复合主键也走这条路)。
#[tokio::test]
async fn relation_entity_roundtrip_via_plain_funnel() {
    let ds = testing::sqlite_memory().await;
    let ctx = ds.system_context();
    let am = testing::rel_link::ActiveModel {
        user_id: Set(302),
        role_id: Set(201),
    };
    ctx.insert(am).await.unwrap();

    let found = ctx
        .select(testing::rel_link::Entity::find())
        .without_row_scope()
        .all()
        .await
        .unwrap();
    assert_eq!(
        found,
        vec![testing::rel_link::Model {
            user_id: 302,
            role_id: 201
        }]
    );
}
