mod common;

use std::collections::HashSet;

use app_data::sea_orm::ActiveValue::Set;
use common::setup_db;
use system::{
    permission::{data_scope::DataScope, service::PermissionService},
    relation::user_role,
};

fn sorted(dept_ids: &[i64]) -> Vec<i64> {
    let mut v = dept_ids.to_vec();
    v.sort_unstable();
    v
}

/// 服务四方法收 ctx。测试统一传 system_context
/// (resolver 自有通道的生产语义;所用读全为豁免方法 —— SysRoleMapper
/// :89 join / SysMenuMapper 零 @DataPermission —— any-ctx 同口径,
/// 用户面 principal-ctx 的行为由 admin 契约测试覆盖)。
async fn svc_and_ctx() -> (PermissionService, app_data::DataSource) {
    (PermissionService::for_test(), setup_db().await)
}

// ============================================================
// get_role_permission
// ============================================================

#[tokio::test]
async fn test_get_role_permission_super_admin() {
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let roles = svc
        .get_role_permission(&ctx, 1761100000000000001)
        .await
        .expect("查询失败");

    assert_eq!(roles, ["superadmin".to_string()].into_iter().collect());
}

#[tokio::test]
async fn test_get_role_permission_normal_user() {
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let roles = svc.get_role_permission(&ctx, 301).await.expect("查询失败");

    assert_eq!(roles, ["role_all".to_string()].into_iter().collect());
}

// ============================================================
// get_menu_permission
// ============================================================

#[tokio::test]
async fn test_get_menu_permission_super_admin() {
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let perms = svc
        .get_menu_permission(&ctx, 1761100000000000001)
        .await
        .expect("查询失败");

    assert_eq!(perms, ["*:*:*".to_string()].into_iter().collect());
}

#[tokio::test]
async fn test_get_menu_permission_normal_user() {
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let perms = svc.get_menu_permission(&ctx, 302).await.expect("查询失败");

    let expected: HashSet<String> = [
        "system:user:list".to_string(),
        "system:role:list".to_string(),
    ]
    .into_iter()
    .collect();
    assert_eq!(perms, expected);
}

// ============================================================
// get_menu_tree
// ============================================================

#[tokio::test]
async fn test_get_menu_tree_super_admin() {
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let menus = svc
        .get_menu_tree(&ctx, 1761100000000000001)
        .await
        .expect("查询失败");

    let menu_ids: HashSet<i64> = menus.iter().map(|m| m.menu_id).collect();
    let expected: HashSet<i64> = [501, 502, 504].into_iter().collect();
    assert_eq!(menu_ids, expected, "超管应看到全部 M/C 菜单(不含 F 和禁用)");
}

#[tokio::test]
async fn test_get_menu_tree_normal_user() {
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let menus = svc.get_menu_tree(&ctx, 302).await.expect("查询失败");

    let menu_ids: HashSet<i64> = menus.iter().map(|m| m.menu_id).collect();
    let expected: HashSet<i64> = [501, 502, 504].into_iter().collect();
    assert_eq!(
        menu_ids, expected,
        "u_custom 应通过 role_menu 看到 501,502,504"
    );
}

// ============================================================
// get_data_scope — 6 级数据权限
// ============================================================

#[tokio::test]
async fn test_data_scope_super_admin() {
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let scope = svc
        .get_data_scope(&ctx, 1761100000000000001, Some(100))
        .await
        .expect("查询失败");

    assert!(scope.is_all(), "超管应为 All");
}

#[tokio::test]
async fn test_data_scope_level_1_all() {
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let scope = svc
        .get_data_scope(&ctx, 301, Some(101))
        .await
        .expect("查询失败");

    assert!(scope.is_all(), "data_scope=1 应为 All");
}

#[tokio::test]
async fn test_data_scope_level_2_custom() {
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let scope = svc
        .get_data_scope(&ctx, 302, Some(101))
        .await
        .expect("查询失败");

    match scope {
        DataScope::Custom(ids) => assert_eq!(sorted(&ids), vec![101, 103]),
        other => panic!("data_scope=2 应为 Custom, got {:?}", other),
    }
}

#[tokio::test]
async fn test_data_scope_level_3_dept() {
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let scope = svc
        .get_data_scope(&ctx, 303, Some(101))
        .await
        .expect("查询失败");

    match scope {
        DataScope::Custom(ids) => assert_eq!(ids, vec![101]),
        other => panic!("data_scope=3 应为 Custom([101]), got {:?}", other),
    }
}

#[tokio::test]
async fn test_data_scope_level_4_dept_children() {
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let scope = svc
        .get_data_scope(&ctx, 304, Some(101))
        .await
        .expect("查询失败");

    match scope {
        DataScope::Custom(ids) => assert_eq!(sorted(&ids), vec![101, 103, 104]),
        other => {
            panic!("data_scope=4 应为 Custom([101,103,104]), got {:?}", other)
        }
    }
}

#[tokio::test]
async fn test_data_scope_level_5_self_only() {
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let scope = svc
        .get_data_scope(&ctx, 305, Some(101))
        .await
        .expect("查询失败");

    match scope {
        DataScope::SelfOnly(uid) => assert_eq!(uid, 305),
        other => panic!("data_scope=5 应为 SelfOnly(305), got {:?}", other),
    }
}

#[tokio::test]
async fn test_data_scope_level_6_dept_children_or_self() {
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let scope = svc
        .get_data_scope(&ctx, 306, Some(101))
        .await
        .expect("查询失败");

    match scope {
        DataScope::CustomOrSelf { dept_ids, user_id } => {
            assert_eq!(sorted(&dept_ids), vec![101, 103, 104]);
            assert_eq!(user_id, 306);
        }
        other => panic!("data_scope=6 应为 CustomOrSelf, got {:?}", other),
    }
}

#[tokio::test]
async fn test_data_scope_no_role() {
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let scope = svc
        .get_data_scope(&ctx, 307, Some(101))
        .await
        .expect("查询失败");

    match scope {
        DataScope::SelfOnly(uid) => assert_eq!(uid, 307),
        other => panic!("无角色应为 SelfOnly(307), got {:?}", other),
    }
}

// ============================================================
// 多角色 OR 并集
// ============================================================

#[tokio::test]
async fn test_data_scope_multi_role_custom_union_dept_and_below() {
    // 用户 308 同时拥有:
    //   - role 201(自定义,depts 101/103)
    //   - role 203(本部门及以下,dept_id=101 → 子树 101/103/104)
    // 并集 = {101,103} ∪ {101,103,104} = {101,103,104},无仅本人贡献 → Custom
    let (svc, ds) = svc_and_ctx().await;
    let ctx = ds.system_context();

    let scope = svc
        .get_data_scope(&ctx, 308, Some(101))
        .await
        .expect("查询失败");

    match scope {
        DataScope::Custom(ids) => assert_eq!(sorted(&ids), vec![101, 103, 104]),
        other => {
            panic!("多角色并集应为 Custom([101,103,104]), got {:?}", other)
        }
    }
}

#[tokio::test]
async fn test_data_scope_multi_role_all_dominates() {
    // 用户 308 若额外拥有 role 200(data_scope=1),则整体应为 All。
    // 这里通过新建一条 user_role 关联模拟(关联表无审计列,漏斗 insert 直插)。
    let ds = setup_db().await;
    let ctx = ds.system_context();
    ctx.insert(user_role::ActiveModel {
        user_id: Set(308),
        role_id: Set(200),
    })
    .await
    .expect("insert ur 失败");
    let svc = PermissionService::for_test();

    let scope = svc
        .get_data_scope(&ctx, 308, Some(101))
        .await
        .expect("查询失败");

    assert!(scope.is_all(), "任一角色为 All → 整体 All");
}
