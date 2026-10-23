//! JobService —— app-data 漏斗内执行。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档;返回保持 `anyhow::Result`
//! (`DataError` 经 `?` 自动转 anyhow;handler 端 downcast 回 DataError 映
//! HTTP 语义)。**豁免裁定**:Java ruoyi-job 只有 SnailJob 执行器、无对应
//! Mapper —— 调度配置非行级数据,读方法显式 `without_row_scope()`
//! (policy 落 CONSTRAINED=false,见 `crate::policy` 头注)。
//!
//! **手工盖章保留**:sys_job 审计列是 `Option<String>` /
//! `Option<NaiveDateTime>`,与漏斗盖章签名(i64 / DateTimeUtc)类型不
//! 匹配(policy audit=None)→ create/update/change_status/delete 保留
//! `operator: &str` 手工盖 create_by/update_by/create_time/update_time。
//! 软删存活过滤由漏斗施加(policy soft_delete=(DelFlag,"0"))。
use std::sync::{Arc, Mutex};

use app_data::{
    DataContext, DataSource,
    sea_orm::{
        ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
    },
};
use chrono::Utc;
use snowflake::SnowflakeIdGenerator;

use crate::entity::sys_job::{self, ActiveModel, Entity as SysJob};

pub struct JobService {
    /// 装配期快照(豁免读不解析 resolver,降级快照无影响;
    /// monitor/logininfo repo 同款处置)。
    ds: Arc<DataSource>,
    id_gen: Mutex<SnowflakeIdGenerator>,
}

impl Clone for JobService {
    fn clone(&self) -> Self {
        Self {
            ds: self.ds.clone(),
            id_gen: Mutex::new(SnowflakeIdGenerator::new(1, 1)),
        }
    }
}

impl JobService {
    /// 装配入口(app.rs/testkit.rs 涟漪最小化)。
    pub fn new(infra: &app_core::infra::Infrastructure) -> Self {
        Self::new_ds(infra.data_source())
    }

    /// 测试/装配直供入口:快照 `Arc<DataSource>`。
    pub fn new_ds(ds: Arc<DataSource>) -> Self {
        Self {
            ds,
            id_gen: Mutex::new(SnowflakeIdGenerator::new(1, 1)),
        }
    }

    fn next_id(&self) -> i64 {
        self.id_gen.lock().unwrap().real_time_generate()
    }

    pub async fn page(
        &self,
        ctx: &DataContext<'_>,
        page: u64,
        page_size: u64,
        job_name: Option<&str>,
        status: Option<&str>,
    ) -> anyhow::Result<app_data::Page<sys_job::Model>> {
        // 软删存活过滤由漏斗施加
        let mut q = SysJob::find();
        if let Some(name) = job_name
            && !name.is_empty()
        {
            q = q.filter(sys_job::Column::JobName.contains(name));
        }
        if let Some(s) = status
            && !s.is_empty()
        {
            q = q.filter(sys_job::Column::Status.eq(s));
        }
        let p = ctx
            .select(q.order_by_desc(sys_job::Column::JobId))
            .without_row_scope()
            .page(page as i64, page_size as i64)
            .await?;
        Ok(p)
    }

    /// 已删/不存在 → `DataError::NotFound`(软删存活过滤由漏斗施加,
    /// 已删任务不可查回)。漏斗 `one()` 返回 `None` 处由本 service
    /// (漏斗 API 的调用方)收口为 NotFound;handler 经 downcast 映
    /// `AppError::NotFound`。
    pub async fn get(
        &self,
        ctx: &DataContext<'_>,
        job_id: i64,
    ) -> anyhow::Result<sys_job::Model> {
        let job = ctx
            .select(SysJob::find_by_id(job_id))
            .without_row_scope()
            .one()
            .await?
            .ok_or(app_data::DataError::NotFound)?;
        Ok(job)
    }

    /// 手工盖审计五列(**漏斗不盖**,policy audit=None)。
    pub async fn create(
        &self,
        ctx: &DataContext<'_>,
        mut model: ActiveModel,
        operator: &str,
    ) -> anyhow::Result<sys_job::Model> {
        let now = Utc::now().naive_utc();
        model.job_id = Set(self.next_id());
        model.create_by = Set(Some(operator.to_string()));
        model.update_by = Set(Some(operator.to_string()));
        model.create_time = Set(Some(now));
        model.update_time = Set(Some(now));
        model.del_flag = Set("0".to_string());
        Ok(ctx.insert(model).await?)
    }

    /// `ctx.update` 的 WHERE = 主键 + 软删存活(漏斗施加),0 行 →
    /// `DataError::NotFound`。手工盖 update_by/update_time 保留。
    pub async fn update(
        &self,
        ctx: &DataContext<'_>,
        mut model: ActiveModel,
        operator: &str,
    ) -> anyhow::Result<sys_job::Model> {
        model.update_by = Set(Some(operator.to_string()));
        model.update_time = Set(Some(Utc::now().naive_utc()));
        Ok(ctx.update(model).await?)
    }

    /// 批量软删。**不能走 `delete_where`**(带软删列 fail-closed),漏斗也
    /// 无"按主键软删"原语 → social_repo 软删惯用法:逐条 ActiveModel 置
    /// del_flag='2' + `tx.update`(WHERE = 主键 + 软删存活,0 行/已删 →
    /// `DataError::NotFound` 上抛,handler 映射 R.fail,Java
    /// `deleteJobByIds` rows<1 → fail 等价)。包事务保原子性
    /// (user_service 先例)。`operator` 盖 update_by(对齐 Java:
    /// MetaObjectHandler 在 logic delete 时回填 update_by)。
    pub async fn delete(
        &self,
        ctx: &DataContext<'_>,
        job_ids: &[i64],
        operator: &str,
    ) -> anyhow::Result<()> {
        let now = Utc::now().naive_utc();
        // owned 行先进闭包(HRTB 不收借用切片,user_service 同注)
        let rows: Vec<sys_job::ActiveModel> = job_ids
            .iter()
            .map(|&id| sys_job::ActiveModel {
                job_id: Set(id),
                del_flag: Set("2".to_string()),
                update_by: Set(Some(operator.to_string())),
                update_time: Set(Some(now)),
                ..Default::default()
            })
            .collect();
        ctx.transaction(|tx| {
            Box::pin(async move {
                for m in rows {
                    tx.update(m).await?; // 0 行/已删 → NotFound
                }
                Ok(())
            })
        })
        .await?;
        Ok(())
    }

    /// `ctx.update`:不存在/已删 → NotFound。
    pub async fn change_status(
        &self,
        ctx: &DataContext<'_>,
        job_id: i64,
        status: &str,
        operator: &str,
    ) -> anyhow::Result<()> {
        let model = ActiveModel {
            job_id: Set(job_id),
            status: Set(status.to_string()),
            update_by: Set(Some(operator.to_string())),
            update_time: Set(Some(Utc::now().naive_utc())),
            ..Default::default()
        };
        ctx.update(model).await?;
        Ok(())
    }
}
