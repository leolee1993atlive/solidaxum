//! category_service 删除判例:0 行语义对齐 Java 静默 ——
//! `FlwCategoryServiceImpl.java:283`(deleteWithValidByIds)对不存在/已删
//! id 静默返回,两族(delete/spel)口径由此统一为 Java 静默。判例:
//! ① 删不存在的 id → Ok 静默;
//! ② 已删行不重复计数、不被二次盖写(`update_where` 的软删存活过滤);
//! ③ `has_children` 守卫保留(移植期既有行为)。
use app_core::{
    middleware::extractors::CurrentUser, model::login_user::LoginUser,
};
use app_data::{
    DataSource,
    sea_orm::{ActiveValue::Set, EntityTrait},
    testing::sqlite_memory_empty,
};
use flow::{
    repository::entity::flow_category,
    service::category_service::CategoryService,
};

fn test_user(user_id: i64) -> CurrentUser {
    CurrentUser(LoginUser {
        user_id,
        dept_id: Some(100),
        username: "test".to_string(),
        nick_name: Some("测试".to_string()),
        user_type: Some("sys_user".to_string()),
        client_id: "client".to_string(),
        client_key: None,
        device_type: None,
        permissions: std::collections::HashSet::new(),
        roles: std::collections::HashSet::new(),
        login_ip: None,
        login_date: None,
    })
}

async fn ds_seeded(rows: &[(i64, i64, &str)]) -> DataSource {
    let ds = sqlite_memory_empty().await;
    ds.create_table(flow_category::Entity)
        .await
        .expect("建 flow_category 表");
    let ctx = ds.system_context();
    for (id, parent_id, del_flag) in rows {
        ctx.insert(flow_category::ActiveModel {
            category_id: Set(*id),
            parent_id: Set(*parent_id),
            ancestors: Set(Some("0".to_string())),
            category_name: Set(format!("cat{id}")),
            order_num: Set(0),
            del_flag: Set(Some((*del_flag).to_string())),
            ..Default::default()
        })
        .await
        .expect("种子插入");
    }
    ds
}

/// 含墓碑行的全量读(读侧逃生舱 include_deleted)。
async fn raw_rows(
    ctx: &app_data::DataContext<'_>,
) -> Vec<(i64, Option<String>, Option<i64>)> {
    let rows = ctx
        .select(flow_category::Entity::find())
        .without_row_scope()
        .include_deleted()
        .all()
        .await
        .expect("全量读");
    rows.into_iter()
        .map(|m| (m.category_id, m.del_flag, m.update_by))
        .collect()
}

/// ① 删不存在的 id → Ok 静默(Java `FlwCategoryServiceImpl.java:283`
/// 平价)。
#[tokio::test]
async fn delete_nonexistent_id_is_silent_ok() {
    let ds = ds_seeded(&[]).await;
    let ctx = ds.system_context();

    CategoryService::new()
        .delete(&ctx, 999_999, &test_user(1))
        .await
        .expect("删不存在的分类必须静默 Ok,不得 NotFound/404");

    assert!(
        raw_rows(&ctx).await.is_empty(),
        "静默删除不得产生任何副作用"
    );
}

/// ② 已删行不重复计数、不被二次盖写:第一次删除落 tombstone
/// (del_flag='1' + update_by);第二次删除(换人)静默 Ok 且墓碑的
/// update_by 保持首位删除者 —— `update_where` 的软删存活过滤挡下
/// 对已删行的重写(原 update_many 无存活过滤时的重删重复盖写不复存在)。
#[tokio::test]
async fn delete_already_deleted_row_is_silent_and_keeps_tombstone() {
    let ds = ds_seeded(&[(1, 0, "0")]).await;
    let ctx = ds.system_context();

    // 第一次删除(user 1):tombstone 落库
    CategoryService::new()
        .delete(&ctx, 1, &test_user(1))
        .await
        .expect("存活行删除必须成功");
    assert_eq!(
        raw_rows(&ctx).await,
        vec![(1, Some("1".to_string()), Some(1))],
        "tombstone:del_flag='1' + update_by=1 + update_time(不断言时刻)"
    );

    // 第二次删除(user 2,同一 id):静默 Ok,墓碑不被二次盖写
    CategoryService::new()
        .delete(&ctx, 1, &test_user(2))
        .await
        .expect("删已删的分类必须静默 Ok,不得 NotFound/404");
    assert_eq!(
        raw_rows(&ctx).await,
        vec![(1, Some("1".to_string()), Some(1))],
        "已删行不被二次盖写:update_by 保持首位删除者(1),未被 2 重写"
    );
}

/// ③ `has_children` 守卫保留(移植期既有行为):
/// 存在子分类 → BadRequest,父行原样。
#[tokio::test]
async fn delete_with_children_is_rejected() {
    let ds = ds_seeded(&[(1, 0, "0"), (2, 1, "0")]).await;
    let ctx = ds.system_context();

    let err = CategoryService::new()
        .delete(&ctx, 1, &test_user(1))
        .await
        .expect_err("存在子分类必须拒绝删除");
    assert!(err.to_string().contains("存在子分类"), "守卫文案: {err}");

    assert_eq!(
        raw_rows(&ctx).await,
        vec![
            (1, Some("0".to_string()), None),
            (2, Some("0".to_string()), None),
        ],
        "被拒的删除不得产生任何副作用"
    );
}
