//! operlog 服务 —— app-data 漏斗内的编排层。
//!
//! ctx 由 handler 请求期构造并透传(全豁免读不解析 resolver,降级快照
//! 无影响);oper_log 中间件的后台异步落库路径经
//! [`Self::system_context`] 桥接(实体无审计列,与用户 ctx 行为逐点一致)。

use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
    util::excel::guard_export_rows,
    vo::page::{TableDataInfo, build_order_by, normalize_paging},
};
use app_data::DataContext;

use super::{
    model::{OperLog, OperLogQuery},
    repo::OperLogRepository,
};

#[derive(Clone)]
pub struct OperLogService {
    repo: OperLogRepository,
}

impl OperLogService {
    /// 装配期只取 DataSource 快照:全部方法只**转发** ctx(豁免读不解析
    /// resolver,降级快照无影响)。
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            repo: OperLogRepository::new(infra.data_source()),
        }
    }

    /// 无会话桥(借 repo 的):oper_log 中间件在响应成形后 tokio::spawn
    /// 后台落库,该执行面没有请求身份。sys_oper_log 无审计列/软删列/
    /// 范围列,与用户 ctx 逐点一致。
    pub fn system_context(&self) -> DataContext<'_> {
        self.repo.system_context()
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn record(
        &self,
        ctx: &DataContext<'_>,
        log: &OperLog,
    ) -> AppResult<()> {
        self.repo.insert(ctx, log).await.map_err(|e| {
            AppError::Internal(format!("记录操作日志失败: {}", e))
        })?;
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn page(
        &self,
        ctx: &DataContext<'_>,
        query: OperLogQuery,
    ) -> AppResult<TableDataInfo<OperLog>> {
        let (current, size, _offset) = normalize_paging(&query.page);
        let allowed = [
            ("operId", "oper_id"),
            ("title", "title"),
            ("businessType", "business_type"),
            ("requestMethod", "request_method"),
            ("operName", "oper_name"),
            ("operUrl", "oper_url"),
            ("operIp", "oper_ip"),
            ("status", "status"),
            ("operTime", "oper_time"),
            ("costTime", "cost_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "oper_time desc".to_string());

        let page_result = self
            .repo
            .select_page(ctx, &query, current as u64, size as u64, &order_by)
            .await?;
        tracing::info!(
            current,
            size,
            total = page_result.total,
            "操作日志分页查询成功"
        );
        Ok(page_result)
    }

    #[tracing::instrument(skip(self, ctx))]
    /// 导出用全量查询。操作日志无上限增长,取数前先 COUNT 拦一道 ——
    /// 否则「不带筛选条件导出」会把整张表拉进内存再抛错。
    pub async fn list_all(
        &self,
        ctx: &DataContext<'_>,
        query: OperLogQuery,
    ) -> AppResult<Vec<OperLog>> {
        let total = self.repo.count_by_query(ctx, &query).await?;
        guard_export_rows(total as usize)?;
        let list = self.repo.select_list(ctx, &query).await?;
        Ok(list)
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn delete_by_ids(
        &self,
        ctx: &DataContext<'_>,
        ids: Vec<i64>,
    ) -> AppResult<u64> {
        if ids.is_empty() {
            return Err(AppError::BadRequest("操作日志ID不能为空".to_string()));
        }
        let count = self.repo.delete_by_ids(ctx, &ids).await?;
        tracing::info!(ids = ?ids, count, "删除操作日志成功");
        Ok(count)
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn clean_all(&self, ctx: &DataContext<'_>) -> AppResult<u64> {
        let count = self.repo.clean_all(ctx).await?;
        tracing::info!(count, "清空操作日志成功");
        Ok(count)
    }
}
