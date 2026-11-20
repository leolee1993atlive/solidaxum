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
