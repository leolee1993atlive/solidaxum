//! notice 服务 —— 编排层。
//!
//! ctx 由 handler 请求期构造(`data_source().context(Principal::..)`)并
//! 透传;notice 是全豁免模块(SysNoticeMapper 无 @DataPermission),
//! 读在 repo 内显式 without_row_scope,写由漏斗盖审计。
//!
//! 定向公告可见性:`send_status='1'` 且 `target_type='ALL'` 或当前用户
//! 命中 `user_ids` CSV;发布时为目标用户补建 `sys_notice_read` 回执行,
//! ALL 不建行(全部用户默认未读)。

use chrono::Utc;

use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
    middleware::extractors::CurrentUser,
    vo::page::{TableDataInfo, build_order_by, normalize_paging},
};
use app_data::DataContext;

use crate::{
    notice::{
        model::notice::{
            Notice, NoticeCreateBo, NoticeListVo, NoticeMyQuery, NoticeMyVo,
            NoticePageQuery, NoticeUpdateBo, SEND_STATUS_PUBLISHED,
            TARGET_USER,
        },
        repository::{
            notice_read_repo::NoticeReadRepository,
            notice_repo::NoticeRepository,
        },
    },
    push::{message::PushMessage, sse::PushHub},
};

/// user_ids 列上限(与 DDL varchar(2000) 对齐)。
const MAX_USER_IDS_LEN: usize = 2000;

#[derive(Clone)]
pub struct NoticeService {
    notice_repo: NoticeRepository,
    read_repo: NoticeReadRepository,
    push_hub: PushHub,
}

impl NoticeService {
    /// 装配期只取 DataSource 快照:全部方法只**转发** ctx(豁免读不解析
    /// resolver,降级快照无影响)。
    pub fn new(infra: &Infrastructure, push_hub: PushHub) -> Self {
        Self {
            notice_repo: NoticeRepository::new(infra.data_source()),
            read_repo: NoticeReadRepository::new(infra.data_source()),
            push_hub,
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

    /// 当前用户可见的已发布公告(含已读/星标状态)。
    #[tracing::instrument(skip(self, ctx), fields(user_id = user_id, page = ?query.page.page_num, size = ?query.page.page_size))]
    pub async fn my_page(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
        query: NoticeMyQuery,
    ) -> AppResult<TableDataInfo<NoticeMyVo>> {
        let (current, size, _offset) = normalize_paging(&query.page);
        let allowed = [
            ("noticeId", "notice_id"),
            ("noticeTitle", "notice_title"),
            ("noticeType", "notice_type"),
            ("createTime", "create_time"),
            ("sendTime", "send_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "send_time desc, notice_id desc".to_string());

        let page = self
            .notice_repo
            .select_visible_page(
                ctx,
                user_id,
                current as u64,
                size as u64,
                &order_by,
            )
            .await?;
        if page.total <= 0 {
            return Ok(TableDataInfo::empty());
        }

        let notice_ids: Vec<i64> =
            page.rows.iter().filter_map(|n| n.notice_id).collect();
        let reads = self
            .read_repo
            .select_by_notice_ids(ctx, user_id, &notice_ids)
            .await?;
        let reads: std::collections::HashMap<i64, _> =
            reads.into_iter().map(|r| (r.notice_id, r)).collect();

        let rows = page
            .rows
            .iter()
            .map(|notice| {
                let read = notice.notice_id.and_then(|id| reads.get(&id));
                NoticeMyVo {
                    notice_id: notice.notice_id.unwrap_or_default(),
                    notice_title: notice.notice_title.clone(),
                    notice_type: notice.notice_type.clone(),
                    notice_content: notice.notice_content.clone(),
                    remark: notice.remark.clone(),
                    target_type: notice.target_type.clone(),
                    send_time: notice.send_time,
                    read_flag: read
                        .map(|r| r.read_flag.clone())
                        .unwrap_or_else(|| "0".to_string()),
                    read_time: read.and_then(|r| r.read_time),
                    star_flag: read
                        .map(|r| r.star_flag.clone())
                        .unwrap_or_else(|| "0".to_string()),
                    create_time: notice.base_entity.create_time,
                }
            })
            .collect();
        Ok(page.with_rows(rows))
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
        validate_target(&notice)?;
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
        validate_target(&notice)?;
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

    /// 发布公告:置已发布、补建回执行(USER)、按目标推送 SSE。
    /// 已发布的公告拒绝重复发布(避免重置 send_time 并重复广播)。
    #[tracing::instrument(skip(self, ctx, current_user), fields(notice_id = id, operator = %current_user.0.username))]
    pub async fn publish(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        current_user: &CurrentUser,
    ) -> AppResult<Notice> {
        let mut notice = self.get_by_id(ctx, id).await?;
        if notice.send_status == SEND_STATUS_PUBLISHED {
            return Err(AppError::BadRequest(format!(
                "通知公告已发布,请勿重复发布 (id={id})"
            )));
        }
        validate_target(&notice)?;
        let now = Utc::now();
        notice.send_status = SEND_STATUS_PUBLISHED.to_string();
        notice.send_time = Some(now);
        notice.base_entity.update(current_user.0.user_id);
        self.notice_repo.update_by_id(ctx, &notice, &id).await?;

        let targets = notice.target_user_id_list();
        if notice.target_type == TARGET_USER {
            self.read_repo.create_missing(ctx, id, &targets).await?;
        }

        let path = format!("/system/notice?noticeId={id}");
        let msg = PushMessage::notice(
            id,
            notice.notice_title.clone(),
            serde_json::json!({
                "noticeId": id,
                "noticeTitle": notice.notice_title,
                "noticeType": notice.notice_type,
                "path": path,
            }),
            Some(path),
        );
        if notice.target_type == TARGET_USER {
            for user_id in targets {
                self.push_hub.push_to_user(user_id, msg.clone());
            }
        } else if let Err(e) = self.push_hub.try_push_all(msg) {
            tracing::warn!(notice_id = id, error = %e, "公告广播失败");
        }

        tracing::info!(
            notice_id = id,
            target_type = %notice.target_type,
            operator = %current_user.0.username,
            "发布通知公告成功"
        );
        Ok(notice)
    }

    /// 标记单条公告已读(幂等)。不可见/未发布公告视为不存在。
    pub async fn mark_read(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        user_id: i64,
    ) -> AppResult<()> {
        let notice = self.get_by_id(ctx, id).await?;
        if notice.send_status != SEND_STATUS_PUBLISHED
            || !notice.visible_to(user_id)
        {
            return Err(AppError::NotFound(format!(
                "通知公告不存在或不可见 (id={})",
                id
            )));
        }
        self.read_repo
            .mark_read(ctx, id, user_id, Utc::now())
            .await?;
        Ok(())
    }

    /// 全部已读:对当前用户可见的每条已发布公告补/更新回执。
    pub async fn mark_all_read(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> AppResult<usize> {
        let ids = self.notice_repo.select_visible_ids(ctx, user_id).await?;
        let now = Utc::now();
        for id in &ids {
            self.read_repo.mark_read(ctx, *id, user_id, now).await?;
        }
        Ok(ids.len())
    }
}

/// 定向目标校验:USER 必须至少一个用户;CSV 长度不超列宽。
fn validate_target(notice: &Notice) -> AppResult<()> {
    if notice.target_type == TARGET_USER
        && notice.target_user_id_list().is_empty()
    {
        return Err(AppError::BadRequest(
            "定向公告必须选择至少一个用户".to_string(),
        ));
    }
    if notice
        .user_ids
        .as_deref()
        .is_some_and(|s| s.len() > MAX_USER_IDS_LEN)
    {
        return Err(AppError::BadRequest("目标用户列表过长".to_string()));
    }
    Ok(())
}
