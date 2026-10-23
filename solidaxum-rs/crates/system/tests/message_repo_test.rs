//! message 仓储集成测试(全豁免)。
//!
//! 覆盖两组语义:
//! 1. 基础读路径(经 `system_context` 桥接);
//! 2. **全豁免判例**:即便 resolver 给出 DeptIn,
//!    消息盒子读照见全部目标行 —— SysMessageMapper 无 @DataPermission,
//!    站内消息的可见性由 send_user_ids(目标串)表达,不经数据范围。

mod common;

use std::sync::Arc;

use app_data::{
    DataSource, Principal, ScopeFilter, sea_orm::ActiveValue::Set,
    testing::FixedResolver,
};
use chrono::Utc;
use common::setup_db;
use system::message::repository::message_repo::{
    self as message_entity, MessageRepository,
};

/// 消息种子(免盖章:显式 create_dept/create_by 原样落库;create_time 同
/// 原 datetime('now') —— select_message_box 按 create_time DESC 排序)。
async fn seed_messages(ds: &DataSource) {
    let ctx = ds.system_context();
    for (id, category, ty, source, title, message, data_json, send_user_ids) in [
        (
            8001,
            "system",
            "MESSAGE",
            "BACKEND",
            "系统消息1",
            "msg1",
            None,
            "0",
        ),
        (
            8002,
            "system",
            "MESSAGE",
            "BACKEND",
            "系统消息2",
            "msg2",
            None,
            "123",
        ),
        (
            8003,
            "notice",
            "NOTICE",
            "NOTICE",
            "通知消息1",
            "notice1",
            None,
            "123,456",
        ),
        (
            8004,
            "workflow",
            "TASK",
            "WORKFLOW",
            "工作流消息1",
            "task1",
            None,
            "999",
        ),
    ] {
        seed_message(
            &ctx,
            id,
            category,
            ty,
            source,
            title,
            message,
            data_json,
            send_user_ids,
            None,
        )
        .await;
    }
}

#[allow(clippy::too_many_arguments)]
async fn seed_message(
    ctx: &app_data::DataContext<'_>,
    id: i64,
    category: &str,
    ty: &str,
    source: &str,
    title: &str,
    message: &str,
    data_json: Option<&str>,
    send_user_ids: &str,
    create_dept: Option<i64>,
) {
    ctx.insert_unstamped(message_entity::ActiveModel {
        message_id: Set(id),
        category: Set(category.into()),
        r#type: Set(ty.into()),
        source: Set(source.into()),
        title: Set(Some(title.into())),
        message: Set(Some(message.into())),
        data_json: Set(data_json.map(Into::into)),
        send_user_ids: Set(send_user_ids.into()),
        create_dept: Set(create_dept),
        create_time: Set(Some(Utc::now())),
        ..Default::default()
    })
    .await
    .expect("种消息失败");
}

fn repo_system(ds: DataSource) -> (Arc<DataSource>, MessageRepository) {
    let ds = Arc::new(ds);
    (ds.clone(), MessageRepository::new(ds))
}

fn repo_scoped(
    ds: DataSource,
    sf: ScopeFilter,
) -> (Arc<DataSource>, MessageRepository) {
    let ds = Arc::new(ds.with_resolver(Arc::new(FixedResolver(sf))));
    (ds.clone(), MessageRepository::new(ds))
}

#[tokio::test]
async fn test_message_box_global_and_targeted() {
    let ds = setup_db().await;
    seed_messages(&ds).await;
    let (ds, repo) = repo_system(ds);
    let ctx = ds.system_context();

    // 用户 123: 全局(8001) + 精确(8002) + 中间(8003)
    let box_vo = repo.select_message_box(&ctx, 123).await.unwrap();
    assert_eq!(box_vo.system_list.len(), 2);
    assert_eq!(box_vo.notice_list.len(), 1);
    assert_eq!(box_vo.workflow_list.len(), 0);
}

#[tokio::test]
async fn test_message_box_global_visible_to_all() {
    let ds = setup_db().await;
    seed_messages(&ds).await;
    let (ds, repo) = repo_system(ds);
    let ctx = ds.system_context();

    // 用户 456: 全局(8001) + 中间(8003)
    let box_vo = repo.select_message_box(&ctx, 456).await.unwrap();
    assert_eq!(box_vo.system_list.len(), 1);
    assert_eq!(box_vo.system_list[0].message_id, 8001);
    assert_eq!(box_vo.notice_list.len(), 1);
}

#[tokio::test]
async fn test_message_box_excludes_other_users() {
    let ds = setup_db().await;
    seed_messages(&ds).await;
    let (ds, repo) = repo_system(ds);
    let ctx = ds.system_context();

    // 用户 777: 仅全局(8001)
    let box_vo = repo.select_message_box(&ctx, 777).await.unwrap();
    assert_eq!(box_vo.system_list.len(), 1);
    assert_eq!(box_vo.system_list[0].message_id, 8001);
    assert_eq!(box_vo.notice_list.len(), 0);
    assert_eq!(box_vo.workflow_list.len(), 0);
}

#[tokio::test]
async fn test_data_json_parsed_to_value() {
    let ds = setup_db().await;
    seed_message(
        &ds.system_context(),
        9001,
        "system",
        "MESSAGE",
        "BACKEND",
        "带扩展数据",
        "msg",
        Some(r#"{"orderId": 42}"#),
        "0",
        None,
    )
    .await;
    let (ds, repo) = repo_system(ds);
    let ctx = ds.system_context();

    let box_vo = repo.select_message_box(&ctx, 1).await.unwrap();
    let msg = &box_vo.system_list[0];
    assert_eq!(msg.message_id, 9001);
    let data = msg.data.as_ref().expect("data 应被解析");
    assert_eq!(data["orderId"], 42);
}

/// **全豁免判例**:resolver 给出 DeptIn([101]) 时,消息盒子读必须照见
/// 全部目标行(含 create_dept=102 的越权部门行)—— SysMessageMapper 无
/// @DataPermission,消息可见性由 send_user_ids 目标串表达,不是数据权限;
/// 若 select_category_list 漏标 `without_row_scope`,CONSTRAINED=false 的
/// 驱动会 fail-closed 报 Unsupported,本用例变红。
#[tokio::test]
async fn exempt_message_box_reads_see_all_rows_despite_scope() {
    let ds = setup_db().await;
    seed_messages(&ds).await;
    // 补一行越权部门(create_dept=102)的全局消息:范围过滤存在时它必须仍可见
    seed_message(
        &ds.system_context(),
        8005,
        "system",
        "MESSAGE",
        "BACKEND",
        "越权部门消息",
        "msg5",
        None,
        "0",
        Some(102),
    )
    .await;
    let (ds, repo) = repo_scoped(ds, ScopeFilter::DeptIn(vec![101]));
    let ctx = ds.context(Principal {
        user_id: 301,
        dept_id: Some(101),
    });

    let box_vo = repo.select_message_box(&ctx, 301).await.unwrap();
    assert_eq!(
        box_vo.system_list.len(),
        2,
        "用户 301 的 system 分类 = 全局 8001 + 全局 8005(越权部门行);\
         8002 的 send_user_ids='123' 不含 301,不计"
    );
    assert!(
        box_vo.system_list.iter().any(|m| m.message_id == 8005),
        "越权部门(create_dept=102)的全局消息必须照常可见"
    );
}
