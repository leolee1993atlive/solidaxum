//! menu 仓储集成测试(全豁免)。
//!
//! 覆盖三组语义:
//! 1. 基础读/关联表路径(经 `system_context` 桥接);
//! 2. **全豁免判例**:即便 resolver 给出 DeptIn,
//!    所有读方法照见全部行 —— SysMenuMapper 无 @DataPermission,
//!    权限树数据非行级(漏判的反向钉死);
//! 3. **写路径**:insert/update/delete 经漏斗
//!    原语,审计五列由 principal 盖章,0 行删除 → NotFound。

mod common;

use std::{collections::HashSet, sync::Arc};

use app_core::model::entity::BaseEntity;
use app_data::{
    DataError, DataSource, Principal, ScopeFilter, sea_orm::ActiveValue::Set,
    testing::FixedResolver,
};
use common::setup_db;
use system::menu::{
    model::menu::Menu,
    repository::menu_repo::{self as menu_entity, MenuRepository},
};

/// 仓库 + 无 resolver 的 DataSource(system_context 语义,ScopeFilter 恒 All)。
fn repo_system(ds: DataSource) -> (Arc<DataSource>, MenuRepository) {
    (Arc::new(ds), MenuRepository::new())
}

/// 仓库 + 固定 ScopeFilter 的 DataSource(范围用例,免搭业务角色语义)。
fn repo_scoped(
    ds: DataSource,
    sf: ScopeFilter,
) -> (Arc<DataSource>, MenuRepository) {
    (
        Arc::new(ds.with_resolver(Arc::new(FixedResolver(sf)))),
        MenuRepository::new(),
    )
}

/// 范围用例的补充种子:601(create_dept=101)与 602(create_dept=102)
/// —— 若菜单被误施加行级范围,DeptIn([101]) 下 602/NULL 行会消失。
/// 免盖章种子:显式审计值原样落库(insert_unstamped)。
async fn seed_scope_menus(ds: &DataSource) {
    let ctx = ds.system_context();
    for (id, name, order, path, perms, create_dept, create_by) in [
        (601, "范围部门菜单", 10, "scoped-a", "system:a:list", 101, 7),
        (602, "越权部门菜单", 11, "scoped-b", "system:b:list", 102, 8),
    ] {
        ctx.insert_unstamped(menu_entity::ActiveModel {
            menu_id: Set(id),
            menu_name: Set(name.into()),
            parent_id: Set(0),
            order_num: Set(order),
            path: Set(path.into()),
            is_frame: Set("1".into()),
            is_cache: Set("0".into()),
            menu_type: Set("C".into()),
            visible: Set("0".into()),
            status: Set("0".into()),
            perms: Set(Some(perms.into())),
            create_dept: Set(Some(create_dept)),
            create_by: Set(Some(create_by)),
            ..Default::default()
        })
        .await
        .expect("种范围菜单行失败");
    }
}

fn mk_menu(id: i64, name: &str) -> Menu {
    Menu {
        menu_id: id,
        parent_id: 0,
        menu_name: name.to_string(),
        order_num: 0,
        path: format!("menu-{id}"),
        component: None,
        query_param: None,
        is_frame: "1".to_string(),
        is_cache: "0".to_string(),
        menu_type: "C".to_string(),
        visible: "0".to_string(),
        status: "0".to_string(),
        perms: None,
        icon: None,
        active_menu: None,
        ext: None,
        remark: None,
        base_entity: BaseEntity::new(0),
    }
}

#[tokio::test]
async fn test_select_menu_tree_all() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let menus = repo.select_menu_tree_all(&ctx).await.expect("查询失败");

    let menu_ids: HashSet<i64> = menus.iter().map(|m| m.menu_id).collect();
    let expected: HashSet<i64> = [501, 502, 504].into_iter().collect();
    assert_eq!(menu_ids, expected, "应只含 M/C 类型且 status=0 的菜单");
}

#[tokio::test]
async fn test_select_menu_tree_by_user_id() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let menus = repo
        .select_menu_tree_by_user_id(&ctx, 302)
        .await
        .expect("查询失败");

    let menu_ids: HashSet<i64> = menus.iter().map(|m| m.menu_id).collect();
    let expected: HashSet<i64> = [501, 502, 504].into_iter().collect();
    assert_eq!(
        menu_ids, expected,
        "u_custom 应通过 role_menu 关联到 501,502,504"
    );
}

#[tokio::test]
async fn test_select_menu_perms_by_user_id() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let perms = repo
        .select_menu_perms_by_user_id(&ctx, 302)
        .await
        .expect("查询失败");

    let perm_set: HashSet<&str> = perms.iter().map(|s| s.as_str()).collect();
    assert!(
        perm_set.contains("system:user:list"),
        "应包含 system:user:list, got: {:?}",
        perms
    );
    assert!(
        perm_set.contains("system:role:list"),
        "应包含 system:role:list, got: {:?}",
        perms
    );
}

#[tokio::test]
async fn test_select_menu_perms_by_user_id_no_role() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let perms = repo
        .select_menu_perms_by_user_id(&ctx, 307)
        .await
        .expect("查询失败");

    assert!(perms.is_empty(), "无角色用户应无权限标识");
}

// ─── 关联表 sys_role_menu 路径(menu_repo 审计表对应行)───
// 「原生 SQL 归零」等价性护栏:断言结构化 API 与原手写 SQL 行为一致。

/// 原 `SELECT COUNT(*) FROM sys_role_menu WHERE menu_id = ?`
#[tokio::test]
async fn test_count_role_menu_by_menu_id() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // seed: role_menu (201→501),(201→502),(201→504) → menu 501/502/504 各 1 条
    assert_eq!(
        repo.count_role_menu(&ctx, 501).await.unwrap(),
        1,
        "menu 501 应被 1 个角色关联"
    );
    assert_eq!(
        repo.count_role_menu(&ctx, 502).await.unwrap(),
        1,
        "menu 502 应被 1 个角色关联"
    );
    // 不存在的 menu_id → 0
    assert_eq!(
        repo.count_role_menu(&ctx, 9999).await.unwrap(),
        0,
        "不存在的 menu_id 应返回 0"
    );
}

/// 原 `DELETE FROM sys_role_menu WHERE menu_id IN ({})`。
/// 生产调用点在 cascade 事务闭包内,收 TxnContext —— 测试经
/// `ctx.transaction` 同一入口驱动。
#[tokio::test]
async fn test_delete_role_menus_by_menu_ids() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // 删除 menu 501, 502 的全部角色关联(seed 各 1 条)
    ctx.transaction(|tx| {
        Box::pin(async move {
            MenuRepository::delete_role_menus_by_menu_ids(tx, &[501, 502]).await
        })
    })
    .await
    .expect("删除失败");

    assert_eq!(
        repo.count_role_menu(&ctx, 501).await.unwrap(),
        0,
        "删除后 menu 501 应无关联"
    );
    assert_eq!(
        repo.count_role_menu(&ctx, 502).await.unwrap(),
        0,
        "删除后 menu 502 应无关联"
    );
    // 未在删除清单中的 menu 504 应保持
    assert_eq!(
        repo.count_role_menu(&ctx, 504).await.unwrap(),
        1,
        "menu 504 不应被删除"
    );

    // 空集合早退:不应报错,也不应影响数据
    ctx.transaction(|tx| {
        Box::pin(async move {
            MenuRepository::delete_role_menus_by_menu_ids(tx, &[]).await
        })
    })
    .await
    .expect("空集合应早退不报错");
}

/// 原 `SELECT menu_id FROM sys_role_menu WHERE role_id = ?`
#[tokio::test]
async fn test_select_menu_ids_by_role_id() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // seed: role 201 → menu 501/502/504
    let mut ids = repo.select_menu_ids_by_role_id(&ctx, 201).await.unwrap();
    ids.sort_unstable();
    assert_eq!(ids, vec![501, 502, 504], "role 201 应关联 501/502/504");

    // 无关联的角色 → 空
    let ids = repo.select_menu_ids_by_role_id(&ctx, 200).await.unwrap();
    assert!(ids.is_empty(), "role 200 无菜单关联应返回空");
}

// ─── 全豁免判例 + 写路径 ───

/// **全豁免判例**:resolver 给出 DeptIn([101]) 时,菜单读必须照见全部行
/// (含 create_dept=102 的越权行与 create_dept=NULL 的种子行)——
/// SysMenuMapper 无 @DataPermission,权限树数据非行级;若任何方法漏标
/// `without_row_scope`,CONSTRAINED=false 的驱动会 fail-closed 报
/// Unsupported,或(误加范围列时)静默砍行,两者都会在此变红。
#[tokio::test]
async fn exempt_menu_reads_see_all_rows_despite_scope() {
    let ds = setup_db().await;
    seed_scope_menus(&ds).await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    // select_all:种子 5 行 + 补充 2 行(601 同部门 / 602 越权部门)全见
    let menus = repo.select_all(&ctx).await.unwrap();
    let ids: HashSet<i64> = menus.iter().map(|m| m.menu_id).collect();
    assert_eq!(
        ids,
        [501, 502, 503, 504, 505, 601, 602].into_iter().collect(),
        "菜单树读不得被部门范围收窄(全豁免)"
    );

    // select_list 同口径;select_by_id 越权行照见
    let menus = repo.select_list(&ctx, &None, &None).await.unwrap();
    assert_eq!(menus.len(), 7, "列表读同豁免");
    assert!(
        repo.select_by_id(&ctx, 602).await.unwrap().is_some(),
        "越权部门的菜单行对详情读照常可见"
    );

    // 守卫类读:has_child(501 有子 502/503/504/505)与 count_role_menu 照常
    assert!(
        repo.has_child(&ctx, 501).await.unwrap(),
        "子菜单守卫须真实计数"
    );
    assert_eq!(
        repo.count_role_menu(&ctx, 501).await.unwrap(),
        1,
        "分配守卫须真实计数"
    );

    // 唯一性:exists_by_parent_and_name 须跨部门全量(602 名字在越权部门)
    assert!(
        repo.exists_by_parent_and_name(&ctx, 0, "越权部门菜单", None)
            .await
            .unwrap(),
        "唯一性校验不得被范围砍小"
    );

    // 菜单树(perms/tree)路径同豁免:超管树与用户树均不受影响
    // (601/602 也是 M/C 且 status=0 → 入树;树读不含 F 型 503 与停用 505)
    let tree = repo.select_menu_tree_all(&ctx).await.unwrap();
    let tree_ids: HashSet<i64> = tree.iter().map(|m| m.menu_id).collect();
    assert_eq!(
        tree_ids,
        [501, 502, 504, 601, 602].into_iter().collect(),
        "树读同豁免,不得被部门范围收窄"
    );
}

/// **写路径换轨**:insert 审计五列由漏斗按 principal 盖章(领域层传入的
/// 0 被覆盖);update 盖 update_*;delete 0 行 → NotFound。
#[tokio::test]
async fn insert_update_delete_roundtrip_stamps_audit() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: Some(101),
    });

    // insert:领域层 base_entity=BaseEntity::new(0) 的审计值被漏斗覆盖
    let menu = mk_menu(9001, "写路径菜单");
    let id = repo.insert(&ctx, &menu).await.expect("插入失败");
    assert_eq!(id, 9001);
    let loaded = repo
        .select_by_id(&ctx, 9001)
        .await
        .unwrap()
        .expect("插入后应可读回");
    assert_eq!(
        loaded.base_entity.create_by,
        Some(7),
        "create_by = principal"
    );
    assert_eq!(
        loaded.base_entity.create_dept,
        Some(101),
        "create_dept = principal 部门"
    );
    assert!(
        loaded.base_entity.create_time.is_some(),
        "create_time 漏斗盖"
    );

    // update:字段生效 + update_* 盖章(同一 principal)
    let mut edited = loaded;
    edited.menu_name = "写路径菜单-改".to_string();
    repo.update_by_id(&ctx, &edited).await.expect("更新失败");
    let reloaded = repo
        .select_by_id(&ctx, 9001)
        .await
        .unwrap()
        .expect("更新后应可读回");
    assert_eq!(reloaded.menu_name, "写路径菜单-改");
    assert_eq!(
        reloaded.base_entity.update_by,
        Some(7),
        "update_by = principal"
    );
    assert!(
        reloaded.base_entity.update_time.is_some(),
        "update_time 漏斗盖"
    );

    // delete:成功后再删 → NotFound(0 行语义)
    repo.delete_by_id(&ctx, 9001).await.expect("删除失败");
    let err = repo.delete_by_id(&ctx, 9001).await.unwrap_err();
    assert!(
        matches!(err, DataError::NotFound),
        "重复删除应 NotFound,实际: {err:?}"
    );
    assert!(
        repo.select_by_id(&ctx, 9001).await.unwrap().is_none(),
        "删除后不可再读"
    );
}
