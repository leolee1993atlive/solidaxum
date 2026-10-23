mod common;

use std::sync::Arc;

use app_core::{
    infra::{ConnectionPool, Infrastructure},
    middleware::extractors::CurrentUser,
    model::login_user::LoginUser,
};
use app_data::{
    DataSource, Principal, ScopeFilter, sea_orm::ActiveValue::Set,
    testing::FixedResolver,
};
use common::{setup_db, setup_schema_and_seeds};
use system::dept::{
    model::dept::DeptUpdateBo, repository::dept_repo as dept_entity,
    service::dept_service::DeptService,
};

fn mock_current_user() -> CurrentUser {
    CurrentUser(LoginUser {
        user_id: 1,
        dept_id: Some(100),
        username: "test_admin".to_string(),
        nick_name: Some("测试".to_string()),
        user_type: None,
        client_id: "test".to_string(),
        client_key: None,
        device_type: None,
        permissions: std::collections::HashSet::new(),
        roles: std::collections::HashSet::new(),
        login_ip: None,
        login_date: None,
    })
}

/// 回归测试: 移动部门到新父部门时，ancestors 级联更新不应损坏新父部门或兄弟部门
///
/// 初始结构:
///   100 总部
///     101 技术部
///       103 前端组
///       104 后端组
///         1101 后端子组
///
/// 操作: 将 104 从 101 移动到 103
/// 验证:
///   - 104 的 ancestors 变为 "0,100,101,103"
///   - 103 (新父部门) 的 ancestors 保持 "0,100,101" 不变
///   - 1101 (104 的子部门) 的 ancestors 变为 "0,100,101,103,104"
///   - 101 (原父部门) 的 ancestors 保持 "0,100" 不变
#[tokio::test]
async fn test_dept_move_preserves_siblings_and_parent() {
    let ds = setup_db().await;

    // 添加 104 的子部门 1101(免盖章种子,同原裸 INSERT)
    ds.system_context()
        .insert_unstamped(dept_entity::ActiveModel {
            dept_id: Set(1101),
            parent_id: Set(104),
            ancestors: Set(Some("0,100,101,104".into())),
            dept_name: Set("后端子组".into()),
            order_num: Set(0),
            status: Set("0".into()),
            del_flag: Set("0".into()),
            ..Default::default()
        })
        .await
        .expect("插入子部门失败");

    let service = DeptService::for_test();
    let user = mock_current_user();
    let ctx = ds.system_context();

    // 将 104 从 101 移动到 103
    let bo = DeptUpdateBo {
        dept_id: Some(104),
        parent_id: 103,
        dept_name: "后端组".to_string(),
        dept_category: None,
        order_num: 0,
        leader: None,
        phone: None,
        email: None,
        status: Some("0".to_string()),
    };

    service
        .update_by_bo(&ctx, bo, &user)
        .await
        .expect("移动部门失败");

    // 验证 104 的 ancestors 正确更新(复核走 service 自持 ds 的 system 桥)
    let vctx = ds.system_context();
    let dept_104 = service
        .dept_repo()
        .select_by_id(&vctx, &104)
        .await
        .expect("查询失败")
        .expect("部门 104 不存在");
    assert_eq!(
        dept_104.ancestors.as_deref(),
        Some("0,100,101,103"),
        "104 的 ancestors 应为 0,100,101,103"
    );

    // 关键验证: 103 (新父部门) 的 ancestors 不应被修改
    let dept_103 = service
        .dept_repo()
        .select_by_id(&vctx, &103)
        .await
        .expect("查询失败")
        .expect("部门 103 不存在");
    assert_eq!(
        dept_103.ancestors.as_deref(),
        Some("0,100,101"),
        "103 (新父部门) 的 ancestors 不应被修改"
    );

    // 验证 101 (原父部门) 不受影响
    let dept_101 = service
        .dept_repo()
        .select_by_id(&vctx, &101)
        .await
        .expect("查询失败")
        .expect("部门 101 不存在");
    assert_eq!(
        dept_101.ancestors.as_deref(),
        Some("0,100"),
        "101 的 ancestors 不应被修改"
    );

    // 验证 1101 (104 的子部门) ancestors 正确级联更新
    let dept_1101 = service
        .dept_repo()
        .select_by_id(&vctx, &1101)
        .await
        .expect("查询失败")
        .expect("部门 1101 不存在");
    assert_eq!(
        dept_1101.ancestors.as_deref(),
        Some("0,100,101,103,104"),
        "1101 的 ancestors 应级联更新为 0,100,101,103,104"
    );
}

/// 测试: 防止将部门移动到自己的子部门下 (循环检测)
#[tokio::test]
async fn test_dept_move_rejects_cycle() {
    let ds = setup_db().await;
    let service = DeptService::for_test();
    let user = mock_current_user();
    let ctx = ds.system_context();

    // 尝试将 101 移动到 103 (103 是 101 的子部门)
    let bo = DeptUpdateBo {
        dept_id: Some(101),
        parent_id: 103,
        dept_name: "技术部".to_string(),
        dept_category: None,
        order_num: 0,
        leader: None,
        phone: None,
        email: None,
        status: Some("0".to_string()),
    };

    let result = service.update_by_bo(&ctx, bo, &user).await;
    assert!(result.is_err(), "将部门移动到自己的子部门下应被拒绝");
}

/// 测试: 防止将部门的父部门设为自己
#[tokio::test]
async fn test_dept_move_rejects_self_parent() {
    let ds = setup_db().await;
    let service = DeptService::for_test();
    let user = mock_current_user();
    let ctx = ds.system_context();

    let bo = DeptUpdateBo {
        dept_id: Some(101),
        parent_id: 101,
        dept_name: "技术部".to_string(),
        dept_category: None,
        order_num: 0,
        leader: None,
        phone: None,
        email: None,
        status: Some("0".to_string()),
    };

    let result = service.update_by_bo(&ctx, bo, &user).await;
    assert!(result.is_err(), "将部门的父部门设为自己应被拒绝");
}

// ═════════════ checkDeptDataScope 探针(funnel-native) ═════════════

/// 惰性 Redis 池(不真正连接)—— 与 user_service_test 同一处置,
/// 本节新增路径不触 Redis。
fn lazy_redis_pool() -> Arc<ConnectionPool> {
    let pool = deadpool_redis::Config::from_url("redis://127.0.0.1:6379")
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("创建惰性 redis 池失败");
    Arc::new(ConnectionPool { redis_pool: pool })
}

/// scoped infra 的建库:经 `sqlite_memory_empty` 自持 DataSource(装配层,
/// 设计 9.4 的出口),建表/种子复用 common 的同一套。返回 resolver-less
/// 母体,由 [`scoped_infra`] 派生受辖 ds。
async fn setup_conn() -> Arc<DataSource> {
    let ds = Arc::new(app_data::testing::sqlite_memory_empty().await);
    setup_schema_and_seeds(&ds).await;
    ds
}

/// 受辖 DataSource + 基础设施(生产装配同构:ds0 → derive_with_resolver →
/// set_data_source → 服务构造在装配后)。返回 (ds, infra):探针测试用
/// ds.context(...) 建受辖 ctx,handler 组合测试再用同一 infra 建
/// UserService(与生产 state.infra 同源)。resolver 用 FixedResolver 免搭
/// 角色语义。
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

/// 探针判例:受限 DeptIn([101]) 操作者 ——
/// ① 目标部门 102(销售部,越权)→ NotFound;② 目标 101(本部门)
/// → 放行;④ dept_id None → 放行(Java `ObjectUtil.isNull` 早退)。
/// ③(超管)见下方独立用例。对齐 Java checkUserDataScope 探针先例
/// (user_service_test),不存在与越权同形态 NotFound,不泄露存在性。
#[tokio::test]
async fn check_dept_data_scope_probe_verdicts() {
    let db = setup_conn().await;
    let (ds, infra) = scoped_infra(db, ScopeFilter::DeptIn(vec![101]));
    let svc = DeptService::new(&infra);
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    let err = svc.check_dept_data_scope(&ctx, Some(102)).await;
    assert!(
        matches!(err, Err(app_core::error::AppError::NotFound(_))),
        "越权部门(102 销售部)→ NotFound,实际: {err:?}"
    );
    svc.check_dept_data_scope(&ctx, Some(101))
        .await
        .expect("本部门(101 ∈ DeptIn)应放行");
    svc.check_dept_data_scope(&ctx, None)
        .await
        .expect("dept_id=None 应放行(Java 空值早退)");
}

/// ③ 超管 → 任意部门放行。生产路径超管由 CachedScopeResolver 解析为
/// All(get_data_scope 对 user_id=1 直通 DataScope::All),FixedResolver(All)
/// 是同一过滤器的忠实替身 —— 探针经漏斗自然通过,无需 isSuperAdmin
/// 特判(与 check_user_data_scope 同口径)。
#[tokio::test]
async fn check_dept_data_scope_superadmin_passes_any_dept() {
    let db = setup_conn().await;
    let (ds, infra) = scoped_infra(db, ScopeFilter::All);
    let svc = DeptService::new(&infra);
    let ctx = ds.context(Principal {
        user_id: app_core::SUPER_ADMIN_USER_ID,
        dept_id: Some(100),
    });

    // 种子全部门(100 总部 / 101 / 102 / 103 / 104)逐一放行
    for target in [100, 101, 102, 103, 104] {
        svc.check_dept_data_scope(&ctx, Some(target))
            .await
            .unwrap_or_else(|e| panic!("超管对部门 {target} 应放行: {e}"));
    }
}

/// handler 组合(user add,Java SysUserController:177 平价):
/// check_dept_data_scope → insert_by_bo —— 越权部门创建用户被探针
/// 拒绝且不落库;本部门照常落库。HTTP 层的组合受 admin testkit 恒 All
/// resolver 限定(testkit.rs 的既定分工),范围分支在 system 层钉住。
#[tokio::test]
async fn user_add_in_foreign_dept_rejected_before_insert() {
    let db = setup_conn().await;
    let (ds, infra) = scoped_infra(db, ScopeFilter::DeptIn(vec![101]));
    let dept_svc = DeptService::new(&infra);
    let user_svc =
        system::user::service::user_service::UserService::new(&infra);
    let repo = system::user::repository::user_repo::UserRepository::new();
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });
    let mut operator = mock_current_user();
    operator.0.user_id = 301;
    operator.0.dept_id = Some(101);

    let make_bo = |dept_id: i64, user_name: &str| {
        system::user::model::user::UserCreateBo {
            user_name: user_name.to_string(),
            password: "123456".to_string(),
            dept_id: Some(dept_id),
            nick_name: "探针组合用户".to_string(),
            phone_number: None,
            email: None,
            gender: None,
            status: Some("0".to_string()),
            remark: None,
            post_ids: None,
            role_ids: None,
        }
    };

    // 越权:目标部门 102(销售部)→ 探针拒,insert 不被调用
    let bo_foreign = make_bo(102, "foreign_dept_user");
    let verdict = dept_svc
        .check_dept_data_scope(&ctx, bo_foreign.dept_id)
        .await;
    assert!(
        matches!(verdict, Err(app_core::error::AppError::NotFound(_))),
        "越权部门创建用户应被探针拒绝,实际: {verdict:?}"
    );
    assert!(
        repo.select_by_user_name(&ds.system_context(), "foreign_dept_user")
            .await
            .expect("复核查询失败")
            .is_none(),
        "被拒的用户不得落库"
    );

    // 对照:目标部门 101(本部门)→ 探针过 + 落库成功
    let bo_own = make_bo(101, "own_dept_user");
    dept_svc
        .check_dept_data_scope(&ctx, bo_own.dept_id)
        .await
        .expect("本部门应放行");
    user_svc
        .insert_by_bo(&ctx, bo_own, &operator)
        .await
        .expect("本部门创建用户应成功");
    assert!(
        repo.select_by_user_name(&ds.system_context(), "own_dept_user")
            .await
            .expect("复核查询失败")
            .is_some(),
        "放行后用户应已落库"
    );
}

// ═════════════ service 级范围判例 ═════════════

/// dept_tree 范围语义锚:DeptAndBelow(生产 resolver 已展开为
/// DeptIn([101,103,104]))经受辖 selectDeptList(SysDeptMapper.java:36)
/// → service.list 只回本部门子树 —— 由漏斗按 sys_dept DataPolicy
/// (dept_id)施加(子树展开在 resolver 期)。
#[tokio::test]
async fn service_list_under_restricted_ctx_returns_own_subtree_only() {
    let db = setup_conn().await;
    let (ds, infra) =
        scoped_infra(db, ScopeFilter::DeptIn(vec![101, 103, 104]));
    let svc = DeptService::new(&infra);
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    let vos = svc
        .list(
            &ctx,
            system::dept::model::dept::DeptQueryBo {
                dept_name: None,
                status: None,
                dept_id: None,
                dept_category: None,
            },
        )
        .await
        .expect("受限列表查询失败");
    let ids: std::collections::HashSet<i64> =
        vos.iter().map(|d| d.dept_id).collect();
    let expected: std::collections::HashSet<i64> =
        [101, 103, 104].into_iter().collect();
    assert_eq!(ids, expected, "受限 ctx 下列表只见本部门子树");
}

/// Java remove:143 探针(居守卫之后):DeptAndBelow 展开集下删越权部门
/// 102 —— 102 是叶子、无用户、无岗位,守卫放行,由探针拒(NotFound,
/// 不泄露存在性);范围内叶子 105(新插)照常软删成功。
#[tokio::test]
async fn service_delete_foreign_dept_denied_by_probe() {
    let db = setup_conn().await;
    // 范围内叶子 105(新插,无用户/无岗位,守卫必放行;免盖章种子)
    db.system_context()
        .insert_unstamped(dept_entity::ActiveModel {
            dept_id: Set(105),
            parent_id: Set(101),
            ancestors: Set(Some("0,100,101".into())),
            dept_name: Set("测试叶子部门".into()),
            order_num: Set(9),
            status: Set("0".into()),
            del_flag: Set("0".into()),
            ..Default::default()
        })
        .await
        .expect("插入叶子部门失败");
    let (ds, infra) =
        scoped_infra(db, ScopeFilter::DeptIn(vec![101, 103, 104, 105]));
    let svc = DeptService::new(&infra);
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });
    let user = mock_current_user();

    // 越权:删 102(销售部,叶子/无用户/无岗位)→ 守卫放行,探针拒
    let err = svc.delete_by_id(&ctx, 102, &user).await;
    assert!(
        matches!(err, Err(app_core::error::AppError::NotFound(_))),
        "越权删除应被探针拒绝(NotFound),实际: {err:?}"
    );

    // 对照:范围内叶子 105 → 守卫过 + 探针过 + 软删落库
    svc.delete_by_id(&ctx, 105, &user)
        .await
        .expect("范围内叶子删除应成功");
    let vctx = ds.system_context();
    let gone = svc
        .dept_repo()
        .select_by_id(&vctx, &105)
        .await
        .expect("复核查询失败");
    assert!(gone.is_none(), "软删后 105 不得再可见");
}

/// Java ServiceImpl:344-346 换父探针:受限 DeptIn([101]) 把 101 挂到
/// 越权父 102 → NotFound(新父部门必须可见)。bo.deptId 的探针
/// (Java edit:106)在 handler 承担,HTTP 组合受 admin testkit 恒 All
/// resolver 限定,范围分支在 system 层钉住。
#[tokio::test]
async fn service_update_reparent_into_foreign_dept_denied() {
    let db = setup_conn().await;
    let (ds, infra) = scoped_infra(db, ScopeFilter::DeptIn(vec![101]));
    let svc = DeptService::new(&infra);
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });
    let user = mock_current_user();

    let bo = DeptUpdateBo {
        dept_id: Some(101),
        parent_id: 102,
        dept_name: "技术部".to_string(),
        dept_category: None,
        order_num: 1,
        leader: None,
        phone: None,
        email: None,
        status: Some("0".to_string()),
    };
    let err = svc.update_by_bo(&ctx, bo, &user).await;
    assert!(
        matches!(err, Err(app_core::error::AppError::NotFound(_))),
        "挂到越权父部门应 NotFound,实际: {err:?}"
    );

    // 复核:101 未被改动(仍挂 100 下)
    let vctx = ds.system_context();
    let dept = svc
        .dept_repo()
        .select_by_id(&vctx, &101)
        .await
        .expect("复核查询失败")
        .expect("101 应存在");
    assert_eq!(dept.parent_id, 100, "被拒后 parent_id 不得变化");
}

/// 换父入口对已删靶标的拒绝契约。写路径不施行级范围,跨范围子部门
/// 不再是失败源(见 write_scope_rollback_test 的
/// dept_reparent_cascade_succeeds_across_scope_boundary),钉:靶标已软删
/// → 自身 update 命中 0 行(软删存活过滤)→ NotFound。事务包裹保留
/// (Java @Transactional ServiceImpl:337 平价);级联写无数据级失败源,
/// 中途回滚无可移植触发,不单独立判。
#[tokio::test]
async fn service_update_reparent_rejects_soft_deleted_target() {
    let db = setup_conn().await;
    // 把 103 软删 —— 后续换父的 update 命中 0 行(软删存活过滤)→ NotFound
    db.system_context()
        .update_without_row_scope(dept_entity::ActiveModel {
            dept_id: Set(103),
            del_flag: Set("1".into()),
            ..Default::default()
        })
        .await
        .expect("软删 103 失败");

    let (ds, infra) = scoped_infra(db, ScopeFilter::All);
    let svc = DeptService::new(&infra);
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(100),
    });
    let user = mock_current_user();

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

    let err = svc.update_by_bo(&ctx, bo, &user).await;
    assert!(err.is_err(), "已删靶标的换父必须失败");
}
