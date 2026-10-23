//! monitor 模块测试:operlog / logininfo 纯逻辑(无 Redis/DB)。
//!
//! 覆盖:
//! - business_type_from_method(POST/PUT/DELETE/OTHER 映射)
//! - OperLog::new 字段构造 + status 失败标记
//! - LoginInfo::new 字段构造(成功/失败/登出)
//! - 缓存 INFO 解析(在 cache::handler::tests 中)

use monitor::{
    logininfo::model::LoginInfo,
    operlog::model::{BusinessType, OperLog, business_type_from_method},
};

// ============================================================
// business_type_from_method
// ============================================================

#[test]
fn post_maps_to_insert() {
    assert_eq!(business_type_from_method("POST"), BusinessType::INSERT);
}

#[test]
fn put_maps_to_update() {
    assert_eq!(business_type_from_method("PUT"), BusinessType::UPDATE);
}

#[test]
fn delete_maps_to_delete() {
    assert_eq!(business_type_from_method("DELETE"), BusinessType::DELETE);
}

#[test]
fn get_maps_to_other() {
    assert_eq!(business_type_from_method("GET"), BusinessType::OTHER);
}

#[test]
fn unknown_method_maps_to_other() {
    assert_eq!(business_type_from_method("PATCH"), BusinessType::OTHER);
    assert_eq!(business_type_from_method(""), BusinessType::OTHER);
}

// ============================================================
// OperLog::new
// ============================================================

#[test]
fn oper_log_new_sets_all_fields() {
    let log = OperLog::new(
        "用户管理".to_string(),
        BusinessType::INSERT,
        1, // operator_type
        "admin".to_string(),
        "/system/user".to_string(),
        "POST".to_string(),
        "127.0.0.1".to_string(),
        r#"{"userName":"u1"}"#.to_string(),
        r#"{"code":200}"#.to_string(),
        0, // status success
        String::new(),
        42,
        Some(1),
        Some(100),
        Some("技术部".to_string()),
        Some("pc".to_string()),
        None,
        Some("Chrome".to_string()),
        Some("Windows 10".to_string()),
    );

    assert_eq!(log.title, "用户管理");
    assert_eq!(log.business_type, BusinessType::INSERT);
    assert_eq!(log.oper_name, "admin");
    assert_eq!(log.oper_url, "/system/user");
    assert_eq!(log.request_method, "POST");
    assert_eq!(log.oper_ip, "127.0.0.1");
    assert_eq!(log.status, 0, "status=0 表示成功");
    assert_eq!(log.cost_time, 42);
    assert_eq!(log.user_id, Some(1));
    assert_eq!(log.dept_id, Some(100));
    assert_eq!(log.dept_name.as_deref(), Some("技术部"));
    assert_eq!(log.client_key.as_deref(), Some("pc"));
    assert_eq!(log.browser.as_deref(), Some("Chrome"));
    assert_eq!(log.os.as_deref(), Some("Windows 10"));
    assert!(log.oper_id.is_some(), "应自动生成 oper_id");
    assert!(log.oper_time.is_some(), "应自动设置 oper_time");
}

#[test]
fn oper_log_failed_record_carries_error_msg() {
    let log = OperLog::new(
        "登录".to_string(),
        BusinessType::OTHER,
        1,
        "user".to_string(),
        "/auth/login".to_string(),
        "POST".to_string(),
        "1.2.3.4".to_string(),
        "{}".to_string(),
        "Internal error".to_string(),
        1, // status=1 失败
        "数据库连接失败".to_string(),
        100,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    assert_eq!(log.status, 1, "status=1 表示失败");
    assert_eq!(log.error_msg, "数据库连接失败");
}

// ============================================================
// LoginInfo::new
// ============================================================

#[test]
fn login_info_success_record() {
    let info = LoginInfo::new(
        "admin".to_string(),
        "0".to_string(),
        "登录成功".to_string(),
        "1.2.3.4".to_string(),
        "Chrome".to_string(),
        "Windows 10".to_string(),
        Some("pc".to_string()),
        None,
    );
    assert_eq!(info.user_name, "admin");
    assert_eq!(info.status, "0", "status=0 成功");
    assert_eq!(info.msg, "登录成功");
    assert_eq!(info.ipaddr, "1.2.3.4");
    assert_eq!(info.browser, "Chrome");
    assert_eq!(info.os, "Windows 10");
    assert_eq!(info.client_key.as_deref(), Some("pc"));
    assert!(info.device_type.is_none());
    assert!(info.info_id.is_some());
    assert!(info.login_time.is_some());
}

#[test]
fn login_info_failure_uses_status_one() {
    let info = LoginInfo::new(
        "user".to_string(),
        "1".to_string(),
        "登录失败: 密码错误".to_string(),
        String::new(),
        String::new(),
        String::new(),
        None,
        None,
    );
    assert_eq!(info.status, "1", "status=1 失败");
    assert!(info.ipaddr.is_empty());
    assert!(info.browser.is_empty());
}
