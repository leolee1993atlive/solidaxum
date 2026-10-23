//! 关联表实体往返测试:insert 一行 -> find -> 断言存在。
//! 验证 4 张关联表实体(sys_role_menu/role_dept/user_role/user_post)的结构化 API 可用。
//! 往返经漏斗原语(关联实体无审计/软删列、CONSTRAINED=false,
//! `ctx.insert` + `select + without_row_scope`)。
mod common;

use app_data::sea_orm::{
    ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter,
};
use common::setup_db;
use system::relation::{role_dept, role_menu, user_post, user_role};

#[tokio::test]
async fn role_menu_roundtrip() {
    let ds = setup_db().await;
    let ctx = ds.system_context();
    // seed 已含 (201,501)/(201,502)/(201,504),此处用未占用的组合避免冲突。
    ctx.insert(role_menu::ActiveModel {
        role_id: Set(1),
        menu_id: Set(10),
    })
    .await
    .unwrap();
    let found = ctx
        .select(
            role_menu::Entity::find()
                .filter(role_menu::Column::RoleId.eq(1))
                .filter(role_menu::Column::MenuId.eq(10)),
        )
        .without_row_scope()
        .one()
        .await
        .unwrap();
    assert!(found.is_some(), "sys_role_menu 往返应能查回插入行");
}

#[tokio::test]
async fn role_dept_roundtrip() {
    let ds = setup_db().await;
    let ctx = ds.system_context();
    ctx.insert(role_dept::ActiveModel {
        role_id: Set(1),
        dept_id: Set(10),
    })
    .await
    .unwrap();
    let found = ctx
        .select(
            role_dept::Entity::find()
                .filter(role_dept::Column::RoleId.eq(1))
                .filter(role_dept::Column::DeptId.eq(10)),
        )
        .without_row_scope()
        .one()
        .await
        .unwrap();
    assert!(found.is_some(), "sys_role_dept 往返应能查回插入行");
}

#[tokio::test]
async fn user_role_roundtrip() {
    let ds = setup_db().await;
    let ctx = ds.system_context();
    ctx.insert(user_role::ActiveModel {
        user_id: Set(1),
        role_id: Set(10),
    })
    .await
    .unwrap();
    let found = ctx
        .select(
            user_role::Entity::find()
                .filter(user_role::Column::UserId.eq(1))
                .filter(user_role::Column::RoleId.eq(10)),
        )
        .without_row_scope()
        .one()
        .await
        .unwrap();
    assert!(found.is_some(), "sys_user_role 往返应能查回插入行");
}

#[tokio::test]
async fn user_post_roundtrip() {
    let ds = setup_db().await;
    let ctx = ds.system_context();
    ctx.insert(user_post::ActiveModel {
        user_id: Set(1),
        post_id: Set(10),
    })
    .await
    .unwrap();
    let found = ctx
        .select(
            user_post::Entity::find()
                .filter(user_post::Column::UserId.eq(1))
                .filter(user_post::Column::PostId.eq(10)),
        )
        .without_row_scope()
        .one()
        .await
        .unwrap();
    assert!(found.is_some(), "sys_user_post 往返应能查回插入行");
}
