#![cfg(feature = "test-util")]
//! 以"业务 crate 测试"的视角消费 app_data::testing —— 这正是业务侧迁移
//! 里各 crate 测试将要走的路(设计 9 节第 4 条)。
//!
//! 本文件刻意**不依赖 sea-orm crate 本身**:所有构造件都来自受限 re-export
//! `app_data::sea_orm`,所有脚手架都来自 `app_data::testing` —— 业务 crate
//! 的 [dev-dependencies] 里只有 `app_data = { features = ["test-util"] }`。
use std::sync::Arc;

use app_data::{
    Principal, ScopeFilter,
    sea_orm::{DbBackend, EntityTrait},
    testing::{self, FixedResolver},
};

#[tokio::test]
async fn external_consumer_can_build_and_query() {
    let ds = testing::sqlite_memory().await;
    testing::seed(&ds, vec![(1, 10, 1, "0"), (2, 99, 2, "0")]).await;
    let ds = ds
        .with_resolver(Arc::new(FixedResolver(ScopeFilter::DeptIn(vec![10]))));
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(10),
    });
    let rows = ctx
        .select(testing::sandbox::Entity::find())
        .all()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "只有本部门行可见");
}

/// **这条用例代表 admin/src/testkit.rs 迁移后的写法**:空库 + 自带实体建表。
/// 它绿了,才说明 test-util 交付的是设计 9.4 要的东西,而不只是 app-data 的自用夹具。
#[tokio::test]
async fn external_consumer_can_create_its_own_tables() {
    let ds = testing::sqlite_memory_empty().await;
    ds.create_table(testing::sys_user::Entity).await.unwrap();

    let ctx = ds.system_context();
    let rows = ctx
        .select(testing::sys_user::Entity::find())
        .all()
        .await
        .unwrap();
    assert!(rows.is_empty(), "表建出来了,只是没有数据");
}

/// 设计 9.1 的 SQL 断言在业务侧的用法:只 build 不连库。
#[tokio::test]
async fn external_consumer_can_assert_generated_sql() {
    let ds = testing::sqlite_memory_empty()
        .await
        .with_resolver(Arc::new(FixedResolver(ScopeFilter::SelfOnly(7))));
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(10),
    });
    let sql = ctx
        .select(testing::sys_user::Entity::find())
        .into_sql(DbBackend::Sqlite)
        .await
        .unwrap();
    assert!(
        sql.contains("\"sys_user\".\"user_id\" = 7"),
        "SELF 档落在 owner 列上"
    );
    assert!(
        sql.contains("\"sys_user\".\"del_flag\" = '0'"),
        "软删除过滤自动在"
    );
}
