//! role_menu/role_dept 关联原语集成测试(经漏斗,
//! 关联表 DataPolicy 全 None → ctx.insert 逐行 / ctx.delete_where 批量删)。
//! COUNT 直读走 `select + without_row_scope` 整行读计数
//! (逃生舱无投影)。

mod common;

use std::sync::Arc;

use app_data::{
    DataContext,
    sea_orm::{ColumnTrait, EntityTrait, QueryFilter},
};
use common::setup_db;
use system::{
    relation::role_menu, role::repository::role_repo::RoleRepository,
};

/// 直读 sys_role_menu 行数(关联表 CONSTRAINED=false → without_row_scope)。
async fn role_menu_count(ctx: &DataContext<'_>, role_id: i64) -> usize {
    ctx.select(
        role_menu::Entity::find().filter(role_menu::Column::RoleId.eq(role_id)),
    )
    .without_row_scope()
    .all()
    .await
    .unwrap()
    .len()
}

#[tokio::test]
async fn test_insert_and_delete_role_menus() {
    let ds = Arc::new(setup_db().await);
    let repo = RoleRepository::new();
    let ctx = ds.system_context();

    // 角色 200 无菜单关联 (seed 中无记录), 插入 501, 502, 503
    repo.insert_role_menus(&ctx, 200, &[501, 502, 503])
        .await
        .unwrap();

    // 验证: 通过直接查询关联表
    assert_eq!(
        role_menu_count(&ctx, 200).await,
        3,
        "插入后应有 3 条菜单关联"
    );

    // 删除所有菜单
    repo.delete_role_menus_by_role_id(&ctx, 200).await.unwrap();

    assert_eq!(role_menu_count(&ctx, 200).await, 0, "删除后应无菜单关联");

    // 重新插入
    repo.insert_role_menus(&ctx, 200, &[501, 502])
        .await
        .unwrap();

    assert_eq!(
        role_menu_count(&ctx, 200).await,
        2,
        "重新插入后应有 2 条菜单关联"
    );
}

#[tokio::test]
async fn test_insert_and_delete_role_depts() {
    let ds = Arc::new(setup_db().await);
    let repo = RoleRepository::new();
    let ctx = ds.system_context();

    // 角色 200 无部门关联 (seed 中无记录), 插入 101, 102
    repo.insert_role_depts(&ctx, 200, &[101, 102])
        .await
        .unwrap();

    let dept_ids = repo
        .select_dept_ids_by_role_ids(&ctx, &[200])
        .await
        .unwrap();
    assert_eq!(dept_ids.len(), 2, "插入后应有 2 个部门关联");
    assert!(dept_ids.contains(&101));
    assert!(dept_ids.contains(&102));

    // 删除所有部门
    repo.delete_role_depts_by_role_id(&ctx, 200).await.unwrap();

    let dept_ids = repo
        .select_dept_ids_by_role_ids(&ctx, &[200])
        .await
        .unwrap();
    assert!(dept_ids.is_empty(), "删除后应无部门关联");

    // 重新插入
    repo.insert_role_depts(&ctx, 200, &[101]).await.unwrap();

    let dept_ids = repo
        .select_dept_ids_by_role_ids(&ctx, &[200])
        .await
        .unwrap();
    assert_eq!(dept_ids.len(), 1, "重新插入后应有 1 个部门关联");
    assert!(dept_ids.contains(&101));
}

#[tokio::test]
async fn test_select_dept_ids_by_role_ids() {
    let ds = Arc::new(setup_db().await);
    let repo = RoleRepository::new();
    let ctx = ds.system_context();

    // 角色 201 关联部门 101, 103 (seed)
    let dept_ids = repo
        .select_dept_ids_by_role_ids(&ctx, &[201])
        .await
        .unwrap();
    assert_eq!(dept_ids.len(), 2, "角色 201 应关联 2 个部门");
    assert!(dept_ids.contains(&101), "应包含技术部 (101)");
    assert!(dept_ids.contains(&103), "应包含前端组 (103)");
}
