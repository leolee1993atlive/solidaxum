//! `SelectQuery::count` 判例:与 page().total 同口径,但只发一条 COUNT。
use app_data::{
    sea_orm::{ColumnTrait, EntityTrait, QueryFilter},
    testing::{sandbox, seed, sqlite_memory},
};

#[tokio::test]
async fn count_matches_page_total_and_applies_soft_delete() {
    let ds = sqlite_memory().await;
    // (id, dept, owner, del_flag)
    seed(&ds, vec![(1, 10, 1, "0"), (2, 10, 2, "0"), (3, 10, 3, "1")]).await;
    let ctx = ds.system_context();

    let n = ctx
        .select(sandbox::Entity::find())
        .without_row_scope()
        .count()
        .await
        .unwrap();
    assert_eq!(n, 2, "软删行不计入(del_flag='1' 的 3 号被过滤)");

    let p = ctx
        .select(sandbox::Entity::find())
        .without_row_scope()
        .page(1, 10)
        .await
        .unwrap();
    assert_eq!(n as i64, p.total, "count 必须与 page().total 同口径");
}

#[tokio::test]
async fn count_applies_row_scope() {
    use std::sync::Arc;

    use app_data::{Principal, ScopeFilter, testing::FixedResolver};

    let ds = sqlite_memory()
        .await
        .with_resolver(Arc::new(FixedResolver(ScopeFilter::DeptIn(vec![10]))));
    seed(&ds, vec![(1, 10, 1, "0"), (2, 99, 2, "0")]).await;
    let ctx = ds.context(Principal {
        user_id: 1,
        dept_id: Some(10),
    });

    let n = ctx.select(sandbox::Entity::find()).count().await.unwrap();
    assert_eq!(n, 1, "越权部门的行不计入");
}

#[tokio::test]
async fn count_respects_caller_filters() {
    let ds = sqlite_memory().await;
    seed(&ds, vec![(1, 10, 1, "0"), (2, 20, 2, "0")]).await;
    let ctx = ds.system_context();

    let n = ctx
        .select(sandbox::Entity::find().filter(sandbox::Column::DeptId.eq(20)))
        .without_row_scope()
        .count()
        .await
        .unwrap();
    assert_eq!(n, 1);
}
