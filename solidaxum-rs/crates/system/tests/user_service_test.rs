//! user 服务层集成测试。
//!
//! 与 user_repo_test(repo 级)的分工:这里钉**请求路径语义** —— handler
//! 构造的用户 ctx 穿过 service 后,列表/分页的范围收窄、关联表
//! delete-then-insert 事务、审计人盖章。resolver 用 FixedResolver 免搭角色
//! 语义。(写路径不施行级范围,越权拦截由 handler 探针承担,判例见
//! write_scope_rollback_test。)

mod common;

use std::sync::Arc;

use app_core::{
    infra::{ConnectionPool, Infrastructure},
    middleware::extractors::CurrentUser,
    model::login_user::LoginUser,
    vo::page::PageQuery,
};
use app_data::{
    DataSource, Principal, ScopeFilter,
    sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter},
    testing::FixedResolver,
};
use common::setup_schema_and_seeds;
use system::user::{
    handler::user_handler::UserPageQuery,
    repository::{user_repo as user_entity, user_repo::UserRepository},
    service::user_service::UserService,
};

/// 惰性 Redis 池(不真正连接)—— 与 admin testkit 同一处置,
/// UserService 仅在 unlock 路径触 Redis,本文件不触。
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

/// 服务 + 固定 ScopeFilter 的 DataSource(生产装配同构:ds0 → derive →
/// set_data_source → 服务构造在装配后,服务的 repo 句柄即带 resolver)。
fn service_scoped(
    ds0: Arc<DataSource>,
    sf: ScopeFilter,
) -> (Arc<DataSource>, UserService) {
    let ds = Arc::new(ds0.derive_with_resolver(Arc::new(FixedResolver(sf))));
    let infra = Infrastructure::new(lazy_redis_pool(), false, 1800);
    assert!(
        infra.set_data_source(ds.clone()),
        "测试装配 DataSource 只此一次"
    );
    (ds.clone(), UserService::new(&infra))
}

/// 测试用登录身份(insert_by_bo 的审计人来自 ctx principal,
/// CurrentUser 只参与日志)。
fn current_user(uid: i64, dept: i64) -> CurrentUser {
    CurrentUser(LoginUser {
        user_id: uid,
        dept_id: Some(dept),
        username: format!("u{uid}"),
        nick_name: Some(format!("nick{uid}")),
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

fn page_query() -> UserPageQuery {
    UserPageQuery {
        page: PageQuery {
            page_num: Some(1),
            page_size: Some(50),
            order_by_column: None,
            is_asc: None,
        },
        dept_id: None,
        user_name: None,
        nick_name: None,
        phone_number: None,
        status: None,
        begin_time: None,
        end_time: None,
    }
}

/// 补种:create_by=7(101 部门)与 102 部门(越权)各一行。
/// 免盖章种子:显式 create_by 原样落库(insert_unstamped)。
async fn seed_scope_rows(ds: &DataSource) {
    let ctx = ds.system_context();
    for (id, dept_id, user_name, nick_name, create_by) in [
        (
            1761300000000000101_i64,
            101,
            "scoped_owner",
            "建单人用户",
            Some(7),
        ),
        (
            1761300000000000201,
            102,
            "foreign_dept",
            "越权部门用户",
            None,
        ),
    ] {
        ctx.insert_unstamped(user_entity::ActiveModel {
            user_id: Set(id),
            dept_id: Set(Some(dept_id)),
            user_name: Set(user_name.into()),
            nick_name: Set(nick_name.into()),
            password: Set("$2a$10$placeholder".into()),
            status: Set("0".into()),
            del_flag: Set("0".into()),
            create_by: Set(create_by),
            ..Default::default()
        })
        .await
        .expect("种范围用户行失败");
    }
}

/// SelfOnly 用户的服务级列表:只见 create_by=本人 的行(owner 列 =
/// create_by,Java SysUserMapper.java:39;repo 级判例在
/// service 级的复证 —— 同一语义经 service 生效)。
#[tokio::test]
async fn self_only_list_sees_only_created_by_rows() {
    let ds0 = setup_conn().await;
    seed_scope_rows(&ds0).await;
    let (ds, svc) = service_scoped(ds0, ScopeFilter::SelfOnly(7));
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    let rows = svc.list(&ctx).await.expect("查询失败");
    let ids: Vec<i64> = rows.iter().map(|u| u.user_id).collect();
    assert_eq!(
        ids,
        vec![1761300000000000101],
        "SelfOnly(7) 服务级列表只见 create_by=7 的行,种子行(NULL)不可见"
    );
}

/// DeptIn 用户的服务级分页:越权部门行不计入 total 也不出现在页里。
#[tokio::test]
async fn dept_in_page_excludes_foreign_dept_rows() {
    let ds0 = setup_conn().await;
    seed_scope_rows(&ds0).await;
    let (ds, svc) = service_scoped(ds0, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(101),
    });

    let result = svc.page(&ctx, page_query()).await.expect("分页失败");
    // 种子 dept=101 的 9 行(301-308, 402)+ 补种的 101 行 = 10;
    // 102 行、100 部门超管/401、103/104 部门行全部不可见。
    assert_eq!(result.total, 10, "total 是过滤后总数");
    assert_eq!(result.rows.len(), 10);
    assert!(
        result.rows.iter().all(|r| r.dept_id == Some(101)),
        "页内不得出现越权部门行"
    );
}

/// 写路径不施行级范围:service 层不带探针,DeptIn([101]) 的
/// 操作者经 service 直改 100 部门用户状态**改得动** —— 越权拦截在
/// handler 的 `check_user_data_scope`(Java changeStatus:273 对位),
/// 判例见 write_scope_rollback_test;本用例钉住 service 层语义。
#[tokio::test]
async fn change_status_on_foreign_dept_target_succeeds_after_rollback() {
    let ds0 = setup_conn().await;
    seed_scope_rows(&ds0).await;
    let (ds, svc) = service_scoped(ds0, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    // 401 在 100 部门(总部)→ 越权,但 service 层不拦
    svc.change_status(&ctx, 401, "1".to_string())
        .await
        .expect("回退后越权目标的 change_status 应成功(拦截在 handler 探针)");
    // 本部门行照常可改
    svc.change_status(&ctx, 402, "1".to_string())
        .await
        .expect("本部门行应可改");

    // 越权行的改动必须落库(整行读回复核,逃生舱无投影)
    let row = ds
        .system_context()
        .select(
            user_entity::Entity::find()
                .filter(user_entity::Column::UserId.eq(401)),
        )
        .one()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.status, "1", "回退后越权的 status 更新应落库");
}

/// 读路径越权探针(Java checkUserDataScope → countUserById 带注解):
/// 本部门 Ok / 越权 NotFound / 不存在 NotFound(不泄露存在性区分)。
#[tokio::test]
async fn check_user_data_scope_probe_verdicts() {
    let ds0 = setup_conn().await;
    seed_scope_rows(&ds0).await;
    let (ds, svc) = service_scoped(ds0, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    svc.check_user_data_scope(&ctx, 301)
        .await
        .expect("本部门行应放行");
    let err = svc.check_user_data_scope(&ctx, 401).await;
    assert!(
        matches!(err, Err(app_core::error::AppError::NotFound(_))),
        "越权部门 → NotFound,实际: {err:?}"
    );
    let err = svc.check_user_data_scope(&ctx, 999_999).await;
    assert!(
        matches!(err, Err(app_core::error::AppError::NotFound(_))),
        "不存在 → NotFound(与越权同形态)"
    );
}

/// unlock 不做数据范围探针(Java SysUserController:283-297 的平价:unlock
/// 只走 selectUserById,方法体无 checkUserDataScope —— 与 edit/changeStatus
/// 先探针(:147/:256 等)相反)。Rust 链路 unlock → get_user_by_id →
/// select_by_id 为 without_row_scope 豁免读(repo 平价表:user_repo.rs
/// `selectUserById → selectVoById` 无注解锚),故 DeptIn 操作者解锁越权
/// 部门用户不被范围拒绝,仅"不存在"仍 NotFound(存在性照判)。
/// Redis DEL 在 unlock 内整体吞错,无需真实 Redis。
#[tokio::test]
async fn unlock_skips_data_scope_probe_on_foreign_dept_target() {
    let ds0 = setup_conn().await;
    let (ds, svc) = service_scoped(ds0, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    // 401 在 100 部门(越权目标):探针路径(check_user_data_scope)会
    // NotFound,unlock 放行 —— 放宽是被 Java 锚点固化的行为。
    svc.unlock(&ctx, 401)
        .await
        .expect("unlock 不做范围探针,越权部门用户应照常解锁");
    // 存在性仍校验:不存在 → NotFound(Java user 为 null → R.fail)
    let err = svc.unlock(&ctx, 999_999).await;
    assert!(
        matches!(err, Err(app_core::error::AppError::NotFound(_))),
        "不存在用户仍 NotFound,实际: {err:?}"
    );
}

/// 关联表 delete-then-insert 事务(insert_user_auth):整替语义 + 空列表
/// 清空 + 经漏斗 delete_where/tx.insert。
#[tokio::test]
async fn insert_user_auth_replaces_roles_via_funnel_txn() {
    let ds0 = setup_conn().await;
    let (ds, svc) = service_scoped(ds0, ScopeFilter::All);
    let ctx = ds.system_context();
    let repo = UserRepository::new();

    // 种子:user 308 → roles [201, 203]
    svc.insert_user_auth(&ctx, 308, &[205, 206])
        .await
        .expect("重授角色失败");
    let ids = repo.select_role_ids_by_user_id(&ctx, 308).await.unwrap();
    assert_eq!(ids, vec![205, 206], "先删后插 = 整替,不是追加");

    // 空列表 → 清空(原 SQL 语义:delete-then-insert 空集)
    svc.insert_user_auth(&ctx, 308, &[])
        .await
        .expect("清空角色失败");
    let ids = repo.select_role_ids_by_user_id(&ctx, 308).await.unwrap();
    assert!(ids.is_empty(), "空列表必须清空关联");
}

/// 关联表 delete-then-insert 事务(update_user_posts):user 302 种子
/// posts [ceo, pm] → 整替为 [pm]。
#[tokio::test]
async fn update_user_posts_replaces_posts_via_funnel_txn() {
    let ds0 = setup_conn().await;
    let (ds, svc) = service_scoped(ds0, ScopeFilter::All);
    let ctx = ds.system_context();
    let repo = UserRepository::new();

    // 种子:user 302 → posts [1761200000000000001, 1761200000000000002]
    let pid = 1761200000000000002;
    svc.update_user_posts(&ctx, 302, &[pid])
        .await
        .expect("更新岗位失败");
    let ids = repo.select_post_ids_by_user_id(&ctx, 302).await.unwrap();
    assert_eq!(ids, vec![pid], "先删后插 = 整替");
}

/// 审计人盖章:insert_by_bo 以用户
/// ctx 调用后,create_by = 操作人、create_dept = 操作人部门(漏斗盖章,
/// 领域层 from_create_bo 不填审计)。
#[tokio::test]
async fn insert_by_bo_restores_creator_audit_from_ctx() {
    let ds0 = setup_conn().await;
    let (ds, svc) = service_scoped(ds0, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 42,
        dept_id: Some(101),
    });

    svc.insert_by_bo(
        &ctx,
        system::user::model::user::UserCreateBo {
            user_name: "audit_probe_42".to_string(),
            password: "probe12345".to_string(),
            dept_id: Some(101),
            nick_name: "审计探针".to_string(),
            phone_number: None,
            email: None,
            gender: None,
            status: None,
            remark: None,
            post_ids: None,
            role_ids: None,
        },
        &current_user(42, 101),
    )
    .await
    .expect("创建用户失败");

    let repo = UserRepository::new();
    let sctx = ds.system_context();
    let user = repo
        .select_by_user_name(&sctx, "audit_probe_42")
        .await
        .unwrap()
        .expect("创建后应可按名查回");
    assert_eq!(user.base_entity.create_by, Some(42), "create_by = 操作人");
    assert_eq!(
        user.base_entity.create_dept,
        Some(101),
        "create_dept = 操作人部门"
    );
    assert!(
        user.base_entity.create_time.is_some(),
        "create_time 由漏斗盖"
    );
    assert_eq!(
        user.base_entity.update_by,
        Some(42),
        "RuoYi 习惯:插入也写 update_by"
    );
}
