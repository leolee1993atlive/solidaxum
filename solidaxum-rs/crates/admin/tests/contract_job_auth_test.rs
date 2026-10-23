//! job / jobLog 路由鉴权回归测试(SEC-1)。
//!
//! 运行: `cargo test -p admin --features test-util`
//!
//! 历史漏洞:`router.rs` 中 `/job` 与 `/monitor/jobLog` 未经 `apply_auth`
//! 包裹,匿名请求可增删改并立即执行定时任务。本文件锁定修复:
//! 未登录请求必须返回 401。

#![cfg(feature = "test-util")]

use admin::testkit::{TestApp, code};
use serde_json::json;

/// 未登录(不带 `x-test-login-user` 头)请求 job 各端点,应全部 401。
#[tokio::test]
async fn job_routes_reject_anonymous() {
    let app = TestApp::new().await;

    // 列表
    let (status, body) = app.get("/job/list", None).await;
    assert_eq!(status.as_u16(), 401, "GET /job/list 未登录应 401: {body}");
    assert_eq!(code(&body), 401, "GET /job/list 未登录应 code=401: {body}");

    // 详情
    let (status, body) = app.get("/job/1", None).await;
    assert_eq!(status.as_u16(), 401, "GET /job/1 未登录应 401: {body}");
    assert_eq!(code(&body), 401, "GET /job/1 未登录应 code=401: {body}");

    // 新增 / 修改。注意:nest("/job") 下内层根路由的实际路径是 /job
    // (axum 0.8 nest 语义;/job/ 带斜杠本就 404,与鉴权无关)。
    let (status, body) = app.post("/job", json!({}), None).await;
    assert_eq!(status.as_u16(), 401, "POST /job 未登录应 401: {body}");
    assert_eq!(code(&body), 401, "POST /job 未登录应 code=401: {body}");

    let (status, body) = app.put("/job", json!({}), None).await;
    assert_eq!(status.as_u16(), 401, "PUT /job 未登录应 401: {body}");
    assert_eq!(code(&body), 401, "PUT /job 未登录应 code=401: {body}");

    // 状态切换
    let (status, body) = app
        .put(
            "/job/changeStatus",
            json!({"jobId": 1, "status": "0"}),
            None,
        )
        .await;
    assert_eq!(
        status.as_u16(),
        401,
        "PUT /job/changeStatus 未登录应 401: {body}"
    );
    assert_eq!(
        code(&body),
        401,
        "PUT /job/changeStatus 未登录应 code=401: {body}"
    );

    // 立即执行(本漏洞最危险的路径)
    let (status, body) = app.put("/job/run", json!({"jobId": 1}), None).await;
    assert_eq!(status.as_u16(), 401, "PUT /job/run 未登录应 401: {body}");
    assert_eq!(code(&body), 401, "PUT /job/run 未登录应 code=401: {body}");

    // 删除
    let (status, body) = app.delete("/job/1", None).await;
    assert_eq!(status.as_u16(), 401, "DELETE /job/1 未登录应 401: {body}");
    assert_eq!(code(&body), 401, "DELETE /job/1 未登录应 code=401: {body}");
}

/// 未登录请求 jobLog 各端点,应全部 401。
#[tokio::test]
async fn job_log_routes_reject_anonymous() {
    let app = TestApp::new().await;

    let (status, body) = app.get("/monitor/jobLog/list", None).await;
    assert_eq!(
        status.as_u16(),
        401,
        "GET /monitor/jobLog/list 未登录应 401: {body}"
    );
    assert_eq!(
        code(&body),
        401,
        "GET /monitor/jobLog/list 未登录应 code=401: {body}"
    );

    let (status, body) = app.get("/monitor/jobLog/1", None).await;
    assert_eq!(
        status.as_u16(),
        401,
        "GET /monitor/jobLog/1 未登录应 401: {body}"
    );
    assert_eq!(
        code(&body),
        401,
        "GET /monitor/jobLog/1 未登录应 code=401: {body}"
    );

    let (status, body) = app.delete("/monitor/jobLog/1", None).await;
    assert_eq!(
        status.as_u16(),
        401,
        "DELETE /monitor/jobLog/1 未登录应 401: {body}"
    );
    assert_eq!(
        code(&body),
        401,
        "DELETE /monitor/jobLog/1 未登录应 code=401: {body}"
    );

    let (status, body) = app.delete("/monitor/jobLog/clean", None).await;
    assert_eq!(
        status.as_u16(),
        401,
        "DELETE /monitor/jobLog/clean 未登录应 401: {body}"
    );
    assert_eq!(
        code(&body),
        401,
        "DELETE /monitor/jobLog/clean 未登录应 code=401: {body}"
    );
}
