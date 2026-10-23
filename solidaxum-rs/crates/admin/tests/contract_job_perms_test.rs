//! job / jobLog 路由权限回归测试（SEC-1）。
//!
//! 运行: `cargo test -p admin --features test-util --test contract_job_perms_test`
//! (cargo 的位置参数过滤的是**测试函数名**而非文件名,必须用 `--test` 指定目标)
//!
//! 锁定修复：已登录但缺少 monitor:job:* 权限的用户访问 job/jobLog
//! 端点时必须返回 403；超级管理员不受权限层限制。

#![cfg(feature = "test-util")]

use admin::testkit::{TestApp, code, super_admin, user_with_perms};
use serde_json::json;

/// 无权限用户访问 job 各端点，应全部 403。
#[tokio::test]
async fn job_routes_reject_without_permission() {
    let app = TestApp::new().await;
    let user = user_with_perms(&[]);

    let (status, body) = app.get("/job/list", Some(&user)).await;
    assert_eq!(status.as_u16(), 403, "GET /job/list 无权限应 403: {body}");
    assert_eq!(code(&body), 403, "GET /job/list 无权限应 code=403: {body}");

    let (status, body) = app.get("/job/1", Some(&user)).await;
    assert_eq!(status.as_u16(), 403, "GET /job/1 无权限应 403: {body}");
    assert_eq!(code(&body), 403, "GET /job/1 无权限应 code=403: {body}");

    let job_body = json!({
        "job_name": "test_job",
        "job_group": "DEFAULT",
        "invoke_target": "demo",
        "cron_expression": "0/5 * * * * ?"
    });

    let (status, body) = app.post("/job", job_body.clone(), Some(&user)).await;
    assert_eq!(status.as_u16(), 403, "POST /job 无权限应 403: {body}");
    assert_eq!(code(&body), 403, "POST /job 无权限应 code=403: {body}");

    let update_body = json!({
        "job_id": 1,
        "job_name": "test_job",
        "job_group": "DEFAULT",
        "invoke_target": "demo",
        "cron_expression": "0/5 * * * * ?"
    });

    let (status, body) = app.put("/job", update_body, Some(&user)).await;
    assert_eq!(status.as_u16(), 403, "PUT /job 无权限应 403: {body}");
    assert_eq!(code(&body), 403, "PUT /job 无权限应 code=403: {body}");

    let (status, body) = app
        .put(
            "/job/changeStatus",
            json!({"job_id": 1, "status": "0"}),
            Some(&user),
        )
        .await;
    assert_eq!(
        status.as_u16(),
        403,
        "PUT /job/changeStatus 无权限应 403: {body}"
    );
    assert_eq!(
        code(&body),
        403,
        "PUT /job/changeStatus 无权限应 code=403: {body}"
    );

    let (status, body) =
        app.put("/job/run", json!({"jobId": 1}), Some(&user)).await;
    assert_eq!(status.as_u16(), 403, "PUT /job/run 无权限应 403: {body}");
    assert_eq!(code(&body), 403, "PUT /job/run 无权限应 code=403: {body}");

    let (status, body) = app.delete("/job/1", Some(&user)).await;
    assert_eq!(status.as_u16(), 403, "DELETE /job/1 无权限应 403: {body}");
    assert_eq!(code(&body), 403, "DELETE /job/1 无权限应 code=403: {body}");
}

/// 无权限用户访问 jobLog 各端点，应全部 403。
#[tokio::test]
async fn job_log_routes_reject_without_permission() {
    let app = TestApp::new().await;
    let user = user_with_perms(&[]);

    let (status, body) = app.get("/monitor/jobLog/list", Some(&user)).await;
    assert_eq!(
        status.as_u16(),
        403,
        "GET /monitor/jobLog/list 无权限应 403: {body}"
    );
    assert_eq!(
        code(&body),
        403,
        "GET /monitor/jobLog/list 无权限应 code=403: {body}"
    );

    let (status, body) = app.get("/monitor/jobLog/1", Some(&user)).await;
    assert_eq!(
        status.as_u16(),
        403,
        "GET /monitor/jobLog/1 无权限应 403: {body}"
    );
    assert_eq!(
        code(&body),
        403,
        "GET /monitor/jobLog/1 无权限应 code=403: {body}"
    );

    let (status, body) = app.delete("/monitor/jobLog/1", Some(&user)).await;
    assert_eq!(
        status.as_u16(),
        403,
        "DELETE /monitor/jobLog/1 无权限应 403: {body}"
    );
    assert_eq!(
        code(&body),
        403,
        "DELETE /monitor/jobLog/1 无权限应 code=403: {body}"
    );

    let (status, body) = app.delete("/monitor/jobLog/clean", Some(&user)).await;
    assert_eq!(
        status.as_u16(),
        403,
        "DELETE /monitor/jobLog/clean 无权限应 403: {body}"
    );
    assert_eq!(
        code(&body),
        403,
        "DELETE /monitor/jobLog/clean 无权限应 code=403: {body}"
    );
}

/// 超级管理员访问 job 列表时不应被权限层拦截（测试库缺少 sys_job 表可能 500，但绝不可 403）。
#[tokio::test]
async fn job_list_allows_super_admin() {
    let app = TestApp::new().await;
    let admin = super_admin();

    let (status, body) = app.get("/job/list", Some(&admin)).await;
    assert_ne!(
        status.as_u16(),
        403,
        "GET /job/list 超级管理员不应返回 403: {body}"
    );
    assert_ne!(
        code(&body),
        403,
        "GET /job/list 超级管理员不应返回 code=403: {body}"
    );
}
