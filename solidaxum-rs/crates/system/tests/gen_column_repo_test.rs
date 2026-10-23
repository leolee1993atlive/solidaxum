//! gen_table_column 仓储集成测试(全豁免)。
//!
//! 覆盖:**全豁免判例**(受限 ctx 照见全部行,gen_table 同锚)+
//! 批量写路径(insert_batch 逐行 ctx.insert / update_batch 逐行
//! ctx.update / delete_by_table_ids → delete_where)。

mod common;

use std::sync::Arc;

use app_core::model::entity::BaseEntity;
use app_data::{DataSource, Principal, ScopeFilter, testing::FixedResolver};
use common::setup_db;
use system::codegen::{
    model::gen_column::GenTableColumn,
    repository::gen_column_repo::GenColumnRepository,
};

fn sample_column(
    column_id: i64,
    table_id: i64,
    name: &str,
    sort: i32,
) -> GenTableColumn {
    GenTableColumn {
        column_id,
        table_id,
        column_name: name.to_string(),
        column_comment: Some(format!("{}注释", name)),
        column_type: "varchar(64)".to_string(),
        java_type: "String".to_string(),
        java_field: name.to_string(),
        is_pk: "0".to_string(),
        is_increment: "0".to_string(),
        is_required: "0".to_string(),
        is_insert: "1".to_string(),
        is_edit: "1".to_string(),
        is_list: "1".to_string(),
        is_query: "1".to_string(),
        query_type: "EQ".to_string(),
        html_type: "input".to_string(),
        dict_type: None,
        sort,
        cap_java_field: None,
        super_column: None,
        usable_column: None,
        base_entity: BaseEntity {
            create_dept: None,
            create_by: None,
            create_time: None,
            update_by: None,
            update_time: None,
        },
    }
}

fn repo_system(ds: DataSource) -> (Arc<DataSource>, GenColumnRepository) {
    (Arc::new(ds), GenColumnRepository::new())
}

/// **gen_table_column 全豁免判例**:DeptIn([101]) 下,越权部门
/// (create_dept=102)的列元数据对按表读照常可见 —— 与 gen_table
/// 同锚(GenTableMapper 类级 @InterceptorIgnore(dataPermission)
/// —— 代码生成元数据非行级)。
#[tokio::test]
async fn exempt_gen_column_reads_see_all_rows_despite_scope() {
    let ds = Arc::new(setup_db().await.with_resolver(Arc::new(FixedResolver(
        ScopeFilter::DeptIn(vec![101]),
    ))));
    let repo = GenColumnRepository::new();

    // 种子:create_dept 由漏斗按 principal 部门盖章(101 本部门 / 102 越权)
    let ctx_own = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });
    let ctx_other = ds.context(Principal {
        user_id: 302,
        dept_id: Some(102),
    });
    repo.insert_batch(&ctx_own, &[sample_column(19301, 9301, "own_col", 1)])
        .await
        .unwrap();
    repo.insert_batch(
        &ctx_other,
        &[sample_column(19302, 9301, "other_col", 2)],
    )
    .await
    .unwrap();

    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });
    let cols = repo.select_by_table_id(&ctx, 9301).await.unwrap();
    assert_eq!(cols.len(), 2, "按表读不得被范围砍行(全豁免)");
    assert_eq!(cols[0].column_name, "own_col", "sort 升序保持");
    assert_eq!(cols[1].column_name, "other_col");
}

#[tokio::test]
async fn test_insert_batch_and_select_by_table_id() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let columns = vec![
        sample_column(10001, 8001, "user_name", 1),
        sample_column(10002, 8001, "nick_name", 2),
    ];
    repo.insert_batch(&ctx, &columns).await.unwrap();

    let found = repo.select_by_table_id(&ctx, 8001).await.unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].column_name, "user_name");
    assert_eq!(found[1].column_name, "nick_name");
}

#[tokio::test]
async fn test_update_batch() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    let mut col = sample_column(10003, 8002, "status", 1);
    repo.insert_batch(&ctx, &[col.clone()]).await.unwrap();

    col.html_type = "switch".to_string();
    col.query_type = "BETWEEN".to_string();
    repo.update_batch(&ctx, &[col]).await.unwrap();

    let found = repo.select_by_table_id(&ctx, 8002).await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].html_type, "switch");
    assert_eq!(found[0].query_type, "BETWEEN");
}

#[tokio::test]
async fn test_delete_by_table_ids() {
    let (ds, repo) = repo_system(setup_db().await);
    let ctx = ds.system_context();

    repo.insert_batch(
        &ctx,
        &[
            sample_column(10004, 8003, "a", 1),
            sample_column(10005, 8003, "b", 2),
        ],
    )
    .await
    .unwrap();

    repo.delete_by_table_ids(&ctx, &[8003]).await.unwrap();

    let found = repo.select_by_table_id(&ctx, 8003).await.unwrap();
    assert!(found.is_empty());
}
