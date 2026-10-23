//! panic 兜底契约。
//!
//! 运行: `cargo test -p admin --features test-util --test contract_panic_test`
//!
//! handler panic 必须变成 500 + 项目标准 `{code, msg}` 信封。未装 panic
//! 兜底层时,hyper 会直接丢弃连接 —— 客户端拿到的是连接重置而非可读错误,
//! 访问日志里也没有 500,现场排障时等于线索全无。
//!
//! 注意:探针路由本身就是故意 panic 的,跑这个测试时 stderr 会打出一条
//! panic 记录(默认 panic hook 仍会执行,兜底层不抑制它),属预期输出。
#![cfg(feature = "test-util")]

use admin::testkit::{PANIC_PROBE_MESSAGE, TestApp, code};
use axum::http::StatusCode;

#[tokio::test]
async fn a_panicking_handler_returns_a_500_envelope() {
    let app = TestApp::new().await;

    let (status, body) = app.get("/testkit/panic", None).await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(code(&body), 500, "panic 响应要走项目信封: {body}");
    assert!(
        body["msg"].as_str().is_some_and(|m| !m.is_empty()),
        "panic 响应必须带非空 msg: {body}"
    );
}

#[tokio::test]
async fn the_panic_payload_is_not_leaked_to_the_client() {
    let app = TestApp::new().await;

    let (_, body) = app.get("/testkit/panic", None).await;

    let msg = body["msg"].as_str().unwrap_or_default();
    assert!(
        !msg.contains(PANIC_PROBE_MESSAGE),
        "panic 原文只进日志,不得回给客户端: {msg}"
    );
}
