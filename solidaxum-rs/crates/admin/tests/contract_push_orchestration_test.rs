//! Task 2:通用推送编排契约(SSE 优先 + IM/短信/APP 渠道扇出)。
//!
//! 覆盖:未配置渠道落 `SKIPPED(no_driver/not_configured)`、平台 ID 缺失
//! 落 `SKIPPED(no_platform_user_id)`、10s 去重(目标集合展开与顺序无关)、
//! 渠道失败仅落日志不影响调用方、SSE 与渠道日志 batch_id 一致、用户平台
//! ID 经用户 API 往返、flow_notifier 发送日志、权限面不变。
//! 不含任何案件/纠纷业务 fixture —— 业务侧通知在业务仓另行契约。
#![cfg(feature = "test-util")]

use std::time::Duration;

use admin::testkit::{TestApp, assert_ok, code, super_admin, user_with_perms};
use app_core::model::login_user::LoginUser;
use app_data::sea_orm::{
    ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter,
};
use flow::notifier::FlowNotifier;
use serde_json::json;
use system::push::repository::sys_message_send_log::{
    Column as LogColumn, Entity as LogEntity, Model as LogModel,
};
use system::user::repository::user_repo::ActiveModel as UserActiveModel;

fn channel_user() -> LoginUser {
    user_with_perms(&[
        "system:messageChannel:list",
        "system:messageChannel:edit",
        "system:messageChannel:test",
    ])
}

async fn logs_for(app: &TestApp, biz_type: &str) -> Vec<LogModel> {
    let ds = app.state.infra.data_source();
    let ctx = ds.system_context();
    ctx.select(LogEntity::find().filter(LogColumn::BizType.eq(biz_type)))
        .without_row_scope()
        .all()
        .await
        .expect("查询发送日志失败")
}

async fn wait_for_log<F>(
    app: &TestApp,
    biz_type: &str,
    mut predicate: F,
) -> LogModel
where
    F: FnMut(&LogModel) -> bool,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(row) = logs_for(app, biz_type)
            .await
            .into_iter()
            .find(|row| predicate(row))
        {
            return row;
        }
        if tokio::time::Instant::now() >= deadline {
            let all = logs_for(app, biz_type).await;
            panic!("等待发送日志超时: {all:?}");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn set_wechat_user_id(app: &TestApp, user_id: i64, value: &str) {
    let ds = app.state.infra.data_source();
    let ctx = ds.system_context();
    ctx.update_without_row_scope(UserActiveModel {
        user_id: Set(user_id),
        wechat_user_id: Set(Some(value.to_string())),
        ..Default::default()
    })
    .await
    .expect("回写企微平台 ID 失败");
}

#[tokio::test]
async fn notify_writes_sse_success_and_skips_other_five_channels() {
    let app = TestApp::new().await;
    let mut rx = app.push_hub.subscribe();

    let result = app
        .state
        .services
        .push_service
        .notify(
            "push_orchestration_test",
            9_001,
            &[301],
            "通用通知内容",
            Some("/demo/9001"),
        )
        .await;
    assert!(!result.deduped, "首次通知不应被去重: {result:?}");
    assert_eq!(result.sse_sent, 1, "SSE 应逐用户发送: {result:?}");

    let msg = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("等待 SSE 超时")
        .expect("SSE 通道已关闭");
    assert_eq!(msg.source, "workflow");
    assert_eq!(msg.message, "通用通知内容");
    assert_eq!(msg.path.as_deref(), Some("/demo/9001"));
    assert_eq!(msg.to_user_id, Some(301));

    let rows = logs_for(&app, "push_orchestration_test").await;
    assert_eq!(rows.len(), 6, "SSE + 五通道各一行: {rows:?}");
    let sse = rows.iter().find(|r| r.channel == "SSE").expect("SSE 日志");
    assert_eq!(sse.status, "SUCCESS");
    assert_eq!(sse.recipient_user_ids.as_deref(), Some("[301]"));
    // SSE 与渠道发送行同批可聚合:batch_id 一律等于 biz_id 字符串。
    assert_eq!(sse.batch_id.as_deref(), Some("9001"), "{sse:?}");
    assert_eq!(sse.content.as_deref(), Some("通用通知内容"));

    for ch in ["WECHAT", "DINGTALK", "FEISHU"] {
        let row = rows
            .iter()
            .find(|r| r.channel == ch)
            .unwrap_or_else(|| panic!("缺少 {ch} 日志: {rows:?}"));
        assert_eq!(row.status, "SKIPPED", "{ch}: {row:?}");
        assert_eq!(row.skip_reason.as_deref(), Some("not_configured"));
        assert_eq!(
            row.batch_id.as_deref(),
            Some("9001"),
            "{ch} 与 SSE 同批: {row:?}"
        );
    }
    for ch in ["SMS", "APP"] {
        let row = rows
            .iter()
            .find(|r| r.channel == ch)
            .unwrap_or_else(|| panic!("缺少 {ch} 日志: {rows:?}"));
        assert_eq!(row.status, "SKIPPED", "{ch}: {row:?}");
        assert_eq!(row.skip_reason.as_deref(), Some("no_driver"));
        // 模板占位:短信正文以 text 字段填入通知内容。
        assert_eq!(row.content.as_deref(), Some("通用通知内容"));
    }
}

#[tokio::test]
async fn dedupe_suppresses_second_identical_notify_within_window() {
    let app = TestApp::new().await;
    let push = &app.state.services.push_service;
    let first = push
        .notify("dedupe_test", 9_010, &[301, 302], "去重内容", None)
        .await;
    assert!(!first.deduped);

    // 目标展开:同一用户集合乱序重发仍命中去重窗口。
    let second = push
        .notify("dedupe_test", 9_010, &[302, 301], "去重内容", None)
        .await;
    assert!(second.deduped, "窗口内相同业务/用户应被去重: {second:?}");
    assert_eq!(second.sse_sent, 0);

    let rows = logs_for(&app, "dedupe_test").await;
    assert_eq!(rows.len(), 6, "第二次通知不得再落日志: {rows:?}");
}

#[tokio::test]
async fn missing_platform_user_id_writes_no_platform_user_id_skip() {
    let app = TestApp::new().await;
    let me = channel_user();
    let (s, b) = app
        .put(
            "/system/messageChannel/DINGTALK",
            json!({
                "enabled": true,
                "clientId": "ding-test-client",
                "clientSecret": "ding-test-secret",
                "agentId": "4146174005"
            }),
            Some(&me),
        )
        .await;
    assert_ok(s, &b);

    // 301 无 dingtalk_user_id、无手机号 → 不做手机号回填,直接 SKIPPED。
    let result = app
        .state
        .services
        .push_service
        .notify("missing_platform_test", 9_020, &[301], "缺平台ID", None)
        .await;
    assert!(!result.deduped);

    let rows = logs_for(&app, "missing_platform_test").await;
    let ding = rows
        .iter()
        .find(|r| r.channel == "DINGTALK")
        .expect("钉钉日志");
    assert_eq!(ding.status, "SKIPPED", "{ding:?}");
    assert_eq!(ding.skip_reason.as_deref(), Some("no_platform_user_id"));
}

#[tokio::test]
async fn bad_wechat_config_failure_only_logs_and_notify_returns_counts() {
    let app = TestApp::new().await;
    let me = channel_user();
    // 企微缺 AgentId:发送前置失败,不触网;调用方(事件钩子)不得受影响。
    let (s, b) = app
        .put(
            "/system/messageChannel/WECHAT",
            json!({
                "enabled": true,
                "clientId": "corp-test",
                "clientSecret": "corp-secret"
            }),
            Some(&me),
        )
        .await;
    assert_ok(s, &b);
    set_wechat_user_id(&app, 301, "wx-301").await;

    // 直接调用编排:返回计数而非错误,失败仅落日志。
    let result = app
        .state
        .services
        .push_service
        .notify("channel_failure_test", 9_030, &[301], "渠道失败内容", None)
        .await;
    assert!(!result.deduped);
    assert_eq!(result.sse_sent, 1, "SSE 不受渠道失败影响: {result:?}");
    let wechat = result
        .channels
        .iter()
        .find(|c| c.channel == "WECHAT")
        .expect("WECHAT 结果");
    assert_eq!(wechat.status, "FAILED", "{wechat:?}");

    let row =
        wait_for_log(&app, "channel_failure_test", |r| r.channel == "WECHAT")
            .await;
    assert_eq!(row.status, "FAILED", "{row:?}");
    assert!(
        row.error_message
            .as_deref()
            .unwrap_or_default()
            .contains("AgentId"),
        "{row:?}"
    );
    // 其余渠道照常落 SKIPPED 日志(批次完整,不互相影响)。
    let sms =
        wait_for_log(&app, "channel_failure_test", |r| r.channel == "SMS")
            .await;
    assert_eq!(sms.status, "SKIPPED", "{sms:?}");
}

#[tokio::test]
async fn user_platform_ids_roundtrip_through_user_api() {
    let app = TestApp::new().await;
    let admin = super_admin();
    let (s, b) = app
        .post(
            "/system/user",
            json!({
                "userName": "push_id_user",
                "password": "123456",
                "nickName": "推送用户",
                "deptId": 101,
                "wechatUserId": "wx-push-1",
                "dingtalkUserId": "dd-push-1",
                "feishuUserId": "fs-push-1"
            }),
            Some(&admin),
        )
        .await;
    assert_ok(s, &b);

    let (s, b) = app
        .get("/system/user/list?userName=push_id_user", Some(&admin))
        .await;
    assert_ok(s, &b);
    let rows = b["data"]["rows"].as_array().expect("rows");
    let row = rows.first().expect("新建用户应可查");
    assert_eq!(row["wechatUserId"], "wx-push-1", "{row}");
    assert_eq!(row["dingtalkUserId"], "dd-push-1", "{row}");
    assert_eq!(row["feishuUserId"], "fs-push-1", "{row}");
}

#[tokio::test]
async fn flow_notifier_with_send_log_writes_sse_row() {
    let app = TestApp::new().await;
    let notifier =
        admin::flow_notifier::PushHubFlowNotifier::new(app.push_hub.clone())
            .with_send_log(app.state.infra.data_source());
    notifier
        .notify_users(&[301], "流程待办提醒", Some("/task/taskWaiting"))
        .await
        .expect("flow notify 不应失败");

    let row = wait_for_log(&app, "flow_notify", |r| r.channel == "SSE").await;
    assert_eq!(row.status, "SUCCESS", "{row:?}");
    assert_eq!(row.content.as_deref(), Some("流程待办提醒"));
    assert_eq!(row.recipient_user_ids.as_deref(), Some("[301]"));
    // 流程通知无业务批次,biz_id/batch_id 均为空。
    assert_eq!(row.batch_id, None, "{row:?}");
}

#[tokio::test]
async fn permissions_unchanged_for_channel_endpoints() {
    let app = TestApp::new().await;
    let nobody = user_with_perms(&[]);
    let (s, _b) = app.get("/system/messageChannel/list", Some(&nobody)).await;
    assert_eq!(s, 403, "渠道配置权限不得因编排放宽");

    // 超级管理员仍可访问渠道清单(权限面未破坏)。
    let (s, b) = app
        .get("/system/messageChannel/list", Some(&super_admin()))
        .await;
    assert_ok(s, &b);
    assert_eq!(code(&b), 200);
}
