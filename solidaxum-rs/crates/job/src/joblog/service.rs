//! JobLogService —— app-data 漏斗内执行。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档;返回保持 `anyhow::Result`(同
//! JobService 头注)。**豁免裁定**:Java ruoyi-job 无 SysJobLogMapper —
//! 执行日志非行级数据,读方法显式 `without_row_scope()`(policy 落
//! CONSTRAINED=false,见 `crate::policy` 头注)。sys_job_log 无审计列、
//! 无软删列(日志表只增不改,Java 锚即物理 delete/clean)→
//! `delete_where` 放行。
use std::sync::Arc;

use app_data::{
    DataContext, DataSource,
    sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder},
};

use crate::entity::sys_job_log::{self, Entity as SysJobLog};

pub struct JobLogService {
    /// 装配期快照(豁免读不解析 resolver,降级快照无影响)。
    ds: Arc<DataSource>,
}

// 手写 Clone(读 ds 字段):derive(Clone) 在 dead_code 分析里不计为
// 字段读取,ds 会被误判死字段(JobService 同款处置)。
impl Clone for JobLogService {
    fn clone(&self) -> Self {
        Self {
            ds: self.ds.clone(),
        }
    }
}
impl JobLogService {
    /// 装配入口(app.rs/testkit.rs 涟漪最小化)。
    pub fn new(infra: &app_core::infra::Infrastructure) -> Self {
        Self::new_ds(infra.data_source())
    }

    /// 测试/装配直供入口:快照 `Arc<DataSource>`。
    pub fn new_ds(ds: Arc<DataSource>) -> Self {
        Self { ds }
    }

    pub async fn page(
        &self,
        ctx: &DataContext<'_>,
        page: u64,
        page_size: u64,
        job_name: Option<&str>,
        status: Option<&str>,
    ) -> anyhow::Result<app_data::Page<sys_job_log::Model>> {
        let mut q = SysJobLog::find();
        if let Some(name) = job_name
            && !name.is_empty()
        {
            q = q.filter(sys_job_log::Column::JobName.contains(name));
        }
        if let Some(s) = status
            && !s.is_empty()
        {
            q = q.filter(sys_job_log::Column::Status.eq(s));
        }
        let p = ctx
            .select(q.order_by_desc(sys_job_log::Column::JobLogId))
            .without_row_scope()
            .page(page as i64, page_size as i64)
            .await?;
        Ok(p)
    }

    /// 豁免读;不存在 → `None`(由调用方 handler 映 NotFound;
    /// sys_job_log 无软删列)。
    pub async fn get(
        &self,
        ctx: &DataContext<'_>,
        job_log_id: i64,
    ) -> anyhow::Result<Option<sys_job_log::Model>> {
        Ok(ctx
            .select(SysJobLog::find_by_id(job_log_id))
            .without_row_scope()
            .one()
            .await?)
    }

    /// 按主键批量物理删(无软删,`delete_where` 放行)。
    pub async fn delete(
        &self,
        ctx: &DataContext<'_>,
        ids: &[i64],
    ) -> anyhow::Result<u64> {
        Ok(ctx
            .delete_where::<sys_job_log::Entity>(
                Condition::all().add(
                    sys_job_log::Column::JobLogId.is_in(ids.iter().copied()),
                ),
            )
            .await?)
    }

    /// 物理清空(Java clean 接口即 TRUNCATE 语义);
    /// `Condition::all()` = 无 WHERE 的全表 DELETE。
    pub async fn clean(&self, ctx: &DataContext<'_>) -> anyhow::Result<u64> {
        Ok(ctx
            .delete_where::<sys_job_log::Entity>(Condition::all())
            .await?)
    }
}
