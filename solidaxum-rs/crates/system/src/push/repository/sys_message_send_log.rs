//! `sys_message_send_log` 消息发送记录(insert-only)。
//! 一行 = 一次渠道发送(SUCCESS/FAILED/SKIPPED,只记录无重试)。

use serde::{Deserialize, Serialize};

use app_data::sea_orm::{
    self, ActiveModelBehavior, ActiveValue::NotSet, DateTimeUtc,
    DeriveEntityModel, DerivePrimaryKey, DeriveRelation, EntityTrait, EnumIter,
    PrimaryKeyTrait,
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
}

impl Default for SendLogRepository {
    fn default() -> Self {
        Self::new()
    }
}
