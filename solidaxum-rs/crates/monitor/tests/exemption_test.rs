//! 全豁免判例:受限 resolver 下照见全量 —— SysOperLogMapper/
//! SysLoginInfoMapper 零 @DataPermission,menu/client 判例同款。
use app_core::vo::page::PageQuery;
use app_data::{
    DataSource, Principal, ScopeFilter,
    sea_orm::ActiveValue::Set,
    testing::{FixedResolver, sqlite_memory_empty},
};
use monitor::{
    logininfo::{
        model::{LoginInfo, LoginInfoQuery},
        repo::LoginInfoRepository,
    },
    operlog::{
        model::{BusinessType, OperLog, OperLogQuery},
        repo::OperLogRepository,
    },
};

async fn ds_exempt_visible() -> DataSource {
    let ds = sqlite_memory_empty().await;
    ds.create_table(monitor::logininfo::repo::Entity)
        .await
        .unwrap();
    ds.create_table(monitor::operlog::repo::Entity)
        .await
        .unwrap();
    // 种 3 行登录日志(oper_name/归属各不同,豁免下都应可见)
    let ctx = ds.system_context(); // 种子面:免盖章(insert 无审计列可盖)
    for i in 1..=3i64 {
        let info = LoginInfo::new(
            format!("user{i}"),
            "0".to_string(),
            "登录成功".to_string(),
            format!("10.0.0.{i}"), // i 区分 ipaddr
            "Chrome".to_string(),
            "Windows 10".to_string(),
            Some("pc".to_string()),
            Some("pc".to_string()),
        );
        let mut a: monitor::logininfo::repo::ActiveModel = info.into();
        a.info_id = Set(i);
        ctx.insert(a).await.unwrap();
    }
    // 种 3 行操作日志(user_id/dept_id 是数据列不是归属列,豁免下都应可见)
    for i in 1..=3i64 {
        let log = OperLog::new(
            "测试模块".to_string(),
            BusinessType::INSERT,
            1,
            format!("user{i}"), // oper_name 各不同
            "/test".to_string(),
            "POST".to_string(),
            format!("10.0.0.{i}"),
            "{}".to_string(),
            "{}".to_string(),
            0,
            String::new(),
            10,
            Some(i),       // user_id 各不同(数据列)
            Some(100 + i), // dept_id 各不同(数据列)
            Some("技术部".to_string()),
            Some("pc".to_string()),
            Some("pc".to_string()),
            Some("Chrome".to_string()),
            Some("Windows".to_string()),
        );
        let mut a: monitor::operlog::repo::ActiveModel = log.into();
        a.oper_id = Set(i);
        ctx.insert(a).await.unwrap();
    }
    ds
}

/// LoginInfoQuery 无 Default,按 operlog_repo_test.rs 的 empty_query 形态构造。
fn empty_login_query() -> LoginInfoQuery {
    LoginInfoQuery {
        page: PageQuery {
            page_num: Some(1),
            page_size: Some(10),
            order_by_column: None,
            is_asc: None,
        },
        user_name: None,
        status: None,
        ipaddr: None,
        begin_time: None,
        end_time: None,
    }
}

fn empty_oper_query() -> OperLogQuery {
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
async fn logininfo_exempt_read_sees_all_rows_under_limited_scope() {
    // system 判例惯用法:先建库种子,再 with_resolver,再包 Arc(config_repo_test:52)
    let ds = std::sync::Arc::new(ds_exempt_visible().await.with_resolver(
        std::sync::Arc::new(FixedResolver(ScopeFilter::SelfOnly(7))),
    ));
    let repo = LoginInfoRepository::new(ds.clone());
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: None,
    });
    let page = repo
        .select_page(&ctx, &empty_login_query(), 1, 10, "login_time desc")
        .await
        .unwrap();
    assert_eq!(page.total, 3); // SelfOnly(7) 下仍全量 —— 豁免必须显式
}

#[tokio::test]
async fn operlog_exempt_read_sees_all_rows_under_limited_scope() {
    let ds = std::sync::Arc::new(ds_exempt_visible().await.with_resolver(
        std::sync::Arc::new(FixedResolver(ScopeFilter::SelfOnly(7))),
    ));
    let repo = OperLogRepository::new(ds.clone());
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: None,
    });
    let page = repo
        .select_page(&ctx, &empty_oper_query(), 1, 10, "oper_time desc")
        .await
        .unwrap();
    assert_eq!(page.total, 3); // sys_oper_log 的 user_id/dept_id 是数据列,非归属列

    let list = repo.select_list(&ctx, &empty_oper_query()).await.unwrap();
    assert_eq!(list.len(), 3, "导出全量读同豁免");
}
