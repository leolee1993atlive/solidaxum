mod common;

use std::{collections::HashSet, sync::Arc};

use app_data::{
    DataError, DataSource, Principal, ScopeFilter, sea_orm::ActiveValue::Set,
    testing::FixedResolver,
};
use common::setup_db;
use system::dept::repository::dept_repo::{
    self as dept_entity, DeptRepository,
};

/// 仓库 + 无 resolver 的 DataSource(system_context 语义,ScopeFilter 恒 All,
/// 与 post_repo_test 同一处置)。
fn repo_system(ds: DataSource) -> (Arc<DataSource>, DeptRepository) {
    let ds = Arc::new(ds);
    (ds.clone(), DeptRepository::new())
}

#[tokio::test]
async fn test_find_child_ids_tech_dept() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let ids = repo.find_child_ids(&ctx, 101).await.expect("查询失败");

    let expected: HashSet<i64> = [101, 103, 104].into_iter().collect();
    let actual: HashSet<i64> = ids.into_iter().collect();
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn test_find_child_ids_root() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let ids = repo.find_child_ids(&ctx, 100).await.expect("查询失败");

    let expected: HashSet<i64> =
        [100, 101, 102, 103, 104].into_iter().collect();
    let actual: HashSet<i64> = ids.into_iter().collect();
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn test_select_all_excludes_deleted() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // select_all 已并入 select_list(Java excludeChild
    // 走的就是 selectDeptList(new SysDeptBo()))
    let depts = repo
        .select_list(&ctx, &None, &None, &None)
        .await
        .expect("查询失败");

    assert_eq!(depts.len(), 5);
    for d in &depts {
        assert_eq!(d.del_flag.as_deref(), Some("0"));
    }
}

#[tokio::test]
async fn test_select_by_id() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let dept = repo
        .select_by_id(&ctx, &100)
        .await
        .expect("查询失败")
        .expect("部门不存在");

    assert_eq!(dept.dept_name, "总部");
    assert_eq!(dept.parent_id, 0);
}

#[tokio::test]
async fn test_find_descendant_ids() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let ids = repo.find_descendant_ids(&ctx, 101).await.expect("查询失败");

    assert!(ids.contains(&103), "应包含前端组 103");
    assert!(ids.contains(&104), "应包含后端组 104");
}

#[tokio::test]
async fn test_update_children_ancestors() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // 免盖章种子(insert_unstamped):审计列保持 NULL,同原裸 INSERT
    ctx.insert_unstamped(dept_entity::ActiveModel {
        dept_id: Set(1100),
        parent_id: Set(100),
        ancestors: Set(Some("0,100".into())),
        dept_name: Set("测试父部门".into()),
        order_num: Set(0),
        status: Set("0".into()),
        del_flag: Set("0".into()),
        ..Default::default()
    })
    .await
    .expect("插入父部门失败");
    ctx.insert_unstamped(dept_entity::ActiveModel {
        dept_id: Set(1101),
        parent_id: Set(1100),
        ancestors: Set(Some("0,100,1100".into())),
        dept_name: Set("测试子部门".into()),
        order_num: Set(0),
        status: Set("0".into()),
        del_flag: Set("0".into()),
        ..Default::default()
    })
    .await
    .expect("插入子部门失败");

    // 关联函数(无 &self 状态)
    DeptRepository::update_children_ancestors(
        &ctx,
        1100,
        "0,100",
        "0,100,9999",
    )
    .await
    .expect("更新祖先路径失败");

    let child = repo
        .select_by_id(&ctx, &1101)
        .await
        .expect("查询失败")
        .expect("子部门不存在");
    assert_eq!(child.ancestors.as_deref(), Some("0,100,9999,1100"));
}

// ═════════════ 范围/豁免/写路径判例 ═════════════

/// 仓库 + 固定 ScopeFilter 的 DataSource(范围用例,免搭业务角色语义,
/// 与 post_repo_test 同一处置)。
fn repo_scoped(
    ds: DataSource,
    sf: ScopeFilter,
) -> (Arc<DataSource>, DeptRepository) {
    let ds = Arc::new(ds.with_resolver(Arc::new(FixedResolver(sf))));
    (ds.clone(), DeptRepository::new())
}

/// 范围树(Java selectDeptList,SysDeptMapper.java:36-41 带 @DataPermission
/// dept_id):① DeptAndBelow(生产 resolver 已展开为 DeptIn([101,103,104]))
/// → select_list 只见 101 子树,102(销售部)不可见 —— user_handler
/// dept_tree 手工映射回收后由漏斗接管的就是这一语义;② 仅本部门
/// (DataScope=3,DeptIn([101]))→ 只见 101 本行。ScopeFilter 是**已解析**
/// 的部门集,子树展开在 resolver 期(与原手工映射 contains 同构)。
#[tokio::test]
async fn scoped_select_list_shows_own_subtree_only() {
    let (ds, repo) =
        repo_scoped(setup_db().await, ScopeFilter::DeptIn(vec![101, 103, 104]));
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    let depts = repo
        .select_list(&ctx, &None, &None, &None)
        .await
        .expect("查询失败");
    let ids: HashSet<i64> = depts.iter().map(|d| d.dept_id).collect();
    let expected: HashSet<i64> = [101, 103, 104].into_iter().collect();
    assert_eq!(ids, expected, "受限列表只见本部门子树");
}

/// 仅本部门(DataScope=3):DeptIn([101]) 不含子部门 —— 与原手工映射的
/// contains 行为同构(展开是 resolver 的职责,不在查询层)。
#[tokio::test]
async fn scoped_select_list_dept_only_scope_excludes_children() {
    let (ds, repo) =
        repo_scoped(setup_db().await, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 303,
        dept_id: Some(101),
    });

    let depts = repo
        .select_list(&ctx, &None, &None, &None)
        .await
        .expect("查询失败");
    let ids: HashSet<i64> = depts.iter().map(|d| d.dept_id).collect();
    let expected: HashSet<i64> = [101].into_iter().collect();
    assert_eq!(ids, expected, "仅本部门范围:子部门 103/104 不可见");
}

/// 豁免判例:select_by_id(Java selectDeptById → selectVoById,
/// SysDeptServiceImpl:163,无 @DataPermission)→ 越权部门照常可见。
#[tokio::test]
async fn exempt_select_by_id_sees_foreign_dept() {
    let (ds, repo) =
        repo_scoped(setup_db().await, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    let foreign = repo
        .select_by_id(&ctx, &102)
        .await
        .expect("查询失败")
        .expect("豁免方法:越权部门 102 应可见");
    assert_eq!(foreign.dept_name, "销售部");
}

/// 豁免判例:find_descendant_ids(Java selectDeptAndChildById →
/// selectListByParentId,Mapper:76-94,无 @DataPermission)→ 子树展开
/// 不受范围收窄 —— 它是业务搜索/守卫语义(belongDeptId、循环检测),
/// 不是权限。
#[tokio::test]
async fn exempt_find_descendant_ids_sees_full_tree() {
    let (ds, repo) =
        repo_scoped(setup_db().await, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    let ids = repo
        .find_descendant_ids(&ctx, 100)
        .await
        .expect("查询失败")
        .into_iter()
        .collect::<HashSet<_>>();
    let expected: HashSet<i64> =
        [100, 101, 102, 103, 104].into_iter().collect();
    assert_eq!(ids, expected, "豁免方法:受限 ctx 下子树展开仍全量");
}

/// 写路径不施行级范围时 update_by_id 的语义:
/// 受限 DeptIn([101]) 下 ① 越权改 102 **成功**且落库(WHERE 只剩
/// 主键 + 软删存活,越权拦截由 check_dept_data_scope 探针承担,成对
/// 判例见 write_scope_rollback_test);② 软删行改 104 → NotFound
/// (存活过滤仍在 WHERE);③ 本部门改 101 照常成功。
#[tokio::test]
async fn update_by_id_after_rollback_foreign_succeeds_deleted_not_found() {
    let (ds, repo) =
        repo_scoped(setup_db().await, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    // ① 越权:WHERE 不含行级范围,改得动且落库
    let mut foreign = repo
        .select_by_id(&ctx, &102)
        .await
        .expect("查询失败")
        .expect("102 应存在");
    foreign.dept_name = "越权改名".to_string();
    repo.update_by_id(&ctx, &foreign, &102)
        .await
        .expect("回退后越权目标的 update 不应再被行级范围拦住");
    let row = repo
        .select_by_id(&ds.system_context(), &102)
        .await
        .expect("复核查询失败")
        .expect("102 应存在");
    assert_eq!(row.dept_name, "越权改名", "回退后越权 update 应落库");

    // ② 软删行:104 置 del_flag=1 后 update → NotFound
    let mut soft_deleted = repo
        .select_by_id(&ctx, &104)
        .await
        .expect("查询失败")
        .expect("104 应存在");
    soft_deleted.dept_name = "软删行改名".to_string();
    // 直改 del_flag 走 system_context 的漏斗 update(范围 All、行存活,
    // 与原裸 UPDATE 同语义;审计 update_* 被盖 0/now,无害)
    ds.system_context()
        .update(dept_entity::ActiveModel {
            dept_id: Set(104),
            del_flag: Set("1".into()),
            ..Default::default()
        })
        .await
        .expect("软删失败");
    let err = repo.update_by_id(&ctx, &soft_deleted, &104).await;
    assert!(
        matches!(err, Err(DataError::NotFound)),
        "软删行 update 应 NotFound,实际: {err:?}"
    );

    // ③ 本部门照常改
    let mut own = repo
        .select_by_id(&ctx, &101)
        .await
        .expect("查询失败")
        .expect("101 应存在");
    own.dept_name = "技术部改名".to_string();
    repo.update_by_id(&ctx, &own, &101)
        .await
        .expect("本部门 update 应成功");
    let after = repo
        .select_by_id(&ctx, &101)
        .await
        .expect("查询失败")
        .expect("101 应存在");
    assert_eq!(after.dept_name, "技术部改名");
}
