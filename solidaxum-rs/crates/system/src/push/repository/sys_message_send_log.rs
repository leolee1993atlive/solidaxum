//! `sys_message_send_log` 消息发送记录(insert-only)。
//! 一行 = 一次渠道发送(SUCCESS/FAILED/SKIPPED,只记录无重试)。

use app_core::repo::str_trim_or_empty;
use app_core::vo::page::{PageQuery, TableDataInfo};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Deserializer, Serialize};

use app_data::sea_orm::{
    self, ActiveModelBehavior, ActiveValue::NotSet, ColumnTrait, DateTimeUtc,
    DeriveEntityModel, DerivePrimaryKey, DeriveRelation, EntityTrait, EnumIter,
    Order, PrimaryKeyTrait, QueryFilter, QueryOrder,
};
use app_data::{DataContext, DataError};

#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel, Eq,
)]
#[serde(rename_all = "camelCase")]
#[sea_orm(table_name = "sys_message_send_log")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub log_id: i64,
    pub tenant_id: Option<String>,
    pub batch_id: Option<String>,
    pub biz_type: Option<String>,
    pub biz_id: Option<String>,
    /// 渠道 SSE/APP/SMS/WECHAT/DINGTALK/FEISHU
    pub channel: String,
    /// 收件人用户 ID JSON 数组
    pub recipient_user_ids: Option<String>,
    pub content: Option<String>,
    /// SKIPPED/SUCCESS/FAILED
    pub status: String,
    pub skip_reason: Option<String>,
    pub error_message: Option<String>,
    pub provider_response: Option<String>,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTimeUtc>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl ActiveModel {
    pub fn from_new() -> Self {
        Self {
            log_id: NotSet,
            tenant_id: NotSet,
            batch_id: NotSet,
            biz_type: NotSet,
            biz_id: NotSet,
            channel: NotSet,
            recipient_user_ids: NotSet,
            content: NotSet,
            status: NotSet,
            skip_reason: NotSet,
            error_message: NotSet,
            provider_response: NotSet,
            create_dept: NotSet,
            create_by: NotSet,
            create_time: NotSet,
            update_by: NotSet,
            update_time: NotSet,
        }
    }
}

/// 发送历史分页查询。`begin` 含、`end` 含(日期止归一为次日零点)。
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendLogPageQuery {
    #[serde(flatten)]
    pub page: PageQuery,
    pub channel: Option<String>,
    pub status: Option<String>,
    pub biz_type: Option<String>,
    #[serde(
        default,
        deserialize_with = "app_core::util::serde_util::de_opt_datetime_flexible"
    )]
    pub begin: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "de_opt_end_exclusive")]
    pub end: Option<DateTime<Utc>>,
}

fn de_opt_end_exclusive<'de, D>(
    deserializer: D,
) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Ok(Some(dt.with_timezone(&Utc) + Duration::seconds(1)));
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
    {
        return Ok(Some(Utc.from_utc_datetime(&naive) + Duration::seconds(1)));
    }
    if let Ok(date) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        let next = date.succ_opt().ok_or_else(|| {
            serde::de::Error::custom(format!("无法解析日期时间: {raw}"))
        })?;
        return Ok(Some(Utc.from_utc_datetime(
            &next.and_hms_opt(0, 0, 0).expect("合法时刻"),
        )));
    }
    Err(serde::de::Error::custom(format!("无法解析日期时间: {raw}")))
}

/// 发送日志仓储(无状态;追加写,审计列由漏斗盖章)。
pub struct SendLogRepository;

impl SendLogRepository {
    pub fn new() -> Self {
        Self
    }

    /// 追加一条发送记录(只记录无重试)。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        am: ActiveModel,
    ) -> Result<(), DataError> {
        ctx.insert(am).await?;
        Ok(())
    }

    /// 发送历史分页(全局审计流水,系统面读;无行级范围)。
    pub async fn select_page(
        &self,
        ctx: &DataContext<'_>,
        q: &SendLogPageQuery,
        page_no: u64,
        page_size: u64,
    ) -> Result<TableDataInfo<Model>, DataError> {
        let channel = str_trim_or_empty(&q.channel);
        let status = str_trim_or_empty(&q.status);
        let biz_type = str_trim_or_empty(&q.biz_type);
        let mut select = Entity::find();
        if !channel.is_empty() {
            select = select.filter(Column::Channel.eq(&channel));
        }
        if !status.is_empty() {
            select = select.filter(Column::Status.eq(&status));
        }
        if !biz_type.is_empty() {
            select = select.filter(Column::BizType.eq(&biz_type));
        }
        if let Some(begin) = q.begin {
            select = select.filter(Column::CreateTime.gte(begin));
        }
        if let Some(end) = q.end {
            select = select.filter(Column::CreateTime.lt(end));
        }
        select = select
            .order_by(Column::CreateTime, Order::Desc)
            .order_by(Column::LogId, Order::Desc);
        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let rows = page.rows.clone();
        Ok(TableDataInfo::from_page_rows(&page, rows))
    }
}

impl Default for SendLogRepository {
    fn default() -> Self {
        Self::new()
    }
}
