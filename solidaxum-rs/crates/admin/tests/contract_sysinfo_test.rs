//! 系统信息契约测试(GET /system/config/sysInfo)。
//! 锚:原 Java 版系统信息页 —— 顶栏标题/侧栏图标运行时可配。

#![cfg(feature = "test-util")]

use admin::testkit::{TestApp, assert_ok, code, super_admin};
use serde_json::json;

/// sys.title/sys.icon 走真实 Redis 持久缓存,同 binary 内并行用例会互相
/// 回填缓存(自己库里的值覆盖别人的写入)—— 全部触缓存的用例串行化。
/// 跨 binary 无冲突:全测试套件仅本文件读写这两个键(已 grep 确认)。
static SYSINFO_CACHE_LOCK: tokio::sync::Mutex<()> =
    tokio::sync::Mutex::const_new(());

/// 未登录 → 401(sysInfo 仅需登录,无权限注解,但不放行匿名)。
#[tokio::test]
async fn sys_info_requires_login() {
    let app = TestApp::new().await;
    let (status, b) = app.get("/system/config/sysInfo", None).await;
    assert_eq!(status, 401, "未登录应 401: {b}");
}

/// 登录即可读;updateByKey 保存后立即可见。
/// 注意:config 服务走真实 Redis 持久缓存(testkit 已接 config.toml 的
/// Redis),跨测试运行有残留 —— 断言必须先写后读自环绕,不能假设缓存
/// 从零开始(AGENTS「依赖 Redis 缓存的接口暂不在脚手架默认覆盖内」)。
#[tokio::test]
async fn sys_info_read_and_update() {
    let _g = SYSINFO_CACHE_LOCK.lock().await;
    let app = TestApp::new().await;
    let admin = super_admin();

    // 保存(系统信息页走的 updateByKey 两连击)
    let (s, b) = app
        .put(
            "/system/config/updateByKey",
            json!({"configKey": "sys.title", "configValue": "示例公司 | 示例管理系统"}),
            Some(&admin),
        )
        .await;
    assert_ok(s, &b);
    let (s, b) = app
        .put(
            "/system/config/updateByKey",
            json!({"configKey": "sys.icon", "configValue": "http://oss/local/logo.png"}),
            Some(&admin),
        )
        .await;
    assert_ok(s, &b);

    let (s, b) = app.get("/system/config/sysInfo", Some(&admin)).await;
    assert_ok(s, &b);
    assert_eq!(b["data"]["title"], "示例公司 | 示例管理系统");
    assert_eq!(b["data"]["icon"], "http://oss/local/logo.png");
}

/// 普通登录用户(无 system:config 权限)也能读 —— 顶栏展示不设权限门槛。
#[tokio::test]
async fn sys_info_readable_without_config_perm() {
    let _g = SYSINFO_CACHE_LOCK.lock().await;
    let app = TestApp::new().await;
    let user = admin::testkit::user_with_perms(&["system:user:list"]);

    let (s, b) = app.get("/system/config/sysInfo", Some(&user)).await;
    assert_ok(s, &b);
    assert_eq!(code(&b), 200);
}
