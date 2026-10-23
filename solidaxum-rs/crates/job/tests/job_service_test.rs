//! 判例:sys_job 软删过滤 + 手工审计(漏斗不盖 String 审计列)+
//! sys_job_log 豁免 + 写路径 NotFound。自有功能锚:Java ruoyi-job
//! 用 SnailJob 无对应 Mapper —— 调度配置/执行日志非行级数据。
use std::sync::Arc;

use app_data::{
    DataSource, Principal, ScopeFilter,
    sea_orm::{ActiveValue::Set, EntityTrait},
    testing::{FixedResolver, sqlite_memory_empty},
};
use job::{
    entity::{sys_job, sys_job_log},
    job::service::JobService,
    joblog::service::JobLogService,
};

async fn setup() -> DataSource {
    let ds = sqlite_memory_empty().await;
    ds.create_table(sys_job::Entity).await.unwrap();
    ds.create_table(sys_job_log::Entity).await.unwrap();
    ds
}

fn sample_job(name: &str) -> sys_job::ActiveModel {
    sys_job::ActiveModel {
        job_name: Set(name.into()),
        job_group: Set("DEFAULT".into()),
        invoke_target: Set("demoTask.noParams".into()),
        cron_expression: Set("0/10 * * * * ?".into()),
        // misfire_policy/concurrent 非空列,缺省 NotSet 会撞 NOT NULL
        // 约束;取 handler 默认值(create_job 的 unwrap_or(1))。
        misfire_policy: Set(1),
        concurrent: Set(1),
        status: Set("1".into()),
        ..Default::default()
    }
}

#[tokio::test]
async fn soft_deleted_job_invisible_and_delete_is_soft() {
    // new_ds 收 Arc<DataSource>(DataSource 非 Clone),先包 Arc —
    // monitor 判例「先建库种子,再(可选)with_resolver,再包 Arc」同款。
    let ds = Arc::new(setup().await);
    let svc = JobService::new_ds(ds.clone());
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: None,
    });
    let created = svc.create(&ctx, sample_job("a"), "alice").await.unwrap();
    assert_eq!(created.create_by.as_deref(), Some("alice")); // 手工审计原样

    // 种一行已软删行(del_flag='2',免盖章种子面)
    let seed_ctx = ds.system_context();
    let mut dead = sample_job("dead");
    dead.job_id = Set(99);
    dead.del_flag = Set("2".into());
    dead.create_by = Set(None);
    seed_ctx.insert_unstamped(dead).await.unwrap();

    // page 只见存活行;get 已删行 → NotFound
    let p = svc.page(&ctx, 1, 10, None, None).await.unwrap();
    let (list, total) = (p.rows, p.total);
    assert_eq!(total, 1);
    assert_eq!(list.len(), 1);
    assert!(svc.get(&ctx, 99).await.is_err());

    // 批删 = 逐条软删:行还在,del_flag='2'(种软删行读取面断言)
    svc.delete(&ctx, &[created.job_id], "alice").await.unwrap();
    let total2 = svc.page(&ctx, 1, 10, None, None).await.unwrap().total;
    assert_eq!(total2, 0);
}

#[tokio::test]
async fn joblog_exempt_read_and_clean() {
    // FixedResolver(SelfOnly(7)) 下 page 照见全量;clean 后 rows_affected=种子数
    let ds = Arc::new(
        setup()
            .await
            .with_resolver(Arc::new(FixedResolver(ScopeFilter::SelfOnly(7)))),
    );
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: None,
    });
    let seed_ctx = ds.system_context();
    for i in 1..=2i64 {
        let mut l = sys_job_log::ActiveModel {
            job_name: Set("a".into()),
            job_group: Set("DEFAULT".into()),
            invoke_target: Set("t".into()),
            status: Set("1".into()),
            ..Default::default()
        };
        l.job_log_id = Set(i);
        seed_ctx.insert_unstamped(l).await.unwrap();
    }
    let svc = JobLogService::new_ds(ds.clone());
    let total = svc.page(&ctx, 1, 10, None, None).await.unwrap().total;
    assert_eq!(total, 2);
    assert_eq!(svc.clean(&ctx).await.unwrap(), 2);
}

#[tokio::test]
async fn notfound_on_missing_or_soft_deleted_write() {
    // 写路径判例:update/change_status/delete 对不存在或已软删行 →
    // Err(NotFound);二次 delete 同一 id → Err。
    let ds = Arc::new(setup().await);
    let svc = JobService::new_ds(ds.clone());
    let ctx = ds.context(Principal {
        user_id: 7,
        dept_id: None,
    });
    let created = svc
        .create(&ctx, sample_job("alive"), "alice")
        .await
        .unwrap();

    // 种一行已软删行(id=99,免盖章种子面,同上一判例)
    let seed_ctx = ds.system_context();
    let mut dead = sample_job("dead");
    dead.job_id = Set(99);
    dead.del_flag = Set("2".into());
    dead.create_by = Set(None);
    seed_ctx.insert_unstamped(dead).await.unwrap();

    // 已软删行:三条写路径均 Err;抽 change_status 钉死正是
    // DataError::NotFound(而非别的错误)
    let mut m = sample_job("x");
    m.job_id = Set(99);
    assert!(svc.update(&ctx, m, "alice").await.is_err());
    let err = svc.change_status(&ctx, 99, "0", "alice").await.unwrap_err();
    assert!(matches!(
        err.downcast_ref::<app_data::DataError>(),
        Some(app_data::DataError::NotFound)
    ));
    assert!(svc.delete(&ctx, &[99], "alice").await.is_err());

    // delete 失败 → 事务整体回滚:已删行未被二次改动
    // (del_flag 仍 '2',update_by 未被盖;经漏斗读不到已删行,
    // 用 system_context + include_deleted 直读)
    let row = seed_ctx
        .select(sys_job::Entity::find_by_id(99))
        .include_deleted()
        .without_row_scope()
        .one()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.del_flag, "2");
    assert_eq!(row.update_by, None);

    // 不存在 id:三条写路径同款 Err
    let mut m = sample_job("y");
    m.job_id = Set(123456);
    assert!(svc.update(&ctx, m, "alice").await.is_err());
    assert!(svc.change_status(&ctx, 123456, "0", "alice").await.is_err());
    assert!(svc.delete(&ctx, &[123456], "alice").await.is_err());

    // 二次 delete 同一 id → Err(首次成功后行已软删,与"已删"同路径)
    svc.delete(&ctx, &[created.job_id], "alice").await.unwrap();
    assert!(svc.delete(&ctx, &[created.job_id], "alice").await.is_err());
}
