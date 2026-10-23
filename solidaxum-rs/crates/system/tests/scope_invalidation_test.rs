//! invalidate 接线的失效语义测试(真实 SQLite + 真服务)。
//!
//! 不用 mock 计数器,用**值变化只能来自重查**的行为级证明:
//! - 缓存命中返回旧值 —— 由「未接线对照组」钉住(TTL 内必须陈旧);
//! - 接线后同一变更必须立即可见 —— 新旧值不同,唯一来源是重查库。
//!
//! 种子(common/mod.rs):user 302 ← role 201('2' 自定义,dept {101,103})
//! → DeptIn([101,103]);user 305 ← role 204('5' 仅本人)→ SelfOnly(305);
//! user 304(dept 101)← role 203('4' 本部门及以下)→ 子树 {101,103,104}。
//!
//! dept 写路径(insert/update/delete):部门树变更改变
//! DeptAndBelow 范围条目内嵌的子树展开 —— 新增扩大子树、换父迁移子树、
//! 软删收缩子树,全部用 304 的 {101,103,104} 作基线钉重查。

mod common;

use std::sync::{Arc, OnceLock};

use app_core::{
    infra::{ConnectionPool, Infrastructure},
    middleware::extractors::CurrentUser,
    model::login_user::LoginUser,
};
use app_data::{
    DataSource, Principal, ScopeFilter, ScopeResolver,
    sea_orm::ActiveValue::Set,
};
use common::setup_schema_and_seeds;
use system::{
    dept::{
        model::dept::{DeptCreateBo, DeptUpdateBo},
        service::dept_service::DeptService,
    },
    permission::{
        scope_resolver::{CachedScopeResolver, ScopeCacheSlot},
        service::PermissionService,
    },
    role::{
        model::role::RoleUpdateBo, repository::role_repo as role_entity,
        service::role_service::RoleService,
    },
    user::{
        model::user::UserUpdateBo, repository::user_repo as user_entity,
        service::user_service::UserService,
    },
};

/// 同一 SQLite 上的完整装配:PermissionService → CachedScopeResolver →
/// 共享槽 → role/user/dept 服务(生产 ServiceContainer::new +
/// set_scope_cache 的同构缩影)。`wired=false` 造**未接线**对照组
/// (服务持空槽,失效 no-op)。
struct Rig {
    resolver: Arc<CachedScopeResolver>,
    role_service: RoleService,
    user_service: UserService,
    dept_service: DeptService,
    ds: Arc<DataSource>,
}

fn lazy_redis_pool() -> deadpool_redis::Pool {
    deadpool_redis::Config::from_url("redis://127.0.0.1:6379")
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("创建惰性 redis 池失败")
}

async fn rig(wired: bool) -> Rig {
    // 经 `sqlite_memory_empty` 自持 DataSource(装配层,设计 9.4 的出口),
    // 建表/种子复用 common 的同一套。ds0 是 resolver-less 母体:既作
    // Rig.ds(直改通道),又是 resolver 的 unscoped 通道(生产 app.rs 同款
    // —— derive 不染母体)。
    let ds0 = Arc::new(app_data::testing::sqlite_memory_empty().await);
    setup_schema_and_seeds(&ds0).await;
    let ds = ds0.clone();
    // 第一参是 resolver 自有 unscoped 通道(resolver-free
    // 快照,与生产 admin/app.rs 同款;与 rig.ds 同语义、各自 Arc)。
    let resolver = Arc::new(CachedScopeResolver::new(
        ds0.clone(),
        Arc::new(PermissionService::for_test()),
    ));
    let slot: ScopeCacheSlot = Arc::new(OnceLock::new());
    assert!(slot.set(resolver.clone()).is_ok(), "测试槽装配只此一次");
    let infra = Infrastructure::new(
        Arc::new(ConnectionPool {
            redis_pool: lazy_redis_pool(),
        }),
        false,
        1800,
    );
    // wired=false 刻意不给服务接槽(空槽):失效调用全部 no-op,
    // 用来钉「缓存确实在命中」的陈旧基线。
    let (role_service, user_service, dept_service) = if wired {
        (
            RoleService::for_test().with_scope_cache(slot.clone()),
            UserService::new(&infra).with_scope_cache(slot.clone()),
            DeptService::for_test().with_scope_cache(slot),
        )
    } else {
        (
            RoleService::for_test(),
            UserService::new(&infra),
            DeptService::for_test(),
        )
    };
    Rig {
        resolver,
        role_service,
        user_service,
        dept_service,
        ds,
    }
}

impl Rig {
    async fn resolve(&self, user_id: i64, dept_id: Option<i64>) -> ScopeFilter {
        self.resolver
            .resolve(&Principal { user_id, dept_id })
            .await
            .expect("resolve 失败")
    }

    /// 绕过服务写路径的直改(漏斗 update,无任何失效调用)——
    /// 只有「真的重查了库」才能看见,与原裸 SQL UPDATE 同语义
    /// (审计 update_* 被盖 0/now,无害)。
    async fn set_role_data_scope(&self, role_id: i64, data_scope: &str) {
        self.ds
            .system_context()
            .update(role_entity::ActiveModel {
                role_id: Set(role_id),
                data_scope: Set(data_scope.into()),
                ..Default::default()
            })
            .await
            .expect("直改 data_scope 失败");
    }

    /// 同上:直改 user 的 dept_id(转部门前置)。
    async fn set_user_dept(&self, user_id: i64, dept_id: i64) {
        self.ds
            .system_context()
            .update(user_entity::ActiveModel {
                user_id: Set(user_id),
                dept_id: Set(Some(dept_id)),
                ..Default::default()
            })
            .await
            .expect("直改 dept_id 失败");
    }
}

fn test_user() -> CurrentUser {
    CurrentUser(LoginUser {
        user_id: 1,
        dept_id: Some(100),
        username: "test".to_string(),
        nick_name: Some("测试".to_string()),
        user_type: Some("sys_user".to_string()),
        client_id: "client".to_string(),
        client_key: None,
        device_type: None,
        permissions: std::collections::HashSet::new(),
        roles: std::collections::HashSet::new(),
        login_ip: None,
        login_date: None,
    })
}

fn role_update_bo(data_scope: &str) -> RoleUpdateBo {
    RoleUpdateBo {
        role_id: Some(201),
        role_name: "自定义数据角色".to_string(),
        role_key: "role_custom".to_string(),
        parent_id: None,
        role_sort: None,
        data_scope: Some(data_scope.to_string()),
        status: "0".to_string(),
        remark: None,
    }
}

// ============================================================
// 对照组:未接线时缓存确实命中(TTL 内陈旧)—— 上面断言「新值」的
// 测试若拆掉接线必然落回本行为,从而必然失败。
// ============================================================

#[tokio::test]
async fn unwired_role_change_keeps_stale_cache_within_ttl() {
    let rig = rig(false).await;
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::DeptIn(vec![101, 103]),
        "前置:自定义角色 → DeptIn([101,103])"
    );
    // 同一变更加在**未接线**的服务上(空槽,失效 no-op)
    let ctx = rig.ds.system_context();
    rig.role_service
        .update_by_bo(&ctx, role_update_bo("5"), &test_user())
        .await
        .unwrap();
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::DeptIn(vec![101, 103]),
        "未接线时 60s TTL 内必须返回旧值(缓存命中,非重查)"
    );
}

// ============================================================
// 角色级变更 → invalidate_all
// ============================================================

#[tokio::test]
async fn role_update_data_scope_invalidates_all() {
    let rig = rig(true).await;
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::DeptIn(vec![101, 103])
    );
    // data_scope '2' → '5':角色仍持有,但范围变 SelfOnly
    let ctx = rig.ds.system_context();
    rig.role_service
        .update_by_bo(&ctx, role_update_bo("5"), &test_user())
        .await
        .unwrap();
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::SelfOnly(302),
        "角色数据范围变更后必须重查(不得等 TTL)"
    );
}

#[tokio::test]
async fn role_update_permission_invalidates_all() {
    let rig = rig(true).await;
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::DeptIn(vec![101, 103])
    );
    let ctx = rig.ds.system_context();
    rig.role_service
        .update_role_permission(
            &ctx,
            201,
            &[],
            &[],
            "5",
            true,
            true,
            &test_user(),
        )
        .await
        .unwrap();
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::SelfOnly(302),
        "data_scope/role_dept 写路径提交后必须全清"
    );
}

#[tokio::test]
async fn role_disable_invalidates_all() {
    let rig = rig(true).await;
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::DeptIn(vec![101, 103])
    );
    // 停用角色:范围查询过滤 status='0',302 将无有效角色 → SelfOnly
    let ctx = rig.ds.system_context();
    rig.role_service
        .change_status(&ctx, 201, "1".to_string())
        .await
        .unwrap();
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::SelfOnly(302),
        "角色停启用改变其是否参与范围计算,必须全清"
    );
}

#[tokio::test]
async fn role_delete_invalidates_all() {
    let rig = rig(true).await;
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::DeptIn(vec![101, 103])
    );
    // 软删:del_flag='1' 被范围查询过滤 → 302 无角色 → SelfOnly
    let ctx = rig.ds.system_context();
    rig.role_service
        .delete_by_id(&ctx, 201, &test_user())
        .await
        .unwrap();
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::SelfOnly(302),
        "角色删除后必须全清"
    );
}

// ============================================================
// 用户级变更(角色侧入口)→ invalidate(user_id) 定点
// ============================================================

#[tokio::test]
async fn role_select_auth_users_invalidates_target_users() {
    let rig = rig(true).await;
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::DeptIn(vec![101, 103])
    );
    // 批量授予:302 追加 role 200('1' 全部)→ All
    let ctx = rig.ds.system_context();
    rig.role_service
        .select_auth_users(&ctx, 200, &[302])
        .await
        .unwrap();
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::All,
        "角色侧批量授权后目标用户必须重查"
    );
}

#[tokio::test]
async fn role_cancel_auth_user_invalidates_target_user() {
    let rig = rig(true).await;
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::DeptIn(vec![101, 103])
    );
    // 取消授权:302 失去唯一角色 → SelfOnly
    let ctx = rig.ds.system_context();
    rig.role_service
        .cancel_auth_user(&ctx, 201, 302)
        .await
        .unwrap();
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::SelfOnly(302),
        "角色侧取消授权后目标用户必须重查"
    );
}

/// **批量**取消授权 —— A、B 各自定点
/// 失效(值必须回到取消后的真实状态),用户 C(未涉)缓存不受牵连。
/// 显影剂设计同 `user_authrole_requeries_target_but_keeps_other_users_cached`:
/// 先批量授予 A/B role 200('1' 全部)并入缓存(All),取消后 A/B 重查
/// 回各自原值;同时裸改 role 203 的 data_scope('4'→'1',只有重查能看见)
/// —— C 若被误全清,重查会得到 All,`dept_ids` 直接 panic。
#[tokio::test]
async fn role_cancel_auth_users_invalidates_each_target_but_not_others() {
    let rig = rig(true).await;
    // 基线入缓存:302 ← 201(自定义);305 ← 204(仅本人);304 ← 203('4')
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::DeptIn(vec![101, 103])
    );
    assert_eq!(
        rig.resolve(305, Some(101)).await,
        ScopeFilter::SelfOnly(305)
    );
    assert_eq!(
        dept_ids(rig.resolve(304, Some(101)).await),
        vec![101, 103, 104]
    );

    // 批量授予 A(302)/B(305)role 200('1' 全部)→ 缓存刷新为 All,
    // 为"取消后值回落"提供可判别的前置状态
    let ctx = rig.ds.system_context();
    rig.role_service
        .select_auth_users(&ctx, 200, &[302, 305])
        .await
        .unwrap();
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::All,
        "前置:302 授予全部数据角色 → 缓存 All"
    );
    assert_eq!(
        rig.resolve(305, Some(101)).await,
        ScopeFilter::All,
        "前置:305 授予全部数据角色 → 缓存 All"
    );

    // 显影剂(仅对 C 有效):裸改 role 203 '4'→'1' —— C 的缓存里还是旧子树
    rig.set_role_data_scope(203, "1").await;

    // 批量取消 A/B 的 role 200:各自定点失效 → 重查回落到仅剩角色
    rig.role_service
        .cancel_auth_users(&ctx, 200, &[302, 305])
        .await
        .unwrap();
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::DeptIn(vec![101, 103]),
        "A(302)定点失效:重查回落到 role 201 的 DeptIn(不再是缓存 All)"
    );
    assert_eq!(
        rig.resolve(305, Some(101)).await,
        ScopeFilter::SelfOnly(305),
        "B(305)定点失效:重查回落到 role 204 的 SelfOnly(不再是缓存 All)"
    );
    // C(304,未涉):缓存不受牵连 —— 仍返回旧子树而非重查的 All
    assert_eq!(
        dept_ids(rig.resolve(304, Some(101)).await),
        vec![101, 103, 104],
        "C 未涉批量取消,缓存命中(若被误清,重查得 203 新值 '1' → All)"
    );
}

// ============================================================
// 用户级变更(authRole)→ invalidate(X) 重查、他人缓存不动
// ============================================================

#[tokio::test]
async fn user_authrole_requeries_target_but_keeps_other_users_cached() {
    let rig = rig(true).await;
    // 302 ← role 201(自定义);305 ← role 204(仅本人)→ 两条都入缓存
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::DeptIn(vec![101, 103])
    );
    assert_eq!(
        rig.resolve(305, Some(101)).await,
        ScopeFilter::SelfOnly(305)
    );
    // 裸改 role 204:'5' → '1'(全部)。绕过服务,只有重查能看见;
    // 305 若被误全清,重查会得到 All —— 正好当作「未失效」的显影剂。
    rig.set_role_data_scope(204, "1").await;
    // authRole:302 的角色集整体替换为 [204](user_service 接线点)
    let ctx = rig.ds.system_context();
    rig.user_service
        .insert_user_auth(&ctx, 302, &[204])
        .await
        .unwrap();
    // 302:被定点失效 → 重查:role 204 已是 '1' → All(旧缓存 DeptIn)
    assert_eq!(
        rig.resolve(302, Some(101)).await,
        ScopeFilter::All,
        "authRole 后该用户必须重查"
    );
    // 305:未被失效 → 仍走缓存:旧值 SelfOnly(305),而非重查的 All
    assert_eq!(
        rig.resolve(305, Some(101)).await,
        ScopeFilter::SelfOnly(305),
        "他人缓存不受影响(返回旧值即证明未重查)"
    );
}

// ============================================================
// 用户级变更(转部门)→ invalidate(user_id):缓存条目内嵌旧部门子树
// ============================================================

#[tokio::test]
async fn user_dept_transfer_recomputes_subtree() {
    let rig = rig(true).await;
    // 304(dept 101)← role 203('4'):子树 {103,104} ∪ 本部门 101
    assert_eq!(
        rig.resolve(304, Some(101)).await,
        ScopeFilter::DeptIn(vec![101, 103, 104])
    );
    // 转部门 101 → 100(登录身份此后带 dept 100)
    let ctx = rig.ds.system_context();
    rig.user_service
        .update_by_bo(
            &ctx,
            UserUpdateBo {
                user_id: Some(304),
                user_name: "u_dept_children".to_string(),
                dept_id: Some(100),
                nick_name: "本部门及以下".to_string(),
                phone_number: None,
                email: None,
                gender: None,
                status: None,
                remark: None,
                post_ids: None,
                role_ids: None,
            },
            &test_user(),
        )
        .await
        .unwrap();
    // 子树必须按新部门 100 重算:{101,102,103,104} ∪ 100
    assert_eq!(
        rig.resolve(304, Some(100)).await,
        ScopeFilter::DeptIn(vec![100, 101, 102, 103, 104]),
        "转部门后必须重查(旧缓存内嵌旧部门子树展开)"
    );
}

// ============================================================
// dept 写路径(insert/update/delete)→ invalidate_all
// —— 部门树变更改变 DeptAndBelow 条目内嵌的子树展开。基线:304
// (dept 101)← role 203('4')→ {101,103,104}。
// ============================================================

/// 从 ScopeFilter 取 DeptIn 的 id 集(顺序无关比较用)。
fn dept_ids(sf: ScopeFilter) -> Vec<i64> {
    match sf {
        ScopeFilter::DeptIn(ids) => {
            let mut v = ids;
            v.sort_unstable();
            v
        }
        other => panic!("期望 DeptIn,实际 {other:?}"),
    }
}

fn dept_create_bo(parent_id: i64, dept_name: &str) -> DeptCreateBo {
    DeptCreateBo {
        parent_id,
        dept_name: dept_name.to_string(),
        dept_category: None,
        order_num: 9,
        leader: None,
        phone: None,
        email: None,
        status: Some("0".to_string()),
    }
}

fn dept_update_bo(
    dept_id: i64,
    parent_id: i64,
    dept_name: &str,
) -> DeptUpdateBo {
    DeptUpdateBo {
        dept_id: Some(dept_id),
        parent_id,
        dept_name: dept_name.to_string(),
        dept_category: None,
        order_num: 9,
        leader: None,
        phone: None,
        email: None,
        status: Some("0".to_string()),
    }
}

#[tokio::test]
async fn unwired_dept_insert_keeps_stale_cache_within_ttl() {
    let rig = rig(false).await;
    assert_eq!(
        dept_ids(rig.resolve(304, Some(101)).await),
        vec![101, 103, 104]
    );
    // 同一变更加在**未接线**的服务上(空槽,失效 no-op):新增子部门
    // 105 后缓存仍返回旧子树 —— 陈旧基线,拆掉接线必然落回本行为。
    let ctx = rig.ds.system_context();
    rig.dept_service
        .insert_by_bo(&ctx, dept_create_bo(101, "测试组"), &test_user())
        .await
        .unwrap();
    assert_eq!(
        dept_ids(rig.resolve(304, Some(101)).await),
        vec![101, 103, 104],
        "未接线时 60s TTL 内必须返回旧子树(缓存命中,非重查)"
    );
}

#[tokio::test]
async fn dept_insert_invalidates_all() {
    let rig = rig(true).await;
    assert_eq!(
        dept_ids(rig.resolve(304, Some(101)).await),
        vec![101, 103, 104]
    );
    // 新增 101 的子部门:DeptAndBelow 子树扩大(新部门 id 为雪花,断言
    // 以「条目数 +1 且含三旧 id」表达,不预测生成 id)。
    let ctx = rig.ds.system_context();
    rig.dept_service
        .insert_by_bo(&ctx, dept_create_bo(101, "测试组"), &test_user())
        .await
        .unwrap();
    let ids = dept_ids(rig.resolve(304, Some(101)).await);
    assert_eq!(ids.len(), 4, "新增子部门后子树必须重查扩容: {ids:?}");
    for known in [101, 103, 104] {
        assert!(ids.contains(&known), "旧子树成员不得丢失: {ids:?}");
    }
}

#[tokio::test]
async fn dept_reparent_invalidates_all() {
    let rig = rig(true).await;
    assert_eq!(
        dept_ids(rig.resolve(304, Some(101)).await),
        vec![101, 103, 104]
    );
    // 换父:104 从 101 迁到 102 → 101 的子树剩 {101,103}。
    // (级联 ancestors 走 ctx.transaction,写-后-失效在事务提交后。)
    let ctx = rig.ds.system_context();
    rig.dept_service
        .update_by_bo(&ctx, dept_update_bo(104, 102, "后端组"), &test_user())
        .await
        .unwrap();
    assert_eq!(
        dept_ids(rig.resolve(304, Some(101)).await),
        vec![101, 103],
        "换父迁移子树后必须重查(旧缓存内嵌旧子树展开)"
    );
}

#[tokio::test]
async fn dept_delete_invalidates_all() {
    let rig = rig(true).await;
    assert_eq!(
        dept_ids(rig.resolve(304, Some(101)).await),
        vec![101, 103, 104]
    );
    // 前置:种子 user 404 在 dept 104 —— 先挪走,让删除守卫(部门下
    // 存在用户 → 拒)让路,探针之后的软删才能到达。
    rig.set_user_dept(404, 101).await;
    // 软删 104:从任何 DeptAndBelow 子树移除 → 子树剩 {101,103}。
    let ctx = rig.ds.system_context();
    rig.dept_service
        .delete_by_id(&ctx, 104, &test_user())
        .await
        .unwrap();
    assert_eq!(
        dept_ids(rig.resolve(304, Some(101)).await),
        vec![101, 103],
        "软删部门后必须重查(旧缓存仍含已删部门)"
    );
}
