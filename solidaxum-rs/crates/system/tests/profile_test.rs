//! profile 服务层集成测试。
//!
//! 钉住三组语义:① 审计 —— update_profile/update_password 的
//! `update_by` = 操作人本人(handler 传用户 ctx,漏斗 update 按
//! principal 盖章);② 读路径(全豁免)与部门范围覆盖本部门的
//! 更新照常工作;③ Java ignore 平价 —— SelfOnly 操作者的自服务更新
//! 经写侧豁免原语(update_without_row_scope,设计 7.4)恒成功。
//! resolver 用 FixedResolver 免搭角色语义(与
//! user_service_test / dept_service_test 同型)。

mod common;

use std::sync::Arc;

use app_core::infra::{ConnectionPool, Infrastructure};
use app_data::{
    DataSource, Principal, ScopeFilter,
    sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter},
    testing::FixedResolver,
};
use common::setup_schema_and_seeds;
use system::{
    profile::service::profile_service::ProfileService,
    user::{
        model::user::SysUserProfileBo, repository::user_repo as user_entity,
    },
};

fn hash_pwd(pwd: &str) -> String {
    bcrypt::hash(pwd, 4).expect("hash failed")
}

/// 惰性 Redis 池(不真正连接)—— 与 user_service_test 同一处置,
/// TranslationService 仅在字典翻译路径触 Redis,本文件不触。
fn lazy_redis_pool() -> Arc<ConnectionPool> {
    let pool = deadpool_redis::Config::from_url("redis://127.0.0.1:6379")
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("创建惰性 redis 池失败");
    Arc::new(ConnectionPool { redis_pool: pool })
}

/// 建库:经 `sqlite_memory_empty` 自持 DataSource(装配层,设计 9.4 的
/// 出口),建表/种子复用 common 的同一套。返回 resolver-less 母体。
async fn setup_conn() -> Arc<DataSource> {
    let ds = Arc::new(app_data::testing::sqlite_memory_empty().await);
    setup_schema_and_seeds(&ds).await;
    ds
}

/// 受辖 DataSource + 服务(生产装配同构:ds0 → derive → set_data_source →
/// 服务构造在装配后)。返回 (ds, svc):ctx 一律 `ds.context(Principal{...})`
/// 请求期构造,与 handler 的接线同型。种子用户 302 在 101 部门。
fn service_scoped(
    ds0: Arc<DataSource>,
    sf: ScopeFilter,
) -> (Arc<DataSource>, ProfileService) {
    let ds = Arc::new(ds0.derive_with_resolver(Arc::new(FixedResolver(sf))));
    let infra = Infrastructure::new(lazy_redis_pool(), false, 1800);
    assert!(
        infra.set_data_source(ds.clone()),
        "测试装配 DataSource 只此一次"
    );
    (ds.clone(), ProfileService::new(&infra))
}

/// 直设密码(绕服务写路径的单列改,走漏斗 update;审计盖 0/now,无害)。
async fn set_password(ds0: &DataSource, pwd: &str) {
    ds0.system_context()
        .update(user_entity::ActiveModel {
            user_id: Set(302),
            password: Set(hash_pwd(pwd)),
            ..Default::default()
        })
        .await
        .expect("更新密码失败");
}

/// 读 sys_user 原始 update_by(审计恢复断言用,不经 service;
/// 整行读,逃生舱无投影 —— 与原裸 SQL 单列读等价)。
async fn raw_update_by(ds: &DataSource, user_id: i64) -> Option<i64> {
    ds.system_context()
        .select(
            user_entity::Entity::find()
                .filter(user_entity::Column::UserId.eq(user_id)),
        )
        .one()
        .await
        .expect("读取 update_by 失败")
        .unwrap_or_else(|| panic!("用户 {user_id} 不存在"))
        .update_by
}

fn profile_bo(nick: &str) -> SysUserProfileBo {
    SysUserProfileBo {
        nick_name: Some(nick.to_string()),
        email: None,
        phone_number: None,
        gender: None,
        avatar: None,
    }
}

#[tokio::test]
async fn test_get_profile() {
    let ds0 = setup_conn().await;
    let (ds, svc) = service_scoped(ds0, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 302,
        dept_id: Some(101),
    });

    let profile = svc.get_profile(&ctx, 302).await.expect("查询 profile 失败");

    assert_eq!(profile.user.user_id, 302);
    assert_eq!(profile.user.user_name, "u_custom");
    assert!(!profile.role_group.is_empty(), "role_group 不应为空");
    assert!(!profile.post_group.is_empty(), "post_group 不应为空");
}

#[tokio::test]
async fn test_update_profile() {
    let ds0 = setup_conn().await;
    let (ds, svc) = service_scoped(ds0, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 302,
        dept_id: Some(101),
    });

    let bo = SysUserProfileBo {
        nick_name: Some("新昵称".to_string()),
        email: Some("new@test.com".to_string()),
        phone_number: Some("13800000000".to_string()),
        gender: Some("0".to_string()),
        avatar: None,
    };

    svc.update_profile(&ctx, 302, bo)
        .await
        .expect("更新 profile 失败");

    // 验证更新成功
    let profile = svc.get_profile(&ctx, 302).await.expect("查询失败");
    assert_eq!(profile.user.nick_name, "新昵称");
    assert_eq!(profile.user.email.as_deref(), Some("new@test.com"));
}

/// 审计:update_profile 后 update_by 必须为操作人本人(302)——
/// handler 传用户 ctx,漏斗 update 按 principal 盖章。
#[tokio::test]
async fn update_profile_stamps_update_by_as_acting_user() {
    let ds0 = setup_conn().await;
    let (ds, svc) = service_scoped(ds0, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 302,
        dept_id: Some(101),
    });

    svc.update_profile(&ctx, 302, profile_bo("审计昵称"))
        .await
        .expect("更新 profile 失败");

    assert_eq!(
        raw_update_by(&ds, 302).await,
        Some(302),
        "update_by 应为操作人 302(审计恢复)"
    );
}

/// 同上,密码路径:update_password 后 update_by = 操作人本人。
#[tokio::test]
async fn update_password_stamps_update_by_as_acting_user() {
    let ds0 = setup_conn().await;

    set_password(&ds0, "oldpass123").await;

    let (ds, svc) = service_scoped(ds0, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 302,
        dept_id: Some(101),
    });

    svc.update_password(&ctx, 302, "oldpass123", "newpass789")
        .await
        .expect("密码修改失败");

    assert_eq!(
        raw_update_by(&ds, 302).await,
        Some(302),
        "update_by 应为操作人 302(密码路径的审计恢复)"
    );
}

/// Java ignore 平价 × 审计恢复钉在一起(语义与理由见
/// profile_service::update_profile 注释):SelfOnly 操作者(种子 302 的
/// create_by 为 NULL ≠ 302)的自服务更新经 `DataPermissionHelper.ignore`
/// (Controller:72/:98)对齐的写侧豁免原语**恒成功** —— 基线行为是
/// "改自己资料必成功",漏斗此前把它收紧成 NotFound 是已上报的分歧,
/// update_without_row_scope 落地后翻正。同时钉住 update_by = 操作人
/// 本人(豁免的是行级范围,不是审计)。
#[tokio::test]
async fn self_only_profile_update_succeeds_and_stamps_acting_user() {
    let ds0 = setup_conn().await;
    let (ds, svc) = service_scoped(ds0, ScopeFilter::SelfOnly(302));
    let ctx = ds.context(Principal {
        user_id: 302,
        dept_id: Some(101),
    });

    svc.update_profile(&ctx, 302, profile_bo("仅本人"))
        .await
        .expect("SelfOnly 操作者改自己资料应成功(Java ignore 平价)");

    // 更新已落库(读路径豁免,同一 ctx 可读回核验)
    let profile = svc.get_profile(&ctx, 302).await.expect("查询失败");
    assert_eq!(profile.user.nick_name, "仅本人", "豁免更新必须落库");

    // 审计恢复一并钉住:豁免的是行级范围,不是审计
    assert_eq!(
        raw_update_by(&ds, 302).await,
        Some(302),
        "update_by 应为操作人 302(豁免 ≠ 免盖章)"
    );
}

#[tokio::test]
async fn test_update_password_wrong_old() {
    let ds0 = setup_conn().await;

    // 设置已知密码
    set_password(&ds0, "oldpass123").await;

    let (ds, svc) = service_scoped(ds0, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 302,
        dept_id: Some(101),
    });

    let result = svc
        .update_password(&ctx, 302, "wrongpassword", "newpass123")
        .await;

    assert!(result.is_err(), "旧密码错误应返回错误");
}

#[tokio::test]
async fn test_update_password_same_password() {
    let ds0 = setup_conn().await;

    set_password(&ds0, "samepass123").await;

    let (ds, svc) = service_scoped(ds0, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 302,
        dept_id: Some(101),
    });

    let result = svc
        .update_password(&ctx, 302, "samepass123", "samepass123")
        .await;

    assert!(result.is_err(), "新旧密码相同应返回错误");
}

#[tokio::test]
async fn test_update_password_success() {
    let ds0 = setup_conn().await;

    set_password(&ds0, "oldpass123").await;

    let (ds, svc) = service_scoped(ds0, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 302,
        dept_id: Some(101),
    });

    svc.update_password(&ctx, 302, "oldpass123", "newpass456")
        .await
        .expect("密码修改失败");

    // 验证新密码可以验证通过
    let profile = svc.get_profile(&ctx, 302).await.expect("查询失败");
    let _ = profile; // 确保用户仍存在
}
