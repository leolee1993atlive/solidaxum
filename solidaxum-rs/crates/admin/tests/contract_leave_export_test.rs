//! leave 导出契约测试。
//!
//! 运行: `cargo test -p admin --features test-util --test contract_leave_export_test`
//! (cargo 的位置参数过滤的是**测试函数名**而非文件名,必须用 `--test` 指定目标)
//!
//! 锁定三条:① 收 form-urlencoded(不是 JSON) ② 产物是 xlsx(不是 .json) ③ 要 export 权限。
//! 交付前的 export_leave 用 AppJson 提取器(Vue download 发 form-urlencoded → 415/400)、
//! 忽略筛选恒全量、把 VO 数组 serde_json::to_string 输出成 .json —— 与全站其余导出全不一致。

#![cfg(feature = "test-util")]

use admin::testkit::{TestApp, user_with_perms};
use axum::http::{Method, StatusCode};

#[tokio::test]
async fn leave_export_returns_xlsx() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["workflow:leave:export"]);
    // Vue download() 发 application/x-www-form-urlencoded,不是 JSON。
    // 四个筛选字段都是 Option,空 body 即「不带筛选」(与 list 不填筛选等价);
    // 不能发 "startLeaveDays="(空串) —— serde_urlencoded 会把空串当 i32 解析 → 422。
    let (status, headers, bytes) = app
        .request_form(
            Method::POST,
            "/workflow/leave/export",
            "startLeaveDays=1&endLeaveDays=10",
            Some(&user),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(ct.contains("spreadsheetml"), "leave 导出未返回 xlsx: {ct}");
    let cd = headers
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(cd.contains(".xlsx"), "leave 导出应是 xlsx 文件名: {cd}");
    assert!(
        bytes.len() > 2 && bytes[0] == 0x50 && bytes[1] == 0x4B,
        "leave 导出内容不是有效 xlsx(zip 魔数 PK)"
    );
}

#[tokio::test]
async fn leave_export_requires_permission() {
    let app = TestApp::new().await;
    let user = user_with_perms(&["workflow:leave:list"]); // 有 list 没有 export
    let (status, _, _) = app
        .request_form(Method::POST, "/workflow/leave/export", "", Some(&user))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
