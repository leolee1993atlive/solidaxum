mod common;

use std::collections::HashSet;

use app_core::{
    middleware::extractors::CurrentUser, model::login_user::LoginUser,
};
use app_data::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use common::setup_db;
use system::{
    menu::repository::menu_repo::MenuRepository,
    role::{
        repository::role_repo::{self as role_entity, RoleRepository},
        service::role_service::RoleService,
    },
};

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
        permissions: HashSet::new(),
        roles: HashSet::new(),
        login_ip: None,
        login_date: None,
    })
}

/// role_service.rs update_role_permission 在单事务内:
///   1. UPDATE sys_role SET data_scope/menu_check_strictly/dept_check_strictly
///   2. DELETE sys_role_menu + INSERT sys_role_menu(delete-then-insert)
///   3. DELETE sys_role_dept + INSERT sys_role_dept(delete-then-insert)
/// 断言「先删后插」后的终态(关联被整体替换,基础信息同步更新)。
/// ctx 经 system_context 桥(ScopeFilter 恒 All)。
#[tokio::test]
async fn test_update_role_permission_replaces_menus_depts_and_scope() {
    let ds = setup_db().await;
    let svc = RoleService::for_test();
    // menu_repo 收 ctx(全豁免),与 role_repo 共用本测试的
    // system_context 桥。
    let menu_repo = MenuRepository::new();
    let role_repo = RoleRepository::new();
    let ctx = ds.system_context();
    let user = test_user();

    // seed 现状:role 201 → menus {501,502,504}, depts {101,103}, data_scope='2'
    let before_menus = menu_repo
        .select_menu_ids_by_role_id(&ctx, 201)
        .await
        .unwrap();
    assert_eq!(
        HashSet::<i64>::from_iter(before_menus.iter().copied()),
        [501, 502, 504].into_iter().collect(),
        "前置:role 201 菜单应为 501/502/504"
    );

    // 整体替换:menus → {501,503},depts → {102},data_scope → '3'
    svc.update_role_permission(
        &ctx,
        201,
        &[501, 503],
        &[102],
        "3",
        true,
        false,
        &user,
    )
    .await
    .expect("更新角色权限失败");

    // (1) sys_role_menu 被整体替换为 {501,503}(旧 502/504 必须消失)
    let menus = menu_repo
        .select_menu_ids_by_role_id(&ctx, 201)
        .await
        .unwrap();
    let menu_set: HashSet<i64> = menus.iter().copied().collect();
    assert_eq!(
        menu_set,
        [501, 503].into_iter().collect(),
        "替换后 role 201 菜单应为 501/503,旧的 502/504 必须删除"
    );

    // (2) sys_role_dept 被整体替换为 {102}(旧 101/103 必须消失)
    let depts = role_repo
        .select_dept_ids_by_role_ids(&ctx, &[201])
        .await
        .unwrap();
    let dept_set: HashSet<i64> = depts.iter().copied().collect();
    assert_eq!(
        dept_set,
        [102].into_iter().collect(),
        "替换后 role 201 部门应为 102,旧的 101/103 必须删除"
    );

    // (3) sys_role 基础信息已更新(data_scope/menu_check_strictly/dept_check_strictly)
    // —— 整行读回(逃生舱无投影),断言落在字段上,与原裸 SQL 等价
    let row = ctx
        .select(
            role_entity::Entity::find()
                .filter(role_entity::Column::RoleId.eq(201)),
        )
        .one()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.data_scope, "3", "data_scope 应更新为 '3'");
    assert!(row.menu_check_strictly, "menu_check_strictly 应为 true");
    assert!(!row.dept_check_strictly, "dept_check_strictly 应为 false");
}

/// 空菜单/空部门集合时,替换后应清空对应关联(DELETE 执行,INSERT 跳过)。
/// 覆盖 `if menu_ids.is_empty()` / `if dept_ids.is_empty()` 早退分支。
#[tokio::test]
async fn test_update_role_permission_empty_sets_clears_associations() {
    let ds = setup_db().await;
    let svc = RoleService::for_test();
    let menu_repo = MenuRepository::new();
    let role_repo = RoleRepository::new();
    let ctx = ds.system_context();
    let user = test_user();

    // 用空集合替换 → 旧关联全部删除
    svc.update_role_permission(&ctx, 201, &[], &[], "1", true, true, &user)
        .await
        .expect("更新失败");

    let menus = menu_repo
        .select_menu_ids_by_role_id(&ctx, 201)
        .await
        .unwrap();
    assert!(menus.is_empty(), "空菜单集合替换后应无菜单关联");

    let depts = role_repo
        .select_dept_ids_by_role_ids(&ctx, &[201])
        .await
        .unwrap();
    assert!(depts.is_empty(), "空部门集合替换后应无部门关联");
}
