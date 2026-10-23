//! user 仓储集成测试。
//!
//! 覆盖四组语义:
//! 1. 基础 CRUD/关联查询(经 `system_context` 桥接);
//! 2. **范围自动施加**:DeptIn → 只见本部门行;SelfOnly →
//!    只见 create_by=本人 行(sys_user 的 owner 列 = create_by,Java 基线
//!    SysUserMapper.java:39 userName=create_by —— 与 post 同构);
//! 3. **方法级豁免**:Java 基线无 @DataPermission 的方法
//!    (详情/登录按名查/唯一性/部门守卫计数/按部门列表)显式
//!    `without_row_scope`,范围不收窄;
//! 4. **写路径**:软删行
//!    不可改;insert 的审计五列由漏斗盖章(create_by=principal);越权
//!    部门 update 改得动(不施行级范围,拦截由 handler 探针承担,判例见
//!    write_scope_rollback_test)。

mod common;

use std::{collections::HashSet, sync::Arc};

use app_data::{
    DataSource, Principal, ScopeFilter,
    sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter},
    testing::FixedResolver,
};
use common::setup_db;
use system::user::{
    handler::user_handler::UserPageQuery,
    model::user::User,
    repository::user_repo::{self as user_entity, UserRepository},
};

/// 仓库 + 无 resolver 的 DataSource(system_context 语义,ScopeFilter 恒 All)。
fn repo_system(ds: DataSource) -> (Arc<DataSource>, UserRepository) {
    (Arc::new(ds), UserRepository::new())
}

/// 仓库 + 固定 ScopeFilter 的 DataSource(范围用例,免搭业务角色语义)。
fn repo_scoped(
    ds: DataSource,
    sf: ScopeFilter,
) -> (Arc<DataSource>, UserRepository) {
    (
        Arc::new(ds.with_resolver(Arc::new(FixedResolver(sf)))),
        UserRepository::new(),
    )
}

fn user_ids(users: &[User]) -> HashSet<i64> {
    users.iter().filter_map(|u| u.user_id).collect()
}

/// 范围用例的补充种子(基础种子:13 用户,create_by 全 NULL):
/// - 101 部门、create_by=7 的行(DeptIn 与 SelfOnly 的交集行)
/// - 102 部门的行(越权部门)
/// - 101 部门、del_flag='1' 的已删行(软删不可见)
///   免盖章种子:显式 create_by / del_flag='1' 原样落库(insert_unstamped)。
async fn seed_scope_rows(ds: &DataSource) {
    let ctx = ds.system_context();
    for (id, dept_id, user_name, nick_name, del_flag, create_by) in [
        (
            1761300000000000101_i64,
            101,
            "scoped_owner",
            "范围交集用户",
            "0",
            Some(7),
        ),
        (
            1761300000000000201,
            102,
            "foreign_dept",
            "越权部门用户",
            "0",
            None,
        ),
        (
            1761300000000000301,
            101,
            "deleted_user",
            "已删用户",
            "1",
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
            del_flag: Set(del_flag.into()),
            create_by: Set(create_by),
            ..Default::default()
        })
        .await
        .expect("种范围用户行失败");
    }
}

// ---------------------------------------------------------------------------
// 基础用例(原 select_all_with_scope 四条,变体合并后统一走 select_all,
// 范围由漏斗按调用方 ctx 施加 —— system_context 得全量,即原 DataScope::All)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_select_all_system_context_sees_all() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let users = repo.select_all(&ctx).await.expect("查询失败");
    assert_eq!(users.len(), 13, "system_context 应返回全部 13 个用户");
}

#[tokio::test]
async fn test_select_all_dept_in_narrows() {
    let ds = setup_db().await;
    seed_scope_rows(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(101),
    });

    let users = repo.select_all(&ctx).await.expect("查询失败");
    let ids = user_ids(&users);
    // 种子 dept=101 的 9 行(301-308, 402)+ 补种的交集行;越权 102 行与
    // 已删行不计入
    let mut expected: HashSet<i64> =
        [301, 302, 303, 304, 305, 306, 307, 308, 402]
            .into_iter()
            .collect();
    expected.insert(1761300000000000101);
    assert_eq!(ids, expected, "DeptIn([101]) 应只见本部门存活用户");
}

/// SelfOnly → 只见 create_by=本人 的行(owner 列 = create_by,对齐 Java
/// SysUserMapper.java:37-40 的 userName=create_by;主表按 SysUserMapper.java:31
/// 对照 —— user 与 post 同锚)。种子行 create_by 全 NULL → 不可见。
#[tokio::test]
async fn test_select_all_self_only_sees_created_by_rows() {
    let ds = setup_db().await;
    seed_scope_rows(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::SelfOnly(7));
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    let users = repo.select_all(&ctx).await.expect("查询失败");
    let ids = user_ids(&users);
    assert_eq!(
        ids,
        [1761300000000000101].into_iter().collect::<HashSet<i64>>(),
        "SelfOnly(7) 应只见 create_by=7 的行,种子行(create_by=NULL)不可见"
    );
}

/// DeptInOrSelf(CustomOrSelf 的漏斗形态):dept IN (...) OR create_by=本人。
#[tokio::test]
async fn test_select_all_dept_in_or_self() {
    let ds = setup_db().await;
    seed_scope_rows(&ds).await;
    let (ds, repo) = repo_scoped(
        ds,
        ScopeFilter::DeptInOrSelf {
            depts: vec![101, 103],
            user_id: 7,
        },
    );
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    let users = repo.select_all(&ctx).await.expect("查询失败");
    let ids = user_ids(&users);
    let mut expected: HashSet<i64> =
        [301, 302, 303, 304, 305, 306, 307, 308, 402, 403]
            .into_iter()
            .collect();
    expected.insert(1761300000000000101);
    assert_eq!(
        ids, expected,
        "应含 dept 101/103 的用户 + create_by=7 的交集行"
    );
}

// ---------------------------------------------------------------------------
// 用户关联 ID 查询(原用例,ctx 适配;关联表 DataPolicy 全 None → 豁免)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_select_role_ids_by_user_id() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // seed: user 308 → roles 201, 203(须按 role_id 升序返回)
    let ids = repo.select_role_ids_by_user_id(&ctx, 308).await.unwrap();
    assert_eq!(ids, vec![201, 203], "user 308 的 role_ids 应升序 [201,203]");

    // 单角色
    let ids = repo.select_role_ids_by_user_id(&ctx, 302).await.unwrap();
    assert_eq!(ids, vec![201], "user 302 仅 role 201");

    // 无角色用户 → 空(保持有序空集,非 None)
    let ids = repo.select_role_ids_by_user_id(&ctx, 307).await.unwrap();
    assert!(ids.is_empty(), "无角色用户应返回空 Vec");
}

#[tokio::test]
async fn test_select_post_ids_by_user_id() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // seed: user 302 → posts ceo(…01), pm(…02)
    let ids = repo.select_post_ids_by_user_id(&ctx, 302).await.unwrap();
    assert_eq!(
        ids,
        vec![1761200000000000001, 1761200000000000002],
        "user 302 的 post_ids 应升序"
    );

    // 无岗位用户 → 空
    let ids = repo.select_post_ids_by_user_id(&ctx, 301).await.unwrap();
    assert!(ids.is_empty(), "无岗位用户应返回空 Vec");
}

// ---------------------------------------------------------------------------
// 用户状态更新(原用例,ctx 适配;写路径 WHERE=pk+软删存活+行级范围)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_user_update_status() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // user 301 (del_flag=0) 停用
    repo.update_status(&ctx, 301, "1").await.expect("更新失败");
    assert_eq!(
        user_status(&ds, 301).await,
        "1",
        "user 301 status 应已置 '1'"
    );

    // 恢复
    repo.update_status(&ctx, 301, "0").await.unwrap();
    assert_eq!(user_status(&ds, 301).await, "0");

    // 不存在的用户 → 0 行受影响 → NotFound
    let err = repo.update_status(&ctx, 99999, "1").await;
    assert!(
        matches!(err, Err(app_data::DataError::NotFound)),
        "不存在的用户应返回 NotFound,实际: {err:?}"
    );
}

/// 直读 status 断言(整行读,逃生舱无投影 —— 与原裸 SQL 单列读等价)。
async fn user_status(ds: &DataSource, user_id: i64) -> String {
    ds.system_context()
        .select(
            user_entity::Entity::find()
                .filter(user_entity::Column::UserId.eq(user_id)),
        )
        .one()
        .await
        .unwrap()
        .unwrap()
        .status
}

// ---------------------------------------------------------------------------
// 方法级豁免(Java 基线无 @DataPermission 的方法全量可见)
// ---------------------------------------------------------------------------

/// 详情/登录按名查/部门守卫计数/按部门列表在 Java 侧均无 @DataPermission
/// (selectVoById / lambda().voOne() / 守卫 lambda 计数 / selectUserListByDept
/// 的 lambda().voList()):即便 resolver 给出 DeptIn([101]),这些方法也必须
/// 看见全部存活行。
#[tokio::test]
async fn exempt_methods_see_all_rows_despite_scope() {
    let ds = setup_db().await;
    seed_scope_rows(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(101),
    });

    // select_by_id(详情,Java selectUserById → selectVoById 无注解)
    let other_dept =
        repo.select_by_id(&ctx, &1761300000000000201).await.unwrap();
    assert!(other_dept.is_some(), "越权部门的详情照常可见(Java 同口径)");

    // select_by_user_name(登录路径,Java lambda().voOne() 无注解)
    let by_name = repo
        .select_by_user_name(&ctx, "foreign_dept")
        .await
        .unwrap();
    assert!(by_name.is_some(), "按用户名查询不受范围收窄");

    // count_normal_by_dept_id(dept 删除守卫,需要真实计数)
    let count = repo.count_normal_by_dept_id(&ctx, 102).await.unwrap();
    assert_eq!(count, 1, "守卫计数不收窄 —— 部门删除守卫需要真实计数");

    // select_list_by_dept_id(Java selectUserListByDept → lambda().voList()
    // 无注解 —— 对齐放宽)
    let rows = repo.select_list_by_dept_id(&ctx, 102).await.unwrap();
    assert_eq!(rows.len(), 1, "按部门列表不受数据范围收窄(Java 同口径)");
}

// ---------------------------------------------------------------------------
// 写路径(软删不可改 / 审计盖章;越权拦截由 handler 探针承担,
// 见 write_scope_rollback_test)
// ---------------------------------------------------------------------------

/// 越权部门的 update_by_id **改得动**:Java
/// `SysUserMapper.updateById` 虽带 @DataPermission(Mapper.java:158-163),
/// 但受辖 update 的失败态是静默 0 行、边界随数据范围抖动(Java 版踩过的
/// 不良事件),本仓写路径统一走豁免面,越权拦截由 handler 的
/// `check_user_data_scope` 探针承担 —— 本用例钉住 repo 层语义。
#[tokio::test]
async fn scoped_update_on_foreign_dept_row_succeeds_after_rollback() {
    let ds = setup_db().await;
    seed_scope_rows(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(101),
    });

    // 经 system_context 取回越权行,改名后经受限 ctx 更新 ——
    // WHERE 只剩主键 + 软删存活,越权目标改得动。
    let sctx = ds.system_context();
    let mut victim = repo
        .select_by_id(&sctx, &1761300000000000201)
        .await
        .unwrap()
        .unwrap();
    victim.nick_name = "越权改名".to_string();
    repo.update_by_id(&ctx, &victim, &1761300000000000201)
        .await
        .expect("回退后越权目标的 update_by_id 不应再被行级范围拦住");

    // 改动必须落库(用 system_context 复核)
    let row = repo
        .select_by_id(&sctx, &1761300000000000201)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.nick_name, "越权改名", "回退后越权 update 应落库");
}

/// 软删行对 update 不可见:已删用户的状态更新 → NotFound(存活过滤进 WHERE,
/// 对齐 @TableLogic 的写路径语义)。
#[tokio::test]
async fn update_on_soft_deleted_row_is_not_found() {
    let ds = setup_db().await;
    seed_scope_rows(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(101),
    });

    let err = repo
        .update_status(&ctx, 1761300000000000301, "1")
        .await
        .expect_err("更新已删行必须报错");
    assert!(
        matches!(err, app_data::DataError::NotFound),
        "软删行不可改 → NotFound,实际: {err:?}"
    );
}

/// insert 的审计五列由漏斗盖章:create_by / create_dept / create_time /
/// update_by / update_time 取自 principal(领域层传入的审计值被覆盖)。
#[tokio::test]
async fn insert_stamps_audit_from_principal() {
    let (ds, repo) = repo_scoped(setup_db().await, ScopeFilter::All);
    let ctx = ds.context(Principal {
        user_id: 42,
        dept_id: Some(103),
    });

    // 领域层刻意带上"会被漏斗覆盖"的假审计值
    let mut user = repo
        .select_by_id(&ds.system_context(), &301)
        .await
        .unwrap()
        .unwrap();
    user.user_id = Some(1761300000000000401);
    user.user_name = "audit_probe".to_string();
    user.nick_name = "审计探针".to_string();
    user.base_entity.create_by = Some(999);
    user.base_entity.create_dept = Some(999);
    repo.insert(&ctx, &user).await.expect("插入失败");

    let saved = repo
        .select_by_id(&ds.system_context(), &1761300000000000401)
        .await
        .unwrap()
        .expect("插入后应可查回");
    assert_eq!(
        saved.base_entity.create_by,
        Some(42),
        "create_by = principal.user_id(领域层假值 999 被覆盖)"
    );
    assert_eq!(
        saved.base_entity.create_dept,
        Some(103),
        "create_dept = principal.dept_id"
    );
    assert!(
        saved.base_entity.create_time.is_some(),
        "create_time 由漏斗盖"
    );
    assert_eq!(
        saved.base_entity.update_by,
        Some(42),
        "RuoYi 习惯:插入也写 update_*"
    );
    assert!(saved.base_entity.update_time.is_some());
}

// ---------------------------------------------------------------------------
// 导出行数闸门的前提:count 与 list 必须同过滤、同范围
// ---------------------------------------------------------------------------

fn export_query(user_name: Option<&str>) -> UserPageQuery {
    UserPageQuery {
        page: Default::default(),
        dept_id: None,
        user_name: user_name.map(str::to_string),
        nick_name: None,
        phone_number: None,
        status: None,
        begin_time: None,
        end_time: None,
    }
}

/// 导出前用 `count_by_query` 拦行数、随后用 `select_list` 取行 —— 两者的
/// WHERE 与受辖面必须逐点一致。一旦漂移就是两种错误:count 不受辖而 list
/// 受辖时,受限管理员会被按**全量**行数误拒;反过来则放过一次超限导出。
///
/// 三种组合各钉一次:全量、行级范围收窄、附加字段过滤。
#[tokio::test]
async fn count_by_query_agrees_with_select_list() {
    // ① system_context:范围恒 All
    let ds = setup_db().await;
    seed_scope_rows(&ds).await;
    let (ds_sys, repo) = repo_system(ds);
    let ctx = ds_sys.system_context();
    let q = export_query(None);
    let listed = repo
        .select_list(&ctx, &q, "user_id desc")
        .await
        .expect("列表查询失败");
    let counted = repo.count_by_query(&ctx, &q).await.expect("计数失败");
    assert_eq!(
        counted as usize,
        listed.len(),
        "system_context 下 count 与 list 行数必须一致"
    );

    // ② DeptIn([101]):count 必须跟着收窄,不能报全量
    let ds = setup_db().await;
    seed_scope_rows(&ds).await;
    let (ds_scoped, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds_scoped.context(Principal {
        user_id: 1,
        dept_id: Some(101),
    });
    let listed = repo
        .select_list(&ctx, &q, "user_id desc")
        .await
        .expect("列表查询失败");
    let counted = repo.count_by_query(&ctx, &q).await.expect("计数失败");
    assert_eq!(
        counted as usize,
        listed.len(),
        "DeptIn 受辖下 count 必须与 list 同范围"
    );
    assert!(
        (counted as usize) < 13,
        "受辖计数应严格小于全量 13,否则说明 count 漏了范围施加(counted={counted})"
    );

    // ③ 附加字段过滤:两边都要吃到 user_name
    let q_named = export_query(Some("scoped_owner"));
    let listed = repo
        .select_list(&ctx, &q_named, "user_id desc")
        .await
        .expect("列表查询失败");
    let counted = repo.count_by_query(&ctx, &q_named).await.expect("计数失败");
    assert_eq!(
        counted as usize,
        listed.len(),
        "带 user_name 过滤时 count 与 list 行数必须一致"
    );
    assert!(
        counted < 13,
        "过滤后计数应小于全量,否则说明 count 没吃到过滤条件(counted={counted})"
    );
}

/// 唯一性校验只需要「存不存在」,不需要整行。
///
/// 这三条断言钉的是**语义**;真正促成改动的是代价:`sys_user.email` 没有索引
/// (两栈的 DDL 都没有,上游 RuoYi 也没有),20 万行数据集上这条校验必然全表扫描,
/// 而取整行要物化约 20 列的宽行 —— 实测 69.3 ms,换成 COUNT 形态 28.1 ms,
/// 补上 email 索引后两者都是 1.0 ms。MySQL 的规划器不会像 PG 那样剪掉没人引用
/// 的列(crates/app-data/src/count.rs 模块头记过同一个坑)。
#[tokio::test]
async fn exists_checks_do_not_fetch_whole_rows() {
    let (ds, repo) = repo_scoped(setup_db().await, ScopeFilter::All);
    let sctx = ds.system_context();

    // 造一行带 email / phone 的用户
    let mut u = repo.select_by_id(&sctx, &301).await.unwrap().unwrap();
    u.user_id = Some(1761300000000000501);
    u.user_name = "uniq_probe".to_string();
    u.email = Some("uniq@example.com".to_string());
    u.phone_number = Some("13800001111".to_string());
    repo.insert(&sctx, &u).await.expect("插入失败");

    // email
    assert!(
        repo.exists_by_email(&sctx, "uniq@example.com", None)
            .await
            .unwrap(),
        "已存在的邮箱必须判为存在"
    );
    assert!(
        !repo
            .exists_by_email(&sctx, "nobody@example.com", None)
            .await
            .unwrap(),
        "不存在的邮箱必须判为不存在"
    );
    assert!(
        !repo
            .exists_by_email(
                &sctx,
                "uniq@example.com",
                Some(1761300000000000501)
            )
            .await
            .unwrap(),
        "排除自己之后不算冲突 —— 改自己的资料不该被自己挡住"
    );

    // phone
    assert!(
        repo.exists_by_phone_number(&sctx, "13800001111", None)
            .await
            .unwrap()
    );
    assert!(
        !repo
            .exists_by_phone_number(
                &sctx,
                "13800001111",
                Some(1761300000000000501)
            )
            .await
            .unwrap(),
        "排除自己之后不算冲突"
    );

    // user_name:此前是取回整行再在内存里比 id,现在下推到 SQL
    assert!(
        repo.exists_by_user_name(&sctx, "uniq_probe", None)
            .await
            .unwrap()
    );
    assert!(
        !repo
            .exists_by_user_name(&sctx, "uniq_probe", Some(1761300000000000501))
            .await
            .unwrap(),
        "排除自己之后不算冲突"
    );
    assert!(
        !repo
            .exists_by_user_name(&sctx, "查无此人", None)
            .await
            .unwrap()
    );
}

/// 唯一性校验必须跨部门全量,否则同邮箱在别的部门会被漏判。
/// 与 `select_by_*` 的 `without_row_scope()` 同一裁定(见本文件头第 3 组语义)。
#[tokio::test]
async fn exists_checks_are_not_narrowed_by_row_scope() {
    let ds = setup_db().await;
    let sctx = ds.system_context();
    let repo = UserRepository::new();
    let mut u = repo.select_by_id(&sctx, &302).await.unwrap().unwrap();
    u.user_id = Some(1761300000000000502);
    u.user_name = "cross_dept_uniq".to_string();
    u.email = Some("cross@example.com".to_string());
    repo.insert(&sctx, &u).await.expect("插入失败");

    // 把范围收窄到 101 部门,而这一行在 102
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(101),
    });
    assert!(
        repo.exists_by_email(&ctx, "cross@example.com", None)
            .await
            .unwrap(),
        "越权部门的同邮箱也必须判为冲突,否则唯一性形同虚设"
    );
}
