//! post 仓储集成测试。
//!
//! 覆盖三组语义:
//! 1. 基础 CRUD/分页(经 `system_context` 桥接);
//! 2. **范围自动施加**:DeptIn → 只见本部门行;SelfOnly → 只见
//!    create_by=本人行(post 的 owner 列 = create_by,Java 基线
//!    SysPostMapper.java:31 userName=create_by);
//! 3. **方法级豁免**:Java 基线里无 @DataPermission 的方法
//!    (下拉/详情/唯一性/按部门计数)显式 `without_row_scope`,范围不收窄;
//!    写路径不施行级范围(Java 写方法无注解、
//!    SysPostController 无探针,post 的写路径本就不受辖,判例见
//!    write_scope_rollback_test)。

mod common;

use std::sync::Arc;

use app_data::{
    DataSource, Principal, ScopeFilter,
    sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter},
    testing::FixedResolver,
};
use common::{setup_db, setup_schema_and_seeds};
use system::post::{
    model::post::PostPageQuery,
    repository::post_repo::{self as post_entity, PostRepository},
};

/// 仓库 + 无 resolver 的 DataSource(system_context 语义,ScopeFilter 恒 All)。
fn repo_system(ds: DataSource) -> (Arc<DataSource>, PostRepository) {
    (Arc::new(ds), PostRepository::new())
}

/// 仓库 + 固定 ScopeFilter 的 DataSource(范围用例,免搭业务角色语义)。
fn repo_scoped(
    ds: DataSource,
    sf: ScopeFilter,
) -> (Arc<DataSource>, PostRepository) {
    (
        Arc::new(ds.with_resolver(Arc::new(FixedResolver(sf)))),
        PostRepository::new(),
    )
}

fn empty_query() -> PostPageQuery {
    PostPageQuery {
        page: app_core::vo::page::PageQuery {
            page_num: Some(1),
            page_size: Some(10),
            order_by_column: None,
            is_asc: None,
        },
        post_code: None,
        post_name: None,
        post_category: None,
        belong_dept_id: None,
        dept_id: None,
        status: None,
    }
}

/// 范围用例的补充种子(基础种子:001/002 均在 dept 101,create_by 为 NULL):
/// - 101 部门、create_by=7 的行(DeptIn 与 SelfOnly 的交集行)
/// - 102 部门的行(越权部门)
/// - 101 部门、del_flag='1' 的已删行(软删不可见)
///   免盖章种子:显式 create_by / del_flag='1' 原样落库(insert_unstamped)。
async fn seed_scope_rows(ds: &DataSource) {
    let ctx = ds.system_context();
    ctx.insert_unstamped(post_entity::ActiveModel {
        post_id: Set(1761200000000000101),
        dept_id: Set(101),
        post_code: Set("dev1".into()),
        post_name: Set("开发岗(建岗人7)".into()),
        post_sort: Set(5),
        status: Set("0".into()),
        del_flag: Set("0".into()),
        create_by: Set(Some(7)),
        ..Default::default()
    })
    .await
    .expect("种 101 部门 create_by=7 行失败");
    ctx.insert_unstamped(post_entity::ActiveModel {
        post_id: Set(1761200000000000201),
        dept_id: Set(102),
        post_code: Set("sales".into()),
        post_name: Set("销售岗".into()),
        post_sort: Set(6),
        status: Set("0".into()),
        del_flag: Set("0".into()),
        ..Default::default()
    })
    .await
    .expect("种 102 部门行失败");
    ctx.insert_unstamped(post_entity::ActiveModel {
        post_id: Set(1761200000000000301),
        dept_id: Set(101),
        post_code: Set("legacy".into()),
        post_name: Set("已删岗".into()),
        post_sort: Set(7),
        status: Set("0".into()),
        del_flag: Set("1".into()),
        ..Default::default()
    })
    .await
    .expect("种已删行失败");
}

#[tokio::test]
async fn test_select_by_post_code() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();
    let post = repo
        .select_by_post_code(&ctx, "ceo")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(post.post_name, "董事长");
}

#[tokio::test]
async fn test_insert_and_delete() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    ctx.insert_unstamped(post_entity::ActiveModel {
        post_id: Set(1761200000000000003),
        dept_id: Set(102),
        post_code: Set("test".into()),
        post_name: Set("测试岗位".into()),
        post_sort: Set(3),
        status: Set("0".into()),
        del_flag: Set("0".into()),
        ..Default::default()
    })
    .await
    .expect("插入岗位失败");

    let post = repo
        .select_by_id(&ctx, &1761200000000000003)
        .await
        .expect("查询失败")
        .expect("岗位不存在");
    assert_eq!(post.post_code, "test");
    assert_eq!(post.post_name, "测试岗位");
    assert_eq!(post.del_flag, "0", "新建岗位 del_flag 应为 '0'");

    repo.delete_by_id(&ctx, &1761200000000000003)
        .await
        .expect("删除失败");

    // 软删：select_by_id 带存活过滤，应查不到
    let deleted = repo
        .select_by_id(&ctx, &1761200000000000003)
        .await
        .expect("查询失败");
    assert!(deleted.is_none(), "删除后应查询不到");

    // 软删：底层记录仍在，del_flag 已置 '1'（对齐 Java @TableLogic）——
    // 经 include_deleted 整行读回(逃生舱无投影,断言落在 del_flag 字段,
    // 与原裸 SQL 单列读等价)
    let row = ctx
        .select(
            post_entity::Entity::find().filter(
                post_entity::Column::PostId.eq(1761200000000000003_i64),
            ),
        )
        .include_deleted()
        .one()
        .await
        .expect("查询失败")
        .expect("软删后记录应仍存在");
    assert_eq!(row.del_flag, "1", "软删后 del_flag 应为 '1'");
}

#[tokio::test]
async fn test_select_page() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let page = repo
        .select_page(&ctx, &empty_query(), None, 1, 10, "post_sort asc")
        .await
        .expect("分页查询失败");

    assert_eq!(page.total, 2);
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.rows[0].post_name, "董事长");
    assert_eq!(page.rows[1].post_name, "项目经理");
}

#[tokio::test]
async fn test_count_by_dept_id() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let count = repo.count_by_dept_id(&ctx, 101).await.expect("统计失败");
    assert_eq!(count, 2);

    let count = repo.count_by_dept_id(&ctx, 102).await.expect("统计失败");
    assert_eq!(count, 0);
}

// ---------------------------------------------------------------------------
// 范围自动施加(scoped 方法 = Java 带 @DataPermission 的路径)
// ---------------------------------------------------------------------------

/// DeptIn → 列表只见本部门行;越权部门与已删行均不计入 total。
#[tokio::test]
async fn scope_dept_in_narrows_page_and_list() {
    let ds = setup_db().await;
    seed_scope_rows(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(101),
    });

    let page = repo
        .select_page(&ctx, &empty_query(), None, 1, 10, "post_sort asc")
        .await
        .expect("分页查询失败");
    assert_eq!(
        page.total, 3,
        "dept 101 三行可见(种子 2 + create_by=7 行);102 的越权行与已删行不计入"
    );
    assert_eq!(page.rows.len(), 3);

    // 导出(select_list)按 Java 基线**豁免**(现网走 selectVoList 基方法,
    // 无 @DataPermission;Mapper 里带注解的 selectPostList 零调用)——
    // 与分页不同口径,4 行存活全可见。这是有意偏差,见 post_repo 模块头。
    let list = repo
        .select_list(&ctx, &empty_query(), None, "post_sort asc")
        .await
        .expect("导出查询失败");
    assert_eq!(
        list.len(),
        4,
        "导出是 Java 豁免路径:4 行存活全可见(软删行仍被挡)"
    );
}

/// SelfOnly → 只见 create_by=本人 的行(owner 列 = create_by)。
#[tokio::test]
async fn scope_self_only_sees_only_own_rows() {
    let ds = setup_db().await;
    seed_scope_rows(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::SelfOnly(7));
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    let page = repo
        .select_page(&ctx, &empty_query(), None, 1, 10, "post_sort asc")
        .await
        .expect("分页查询失败");
    assert_eq!(
        page.total, 1,
        "仅 create_by=7 的行可见,种子行(create_by=NULL)不可见"
    );
    assert_eq!(page.rows[0].post_code, "dev1");
}

// ---------------------------------------------------------------------------
// 方法级豁免(Java 基线无 @DataPermission 的方法全量可见)
// ---------------------------------------------------------------------------

/// 下拉/详情/唯一性/按部门计数在 Java 侧均无 @DataPermission:
/// 即便 resolver 给出 DeptIn([101]),这些方法也必须看见全部存活行。
#[tokio::test]
async fn exempt_methods_see_all_rows_despite_scope() {
    let ds = setup_db().await;
    seed_scope_rows(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(101),
    });

    // select_all(下拉,Java selectPostAll → voList() 无注解)
    let all = repo.select_all(&ctx).await.expect("全量查询失败");
    assert_eq!(
        all.len(),
        4,
        "豁免方法可见 4 行存活数据(101 三行 + 102 一行),已删行仍被软删过滤挡住"
    );

    // select_by_id(详情,Java selectPostById → selectVoById 无注解)
    let other_dept = repo
        .select_by_id(&ctx, &1761200000000000201)
        .await
        .expect("详情查询失败");
    assert!(
        other_dept.is_some(),
        "越权部门的详情查询照常可见(Java 同口径)"
    );

    // count_by_dept_id(部门删除守卫,Java countPostByDeptId → count() 无注解)
    let count = repo.count_by_dept_id(&ctx, 102).await.expect("统计失败");
    assert_eq!(count, 1, "按部门计数不收窄 —— 部门删除守卫需要真实计数");
}

/// 写路径不施行级范围:软删不被行级范围拦住 —— Java
/// SysPostMapper 写方法无 @DataPermission,且 SysPostController 完全没有
/// 数据范围探针,post 的写路径在 Java 侧本就不受辖,本仓照此平价。
/// 本用例钉住 repo 层语义:越权软删成功 + 落库复核;
/// 仍有效的正向锚:已删行再删 → NotFound(存活过滤仍在 WHERE)。
#[tokio::test]
async fn scoped_soft_delete_on_foreign_dept_row_succeeds_after_rollback() {
    let ds = setup_db().await;
    seed_scope_rows(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(101),
    });

    // 越权软删 102 部门的行:WHERE 只剩主键 + 软删存活,删得动。
    repo.delete_by_id(&ctx, &1761200000000000201)
        .await
        .expect("回退后越权岗位的软删不应再被行级范围拦住");

    // 落库复核:经 include_deleted 整行读回,del_flag 应已置 '1'
    // (软删非物理删,行仍在;同 test_insert_and_delete 的复核法)
    let sctx = ds.system_context();
    let row = sctx
        .select(
            post_entity::Entity::find().filter(
                post_entity::Column::PostId.eq(1761200000000000201_i64),
            ),
        )
        .include_deleted()
        .one()
        .await
        .expect("复核查询失败")
        .expect("软删后行应仍在");
    assert_eq!(row.del_flag, "1", "回退后越权软删应落库");

    // 正向锚:已删行再删 → NotFound(存活过滤仍在 WHERE)
    let err = repo
        .delete_by_id(&ctx, &1761200000000000301)
        .await
        .expect_err("已删行再删必须报错");
    assert!(
        matches!(err, app_data::DataError::NotFound),
        "已删行 → NotFound,实际: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// fix round 1:H-3 业务搜索语义(belongDeptId 子树,经 service 层展开)
// 部门种子(common/mod.rs):100 总部 ├─ 101 技术部(├ 103 前端组 / 104 后端组)
//                                          └─ 102 销售部
// ---------------------------------------------------------------------------

/// 供 service 级用例:惰性 Redis 池 + Infrastructure(与 admin testkit 同一
/// 处置,测试路径不触 Redis)。Infrastructure 零连接 —— 需要装配 ds
/// 的用例自行 set_data_source,本文件的 service 构造不读它。
fn infra_over() -> app_core::infra::Infrastructure {
    let redis_pool = deadpool_redis::Config::from_url("redis://127.0.0.1:6379")
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("创建惰性 redis 池失败");
    app_core::infra::Infrastructure::new(
        Arc::new(app_core::infra::ConnectionPool { redis_pool }),
        false,
        1800,
    )
}

/// service 级用例的建库:自持 DataSource + 全套建表/种子/范围种子。
async fn setup_conn_seeded() -> Arc<DataSource> {
    let ds = Arc::new(app_data::testing::sqlite_memory_empty().await);
    setup_schema_and_seeds(&ds).await;
    seed_scope_rows(&ds).await;
    ds
}

fn query_with_belong(belong_dept_id: i64) -> PostPageQuery {
    PostPageQuery {
        belong_dept_id: Some(belong_dept_id),
        ..empty_query()
    }
}

/// All 权限(system_context)+ belongDeptId=X → 列表只含 X 子树。
/// 钉住 H-3:全量可见的用户点击部门树节点,列表必须收窄(Java 同口径)。
#[tokio::test]
async fn belong_dept_id_narrows_list_for_all_scope_user() {
    let ds = setup_conn_seeded().await;
    let svc =
        system::post::service::post_service::PostService::new(&infra_over());
    let ctx = ds.system_context();

    // belongDeptId=101 子树 {101,103,104}:种子 2 行 + create_by=7 行 = 3 行
    let page = svc
        .page(&ctx, query_with_belong(101))
        .await
        .expect("分页查询失败");
    assert_eq!(page.total, 3, "101 子树三行(已删行被软删过滤挡住)");

    // belongDeptId=102 子树 {102}:仅范围种子那 1 行
    let page = svc
        .page(&ctx, query_with_belong(102))
        .await
        .expect("分页查询失败");
    assert_eq!(page.total, 1, "102 子树仅 1 行,101 的行不得混入");

    // 导出同口径(Java 里 page/export 共用 buildQueryWrapper)
    let list = svc
        .list_all(&ctx, query_with_belong(102))
        .await
        .expect("导出查询失败");
    assert_eq!(list.len(), 1, "导出与分页共用同一业务过滤");
}

/// DeptIn 范围 + belongDeptId 子树**正交叠加**(AND):交集之外的行全被挡。
#[tokio::test]
async fn belong_dept_id_intersects_with_row_scope() {
    let ds0 = setup_conn_seeded().await;
    let svc =
        system::post::service::post_service::PostService::new(&infra_over());
    let ds = Arc::new(ds0.derive_with_resolver(Arc::new(FixedResolver(
        ScopeFilter::DeptIn(vec![101]),
    ))));
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(101),
    });

    // 范围 [101] ∩ 子树 {101,103,104} = {101} → 3 行
    let page = svc
        .page(&ctx, query_with_belong(101))
        .await
        .expect("分页查询失败");
    assert_eq!(page.total, 3, "范围与业务搜索 AND 叠加,仍是 101 的三行");

    // 范围 [101] ∩ 子树 {102} = ∅ → 0 行(范围挡住搜索意图外的行)
    let page = svc
        .page(&ctx, query_with_belong(102))
        .await
        .expect("分页查询失败");
    assert_eq!(
        page.rows.len(),
        0,
        "搜索子树在数据范围外 → 空集,不得因业务过滤放行越权行"
    );
}

/// deptId 精确匹配优先(Java buildQueryWrapper 互斥分支):给了 deptId 时
/// belongDeptId 被忽略,不得同时施加两个部门条件。
#[tokio::test]
async fn dept_id_exact_takes_priority_over_belong_dept_id() {
    let ds = setup_conn_seeded().await;
    let svc =
        system::post::service::post_service::PostService::new(&infra_over());
    let ctx = ds.system_context();

    // deptId=102(1 行)优先;belongDeptId=101(子树 3 行)必须被忽略
    let mut q = query_with_belong(101);
    q.dept_id = Some(102);
    let page = svc.page(&ctx, q).await.expect("分页查询失败");
    assert_eq!(
        page.total, 1,
        "deptId 精确匹配优先,belongDeptId 的子树条件不得叠加"
    );
}
