//! 集成测试：验证调度器基础语义（SQLite in-memory 经 app-data 漏斗;
//! 种子与断言走 `system_context` —— 与调度器本身的后台执行面口径
//! 一致;monitor/tests/common + job_service_test 同款脚手架）。

use std::sync::Arc;

use app_data::{
    DataContext, DataSource,
    sea_orm::{ActiveValue::Set, EntityTrait},
    testing::sqlite_memory_empty,
};
use job::{
    entity::{sys_job, sys_job_log},
    handler_trait::{JobHandler, JobRegistry},
    scheduler::SchedulerService,
};

/// 实体衍生 DDL 建 sys_job + sys_job_log 两张表,返回 `DataSource`。
async fn setup_db() -> DataSource {
    let ds = sqlite_memory_empty().await;
    ds.create_table(sys_job::Entity).await.unwrap();
    ds.create_table(sys_job_log::Entity).await.unwrap();
    ds
}

/// 种子/断言共用的系统面句柄(后台执行面口径:无登录用户,审计人=系统;
/// 本测试两实体 policy audit=None,与用户 ctx 逐点一致)。
fn sys_ctx(ds: &DataSource) -> DataContext<'_> {
    ds.system_context()
}

// 注意：由于 SchedulerService::new() 需要真实的 RedisPool，
// 我们测试 run_once() 的执行日志写入路径（不走锁，因为锁只在 schedule_job 的 concurrent==0 时使用）

struct PanicHandler;
#[async_trait::async_trait]
impl JobHandler for PanicHandler {
    async fn execute(&self, _params: &str) -> Result<String, String> {
        Err("intentional failure".to_string())
    }
}

struct OkHandler;
#[async_trait::async_trait]
impl JobHandler for OkHandler {
    async fn execute(&self, params: &str) -> Result<String, String> {
        Ok(format!("done:{}", params))
    }
}

/// 构建最简 RedisPool（不会实际连接，run_once 不用锁）——
/// 故意错端口,因为 run_once 路径不使用 Redis。
fn noop_redis_pool() -> Arc<deadpool_redis::Pool> {
    let redis_cfg =
        deadpool_redis::Config::from_url("redis://127.0.0.1:6399/0");
    Arc::new(
        redis_cfg
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .unwrap(),
    )
}

// run_once 需要 DB 中有任务记录，此测试插入记录再手动触发
#[tokio::test]
async fn test_run_once_success_writes_log() {
    use chrono::Utc;

    // SchedulerService::new 收 Arc<DataSource>(DataSource 非 Clone),
    // 先包 Arc —— job_service_test 同款。
    let ds = Arc::new(setup_db().await);

    // 种子:免盖章插入(sys_job 审计列类型与漏斗签名不匹配,policy
    // audit=None —— 种子面 insert_unstamped 惯用法,job_service_test 同款)。
    let job = sys_job::ActiveModel {
        job_id: Set(1_i64),
        job_name: Set("TestJob".to_string()),
        job_group: Set("DEFAULT".to_string()),
        invoke_target: Set("OkJob".to_string()),
        cron_expression: Set("0 * * * * *".to_string()),
        job_params: Set(Some("hello".to_string())),
        misfire_policy: Set(1),
        concurrent: Set(1),
        status: Set("1".to_string()),
        del_flag: Set("0".to_string()),
        create_time: Set(Some(Utc::now().naive_utc())),
        ..Default::default()
    };
    sys_ctx(&ds).insert_unstamped(job).await.unwrap();

    let mut registry = JobRegistry::new();
    registry.register("OkJob", Arc::new(OkHandler));
    let registry = Arc::new(registry);

    let scheduler =
        SchedulerService::new(ds.clone(), noop_redis_pool(), registry)
            .await
            .unwrap();

    let result = scheduler.run_once(1).await;
    assert!(result.is_ok(), "run_once 应成功");
    assert_eq!(result.unwrap(), "done:hello");

    // 验证日志写入(system_context + 豁免读)
    let logs = sys_ctx(&ds)
        .select(sys_job_log::Entity::find())
        .without_row_scope()
        .all()
        .await
        .unwrap();
    assert_eq!(logs.len(), 1, "应写入一条执行日志");
    assert_eq!(logs[0].status, "1", "日志状态应为成功");
    assert_eq!(logs[0].job_params.as_deref(), Some("hello"));
}

#[tokio::test]
async fn test_run_once_failure_writes_log() {
    use chrono::Utc;

    let ds = Arc::new(setup_db().await);

    let job = sys_job::ActiveModel {
        job_id: Set(2_i64),
        job_name: Set("FailJob".to_string()),
        job_group: Set("DEFAULT".to_string()),
        invoke_target: Set("PanicJob".to_string()),
        cron_expression: Set("0 * * * * *".to_string()),
        job_params: Set(None),
        misfire_policy: Set(1),
        concurrent: Set(1),
        status: Set("1".to_string()),
        del_flag: Set("0".to_string()),
        create_time: Set(Some(Utc::now().naive_utc())),
        ..Default::default()
    };
    sys_ctx(&ds).insert_unstamped(job).await.unwrap();

    let mut registry = JobRegistry::new();
    registry.register("PanicJob", Arc::new(PanicHandler));
    let registry = Arc::new(registry);

    let scheduler =
        SchedulerService::new(ds.clone(), noop_redis_pool(), registry)
            .await
            .unwrap();

    let result = scheduler.run_once(2).await;
    assert!(result.is_err(), "run_once 应返回错误");

    let logs = sys_ctx(&ds)
        .select(sys_job_log::Entity::find())
        .without_row_scope()
        .all()
        .await
        .unwrap();
    assert_eq!(logs.len(), 1, "失败时也应写入日志");
    assert_eq!(logs[0].status, "0", "日志状态应为失败");
    assert!(logs[0].exception_info.is_some(), "应记录异常信息");
}
