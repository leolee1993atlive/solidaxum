//! 删/清判例:operlog/logininfo 的 delete_by_ids / clean_all 走
//! `ctx.delete_where`(豁免实体批量物理删,Java 锚见两 repo 头部对照表)。
//! ctx 用 system_context(豁免实体,判例口径同 exemption_test 的种子面:
//! 操作/登录日志无审计列可盖、无范围列可辖)。
mod common;

use app_core::vo::page::PageQuery;
use common::setup_db;
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

/// LoginInfoQuery 无 Default,按 exemption_test 的 empty_login_query 形态构造。
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

/// 种 1 行操作日志(oper_id 指定,形态同 exemption_test 的种子循环)。
fn seeded_oper_log(i: i64) -> OperLog {
    let mut log = OperLog::new(
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
        Some(i),       // user_id(数据列,非归属列)
        Some(100 + i), // dept_id(数据列,非归属列)
        Some("技术部".to_string()),
        Some("pc".to_string()),
        Some("pc".to_string()),
        Some("Chrome".to_string()),
        Some("Windows 10".to_string()),
    );
    log.oper_id = Some(i);
    log
}

/// 种 1 行登录日志(info_id 指定,形态同 exemption_test 的种子循环)。
fn seeded_login_info(i: i64) -> LoginInfo {
    let mut info = LoginInfo::new(
        format!("user{i}"),
        "0".to_string(),
        "登录成功".to_string(),
        format!("10.0.0.{i}"), // i 区分 ipaddr
        "Chrome".to_string(),
        "Windows 10".to_string(),
        Some("pc".to_string()),
        Some("pc".to_string()),
    );
    info.info_id = Some(i);
    info
}

#[tokio::test]
async fn operlog_delete_by_ids_counts_hits_and_deleted_stay_invisible() {
    let ds = std::sync::Arc::new(setup_db().await);
    let repo = OperLogRepository::new(ds.clone());
    let ctx = ds.system_context();
    for i in 1..=3i64 {
        repo.insert(&ctx, &seeded_oper_log(i)).await.unwrap();
    }

    // 命中计数:删 [1,2] → rows_affected=2(Java 锚 deleteOperLogByIds
    // ServiceImpl:131 → deleteByIds,按主键 in 的命中数)
    let n = repo.delete_by_ids(&ctx, &[1, 2]).await.unwrap();
    assert_eq!(n, 2, "命中两行,rows_affected 必须是 2");

    // 删后不可见:分页与导出全量都只剩 oper_id=3(物理删,非软删遮蔽)
    let page = repo
        .select_page(&ctx, &empty_oper_query(), 1, 10, "oper_time desc")
        .await
        .unwrap();
    assert_eq!(page.total, 1, "删后分页只剩 1 行");
    assert_eq!(page.rows[0].oper_id, Some(3));
    let list = repo.select_list(&ctx, &empty_oper_query()).await.unwrap();
    assert_eq!(list.len(), 1, "导出面同口径,删掉的行不得回流");
    assert_eq!(list[0].oper_id, Some(3));
}

#[tokio::test]
async fn logininfo_clean_all_wipes_every_row_and_reports_seed_count() {
    let ds = std::sync::Arc::new(setup_db().await);
    let repo = LoginInfoRepository::new(ds.clone());
    let ctx = ds.system_context();
    for i in 1..=3i64 {
        repo.insert(&ctx, &seeded_login_info(i)).await.unwrap();
    }

    // 全清:rows_affected=种子数(Java 锚 cleanLoginInfo
    // ServiceImpl:186 物理清空;Condition::all() = 无 WHERE 全表 DELETE)
    let n = repo.clean_all(&ctx).await.unwrap();
    assert_eq!(n, 3, "全表清空,rows_affected 必须等于种子数");

    // 删后为空:分页与导出全量双面验证
    let page = repo
        .select_page(&ctx, &empty_login_query(), 1, 10, "login_time desc")
        .await
        .unwrap();
    assert_eq!(page.total, 0, "清空后分页 total 必须为 0");
    assert!(page.rows.is_empty());
    let list = repo.select_list(&ctx, &empty_login_query()).await.unwrap();
    assert!(list.is_empty(), "清空后导出面必须为空");
}

#[tokio::test]
async fn delete_by_ids_with_unknown_ids_returns_silent_zero() {
    let ds = std::sync::Arc::new(setup_db().await);
    let repo = LoginInfoRepository::new(ds.clone());
    let ctx = ds.system_context();
    for i in 1..=3i64 {
        repo.insert(&ctx, &seeded_login_info(i)).await.unwrap();
    }

    // 删不存在的 ids → Ok(0) 静默(对齐 Java deleteByIds:无命中不报错;
    // operlog/logininfo 两 repo 同用 ctx.delete_where 原语,口径一致)
    let n = repo.delete_by_ids(&ctx, &[9100, 9101]).await.unwrap();
    assert_eq!(n, 0, "未命中任何主键必须 Ok(0),不得报错");

    // 静默零删除不得动存量
    let page = repo
        .select_page(&ctx, &empty_login_query(), 1, 10, "login_time desc")
        .await
        .unwrap();
    assert_eq!(page.total, 3, "零命中删除后存量必须原样 3 行");
}
