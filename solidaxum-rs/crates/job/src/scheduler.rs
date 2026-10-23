//! SchedulerService —— app-data 漏斗内执行。
//!
//! 调度器是**后台执行面**(cron 触发/启动加载,无登录用户),数据访问
//! 统一经 `Arc<DataSource>` + `system_context`(审计人=系统,设计 7.3;
//! 生产消费第 3 处,前 2 处在 monitor)。sys_job/sys_job_log 按豁免锚定
//! (policy CONSTRAINED=false,见 `crate::policy` 头注):读显式
//! `without_row_scope()`,软删存活过滤由漏斗施加;sys_job_log 无审计列
//! (policy audit=None),system_context 与用户 ctx 行为逐点一致。
use std::{collections::HashMap, sync::Arc};

use app_data::{
    DataSource,
    sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter},
};
use chrono::Utc;
use deadpool_redis::Pool as RedisPool;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;

use crate::{
    entity::{
        sys_job::{self, Entity as SysJob},
        sys_job_log,
    },
    handler_trait::JobRegistry,
};

pub fn redis_lock_key(job_id: i64) -> String {
    format!("job:lock:{}", job_id)
}

#[derive(Debug, PartialEq)]
pub enum MisfirePolicy {
    Default,
    RunOnce,
    Abandon,
}

impl From<i8> for MisfirePolicy {
    fn from(v: i8) -> Self {
        match v {
            2 => MisfirePolicy::RunOnce,
            3 => MisfirePolicy::Abandon,
            _ => MisfirePolicy::Default,
        }
    }
}

pub struct SchedulerService {
    scheduler: Arc<Mutex<JobScheduler>>,
    ds: Arc<DataSource>,
    redis_pool: Arc<RedisPool>,
    registry: Arc<JobRegistry>,
    job_uuids: Arc<Mutex<HashMap<i64, Uuid>>>,
}

impl SchedulerService {
    /// 装配入口:`ds` 为 `infra.data_source()` 的同一份 Arc(装配后带
    /// resolver;调度器全走 system_context 不解析 resolver,语义不受
    /// 快照影响)。第三参起签名不变。
    pub async fn new(
        ds: Arc<DataSource>,
        redis_pool: Arc<RedisPool>,
        registry: Arc<JobRegistry>,
    ) -> anyhow::Result<Arc<Self>> {
        let scheduler = JobScheduler::new().await?;
        Ok(Arc::new(Self {
            scheduler: Arc::new(Mutex::new(scheduler)),
            ds,
            redis_pool,
            registry,
            job_uuids: Arc::new(Mutex::new(HashMap::new())),
        }))
    }

    /// 启动时从 DB 加载所有状态='1'的任务
    pub async fn init(self: &Arc<Self>) -> anyhow::Result<()> {
        // 后台启动面:无登录用户,system_context(审计人=系统);豁免读
        // 显式 without_row_scope。软删存活过滤由漏斗施加(policy
        // soft_delete=(DelFlag,"0"))。
        let ctx = self.ds.system_context();
        let jobs = ctx
            .select(SysJob::find().filter(sys_job::Column::Status.eq("1")))
            .without_row_scope()
            .all()
            .await?;

        for job in jobs {
            if let Err(e) = self.schedule_job(&job).await {
                tracing::warn!(job_id = job.job_id, error = %e, "跳过调度失败的任务");
            }
        }
        self.scheduler.lock().await.start().await?;
        tracing::info!("定时任务调度器已启动");
        Ok(())
    }

    /// 将一个任务加入 tokio-cron-scheduler
    pub async fn schedule_job(
        self: &Arc<Self>,
        model: &sys_job::Model,
    ) -> anyhow::Result<()> {
        let handler = match self.registry.get(&model.invoke_target) {
            Some(h) => h,
            None => {
                tracing::warn!(target = %model.invoke_target, "未注册的 JobHandler，跳过");
                return Ok(());
            }
        };

        let job_id = model.job_id;
        let params = model.job_params.clone().unwrap_or_default();
        let cron = model.cron_expression.clone();
        let ds = Arc::clone(&self.ds);
        let redis_pool = Arc::clone(&self.redis_pool);
        let job_name = model.job_name.clone();
        let job_group = model.job_group.clone();
        let invoke_target = model.invoke_target.clone();
        let concurrent = model.concurrent;

        let cron_job = Job::new_async(cron.as_str(), move |_uuid, _lock| {
            let handler = Arc::clone(&handler);
            let params = params.clone();
            let ds = ds.clone();
            let redis_pool = Arc::clone(&redis_pool);
            let lock_key = redis_lock_key(job_id);
            let job_name = job_name.clone();
            let job_group = job_group.clone();
            let invoke_target = invoke_target.clone();

            Box::pin(async move {
                // 分布式锁（允许并发时跳过锁）
                if concurrent == 0 {
                    let mut conn = match redis_pool.get().await {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::error!("获取 Redis 连接失败: {e}");
                            return;
                        }
                    };
                    let acquired: bool = redis::cmd("SET")
                        .arg(&lock_key)
                        .arg("1")
                        .arg("NX")
                        .arg("EX")
                        .arg(60u64)
                        .query_async::<bool>(&mut *conn)
                        .await
                        .unwrap_or(false);
                    if !acquired {
                        tracing::debug!(job_id, "任务正在其他节点执行，跳过");
                        return;
                    }
                }

                let start = Utc::now().naive_utc();
                let result = handler.execute(&params).await;
                let stop = Utc::now().naive_utc();

                let (status, message, exception) = match &result {
                    Ok(msg) => ("1".to_string(), Some(msg.clone()), None),
                    Err(e) => ("0".to_string(), None, Some(e.clone())),
                };

                let log = sys_job_log::ActiveModel {
                    job_name: Set(job_name.clone()),
                    job_group: Set(job_group.clone()),
                    invoke_target: Set(invoke_target.clone()),
                    job_params: Set(Some(params.clone())),
                    job_message: Set(message),
                    status: Set(status),
                    exception_info: Set(exception),
                    start_time: Set(Some(start)),
                    stop_time: Set(Some(stop)),
                    create_time: Set(Some(Utc::now().naive_utc())),
                    ..Default::default()
                };
                // 后台执行面:无登录用户,system_context(审计人=系统;
                // sys_job_log 无审计列,与用户 ctx 行为逐点一致)。
                let ctx = ds.system_context();
                if let Err(e) = ctx.insert_returning_nothing(log).await {
                    tracing::error!(job_id, "写入执行日志失败: {e}");
                }
            })
        })?;

        let uuid = cron_job.guid();
        self.scheduler.lock().await.add(cron_job).await?;
        self.job_uuids.lock().await.insert(job_id, uuid);
        tracing::info!(job_id, cron = %cron, "任务已调度");
        Ok(())
    }

    /// 从调度器移除任务（暂停/删除时调用）
    pub async fn remove_job(&self, job_id: i64) -> anyhow::Result<()> {
        let uuid = self.job_uuids.lock().await.remove(&job_id);
        if let Some(uuid) = uuid {
            self.scheduler.lock().await.remove(&uuid).await?;
            tracing::info!(job_id, "任务已从调度器移除");
        }
        Ok(())
    }

    /// 立即手动执行一次（不走 cron，直接调用 handler）
    pub async fn run_once(
        self: &Arc<Self>,
        job_id: i64,
    ) -> anyhow::Result<String> {
        // 手动触发同样是执行面(handler 只传 job_id,签名无 principal):
        // system_context,审计人=系统。sys_job 为豁免读(显式
        // without_row_scope;软删存活过滤漏斗照常施加,已删任务视为不
        // 存在),sys_job_log 无审计列 —— 与用户 ctx 行为逐点一致。
        let ctx = self.ds.system_context();
        let model = ctx
            .select(SysJob::find_by_id(job_id))
            .without_row_scope()
            .one()
            .await?
            .ok_or_else(|| anyhow::anyhow!("任务不存在: {}", job_id))?;

        let handler =
            self.registry.get(&model.invoke_target).ok_or_else(|| {
                anyhow::anyhow!("未注册的 handler: {}", model.invoke_target)
            })?;

        let params = model.job_params.unwrap_or_default();
        let start = Utc::now().naive_utc();
        let result = handler.execute(&params).await;
        let stop = Utc::now().naive_utc();

        let (status, message, exception) = match &result {
            Ok(msg) => ("1".to_string(), Some(msg.clone()), None),
            Err(e) => ("0".to_string(), None, Some(e.clone())),
        };

        let log = sys_job_log::ActiveModel {
            job_name: Set(model.job_name),
            job_group: Set(model.job_group),
            invoke_target: Set(model.invoke_target),
            job_params: Set(Some(params)),
            job_message: Set(message),
            status: Set(status),
            exception_info: Set(exception.clone()),
            start_time: Set(Some(start)),
            stop_time: Set(Some(stop)),
            create_time: Set(Some(Utc::now().naive_utc())),
            ..Default::default()
        };
        ctx.insert_returning_nothing(log).await?;

        result.map_err(|e| anyhow::anyhow!(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_use_redis_lock_key() {
        let key = redis_lock_key(42);
        assert_eq!(key, "job:lock:42");
    }

    #[test]
    fn test_misfire_policy_parse() {
        assert_eq!(MisfirePolicy::from(1_i8), MisfirePolicy::Default);
        assert_eq!(MisfirePolicy::from(2_i8), MisfirePolicy::RunOnce);
        assert_eq!(MisfirePolicy::from(3_i8), MisfirePolicy::Abandon);
        assert_eq!(MisfirePolicy::from(99_i8), MisfirePolicy::Default); // unknown → default
    }
}
