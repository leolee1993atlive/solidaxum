//! gen_table 仓储集成测试(全豁免)。
//!
//! 覆盖:
//! 1. **全豁免判例**:resolver 给出 DeptIn 时,gen_table 读照见全部行
//!    —— GenTableMapper 类级 @InterceptorIgnore(dataPermission)(Java
//!    侧显式关闭数据权限拦截),代码生成元数据非行级;
//! 2. 写路径语义:insert 审计五列由漏斗按 principal 盖章、
//!    update 0 行 → NotFound(替代 RecordNotUpdated 500 语义)。

mod common;

use std::sync::Arc;

use app_core::{model::entity::BaseEntity, vo::page::PageQuery};
use app_data::{
    DataError, DataSource, Principal, ScopeFilter, testing::FixedResolver,
};
use chrono::{Duration, Utc};
use common::setup_db;
use system::codegen::{
    model::gen_table::{GenTable, GenTableQueryBo},
    repository::gen_table_repo::GenTableRepository,
};

fn sample_table(table_id: i64, table_name: &str, comment: &str) -> GenTable {
    GenTable {
        table_id,
        data_name: "master".to_string(),
        table_name: table_name.to_string(),
        table_comment: comment.to_string(),
        class_name: "SysUser".to_string(),
        tpl_category: "crud".to_string(),
        frontend_type: "svelte".to_string(),
        package_name: "crates.system".to_string(),
        module_name: "system".to_string(),
        business_name: "user".to_string(),
        function_name: "用户".to_string(),
        function_author: "RuoYi".to_string(),
        gen_type: "0".to_string(),
        gen_path: "/".to_string(),
        options: None,
        remark: None,
        pk_column: None,
        columns: None,
        tree_code: None,
        tree_parent_code: None,
        tree_name: None,
        menu_ids: None,
        parent_menu_id: None,
        parent_menu_name: None,
        enable_export: None,
        enable_status: None,
        status_field: None,
        enable_unique: None,
        unique_fields: None,
        enable_sort: None,
        sort_field: None,
        tree_root_value: None,
        tree_ancestors_field: None,
        tree_order_field: None,
        base_entity: BaseEntity {
            create_dept: None,
            create_by: None,
            create_time: None,
            update_by: None,
            update_time: None,
        },
    }
}

fn empty_query() -> GenTableQueryBo {
    GenTableQueryBo {
        table_name: None,
        table_comment: None,
        data_name: None,
        begin_time: None,
        end_time: None,
        page: PageQuery {
            page_num: Some(1),
            page_size: Some(10),
            order_by_column: None,
            is_asc: None,
        },
    }
}

fn repo_system(ds: DataSource) -> (Arc<DataSource>, GenTableRepository) {
    (Arc::new(ds), GenTableRepository::new())
}

fn repo_scoped(
    ds: DataSource,
    sf: ScopeFilter,
) -> (Arc<DataSource>, GenTableRepository) {
    (
        Arc::new(ds.with_resolver(Arc::new(FixedResolver(sf)))),
        GenTableRepository::new(),
    )
}

/// **gen_table 全豁免判例**:DeptIn([101]) 下,越权部门
/// (create_dept=102)的元数据行对按主键读 / 列表读 / 分页读 /
/// 已导入表名读全部照常可见 —— 代码生成元数据非行级(GenTableMapper
/// 类级 @InterceptorIgnore(dataPermission),Java 侧显式关闭数据权限
/// 拦截);漏标 without_row_scope 会 fail-closed 报
/// Unsupported,本用例变红。
#[tokio::test]
async fn exempt_gen_table_reads_see_all_rows_despite_scope() {
    let (ds, repo) =
        repo_scoped(setup_db().await, ScopeFilter::DeptIn(vec![101]));

    // 种子:create_dept 由漏斗按 principal 部门盖章(101 本部门 / 102 越权)
    let ctx_own = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });
    let ctx_other = ds.context(Principal {
        user_id: 302,
        dept_id: Some(102),
    });
    repo.insert(&ctx_own, &sample_table(9201, "sys_exempt_a", "甲表"))
        .await
        .unwrap();
    repo.insert(&ctx_other, &sample_table(9202, "sys_exempt_b", "乙表"))
        .await
        .unwrap();

    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    assert!(
        repo.select_by_id(&ctx, 9202).await.unwrap().is_some(),
        "越权部门的元数据行对按主键读照常可见"
    );
    let list = repo.select_list(&ctx, None, None, None).await.unwrap();
    assert_eq!(list.len(), 2, "列表读不得被范围砍行");

    let (_rows, total) =
        repo.select_page(&ctx, &empty_query(), 1, 10).await.unwrap();
    assert_eq!(total, 2, "分页 total 须是全量计数(全豁免)");

    let names = repo
        .select_imported_table_names(&ctx, "master")
        .await
        .unwrap();
    assert_eq!(names.len(), 2, "已导入表名读(导入排重)须全量");
}

/// **gen_table 写路径**:insert 审计五列由漏斗按 principal 盖章
/// (原空 base_entity 静默写 NULL 的替代)。
#[tokio::test]
async fn gen_table_insert_stamps_audit_from_principal() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.context(Principal {
        user_id: 42,
        dept_id: Some(101),
    });

    repo.insert(&ctx, &sample_table(9203, "sys_stamp", "盖章表"))
        .await
        .unwrap();
    let loaded = repo
        .select_by_id(&ctx, 9203)
        .await
        .unwrap()
        .expect("插入后应可读回");
    assert_eq!(
        loaded.base_entity.create_by,
        Some(42),
        "create_by = principal(漏斗盖章)"
    );
    assert_eq!(
        loaded.base_entity.create_dept,
        Some(101),
        "create_dept = principal 部门(漏斗盖章)"
    );
    assert!(
        loaded.base_entity.create_time.is_some(),
        "create_time 由漏斗盖"
    );
}

/// **gen_table 更新 0 行 → NotFound**(原 RecordNotUpdated 冒 500;
/// 漏斗 update 的 0 行语义,notice/oss 删除路径同款裁定)。
#[tokio::test]
async fn gen_table_update_missing_is_not_found() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let err = repo
        .update_by_id(&ctx, &sample_table(9999, "sys_ghost", "不存在"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, DataError::NotFound),
        "更新不存在的行必须 NotFound,实际: {err:?}"
    );
}

#[tokio::test]
async fn test_insert_and_select() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let table = sample_table(9001, "sys_test", "测试表");
    repo.insert(&ctx, &table).await.unwrap();

    let found = repo.select_by_id(&ctx, 9001).await.unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.table_name, "sys_test");
    assert_eq!(found.table_comment, "测试表");
}

#[tokio::test]
async fn test_select_list_with_filters() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    repo.insert(&ctx, &sample_table(9002, "sys_user", "用户表"))
        .await
        .unwrap();
    repo.insert(&ctx, &sample_table(9003, "sys_role", "角色表"))
        .await
        .unwrap();

    let list = repo
        .select_list(&ctx, Some("user"), None, None)
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].table_name, "sys_user");

    let list = repo
        .select_list(&ctx, None, Some("角色"), None)
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].table_name, "sys_role");
}

#[tokio::test]
async fn test_update() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let mut table = sample_table(9004, "sys_config", "配置表");
    repo.insert(&ctx, &table).await.unwrap();

    table.table_comment = "更新后".to_string();
    table.module_name = "tool".to_string();
    repo.update_by_id(&ctx, &table).await.unwrap();

    let found = repo.select_by_id(&ctx, 9004).await.unwrap().unwrap();
    assert_eq!(found.table_comment, "更新后");
    assert_eq!(found.module_name, "tool");
}

#[tokio::test]
async fn test_delete_by_ids() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    repo.insert(&ctx, &sample_table(9005, "sys_log", "日志表"))
        .await
        .unwrap();
    repo.insert(&ctx, &sample_table(9006, "sys_notice", "通知表"))
        .await
        .unwrap();

    repo.delete_by_ids(&ctx, &[9005, 9006]).await.unwrap();

    assert!(repo.select_by_id(&ctx, 9005).await.unwrap().is_none());
    assert!(repo.select_by_id(&ctx, 9006).await.unwrap().is_none());
}

#[tokio::test]
async fn test_select_page_filters_by_create_time() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    // create_time 由漏斗 insert 盖章(≈ now),不能再由测试侧预设 ——
    // 时间窗过滤用「含 now 的窗」与「纯过去的窗」两个口径钉过滤逻辑。
    repo.insert(&ctx, &sample_table(9007, "sys_t1", "t1"))
        .await
        .unwrap();
    repo.insert(&ctx, &sample_table(9008, "sys_t2", "t2"))
        .await
        .unwrap();

    let all = repo.select_page(&ctx, &empty_query(), 1, 10).await.unwrap();
    assert_eq!(all.1, 2, "全量应返回 2 条");

    let now = Utc::now();
    let fmt = "%Y-%m-%d %H:%M:%S";
    let mut in_window = empty_query();
    in_window.begin_time =
        Some((now - Duration::hours(1)).format(fmt).to_string());
    in_window.end_time =
        Some((now + Duration::hours(1)).format(fmt).to_string());
    let filtered = repo.select_page(&ctx, &in_window, 1, 10).await.unwrap();
    assert_eq!(filtered.1, 2, "含 now 的时间窗应命中全部 2 条");

    let mut past_window = empty_query();
    past_window.begin_time =
        Some((now - Duration::days(2)).format(fmt).to_string());
    past_window.end_time =
        Some((now - Duration::days(1)).format(fmt).to_string());
    let filtered = repo.select_page(&ctx, &past_window, 1, 10).await.unwrap();
    assert_eq!(filtered.1, 0, "纯过去的时间窗应命中 0 条");
}
