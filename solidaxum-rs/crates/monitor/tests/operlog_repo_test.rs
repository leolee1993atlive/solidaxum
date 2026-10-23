mod common;

use app_core::vo::page::PageQuery;
use common::setup_db;
use monitor::operlog::{
    model::{BusinessType, OperLog, OperLogQuery},
    repo::OperLogRepository,
};

fn sample_log(
    oper_id: i64,
    oper_ip: &str,
    client_key: &str,
    browser: &str,
    os: &str,
    device_type: Option<&str>,
) -> OperLog {
    let mut log = OperLog::new(
        "测试模块".to_string(),
        BusinessType::INSERT,
        1,
        "admin".to_string(),
        "/test".to_string(),
        "POST".to_string(),
        oper_ip.to_string(),
        "{}".to_string(),
        "{}".to_string(),
        0,
        String::new(),
        10,
        Some(1),
        Some(100),
        Some("技术部".to_string()),
        Some(client_key.to_string()),
        device_type.map(|s| s.to_string()),
        Some(browser.to_string()),
        Some(os.to_string()),
    );
    log.oper_id = Some(oper_id);
    log
}

fn empty_query() -> OperLogQuery {
    OperLogQuery {
        page: PageQuery {
            page_num: Some(1),
            page_size: Some(10),
            order_by_column: None,
            is_asc: None,
        },
        title: None,
        business_type: None,
        business_types: None,
        status: None,
        oper_name: None,
        oper_ip: None,
        client_key: None,
        browser: None,
        os: None,
        device_type: None,
        begin_time: None,
        end_time: None,
    }
}

#[tokio::test]
async fn test_select_page_filters_by_oper_ip_client_key_browser_os() {
    let ds = std::sync::Arc::new(setup_db().await);
    let repo = OperLogRepository::new(ds.clone());
    let ctx = ds.system_context();

    repo.insert(
        &ctx,
        &sample_log(
            9001,
            "192.168.1.1",
            "pc",
            "Chrome",
            "Windows 10",
            Some("pc"),
        ),
    )
    .await
    .unwrap();
    repo.insert(
        &ctx,
        &sample_log(
            9002,
            "10.0.0.1",
            "mobile",
            "Safari",
            "iOS",
            Some("mobile"),
        ),
    )
    .await
    .unwrap();

    let all = repo
        .select_page(&ctx, &empty_query(), 1, 10, "oper_time desc")
        .await
        .unwrap();
    assert_eq!(all.total, 2, "全量应返回 2 条");

    let mut q = empty_query();
    q.oper_ip = Some("192.168".to_string());
    let data = repo
        .select_page(&ctx, &q, 1, 10, "oper_time desc")
        .await
        .unwrap();
    assert_eq!(data.total, 1, "operIp 模糊过滤应只剩 1 条");

    let mut q = empty_query();
    q.client_key = Some("pc".to_string());
    let data = repo
        .select_page(&ctx, &q, 1, 10, "oper_time desc")
        .await
        .unwrap();
    assert_eq!(data.total, 1, "clientKey 模糊过滤应只剩 1 条");

    let mut q = empty_query();
    q.browser = Some("Chrome".to_string());
    let data = repo
        .select_page(&ctx, &q, 1, 10, "oper_time desc")
        .await
        .unwrap();
    assert_eq!(data.total, 1, "browser 模糊过滤应只剩 1 条");

    let mut q = empty_query();
    q.os = Some("iOS".to_string());
    let data = repo
        .select_page(&ctx, &q, 1, 10, "oper_time desc")
        .await
        .unwrap();
    assert_eq!(data.total, 1, "os 模糊过滤应只剩 1 条");
}

#[tokio::test]
async fn test_select_page_filters_by_device_type() {
    let ds = std::sync::Arc::new(setup_db().await);
    let repo = OperLogRepository::new(ds.clone());
    let ctx = ds.system_context();

    repo.insert(
        &ctx,
        &sample_log(9003, "127.0.0.1", "pc", "Edge", "Windows 11", Some("pc")),
    )
    .await
    .unwrap();
    repo.insert(
        &ctx,
        &sample_log(
            9004,
            "127.0.0.2",
            "app",
            "Firefox",
            "Android",
            Some("mobile"),
        ),
    )
    .await
    .unwrap();

    let all = repo
        .select_page(&ctx, &empty_query(), 1, 10, "oper_time desc")
        .await
        .unwrap();
    assert_eq!(all.total, 2, "全量应返回 2 条");

    let mut q = empty_query();
    q.device_type = Some("mobile".to_string());
    let data = repo
        .select_page(&ctx, &q, 1, 10, "oper_time desc")
        .await
        .unwrap();
    assert_eq!(data.total, 1, "deviceType 模糊过滤应只剩 1 条");
    assert_eq!(data.rows[0].device_type.as_deref(), Some("mobile"));
}
