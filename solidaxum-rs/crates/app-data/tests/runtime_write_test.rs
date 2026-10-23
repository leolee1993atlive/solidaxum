#![cfg(feature = "test-util")]
//! 写路径的对外消费面:`insert_stamps_audit` 与
//! `update_denied_by_scope_is_not_found` 两个用例(src/context.rs 单元
//! 测试的对外视角版),以"业务 crate 测试"的视角走一遍 —— 读路径的
//! 同类视角见 runtime_read_test.rs。
//!
//! 同样刻意不依赖 sea-orm crate:`ActiveValue::Set` 来自受限 re-export,
//! 脚手架与实体来自 `app_data::testing`。
use std::sync::Arc;

use app_data::{
    DataError, Principal, ScopeFilter,
    sea_orm::ActiveValue::Set,
    testing::{self, FixedResolver, seed},
};

/// 审计盖章对外可见:插入即全盖,审计人 = principal.user_id;
/// system_context 的审计人 = 0(系统)。
#[tokio::test]
async fn insert_stamps_audit() {
    let ds = testing::sqlite_memory().await;
    let ctx = ds.context(Principal {
        user_id: 42,
        dept_id: Some(10),
    });
    let m = ctx
        .insert(testing::sandbox_row(1, Some(10), None))
        .await
        .unwrap();
    assert_eq!(m.create_by, Some(42), "审计人 = principal.user_id");
    assert_eq!(m.update_by, Some(42), "RuoYi 习惯:插入也写 update_*");
    assert!(m.create_time.is_some());

    let sctx = ds.system_context();
    let m = sctx
        .insert(testing::sandbox_row(2, Some(10), None))
        .await
        .unwrap();
    assert_eq!(m.create_by, Some(0), "系统上下文审计人 = 0(系统)");
}

/// 行级范围进 WHERE 对外可见:越权行的 update 匹配 0 行 → NotFound,
/// 且必须**真的没改**。
#[tokio::test]
async fn update_denied_by_scope_is_not_found() {
    let ds = testing::sqlite_memory().await;
    seed(&ds, vec![(1, 99, 2, "0")]).await; // 别人部门的行
    let ds = ds
        .with_resolver(Arc::new(FixedResolver(ScopeFilter::DeptIn(vec![10]))));
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(10),
    });
    let am = testing::sandbox::ActiveModel {
        id: Set(1),
        del_flag: Set("2".into()),
        ..Default::default()
    };
    let err = ctx.update(am).await.unwrap_err();
    assert!(
        matches!(err, DataError::NotFound),
        "写越权 → NotFound,绝不静默 Ok"
    );

    // 而且必须**真的没改**:换 system_context 读回来确认还是 '0'
    let row = ds
        .system_context()
        .find_by_id::<testing::sandbox::Entity>(1)
        .await
        .unwrap();
    assert_eq!(row.del_flag, "0", "越权的 update 不得落库");
}
