//!参考实现 挂钩点回归:六个变更点(user/role 写路径)各自在
//! 事务内向 flow_assignee_sync 写一行补偿任务(status='0')。
//!
//! 事务化行为加固(原四个无事务方法包事务)的平价验证:
//! 授权/用户变更的对外语义(返回值、关联表终态)不变,仅多出一行
//! outbox 记录 —— 这里钉 outbox 行的存在与字段,关联表终态由既有
//! user/role 测试套件覆盖(scope_invalidation_test 等)。

mod common;

use app_core::{
    infra::{ConnectionPool, Infrastructure},
    middleware::extractors::CurrentUser,
    model::login_user::LoginUser,
};
use common::setup_db;
use system::{
    assignee_sync::repository::assignee_sync_repo::AssigneeSyncRepository,
    role::service::role_service::RoleService,
    user::{model::user::UserUpdateBo, service::user_service::UserService},
};

fn lazy_redis_pool() -> deadpool_redis::Pool {
    deadpool_redis::Config::from_url("redis://127.0.0.1:6379")
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("创建惰性 redis 池失败")
}

fn user_service() -> UserService {
    let infra = Infrastructure::new(
        std::sync::Arc::new(ConnectionPool {
            redis_pool: lazy_redis_pool(),
        }),
        false,
        1800,
    );
    UserService::new(&infra)
}

fn current_user() -> CurrentUser {
    CurrentUser(LoginUser {
        user_id: 301,
        dept_id: Some(100),
        username: "u301".to_string(),
        nick_name: Some("nick301".to_string()),
        user_type: Some("sys_user".to_string()),
        client_id: "test_client".to_string(),
        client_key: Some("test".to_string()),
        device_type: None,
        permissions: Default::default(),
        roles: Default::default(),
        login_ip: Some("127.0.0.1".to_string()),
        login_date: None,
    })
}

async fn setup_with_outbox() -> app_data::DataSource {
    // flow_assignee_sync 由 common::setup_schema_and_seeds 统一建表
    // (参考实现 后所有 system 测试脚手架都带 outbox 表)
    setup_db().await
}

#[tokio::test]
async fn select_auth_users_enqueues_role_add_row() {
    let ds = setup_with_outbox().await;
    let ctx = ds.system_context();
    let svc = RoleService::for_test();

    // 注:305 种子只持 role 204,授予 201 不撞唯一键(302 已持 201,
    // 直接授会撞 user_role 唯一约束 —— 那是种子重复,不是挂钩缺陷)
    svc.select_auth_users(&ctx, 201, &[305]).await.unwrap();

    let rows = AssigneeSyncRepository::select_pending(&ctx, 5)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "select_auth_users 必须写一行补偿任务");
    let row = &rows[0];
    assert_eq!(row.auth_type, "role");
    assert_eq!(row.oper_type, "add");
    assert_eq!(row.auth_code.as_deref(), Some("role_custom"));
    assert_eq!(row.user_ids.as_deref(), Some("[305]"));
    assert_eq!(row.status, "0");
}

#[tokio::test]
async fn cancel_auth_user_enqueues_role_delete_row() {
    let ds = setup_with_outbox().await;
    let ctx = ds.system_context();
    let svc = RoleService::for_test();

    svc.cancel_auth_user(&ctx, 201, 302).await.unwrap();

    let rows = AssigneeSyncRepository::select_pending(&ctx, 5)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].auth_type, "role");
    assert_eq!(rows[0].oper_type, "delete");
    assert_eq!(rows[0].auth_code.as_deref(), Some("role_custom"));
    assert_eq!(rows[0].user_ids.as_deref(), Some("[302]"));
    assert_eq!(rows[0].status, "0");
}

#[tokio::test]
async fn insert_user_auth_enqueues_user_update_row() {
    let ds = setup_with_outbox().await;
    let ctx = ds.system_context();
    let svc = user_service();

    svc.insert_user_auth(&ctx, 302, &[203]).await.unwrap();

    let rows = AssigneeSyncRepository::select_pending(&ctx, 5)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].auth_type, "user");
    assert_eq!(rows[0].oper_type, "update");
    assert_eq!(rows[0].auth_code.as_deref(), Some("u_custom"));
    assert_eq!(rows[0].user_ids.as_deref(), Some("[302]"));
    assert_eq!(rows[0].status, "0");
}

#[tokio::test]
async fn update_by_bo_enqueues_single_user_update_row() {
    let ds = setup_with_outbox().await;
    let ctx = ds.system_context();
    let svc = user_service();

    let bo = UserUpdateBo {
        user_id: Some(302),
        user_name: "u_custom".to_string(),
        dept_id: Some(101),
        nick_name: "自定义数据".to_string(),
        phone_number: None,
        email: None,
        gender: None,
        status: Some("0".to_string()),
        remark: None,
        post_ids: None,
        role_ids: Some(vec![201]),
    };
    svc.update_by_bo(&ctx, bo, &current_user()).await.unwrap();

    let rows = AssigneeSyncRepository::select_pending(&ctx, 5)
        .await
        .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "update_by_bo 整笔事务只写一行补偿任务(不得因 role_ids 内联替换重复写)"
    );
    assert_eq!(rows[0].auth_type, "user");
    assert_eq!(rows[0].oper_type, "update");
    assert_eq!(rows[0].auth_code.as_deref(), Some("u_custom"));
    assert_eq!(rows[0].user_ids.as_deref(), Some("[302]"));
    assert_eq!(rows[0].status, "0");
}
