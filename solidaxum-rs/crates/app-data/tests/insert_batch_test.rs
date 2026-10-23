//! 写入原语判例:不回读的单条插入 + 批量插入。
use app_data::{
    sea_orm::ActiveValue::Set,
    testing::{sandbox, sqlite_memory},
};

fn row(id: i64, dept: i64) -> sandbox::ActiveModel {
    sandbox::ActiveModel {
        id: Set(id),
        dept_id: Set(Some(dept)),
        owner_id: Set(Some(1)),
        del_flag: Set("0".to_string()),
        ..Default::default()
    }
}

#[tokio::test]
async fn insert_returning_nothing_persists_and_stamps_audit() {
    let ds = sqlite_memory().await;
    let ctx = ds.system_context();

    ctx.insert_returning_nothing(row(1, 10)).await.unwrap();

    let got = ctx
        .find_by_id::<sandbox::Entity>(1)
        .await
        .expect("插入的行应可读回");
    assert_eq!(got.dept_id, Some(10));
    assert!(got.create_time.is_some(), "审计盖章与 insert 同口径");
}

#[tokio::test]
async fn insert_many_persists_all_rows() {
    let ds = sqlite_memory().await;
    let ctx = ds.system_context();

    let n = ctx
        .insert_many(vec![row(1, 10), row(2, 10), row(3, 20)])
        .await
        .unwrap();
    assert_eq!(n, 3, "返回落库行数");

    for id in [1, 2, 3] {
        ctx.find_by_id::<sandbox::Entity>(id)
            .await
            .unwrap_or_else(|e| panic!("{id} 应已落库: {e:?}"));
    }
}

#[tokio::test]
async fn insert_many_on_empty_input_is_noop() {
    let ds = sqlite_memory().await;
    let n = ds
        .system_context()
        .insert_many(Vec::<sandbox::ActiveModel>::new())
        .await
        .unwrap();
    assert_eq!(n, 0, "空集合零语句、零错误");
}

#[tokio::test]
async fn insert_many_maps_unique_conflict() {
    use app_data::DataError;

    let ds = sqlite_memory().await;
    let ctx = ds.system_context();
    ctx.insert_returning_nothing(row(1, 10)).await.unwrap();

    let err = ctx.insert_many(vec![row(1, 10)]).await.unwrap_err();
    assert!(
        matches!(err, DataError::Conflict),
        "唯一键冲突分类与 insert 同口径,实际: {err:?}"
    );
}
