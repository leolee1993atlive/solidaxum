//! flow_instance_biz_ext 集成测试(脚手架切 `app_data::testing`,消费
//! canonical 变体)。
//!
//! 脚手架:`sqlite_memory_empty()`(单连接内存库)+ `DataSource::create_table`
//! 按实体建表;执行面经 `system_context()`(flow 全豁免实体,豁免读不解析
//! 范围,与用户 ctx 行为等价)。本文件不再出现 sea_orm。
use app_core::util::snowflake;
use app_data::{DataSource, testing::sqlite_memory_empty};
use flow::repository::{biz_ext_repo, entity::flow_instance_biz_ext};

async fn setup_ds() -> DataSource {
    let ds = sqlite_memory_empty().await;
    ds.create_table(flow_instance_biz_ext::Entity)
        .await
        .unwrap();
    ds
}

#[tokio::test]
async fn biz_ext_save_and_select_by_instance_id() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    let instance_id = 9001_i64;
    biz_ext_repo::save_or_update_by_instance_id(
        &ctx,
        &flow_instance_biz_ext::Model {
            id: snowflake::next_id(),
            instance_id: Some(instance_id),
            business_id: Some("100".into()),
            business_code: Some("LC20260617001".into()),
            business_title: Some("张三的请假单".into()),
            del_flag: Some("0".into()),
            create_dept: None,
            create_by: None,
            create_time: None,
            update_by: None,
            update_time: None,
        },
    )
    .await
    .unwrap();
    let row = biz_ext_repo::select_by_instance_id(&ctx, instance_id)
        .await
        .unwrap()
        .expect("应能查到");
    assert_eq!(row.business_code.as_deref(), Some("LC20260617001"));
    assert_eq!(row.business_title.as_deref(), Some("张三的请假单"));
}

#[tokio::test]
async fn biz_ext_upsert_updates_existing_row() {
    let ds = setup_ds().await;
    let ctx = ds.system_context();
    let instance_id = 9002_i64;
    let base = flow_instance_biz_ext::Model {
        id: snowflake::next_id(),
        instance_id: Some(instance_id),
        business_id: Some("101".into()),
        business_code: Some("OLD".into()),
        business_title: Some("旧标题".into()),
        del_flag: Some("0".into()),
        create_dept: None,
        create_by: None,
        create_time: None,
        update_by: None,
        update_time: None,
    };
    biz_ext_repo::save_or_update_by_instance_id(&ctx, &base)
        .await
        .unwrap();
    biz_ext_repo::save_or_update_by_instance_id(
        &ctx,
        &flow_instance_biz_ext::Model {
            business_title: Some("新标题".into()),
            ..base
        },
    )
    .await
    .unwrap();
    let row = biz_ext_repo::select_by_instance_id(&ctx, instance_id)
        .await
        .unwrap()
        .expect("应能查到");
    assert_eq!(row.business_title.as_deref(), Some("新标题"));
}
