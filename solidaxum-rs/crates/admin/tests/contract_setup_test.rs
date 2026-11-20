//! 首装初始化向导契约。
//!
//! 运行: `cargo test -p admin --features test-util contract_setup`
//!
//! 覆盖:状态探测(新装 false/老系统键缺失 true)、聚合完成(改密 +
//! 两键落库 + 标记)、防重放、旧密码核对、仅超管可执行。系统名一律走
//! `systemName`(→ sys.title),不出现任何业务化命名。
#![cfg(feature = "test-util")]

use admin::testkit::{TestApp, assert_ok, code, super_admin, user_with_perms};
use serde_json::json;

const STATUS_URI: &str = "/system/setup/status";
const COMPLETE_URI: &str = "/system/setup/complete";
const ADMIN_ID: i64 = 1761100000000000001;

/// sys.title 走真实 Redis 持久缓存,同 binary 内并行用例会互相回填
/// (与 contract_sysinfo_test 同一机型)—— 凡经 /config/configKey 读写
/// sys.title 的用例都拿这把锁串行化;状态探测走 get_value_or_default
/// (直读 DB 不触缓存),无需锁。
static TITLE_CACHE_LOCK: tokio::sync::Mutex<()> =
    tokio::sync::Mutex::const_new(());

/// 给种子里的 admin 换上真实 bcrypt 哈希(种子是 `$2a$10$placeholder`,
/// 无法通过旧密码核对),返回明文。
async fn seed_admin_password(app: &TestApp, plain: &str) -> String {
    let hash = bcrypt::hash(plain, 4).expect("bcrypt 哈希失败");
    let ds = app.state.infra.data_source();
    app_data::testkit::execute_raw(
        &ds,
        &format!(
            "UPDATE sys_user SET password = '{hash}' WHERE user_id = {ADMIN_ID}"
        ),
    )
    .await
    .expect("种 admin 密码失败");
    plain.to_string()
}

fn complete_bo(old: &str, new: &str) -> serde_json::Value {
    json!({
        "systemName": "SolidAxum 演示系统",
        "adminOldPassword": old,
        "adminNewPassword": new,
        "userInitPassword": "Init@456789",
    })
}

async fn config_value(app: &TestApp, key: &str) -> String {
    let (_, body) = app
        .get(
            &format!("/system/config/configKey/{key}"),
            Some(&super_admin()),
        )
        .await;
    assert_eq!(code(&body), 200, "读配置 {key} 失败: {body}");
    body["data"].as_str().unwrap_or_default().to_string()
}

#[tokio::test]
async fn fresh_install_reports_incomplete() {
    // 种子带 sys.setup.completed=false —— 首装必须被判定为未初始化
    let app = TestApp::new().await;

    let (status, body) = app.get(STATUS_URI, Some(&super_admin())).await;

    assert_ok(status, &body);
    assert_eq!(body["data"]["completed"], json!(false));
}

#[tokio::test]
async fn missing_key_means_already_completed() {
    // 老系统升级后没有这个键 —— 不该被强导向导(键不存在=已完成)
    let app = TestApp::new().await;
    let ds = app.state.infra.data_source();
    app_data::testkit::execute_raw(
        &ds,
        "DELETE FROM sys_config WHERE config_key = 'sys.setup.completed'",
    )
    .await
    .expect("删 setup 键失败");

    let (status, body) = app.get(STATUS_URI, Some(&super_admin())).await;

    assert_ok(status, &body);
    assert_eq!(body["data"]["completed"], json!(true));
}

#[tokio::test]
async fn complete_updates_configs_marks_done_and_changes_password() {
    let _title_guard = TITLE_CACHE_LOCK.lock().await;
    let app = TestApp::new().await;
    seed_admin_password(&app, "Admin@123456").await;

    let (status, body) = app
        .post(
            COMPLETE_URI,
            complete_bo("Admin@123456", "New@123456"),
            Some(&super_admin()),
        )
        .await;
    assert_ok(status, &body);

    // 标记已落,状态翻转
    let (_, body) = app.get(STATUS_URI, Some(&super_admin())).await;
    assert_eq!(body["data"]["completed"], json!(true));

    // 系统名称(=sys.title)与用户默认密码已按入参落库
    assert_eq!(config_value(&app, "sys.title").await, "SolidAxum 演示系统");
    assert_eq!(
        config_value(&app, "sys.user.initPassword").await,
        "Init@456789"
    );

    // 管理员密码确实换成了新密码:用新密码走 profile 改密(再改一次)
    // 能通过旧密码核对,即证明新密码已生效
    let (status, body) = app
        .put(
            "/system/user/profile/updatePwd",
            json!({ "oldPassword": "New@123456", "newPassword": "Again@123456" }),
            Some(&super_admin()),
        )
        .await;
    assert_ok(status, &body);
}

#[tokio::test]
async fn complete_rejects_wrong_old_password_and_writes_nothing() {
    let _title_guard = TITLE_CACHE_LOCK.lock().await;
    let app = TestApp::new().await;
    seed_admin_password(&app, "Admin@123456").await;

    let (status, body) = app
        .post(
            COMPLETE_URI,
            complete_bo("Wrong@123456", "New@123456"),
            Some(&super_admin()),
        )
        .await;

    assert_ne!(code(&body), 200, "旧密码错误必须整体失败: {body}");
    let _ = status;

    // 未落任何配置:状态仍 false,sys.title 仍是种子值。
    // (configKey 读走 Redis 缓存且 TestApp 间共享,先 refreshCache
    // 拿 DB 真相,否则会读到并行测试写入的缓存值)
    let (_, body) = app.get(STATUS_URI, Some(&super_admin())).await;
    assert_eq!(body["data"]["completed"], json!(false));
    app.delete("/system/config/refreshCache", Some(&super_admin()))
        .await;
    assert_eq!(config_value(&app, "sys.title").await, "SolidAxum");
}

#[tokio::test]
async fn complete_rejects_short_passwords() {
    // 密码 6-50 的入参契约:过短的密码在改密前就被校验拦下,零写入
    let app = TestApp::new().await;
    seed_admin_password(&app, "Admin@123456").await;

    let (_, body) = app
        .post(
            COMPLETE_URI,
            json!({
                "systemName": "SolidAxum 演示系统",
                "adminOldPassword": "Admin@123456",
                "adminNewPassword": "abc",
                "userInitPassword": "Init@456789",
            }),
            Some(&super_admin()),
        )
        .await;
    assert_ne!(code(&body), 200, "短密码必须被拒绝: {body}");

    let (_, body) = app
        .post(
            COMPLETE_URI,
            json!({
                "systemName": "SolidAxum 演示系统",
                "adminOldPassword": "Admin@123456",
                "adminNewPassword": "New@123456",
                "userInitPassword": "123",
            }),
            Some(&super_admin()),
        )
        .await;
    assert_ne!(code(&body), 200, "短默认密码必须被拒绝: {body}");

    // 状态仍未完成,可重试
    let (_, body) = app.get(STATUS_URI, Some(&super_admin())).await;
    assert_eq!(body["data"]["completed"], json!(false));
}

#[tokio::test]
async fn complete_is_one_shot() {
    // 已初始化的系统再调 complete = 重置 admin 密码的后门,必须关死
    let app = TestApp::new().await;
    seed_admin_password(&app, "Admin@123456").await;

    let (status, body) = app
        .post(
            COMPLETE_URI,
            complete_bo("Admin@123456", "New@123456"),
            Some(&super_admin()),
        )
        .await;
    assert_ok(status, &body);

    let (_, body) = app
        .post(
            COMPLETE_URI,
            complete_bo("New@123456", "Evil@123456"),
            Some(&super_admin()),
        )
        .await;
    assert_ne!(code(&body), 200, "重复初始化必须被拒绝: {body}");
}

#[tokio::test]
async fn complete_requires_super_admin() {
    let app = TestApp::new().await;
    let nobody = user_with_perms(&["system:user:list"]);

    let (_, body) = app
        .post(
            COMPLETE_URI,
            complete_bo("Admin@123456", "New@123456"),
            Some(&nobody),
        )
        .await;

    assert_ne!(code(&body), 200, "普通用户不可执行初始化: {body}");
}

#[tokio::test]
async fn old_system_rejects_complete() {
    // 键不存在=已完成 → complete 直接拒绝,不产生任何写入
    let app = TestApp::new().await;
    let ds = app.state.infra.data_source();
    app_data::testkit::execute_raw(
        &ds,
        "DELETE FROM sys_config WHERE config_key = 'sys.setup.completed'",
    )
    .await
    .expect("删 setup 键失败");
    seed_admin_password(&app, "Admin@123456").await;

    let (_, body) = app
        .post(
            COMPLETE_URI,
            complete_bo("Admin@123456", "New@123456"),
            Some(&super_admin()),
        )
        .await;

    assert_ne!(code(&body), 200, "老系统不可被重复初始化: {body}");
}
