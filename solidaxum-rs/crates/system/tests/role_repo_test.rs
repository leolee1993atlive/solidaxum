//! role 仓储集成测试。
//!
//! 覆盖三组语义:
//! 1. 基础 CRUD/分页/反连接(经 `system_context` 桥接);
//! 2. **范围自动施加**:DeptIn → 只见 create_dept 范围行
//!    (sys_role 的 dept 列 = create_dept,Java SysRoleMapper.java:33/:47/:61
//!    /:75 的 deptName=create_dept);SelfOnly → 只见 create_by=本人行
//!    (owner 列 = create_by,同一批注解的 userName=create_by);
//!    allocated/unallocated 按查询根 sys_user 的 DataPolicy 施加
//!    (SysUserMapper.java:93/:114 的 d.dept_id/u.create_by);
//! 3. **方法级豁免**:Java 基线里无 @DataPermission 的方法
//!    (唯一性校验/按用户查角色)显式 `without_row_scope`,范围不收窄;
//!    写路径不施行级范围(update_status 等走豁免面,
//!    越权拦截由 handler 探针承担,判例见 write_scope_rollback_test)。

mod common;

use std::{collections::HashSet, sync::Arc};

use app_core::vo::page::PageQuery;
use app_data::{
    DataError, DataSource, Principal, ScopeFilter,
    sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter},
    testing::FixedResolver,
};
use common::setup_db;
use system::{
    relation::user_role,
    role::{
        handler::role_handler::RolePageQuery,
        repository::role_repo::{self as role_entity, RoleRepository},
    },
    user::handler::user_handler::UserPageQuery,
};

/// 仓库 + 无 resolver 的 DataSource(system_context 语义,ScopeFilter 恒 All)。
fn repo_system(ds: DataSource) -> (Arc<DataSource>, RoleRepository) {
    (Arc::new(ds), RoleRepository::new())
}

/// 仓库 + 固定 ScopeFilter 的 DataSource(范围用例,免搭业务角色语义)。
fn repo_scoped(
    ds: DataSource,
    sf: ScopeFilter,
) -> (Arc<DataSource>, RoleRepository) {
    (
        Arc::new(ds.with_resolver(Arc::new(FixedResolver(sf)))),
        RoleRepository::new(),
    )
}

fn empty_role_query() -> RolePageQuery {
    RolePageQuery {
        page: PageQuery {
            page_num: Some(1),
            page_size: Some(10),
            order_by_column: None,
            is_asc: None,
        },
        keyword: None,
        role_name: None,
        role_key: None,
        status: None,
        begin_time: None,
        end_time: None,
    }
}

fn page_query() -> UserPageQuery {
    UserPageQuery {
        page: PageQuery {
            page_num: Some(1),
            page_size: Some(10),
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

/// 范围用例的补充种子(基础种子:200~205 的 create_dept/create_by 均为 NULL
/// —— NULL 不属于任何部门范围,DeptIn/SelfOnly 下均不可见):
/// - 210:create_dept=101、create_by=7(DeptIn 与 SelfOnly 的交集行)
/// - 220:create_dept=102 的行(越权部门)
/// - 230:create_dept=101、del_flag='1' 的已删行(软删不可见)
///   免盖章种子:显式审计值与 del_flag='1' 原样落库(insert_unstamped)。
async fn seed_scope_roles(ds: &DataSource) {
    let ctx = ds.system_context();
    for (id, name, key, sort, del_flag, create_dept, create_by) in [
        (210, "范围角色A", "scope_a", 6, "0", 101, 7),
        (220, "范围角色B", "scope_b", 7, "0", 102, 8),
        (230, "已删角色", "scope_dead", 8, "1", 101, 7),
    ] {
        ctx.insert_unstamped(role_entity::ActiveModel {
            role_id: Set(id),
            role_name: Set(name.into()),
            role_key: Set(key.into()),
            role_sort: Set(sort),
            data_scope: Set("1".into()),
            menu_check_strictly: Set(true),
            dept_check_strictly: Set(true),
            status: Set("0".into()),
            del_flag: Set(del_flag.into()),
            create_dept: Set(Some(create_dept)),
            create_by: Set(Some(create_by)),
            ..Default::default()
        })
        .await
        .expect("种范围角色行失败");
    }
}

/// role_repo.rs select_allocated_user_page 分页边界(首页/末页/超出页/空结果/过滤)
///
/// spec §8 把「手写 JOIN + 子查询 count + 手工 LIMIT/OFFSET」列为原生 SQL 归零
/// 最高风险项;行集+total 断言之外,分页边界(首页/末页/
/// 超出页/空页)也要覆盖。本测试用 page_size=2 + 5 个用户构造 3 页,逐边界断言。
#[tokio::test]
async fn test_select_allocated_user_page_boundaries() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // role 200 seed 中仅有 301;追加 302~305 → 共 5 个用户,page_size=2 跨 3 页。
    // (sys_user_role 无 PK 约束,只追加 seed 中不存在的 user_id 避免重复行。)
    repo.insert_user_roles(&ctx, 200, &[302, 303, 304, 305])
        .await
        .unwrap();

    let mut q = page_query();
    q.page.page_size = Some(2);

    // 首页(page 1):2 行,按 user_id 升序应为 301, 302
    q.page.page_num = Some(1);
    let (users, total) = repo
        .select_allocated_user_page(&ctx, 200, &q)
        .await
        .expect("查询失败");
    assert_eq!(total, 5, "total 应为 5");
    assert_eq!(users.len(), 2, "首页应 2 行");
    let ids: Vec<i64> = users.iter().filter_map(|u| u.user_id).collect();
    assert_eq!(ids, vec![301, 302], "首页升序应为 [301, 302]");

    // 中间页(page 2):2 行 [303, 304]
    q.page.page_num = Some(2);
    let (users, total) = repo
        .select_allocated_user_page(&ctx, 200, &q)
        .await
        .expect("查询失败");
    assert_eq!(total, 5);
    assert_eq!(users.len(), 2);
    let ids: Vec<i64> = users.iter().filter_map(|u| u.user_id).collect();
    assert_eq!(ids, vec![303, 304], "第 2 页应为 [303, 304]");

    // 末页(page 3):仅 1 行 [305]
    q.page.page_num = Some(3);
    let (users, total) = repo
        .select_allocated_user_page(&ctx, 200, &q)
        .await
        .expect("查询失败");
    assert_eq!(total, 5);
    assert_eq!(users.len(), 1, "末页应 1 行");
    let ids: Vec<i64> = users.iter().filter_map(|u| u.user_id).collect();
    assert_eq!(ids, vec![305], "末页应 1 行 [305]");

    // 超出页(page 4):空,但 total 仍为 5(分页的 count 与页号无关)
    q.page.page_num = Some(4);
    let (users, total) = repo
        .select_allocated_user_page(&ctx, 200, &q)
        .await
        .expect("查询失败");
    assert_eq!(total, 5, "超出页 total 仍应为 5");
    assert!(users.is_empty(), "超出页应返回空");

    // 空结果:不存在的 role → total 0、行空
    q.page.page_num = Some(1);
    let (users, total) = repo
        .select_allocated_user_page(&ctx, 99999, &q)
        .await
        .expect("查询失败");
    assert_eq!(total, 0, "不存在的 role total 应为 0");
    assert!(users.is_empty(), "不存在的 role 应返回空");

    // 过滤收窄边界:user_name LIKE %u_dept% 命中 u_dept(303) + u_dept_children(304)
    // → total 2;page_size=2 恰好一页
    q.page.page_num = Some(1);
    q.user_name = Some("u_dept".to_string());
    let (users, total) = repo
        .select_allocated_user_page(&ctx, 200, &q)
        .await
        .expect("查询失败");
    assert_eq!(total, 2, "user_name=u_dept 过滤后 total 应为 2");
    assert_eq!(users.len(), 2);
    let ids: Vec<i64> = users.iter().filter_map(|u| u.user_id).collect();
    assert_eq!(ids, vec![303, 304], "过滤后应为 [303, 304]");

    // 过滤后再翻超出页:total 仍为 2,行空
    q.page.page_num = Some(2);
    let (users, total) = repo
        .select_allocated_user_page(&ctx, 200, &q)
        .await
        .expect("查询失败");
    assert_eq!(total, 2, "过滤后超出页 total 仍为 2");
    assert!(users.is_empty(), "过滤后超出页应返回空");
}

#[tokio::test]
async fn test_select_roles_by_user_id() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let roles = repo
        .select_roles_by_user_id(&ctx, 302)
        .await
        .expect("查询失败");

    assert_eq!(roles.len(), 1);
    let role = &roles[0];
    assert_eq!(role.role_id, Some(201));
    assert_eq!(role.role_key, "role_custom");
    assert_eq!(role.data_scope, "2");
}

#[tokio::test]
async fn test_select_roles_by_user_id_no_role() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let roles = repo
        .select_roles_by_user_id(&ctx, 307)
        .await
        .expect("查询失败");

    assert!(roles.is_empty());
}

#[tokio::test]
async fn test_select_dept_ids_by_role_ids() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let dept_ids = repo
        .select_dept_ids_by_role_ids(&ctx, &[201])
        .await
        .expect("查询失败");

    let expected: HashSet<i64> = [101, 103].into_iter().collect();
    let actual: HashSet<i64> = dept_ids.into_iter().collect();
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn test_select_dept_ids_by_empty_role_ids() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let dept_ids = repo
        .select_dept_ids_by_role_ids(&ctx, &[])
        .await
        .expect("查询失败");

    assert!(dept_ids.is_empty());
}

// ─── 角色维度 user_role 关联(role_repo 原 :636/661)───
// 注意:与 user_repo 维度的 insert_user_roles(user_id, role_ids) 不同,
// 这里是 role_repo.insert_user_roles(role_id, user_ids) / delete_user_roles(role_id, user_ids),
// 供「角色授权用户」(authUser/selectAll/cancelAll)使用。

/// role 维度 `INSERT INTO sys_user_role … VALUES {}` +
/// `DELETE FROM sys_user_role WHERE role_id = ? AND user_id IN ({})`
#[tokio::test]
async fn test_role_insert_and_delete_user_roles() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // seed: role 200 → users {301};给 role 200 追加 302, 303
    repo.insert_user_roles(&ctx, 200, &[302, 303])
        .await
        .unwrap();

    let assigned = user_ids_of_role(&ctx, 200).await;
    let expected: HashSet<i64> = [301, 302, 303].into_iter().collect();
    assert_eq!(assigned, expected, "追加后 role 200 应含 301/302/303");

    // 取消 302(只删 role=200 且 user=302 的关联,不影响 301/303)
    repo.delete_user_roles(&ctx, 200, &[302]).await.unwrap();
    let assigned = user_ids_of_role(&ctx, 200).await;
    let expected: HashSet<i64> = [301, 303].into_iter().collect();
    assert_eq!(assigned, expected, "删除 302 后 role 200 应含 301/303");

    // 空集合早退:不报错、不改数据
    repo.insert_user_roles(&ctx, 200, &[]).await.unwrap();
    repo.delete_user_roles(&ctx, 200, &[]).await.unwrap();
    let assigned = user_ids_of_role(&ctx, 200).await;
    assert_eq!(
        assigned,
        [301, 303].into_iter().collect::<HashSet<i64>>(),
        "空集合早退不应改变数据"
    );
}

/// 辅助:直接查 sys_user_role 拿某角色的全部 user_id(不依赖被测方法;
/// 关联表 CONSTRAINED=false → without_row_scope,与原裸 SQL 直读等价)
async fn user_ids_of_role(
    ctx: &app_data::DataContext<'_>,
    role_id: i64,
) -> HashSet<i64> {
    ctx.select(
        user_role::Entity::find().filter(user_role::Column::RoleId.eq(role_id)),
    )
    .without_row_scope()
    .all()
    .await
    .unwrap()
    .into_iter()
    .map(|r| r.user_id)
    .collect()
}

// ─── 角色已分配/未分配用户分页(反连接)───
// JOIN + 动态 WHERE + 子查询反连接,是「原生 SQL 归零」最高风险项
// (spec §8),需断言行集 + total + 分页边界。

#[tokio::test]
async fn test_select_allocated_user_page() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // seed: role 201 → users 302, 308
    let q = page_query();
    let (users, total) = repo
        .select_allocated_user_page(&ctx, 201, &q)
        .await
        .expect("查询失败");

    let ids: HashSet<i64> = users.iter().filter_map(|u| u.user_id).collect();
    let expected: HashSet<i64> = [302, 308].into_iter().collect();
    assert_eq!(ids, expected, "role 201 已分配用户应为 302, 308");
    assert_eq!(total, 2, "total 应为 2");

    // user_name 模糊过滤:只命中 u_custom(302)
    let mut q2 = page_query();
    q2.user_name = Some("u_custom".to_string());
    let (users, total) = repo
        .select_allocated_user_page(&ctx, 201, &q2)
        .await
        .expect("查询失败");
    let ids: Vec<i64> = users.iter().filter_map(|u| u.user_id).collect();
    assert_eq!(ids, vec![302], "user_name 过滤后应只剩 302");
    assert_eq!(total, 1);
}

/// spec §8 最高风险:反连接 `user_id NOT IN (SELECT user_id FROM sys_user_role WHERE role_id=?)`,
/// 命门是结果集必须返回"没有该 role 的用户",绝不能退化成返回已分配者或全部用户。
/// 本测试用大 page_size 一次性取全,断言精确行集(排除已分配者 302/308,且包含全部 11 个未分配者)。
///
/// 计划要求的"SQL 前后对照"由本测试与 [`test_select_unallocated_user_page_boundaries`]
/// 以**行为**承担,不再另写断言生成 SQL 子串的测试:那种写法只能在测试内重建一遍查询
/// 再断言自己拼出的字符串,改动 repository 时照样绿 —— 给的是虚假信心。
/// 语义反转、空子查询、分页边界这三处真正的风险,行为断言都能抓到,且抓的是真实代码路径。
#[tokio::test]
async fn test_select_unallocated_user_page() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // seed 共 13 个 del_flag=0 用户;role 201 占 302, 308 → 未分配 11 个
    let mut q = page_query();
    q.page.page_size = Some(100); // 一次取全,便于断言精确行集
    let (users, total) = repo
        .select_unallocated_user_page(&ctx, 201, &q)
        .await
        .expect("查询失败");

    assert_eq!(total, 11, "role 201 未分配用户 total 应为 11");
    assert_eq!(users.len(), 11, "未分配用户应恰好 11 个");

    // 精确行集:13 个用户剔除已分配的 302/308 → 这 11 个(user_id 升序)。
    let mut ids: Vec<i64> = users.iter().filter_map(|u| u.user_id).collect();
    ids.sort();
    let expected: Vec<i64> = vec![
        301,
        303,
        304,
        305,
        306,
        307,
        401,
        402,
        403,
        404,
        1761100000000000001,
    ];
    assert_eq!(
        ids, expected,
        "未分配行集应精确等于剔除 302/308 后的全部用户"
    );

    // 双保险:已分配者绝不能出现(语义反转的最典型退化)。
    let idset: HashSet<i64> = users.iter().filter_map(|u| u.user_id).collect();
    assert!(!idset.contains(&302), "302 已分配,绝不能出现在未分配列表");
    assert!(!idset.contains(&308), "308 已分配,绝不能出现在未分配列表");

    // —— 反向边界:role_id 不存在(无人持有)⇒ NOT IN (空集) ⇒ 全部 13 个用户都"未分配"。
    // 这验证 NOT IN (空子查询)= TRUE 的 SQL 语义未被改坏(不能退化成空结果)。
    let (users, total) = repo
        .select_unallocated_user_page(&ctx, 99999, &q)
        .await
        .expect("查询失败");
    assert_eq!(total, 13, "不存在的 role:全部用户都未分配,total 应为 13");
    assert_eq!(users.len(), 13, "不存在的 role 应返回全部 13 个用户");
}

/// select_unallocated_user_page 分页边界(首页/末页/超出页/空结果/过滤收窄)。
///
/// 对应 allocated 的边界测试,重点覆盖"空结果"边界:当某 role 分配给**全部**用户时,
/// 未分配列表必须返回空 + total 0(NOT IN 命中所有 user_id)。这是反连接最容易出错的地方。
#[tokio::test]
async fn test_select_unallocated_user_page_boundaries() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // seed: role 201 → users {302, 308};未分配 11 个,page_size=5 跨 3 页。
    let mut q = page_query();
    q.page.page_size = Some(5);

    // 首页(page 1):5 行 [301, 303, 304, 305, 306]
    q.page.page_num = Some(1);
    let (users, total) = repo
        .select_unallocated_user_page(&ctx, 201, &q)
        .await
        .expect("查询失败");
    assert_eq!(total, 11, "total 应为 11");
    assert_eq!(users.len(), 5, "首页应 5 行");
    let ids: Vec<i64> = users.iter().filter_map(|u| u.user_id).collect();
    assert_eq!(
        ids,
        vec![301, 303, 304, 305, 306],
        "首页升序应为 [301..306]"
    );

    // 末页(page 3):1 行 [1761100000000000001](最大 id 排最后)
    q.page.page_num = Some(3);
    let (users, total) = repo
        .select_unallocated_user_page(&ctx, 201, &q)
        .await
        .expect("查询失败");
    assert_eq!(total, 11);
    assert_eq!(users.len(), 1, "末页应 1 行");
    let ids: Vec<i64> = users.iter().filter_map(|u| u.user_id).collect();
    assert_eq!(ids, vec![1761100000000000001], "末页应为最大 user_id");

    // 超出页(page 4):空,但 total 仍为 11
    q.page.page_num = Some(4);
    let (users, total) = repo
        .select_unallocated_user_page(&ctx, 201, &q)
        .await
        .expect("查询失败");
    assert_eq!(total, 11, "超出页 total 仍应为 11");
    assert!(users.is_empty(), "超出页应返回空");

    // —— 过滤收窄(干净 seed 数据):user_name LIKE %dept% 命中 u_dept(303)/u_dept_children(304)
    // /u_dept_self(306);role 201 未分配给它们 ⇒ 未分配 3 个。
    q.page.page_num = Some(1);
    q.user_name = Some("dept".to_string());
    let (users, total) = repo
        .select_unallocated_user_page(&ctx, 201, &q)
        .await
        .expect("查询失败");
    assert_eq!(total, 3, "user_name=dept 过滤后 total 应为 3");
    let ids: Vec<i64> = users.iter().filter_map(|u| u.user_id).collect();
    assert_eq!(ids, vec![303, 304, 306], "过滤后应为 [303, 304, 306]");

    // —— 空结果边界(反连接最易错,放在过滤后):把 role 201 分配给**全部** 13 个 del_flag=0 用户,
    // 则无人"未分配" ⇒ 行空 + total 0(NOT IN 命中所有 user_id)。
    // (302/308 已有,只补其余 11 个,避免重复行。)
    q.user_name = None; // 去掉过滤
    repo.insert_user_roles(
        &ctx,
        201,
        &[
            301,
            303,
            304,
            305,
            306,
            307,
            401,
            402,
            403,
            404,
            1761100000000000001,
        ],
    )
    .await
    .expect("补充分配");
    let (users, total) = repo
        .select_unallocated_user_page(&ctx, 201, &q)
        .await
        .expect("查询失败");
    assert_eq!(total, 0, "全员已分配 role 201:未分配 total 必须为 0");
    assert!(users.is_empty(), "全员已分配:未分配行必须为空");

    // —— 过滤 + 全员已分配:user_name LIKE %target% 命中 4 个 target_* 用户,但它们此刻都已分配
    // role 201 ⇒ 未分配列表仍为空(NOT IN + LIKE 同时生效,过滤不破坏反连接)。
    q.user_name = Some("target".to_string());
    let (users, total) = repo
        .select_unallocated_user_page(&ctx, 201, &q)
        .await
        .expect("查询失败");
    assert_eq!(total, 0, "过滤后(全员已分配)total 仍应为 0");
    assert!(users.is_empty(), "过滤后(全员已分配)应返回空");
}

// ─── 角色状态更新 ───
/// `UPDATE sys_role SET status = ? WHERE role_id = ? AND del_flag = '0'`
/// (WHERE 只剩主键 + 软删存活;0 行 → NotFound)
#[tokio::test]
async fn test_role_update_status() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // role 201 (del_flag=0) 停用
    repo.update_status(&ctx, 201, "1").await.expect("更新失败");

    // 恢复,避免污染(本测试用独立 in-memory db,仅作行为断言)
    repo.update_status(&ctx, 201, "0").await.unwrap();

    // 不存在/已删除的角色 → 0 行受影响 → NotFound
    let err = repo.update_status(&ctx, 99999, "1").await;
    assert!(
        matches!(err, Err(DataError::NotFound)),
        "不存在的角色应返回 NotFound:{err:?}"
    );
}

// ============================================================
// 范围自动施加 + 方法级豁免(Java 注解对齐)
// ============================================================

/// DeptIn → page/list/select_all 只见 create_dept ∈ 范围的行;
/// 越权部门行、NULL create_dept 行、已删行均不计入。
/// (基础种子 200~205 的 create_dept 为 NULL → DeptIn 下全部不可见,
/// 这是与 Java 一致的语义:`create_dept IN (...)` 不含 NULL。)
#[tokio::test]
async fn scope_dept_in_narrows_page_list_and_all() {
    let ds = setup_db().await;
    seed_scope_roles(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    let query = empty_role_query();
    let page = repo
        .select_page(&ctx, &query, 1, 10, "role_sort asc")
        .await
        .expect("分页查询失败");
    assert_eq!(page.total, 1, "DeptIn([101]) 下只有 210 可见");
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].role_id, 210);

    let list = repo
        .select_list(&ctx, &query, "role_sort asc")
        .await
        .expect("导出查询失败");
    assert_eq!(
        list.iter().map(|r| r.role_id).collect::<Vec<_>>(),
        vec![Some(210)],
        "导出与列表同口径(Java 两者都走带注解的 selectRoleList)"
    );

    // status=None:不分页列表口径(selectAll 的空 bo;optionselect
    // 传 Some("0") 才有状态过滤)
    let all = repo.select_all(&ctx, &None).await.expect("全量查询失败");
    assert_eq!(
        all.iter().map(|r| r.role_id).collect::<Vec<_>>(),
        vec![Some(210)],
        "select_all(Java selectRoleAll → 带注解)同样受辖"
    );
}

/// SelfOnly → 只见 create_by=本人的行(owner 列 = create_by,
/// SysRoleMapper.java:33 等注解的 userName=create_by)。
#[tokio::test]
async fn scope_self_only_sees_only_own_rows() {
    let ds = setup_db().await;
    seed_scope_roles(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::SelfOnly(7));
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    let query = empty_role_query();
    let page = repo
        .select_page(&ctx, &query, 1, 10, "role_sort asc")
        .await
        .expect("分页查询失败");
    assert_eq!(page.total, 1, "SelfOnly(7) 下只有 create_by=7 的 210 可见");
    assert_eq!(page.rows[0].role_id, 210);
    // 220(create_by=8)与 NULL create_by 行不得出现
    assert!(
        !page.rows.iter().any(|r| r.role_id == 220),
        "他人创建的 220 不可见"
    );
}

/// 受辖 select_by_id:范围外的行与不存在的行同为 None(Java getInfo →
/// selectRoleById Mapper:75 带注解 —— 越权即查无)。
#[tokio::test]
async fn scope_select_by_id_out_of_range_is_none() {
    let ds = setup_db().await;
    seed_scope_roles(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    assert!(
        repo.select_by_id(&ctx, &210).await.unwrap().is_some(),
        "范围内 210 应可见"
    );
    assert!(
        repo.select_by_id(&ctx, &220).await.unwrap().is_none(),
        "范围外 220 应不可见(越权 ⟺ 不存在)"
    );
    assert!(
        repo.select_by_id(&ctx, &230).await.unwrap().is_none(),
        "已删 230 应不可见(软删)"
    );
}

/// 豁免方法:即便 resolver 给出 DeptIn([101]),唯一性校验与"我的角色"
/// 查询也必须看见全部存活行(Java 基线无注解)。
#[tokio::test]
async fn exempt_methods_see_all_rows_despite_scope() {
    let ds = setup_db().await;
    seed_scope_roles(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    // 唯一性校验(checkRoleKeyUnique / checkRoleNameUnique):
    // 跨部门行(NULL create_dept 的 201)必须可见,否则同名同 key 漏判
    let r = repo.select_by_role_key(&ctx, "role_custom").await.unwrap();
    assert_eq!(
        r.and_then(|x| x.role_id),
        Some(201),
        "唯一性校验须跨部门全量(without_row_scope)"
    );
    let r = repo
        .select_by_role_name(&ctx, "自定义数据角色", None)
        .await
        .unwrap();
    assert_eq!(r.and_then(|x| x.role_id), Some(201));

    // "我的角色"(selectRolesByUserId Mapper:89 无注解):
    // user 302 的角色 201(create_dept=NULL)照常可见
    let roles = repo.select_roles_by_user_id(&ctx, 302).await.unwrap();
    assert_eq!(roles.len(), 1, "我的角色不受范围收窄");
    assert_eq!(roles[0].role_id, Some(201));
}

/// 写路径不施行级范围:update_status 不被行级范围拦住 ——
/// Java 写方法无 @DataPermission,受辖 update 的失败态是静默 0 行、
/// 边界随数据范围抖动,本仓写路径统一走豁免面,越权拦截由 handler 的
/// `check_role_data_scope` 探针承担。本用例钉住 repo 层语义:
/// 越权写成功 + 落库复核。
#[tokio::test]
async fn scoped_update_on_foreign_dept_row_succeeds_after_rollback() {
    let ds = setup_db().await;
    seed_scope_roles(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    // 越权写 220(create_dept=102):WHERE 只剩主键 + 软删存活,
    // 改得动。
    repo.update_status(&ctx, 220, "1")
        .await
        .expect("回退后越权目标的 update 不应再被行级范围拦住");

    // 改动必须落库(用 system_context 复核)
    let row = repo
        .select_by_id(&ds.system_context(), &220)
        .await
        .expect("复核查询失败")
        .expect("220 应存在");
    assert_eq!(row.status, "1", "回退后越权 update 应落库");

    // 范围内行照常成功
    repo.update_status(&ctx, 210, "1")
        .await
        .expect("范围内更新应成功");
}

/// allocated/unallocated 受辖于查询根 sys_user 的 DataPolicy
/// (SysUserMapper.java:93/:114 的 d.dept_id/u.create_by)。
/// DeptIn([101]) 下:allocated 只统计部门 101 的已分配用户,
/// unallocated 只列部门 101 的未分配用户 —— 种子里 dept 101 的用户是
/// 301~308 与 402(401 是 100,403 是 103,404 是 104)。
#[tokio::test]
async fn allocated_unallocated_scoped_by_user_policy() {
    let (ds, repo) =
        repo_scoped(setup_db().await, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    let mut q = page_query();
    q.page.page_size = Some(100);
    // seed:role 201 → users {302, 308},均 dept 101 → allocated 仍 2
    let (users, total) = repo
        .select_allocated_user_page(&ctx, 201, &q)
        .await
        .expect("allocated 查询失败");
    assert_eq!(total, 2, "两个已分配用户都在 dept 101");
    let ids: Vec<i64> = users.iter().filter_map(|u| u.user_id).collect();
    assert_eq!(ids, vec![302, 308]);

    // unallocated:dept 101 的用户共 9 个(301~308 + 402),已分配 302/308
    // → 未分配 7 个;401/403/404(其他部门)与 super_admin(dept 100)不可见
    let (users, total) = repo
        .select_unallocated_user_page(&ctx, 201, &q)
        .await
        .expect("unallocated 查询失败");
    assert_eq!(total, 7, "范围把未分配列表收窄到 dept 101 的 7 个用户");
    let mut ids: Vec<i64> = users.iter().filter_map(|u| u.user_id).collect();
    ids.sort();
    assert_eq!(ids, vec![301, 303, 304, 305, 306, 307, 402]);
}
