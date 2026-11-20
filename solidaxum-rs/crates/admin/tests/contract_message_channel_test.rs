//! 渠道配置契约:清单脱敏 / 保存保留密钥 / 非法渠道拒绝 /
//! 自检缺配置 400。短信与 APP 不在本表(按院方定制,主线只留入口)。

#![cfg(feature = "test-util")]

use admin::testkit::{TestApp, assert_ok, super_admin, user_with_perms};
use serde_json::json;

fn channel_user() -> app_core::model::login_user::LoginUser {
    user_with_perms(&[
        "system:messageChannel:list",
        "system:messageChannel:edit",
        "system:messageChannel:test",
    ])
}

#[tokio::test]
async fn list_returns_three_channels_in_fixed_order() {
    let _env = channel_env_guard().await;
    let app = TestApp::new().await;
    let me = channel_user();
    let (s, b) = app.get("/system/messageChannel/list", Some(&me)).await;
    assert_ok(s, &b);
    let rows = b["data"].as_array().expect("array");
    let names: Vec<&str> =
        rows.iter().filter_map(|r| r["channel"].as_str()).collect();
    assert_eq!(
        names,
        vec!["DINGTALK", "WECHAT", "FEISHU"],
        "仅三渠道(短信/APP 不在此维护): {b}"
    );
    for r in rows {
        assert_eq!(r["configured"], false, "初始未配置: {r}");
        assert_eq!(r["clientSecret"], "", "未配置时密钥为空串");
        assert_eq!(r["source"], "none", "无 DB/env 凭据: {r}");
    }
}

#[tokio::test]
async fn save_then_list_masks_secret() {
    let app = TestApp::new().await;
    let me = channel_user();
    let (s, b) = app
        .put(
            "/system/messageChannel/DINGTALK",
            json!({
                "enabled": true,
                "clientId": "ding-test-client",
                "clientSecret": "super-secret-value",
                "agentId": "4146174005",
                "remark": "测试应用"
            }),
            Some(&me),
        )
        .await;
    assert_ok(s, &b);

    let (s2, b2) = app.get("/system/messageChannel/list", Some(&me)).await;
    assert_ok(s2, &b2);
    let row = b2["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["channel"] == "DINGTALK")
        .expect("钉钉行");
    assert_eq!(row["enabled"], true, "{row}");
    assert_eq!(row["configured"], true, "{row}");
    assert_eq!(row["source"], "db", "{row}");
    assert_eq!(row["clientId"], "ding-test-client");
    // 密钥绝不回显原文
    assert_eq!(row["clientSecret"], "******", "{row}");
    assert!(
        !b2.to_string().contains("super-secret-value"),
        "响应体不得含密钥原文: {b2}"
    );
}

#[tokio::test]
async fn blank_secret_keeps_existing_value() {
    let app = TestApp::new().await;
    let me = channel_user();
    app.put(
        "/system/messageChannel/WECHAT",
        json!({"enabled": true, "clientId": "corp", "clientSecret": "first-secret"}),
        Some(&me),
    )
    .await;
    let (s, b) = app
        .put(
            "/system/messageChannel/WECHAT",
            json!({"enabled": true, "clientId": "corp", "clientSecret": "", "remark": "只改备注"}),
            Some(&me),
        )
        .await;
    assert_ok(s, &b);

    let (_, b2) = app.get("/system/messageChannel/list", Some(&me)).await;
    let row = b2["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["channel"] == "WECHAT")
        .expect("企微行");
    assert_eq!(row["configured"], true, "密钥应仍在: {b2}");
    assert_eq!(row["clientSecret"], "******");
}

/// 部分更新语义:缺省字段 = 保留现值(只改备注不得抹掉凭据/停用通道)。
#[tokio::test]
async fn partial_update_keeps_existing_fields() {
    let app = TestApp::new().await;
    let me = channel_user();
    let (s, b) = app
        .put(
            "/system/messageChannel/FEISHU",
            json!({
                "enabled": true,
                "clientId": "feishu-app-id",
                "clientSecret": "feishu-secret",
                "baseUrl": "https://open.feishu.cn",
                "domain": "example.com",
                "remark": "初始备注"
            }),
            Some(&me),
        )
        .await;
    assert_ok(s, &b);

    // 只改备注:enabled/clientId/baseUrl/domain/密钥全部应保留。
    let (s, b) = app
        .put(
            "/system/messageChannel/FEISHU",
            json!({"remark": "只改备注"}),
            Some(&me),
        )
        .await;
    assert_ok(s, &b);

    let (_, b2) = app.get("/system/messageChannel/list", Some(&me)).await;
    let row = b2["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["channel"] == "FEISHU")
        .expect("飞书行");
    assert_eq!(row["enabled"], true, "缺省 enabled 应保留 true: {row}");
    assert_eq!(row["configured"], true, "凭据应仍在: {row}");
    assert_eq!(row["clientId"], "feishu-app-id", "{row}");
    assert_eq!(row["clientSecret"], "******", "{row}");
    assert_eq!(row["baseUrl"], "https://open.feishu.cn", "{row}");
    assert_eq!(row["domain"], "example.com", "{row}");
    assert_eq!(row["remark"], "只改备注", "{row}");
}

#[tokio::test]
async fn unsupported_channel_rejected() {
    let app = TestApp::new().await;
    let me = channel_user();
    for ch in ["SMS", "APP", "UNKNOWN"] {
        let (s, b) = app
            .put(
                &format!("/system/messageChannel/{ch}"),
                json!({"enabled": true, "clientId": "x", "clientSecret": "y"}),
                Some(&me),
            )
            .await;
        assert_eq!(s, 400, "{ch} 应被拒: {b}");
    }
}

#[tokio::test]
async fn test_endpoint_requires_config() {
    let _env = channel_env_guard().await;
    let app = TestApp::new().await;
    let me = channel_user();
    let (s, b) = app
        .post("/system/messageChannel/FEISHU/test", json!({}), Some(&me))
        .await;
    assert_eq!(s, 400, "未配置时自检报 400: {b}");
    assert!(
        b["msg"].as_str().unwrap_or_default().contains("未配置"),
        "{b}"
    );
}

#[tokio::test]
async fn unsupported_test_channel_rejected() {
    let app = TestApp::new().await;
    let me = channel_user();
    let (s, b) = app
        .post("/system/messageChannel/SMS/test", json!({}), Some(&me))
        .await;
    assert_eq!(s, 400, "短信不在配置页维护: {b}");
}

#[tokio::test]
async fn list_requires_permission() {
    let app = TestApp::new().await;
    let nobody = user_with_perms(&[]);
    let (s, _b) = app.get("/system/messageChannel/list", Some(&nobody)).await;
    assert_eq!(s, 403);
}

#[tokio::test]
async fn edit_and_test_require_permission() {
    let app = TestApp::new().await;
    let list_only = user_with_perms(&["system:messageChannel:list"]);
    let (s, _b) = app
        .put(
            "/system/messageChannel/DINGTALK",
            json!({"enabled": true, "clientId": "x", "clientSecret": "y"}),
            Some(&list_only),
        )
        .await;
    assert_eq!(s, 403, "无 edit 权限不得保存");
    let (s, _b) = app
        .post(
            "/system/messageChannel/DINGTALK/test",
            json!({}),
            Some(&list_only),
        )
        .await;
    assert_eq!(s, 403, "无 test 权限不得自检");
}

#[tokio::test]
async fn super_admin_can_read() {
    let app = TestApp::new().await;
    let (s, b) = app
        .get("/system/messageChannel/list", Some(&super_admin()))
        .await;
    assert_ok(s, &b);
}

// ---- 发送历史(B5 消息发送历史页;全局审计流水,只读) ----

use app_data::sea_orm::ActiveValue::Set;
use chrono::{DateTime, TimeZone, Utc};
use system::push::repository::sys_message_send_log::{
    ActiveModel as LogActiveModel, SendLogRepository,
};

async fn insert_log(
    app: &TestApp,
    log_id: i64,
    channel: &str,
    status: &str,
    biz_type: &str,
    create_time: DateTime<Utc>,
) {
    let ds = app.state.infra.data_source();
    let ctx = ds.system_context();
    let am = LogActiveModel {
        log_id: Set(log_id),
        channel: Set(channel.to_string()),
        status: Set(status.to_string()),
        biz_type: Set(Some(biz_type.to_string())),
        content: Set(Some("正文".to_string())),
        create_time: Set(Some(create_time)),
        ..Default::default()
    };
    SendLogRepository::new()
        .insert(&ctx, am)
        .await
        .expect("插日志");
}

#[tokio::test]
async fn message_log_list_paginates_and_filters() {
    let app = TestApp::new().await;
    let me = channel_user();
    let t0 = Utc.with_ymd_and_hms(2026, 10, 30, 10, 0, 0).unwrap();
    let t1 = Utc.with_ymd_and_hms(2026, 10, 31, 10, 0, 0).unwrap();
    let t2 = Utc.with_ymd_and_hms(2026, 11, 1, 10, 0, 0).unwrap();
    insert_log(&app, 9001, "SSE", "SUCCESS", "flow_notify", t0).await;
    insert_log(&app, 9002, "DINGTALK", "FAILED", "notice", t1).await;
    insert_log(&app, 9003, "WECHAT", "SKIPPED", "notice", t2).await;

    // 全量:倒序(时间,然后 log_id),total=3
    let (s, b) = app
        .get("/system/messageLog/list?pageNum=1&pageSize=10", Some(&me))
        .await;
    assert_ok(s, &b);
    let rows = b["data"]["rows"].as_array().expect("rows");
    assert_eq!(b["data"]["total"], 3, "{b}");
    let ids: Vec<i64> = rows
        .iter()
        .filter_map(|r| r["logId"].as_str()?.parse().ok())
        .collect();
    assert_eq!(ids, vec![9003, 9002, 9001], "新→旧: {b}");

    // 渠道 + 状态过滤
    let (s, b) = app
        .get(
            "/system/messageLog/list?pageNum=1&pageSize=10&channel=DINGTALK&status=FAILED",
            Some(&me),
        )
        .await;
    assert_ok(s, &b);
    let rows = b["data"]["rows"].as_array().expect("rows");
    assert_eq!(b["data"]["total"], 1, "{b}");
    assert_eq!(rows[0]["logId"], "9002", "ID 出站为字符串: {b}");
    assert_eq!(rows[0]["channel"], "DINGTALK");

    // 时间范围:插入走的漏斗会盖审计时间戳(create_time=写入时刻),
    // 无法预制历史行;这里只验证 begin/end 入参被接受并按范围过滤
    // (历史区间自然为空,对齐 bl-mcdms 契约)。
    let (s, b) = app
        .get(
            "/system/messageLog/list?pageNum=1&pageSize=10&begin=2000-01-01&end=2000-01-02",
            Some(&me),
        )
        .await;
    assert_ok(s, &b);
    assert_eq!(b["data"]["total"], 0, "{b}");
}

#[tokio::test]
async fn message_log_list_requires_permission() {
    let app = TestApp::new().await;
    let nobody = user_with_perms(&[]);
    let (s, _b) = app.get("/system/messageLog/list", Some(&nobody)).await;
    assert_eq!(s, 403);
}

// ---- 环境隔离:渠道 env 回退不得渗入契约测试 ----
//
// `resolve_config` 在 DB 行未配齐时回退 DINGTALK_*/WECHAT_*/FEISHU_*/
// MSG_CHANNEL_BASE_URL 环境变量。开发机若导出过真实凭据:
//   - `list_returns_three_channels_in_fixed_order` 的 source=="none" 断言失配;
//   - `test_endpoint_requires_config` 会真的请求开放平台取 token(出网)。
// 因此对依赖"未配置"前提的用例,在测试进程内清空这批变量、结束后恢复。
// std::env::{remove_var,set_var} 在 edition 2024 为 unsafe:临界区由下方
// 全文件共享的异步互斥锁保护,持锁期间同进程其他用例不会读写这批变量,
// 故无数据竞争;恢复在 Drop 中完成,panic 路径同样归还。

static CHANNEL_ENV_LOCK: tokio::sync::Mutex<()> =
    tokio::sync::Mutex::const_new(());

const CHANNEL_ENV_VARS: [&str; 9] = [
    "DINGTALK_CLIENT_ID",
    "DINGTALK_CLIENT_SECRET",
    "DINGTALK_AGENT_ID",
    "WECHAT_CORP_ID",
    "WECHAT_CORP_SECRET",
    "WECHAT_AGENT_ID",
    "FEISHU_APP_ID",
    "FEISHU_APP_SECRET",
    "MSG_CHANNEL_BASE_URL",
];

struct ChannelEnvGuard {
    saved: Vec<(String, Option<String>)>,
    _lock: tokio::sync::MutexGuard<'static, ()>,
}

/// 持锁清空渠道环境变量;Drop 时恢复原值。必须全程持有(跨 await),
/// 否则并行的另一个用例可能在"清空前读到真值"。
async fn channel_env_guard() -> ChannelEnvGuard {
    let lock = CHANNEL_ENV_LOCK.lock().await;
    let saved = CHANNEL_ENV_VARS
        .iter()
        .map(|k| ((*k).to_string(), std::env::var(k).ok()))
        .collect();
    for k in CHANNEL_ENV_VARS {
        // SAFETY: 持 CHANNEL_ENV_LOCK,同进程内无其他代码并发读写这批变量。
        unsafe { std::env::remove_var(k) };
    }
    ChannelEnvGuard { saved, _lock: lock }
}

impl Drop for ChannelEnvGuard {
    fn drop(&mut self) {
        for (k, v) in &self.saved {
            // SAFETY: 与清空同一临界区(锁随本结构存活),恢复期间无并发读写。
            match v {
                Some(val) => unsafe { std::env::set_var(k, val) },
                None => unsafe { std::env::remove_var(k) },
            }
        }
    }
}
