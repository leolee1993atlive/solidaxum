//! notice 服务 —— 编排层。
//!
//! ctx 由 handler 请求期构造(`data_source().context(Principal::..)`)并
//! 透传;notice 是全豁免模块(SysNoticeMapper 无 @DataPermission),
//! 读在 repo 内显式 without_row_scope,写由漏斗盖审计。

use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
    middleware::extractors::CurrentUser,
    vo::page::{TableDataInfo, build_order_by, normalize_paging},
};
use app_data::DataContext;

use crate::notice::{
    model::notice::{
        Notice, NoticeCreateBo, NoticeListVo, NoticePageQuery, NoticeUpdateBo,
    },
    repository::notice_repo::NoticeRepository,
};

#[derive(Clone)]
pub struct NoticeService {
    notice_repo: NoticeRepository,
}

impl NoticeService {
    /// 装配期只取 DataSource 快照:全部方法只**转发** ctx(豁免读不解析
    /// resolver,降级快照无影响)。
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            notice_repo: NoticeRepository::new(infra.data_source()),
        }
    }

    #[tracing::instrument(skip(self, ctx), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
    pub async fn page(
        &self,
        ctx: &DataContext<'_>,
        query: NoticePageQuery,
    ) -> AppResult<TableDataInfo<NoticeListVo>> {
        let (current, size, _offset) = normalize_paging(&query.page);
        let allowed = [
            ("noticeId", "notice_id"),
            ("noticeTitle", "notice_title"),
            ("noticeType", "notice_type"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "create_time desc".to_string());

        let page_result = self
            .notice_repo
            .select_page(ctx, &query, current as u64, size as u64, &order_by)
            .await?;
        if page_result.total <= 0 {
            return Ok(TableDataInfo::empty());
        }
        tracing::info!(
            current,
            size,
            total = page_result.total,
            "通知公告分页查询成功"
        );
        Ok(page_result)
    }

    #[tracing::instrument(skip(self, ctx), fields(notice_id = id))]
    pub async fn get_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<Notice> {
        let notice =
            self.notice_repo.select_by_id(ctx, &id).await?.ok_or_else(
                || AppError::NotFound(format!("通知公告不存在 (id={})", id)),
            )?;
        Ok(notice)
    }

    #[tracing::instrument(skip(self, ctx, bo, current_user), fields(operator = %current_user.0.username))]
    pub async fn insert_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: NoticeCreateBo,
        current_user: &CurrentUser,
    ) -> AppResult<Notice> {
        let notice = Notice::from_create_bo(bo, current_user);
        // 审计五列由漏斗按 ctx principal 盖章
        self.notice_repo.insert(ctx, &notice).await?;
        tracing::info!(
            notice_title = ?notice.notice_title,
            operator = %current_user.0.username,
            "创建通知公告成功"
        );
        Ok(notice)
    }

    #[tracing::instrument(skip(self, ctx, bo, current_user), fields(operator = %current_user.0.username, notice_id = ?bo.notice_id))]
    pub async fn update_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: NoticeUpdateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        let notice_id = bo.notice_id.ok_or_else(|| {
            AppError::BadRequest("更新通知公告失败: 通知公告ID为空".to_string())
        })?;
        let mut notice = self
            .notice_repo
            .select_by_id(ctx, &notice_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "更新通知公告失败: 通知公告不存在 (id={})",
                    notice_id
                ))
            })?;

        notice.from_update_bo(bo, current_user);
        // update_* 由漏斗按 principal 盖章;0 行 → NotFound
        self.notice_repo
            .update_by_id(ctx, &notice, &notice_id)
            .await?;
        tracing::info!(
            notice_id = notice_id,
            operator = %current_user.0.username,
            "更新通知公告成功"
        );
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx), fields(notice_id = id))]
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<()> {
        self.notice_repo.delete_by_id(ctx, &id).await?;
        tracing::info!(notice_id = id, "删除通知公告成功");
        Ok(())
    }
}
