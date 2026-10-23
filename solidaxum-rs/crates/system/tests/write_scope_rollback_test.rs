//! 写路径不施行级范围的判例。
//!
//! 两半各自成对:
//! - **repo 证词**:经 repo 直调(绕过 handler 探针),越权目标的写**成功** ——
//!   证明 update 的 WHERE 里没有行级范围;
//! - **探针证词**:经 service 的 check_*_data_scope,越权目标**被拒** ——
//!   证明安全没丢,只是从 WHERE 挪到了 Java 对位的显式探针。
//!
//! 操作者固定为受限 DeptIn([101])(技术部);越权靶标固定为部门 100 的
//! 用户 401 / 部门 102 的销售部 —— 均由 common 种子提供。dept 判例
//! 另起范围 DeptIn([102,103]),跨范围子 1102 由判例自种。

mod common;

use std::sync::Arc;

use app_core::infra::{ConnectionPool, Infrastructure};
use app_data::{DataSource, Principal, ScopeFilter, testing::FixedResolver};
use common::setup_schema_and_seeds;
use system::user::{
    repository::user_repo::UserRepository, service::user_service::UserService,
};

fn lazy_redis_pool() -> Arc<ConnectionPool> {
    let pool = deadpool_redis::Config::from_url("redis://127.0.0.1:6379")
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("创建惰性 redis 池失败");
    Arc::new(ConnectionPool { redis_pool: pool })
}

async fn setup_conn() -> Arc<DataSource> {
    let ds = Arc::new(app_data::testing::sqlite_memory_empty().await);
    setup_schema_and_seeds(&ds).await;
    ds
}

fn scoped_infra(
    ds0: Arc<DataSource>,
    sf: ScopeFilter,
) -> (Arc<DataSource>, Infrastructure) {
    let ds = Arc::new(ds0.derive_with_resolver(Arc::new(FixedResolver(sf))));
    let infra = Infrastructure::new(lazy_redis_pool(), false, 1800);
    assert!(
        infra.set_data_source(ds.clone()),
        "测试装配 DataSource 只此一次"
    );
    (ds.clone(), infra)
}

/// repo 证词:受限 DeptIn([101]) 的操作者,经 repo 直调改部门 100 的用户 401
/// 的状态 —— WHERE 只剩主键 + 软删存活,越权目标**改得动**。
#[tokio::test]
async fn user_update_status_no_longer_blocked_by_row_scope() {
    let db = setup_conn().await;
    let (ds, _infra) = scoped_infra(db, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 303,
        dept_id: Some(101),
    });
    let repo = UserRepository::new();

    repo.update_status(&ctx, 401, "1")
        .await
        .expect("回退后越权目标的 update 不应再被行级范围拦住");

    let row = repo
        .select_by_id(&ds.system_context(), &401)
        .await
        .expect("复核查询失败")
        .expect("401 应存在");
    assert_eq!(row.status, "1", "状态应已落库");
}

/// 探针证词:同一操作者、同一靶标,经 service 探针 —— 必须被拒。
/// 这是 Java changeStatus:273 / edit:201 / resetPwd:256 的对位拦截。
#[tokio::test]
async fn user_probe_still_denies_foreign_target() {
    let db = setup_conn().await;
    let (ds, infra) = scoped_infra(db, ScopeFilter::DeptIn(vec![101]));
    let svc = UserService::new(&infra);
    let ctx = ds.context(Principal {
        user_id: 303,
        dept_id: Some(101),
    });

    let err = svc.check_user_data_scope(&ctx, 401).await;
    assert!(
        matches!(err, Err(app_core::error::AppError::NotFound(_))),
        "越权靶标必须被探针拒绝,实际: {err:?}"
    );

    svc.check_user_data_scope(&ctx, 402)
        .await
        .expect("本部门(101)靶标应放行");
}

/// repo 证词:角色的 update_status 不被行级范围拦住。
/// 种子角色的 create_dept 为 NULL,对 DeptIn([101]) 恒不可见,
/// 越权写必然成功。
#[tokio::test]
async fn role_update_status_no_longer_blocked_by_row_scope() {
    use system::role::repository::role_repo::RoleRepository;

    let db = setup_conn().await;
    let (ds, _infra) = scoped_infra(db, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 303,
        dept_id: Some(101),
    });
    let repo = RoleRepository::new();

    repo.update_status(&ctx, 200, "1")
        .await
        .expect("回退后越权角色的 update 不应再被行级范围拦住");
}

/// 探针证词:新建的 check_role_data_scope 对同一靶标必须拒绝。
#[tokio::test]
async fn role_probe_denies_out_of_scope_role() {
    use system::role::service::role_service::RoleService;

    let db = setup_conn().await;
    let (ds, infra) = scoped_infra(db, ScopeFilter::DeptIn(vec![101]));
    let svc = RoleService::new(&infra);
    let ctx = ds.context(Principal {
        user_id: 303,
        dept_id: Some(101),
    });

    let err = svc.check_role_data_scope(&ctx, 200).await;
    assert!(
        matches!(err, Err(app_core::error::AppError::NotFound(_))),
        "create_dept 为 NULL 的角色对受限范围不可见,必须被探针拒绝,实际: {err:?}"
    );
}

/// 探针对超管形态(ScopeFilter::All)放行 —— 对齐 Java
/// checkRoleDataScope 的 `LoginHelper.isSuperAdmin()` 早退。
#[tokio::test]
async fn role_probe_allows_all_scope() {
    use system::role::service::role_service::RoleService;

    let db = setup_conn().await;
    let (ds, infra) = scoped_infra(db, ScopeFilter::All);
    let svc = RoleService::new(&infra);
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    svc.check_role_data_scope(&ctx, 200)
        .await
        .expect("All 范围应放行");
}

/// 换父级联对跨范围子部门不整单失败。
///
/// 级联写不受辖:读半边 without_row_scope 取到全部后代,写半边
/// update 打到跨范围子 1102 照常成功,子孙 ancestors 正确级联
/// (若写半边受辖,此处 0 行 → NotFound → 整笔回滚 → HTTP 404,
/// 自定义范围管理员**永远**做不成换父)。
#[tokio::test]
async fn dept_reparent_cascade_succeeds_across_scope_boundary() {
    use app_core::{
        middleware::extractors::CurrentUser, model::login_user::LoginUser,
    };
    use app_data::sea_orm::ActiveValue::Set;
    use system::dept::{
        model::dept::DeptUpdateBo, repository::dept_repo as dept_entity,
        service::dept_service::DeptService,
    };

    let db = setup_conn().await;
    // 103(前端组)下插一个跨范围子 1102
    db.system_context()
        .insert_unstamped(dept_entity::ActiveModel {
            dept_id: Set(1102),
            parent_id: Set(103),
            ancestors: Set(Some("0,100,101,103".into())),
            dept_name: Set("跨范围子组".into()),
            order_num: Set(0),
            status: Set("0".into()),
            del_flag: Set("0".into()),
            ..Default::default()
        })
        .await
        .expect("插入跨范围子部门失败");

    // 操作者范围 = [102, 103]:103(靶标)与 102(新父)可见,子 1102 不可见
    let (ds, infra) = scoped_infra(db, ScopeFilter::DeptIn(vec![102, 103]));
    let svc = DeptService::new(&infra);
    let ctx = ds.context(Principal {
        user_id: 303,
        dept_id: Some(101),
    });
    let user = CurrentUser(LoginUser {
        user_id: 303,
        dept_id: Some(101),
        username: "u_dept".to_string(),
        nick_name: None,
        user_type: None,
        client_id: "test".to_string(),
        client_key: None,
        device_type: None,
        permissions: std::collections::HashSet::new(),
        roles: std::collections::HashSet::new(),
        login_ip: None,
        login_date: None,
    });

    let bo = DeptUpdateBo {
        dept_id: Some(103),
        parent_id: 102,
        dept_name: "前端组".to_string(),
        dept_category: None,
        order_num: 0,
        leader: None,
        phone: None,
        email: None,
        status: Some("0".to_string()),
    };

    svc.update_by_bo(&ctx, bo, &user)
        .await
        .expect("换父不应因跨范围子部门整单失败");

    // 复核:103 已挂到 102 下,且子 1102 的 ancestors 级联更新
    let vctx = ds.system_context();
    let d103 = svc
        .dept_repo()
        .select_by_id(&vctx, &103)
        .await
        .expect("查询失败")
        .expect("103 应存在");
    assert_eq!(d103.parent_id, 102, "103 应已挂到 102 下");
    assert_eq!(
        d103.ancestors.as_deref(),
        Some("0,100,102"),
        "103 的 ancestors 应重算"
    );

    let d1102 = svc
        .dept_repo()
        .select_by_id(&vctx, &1102)
        .await
        .expect("查询失败")
        .expect("1102 应存在");
    assert_eq!(
        d1102.ancestors.as_deref(),
        Some("0,100,102,103"),
        "跨范围子部门的 ancestors 必须级联更新"
    );
}

/// repo 证词:岗位软删不被行级范围拦住。
/// 种子岗位 1761200000000000001(ceo)在部门 101;操作者范围
/// DeptIn([102]) 对它恒不可见,越权软删必然成功。
#[tokio::test]
async fn post_soft_delete_no_longer_blocked_by_row_scope() {
    use system::post::repository::post_repo::PostRepository;

    let db = setup_conn().await;
    let (ds, _infra) = scoped_infra(db, ScopeFilter::DeptIn(vec![102]));
    let ctx = ds.context(Principal {
        user_id: 303,
        dept_id: Some(102),
    });
    let repo = PostRepository::new();

    repo.delete_by_id(&ctx, &1761200000000000001)
        .await
        .expect("回退后越权岗位的软删不应再被行级范围拦住");
}
