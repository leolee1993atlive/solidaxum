//! `sys_message_channel` 统一渠道配置(钉钉/企微/飞书)。
//!
//! 三渠道凭据形状同构(enabled / clientId / clientSecret / agentId /
//! agentSecret / baseUrl / domain),故并一表 + `channel` 判别列。
//! 短信与 APP 按院方定制,不在本表维护(主线只留发送 trait 入口)。
//!
//! 密钥列 `client_secret` / `agent_secret` 落库前经核心 `FieldEncryptor`
//! AES 加密(与 `sys_oss_config` 同先例),对外回读一律掩码。

use app_data::{
    DataContext, DataError,
    sea_orm::{
        self, ActiveModelBehavior, ActiveValue::Set, DateTimeUtc,
        DeriveEntityModel, DerivePrimaryKey, DeriveRelation, EntityTrait,
        EnumIter, PrimaryKeyTrait,
    },
};
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel, Eq,
)]
#[serde(rename_all = "camelCase")]
#[sea_orm(table_name = "sys_message_channel")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub channel_id: i64,
    /// 渠道标识:DINGTALK / WECHAT / FEISHU
    pub channel: String,
    pub channel_name: String,
    /// "1" 启用 / "0" 停用
    pub enabled: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub agent_id: Option<String>,
    pub agent_secret: Option<String>,
    pub base_url: Option<String>,
    pub domain: Option<String>,
    pub del_flag: String,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTimeUtc>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTimeUtc>,
    pub remark: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// 渠道配置领域对象(密钥已解密,供发送器使用)。
#[derive(Clone, Debug, Default)]
pub struct ChannelConfig {
    pub channel: String,
    pub enabled: bool,
    pub client_id: String,
    pub client_secret: String,
    pub agent_id: String,
    pub agent_secret: String,
    pub base_url: String,
    pub domain: String,
}

impl ChannelConfig {
    /// 凭据完整性:定向发送至少需要 client_id + client_secret。
    pub fn is_configured(&self) -> bool {
        !self.client_id.trim().is_empty()
            && !self.client_secret.trim().is_empty()
    }

    /// 默认基址(公有云);`base_url` 非空时用私域/代理地址。
    pub fn api_base(&self, default: &str) -> String {
        let b = self.base_url.trim();
        if b.is_empty() {
            default.trim_end_matches('/').to_string()
        } else {
            b.trim_end_matches('/').to_string()
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ChannelRepository;

impl ChannelRepository {
    pub fn new() -> Self {
        Self
    }

    /// 全量清单(含停用;软删由漏斗 DataPolicy 自动剔除)。
    /// 渠道表是全局配置,豁免行级范围。
    pub async fn select_all(
        &self,
        ctx: &DataContext<'_>,
    ) -> Result<Vec<Model>, DataError> {
        use app_data::sea_orm::{Order, QueryOrder};
        ctx.select(Entity::find().order_by(Column::ChannelId, Order::Asc))
            .without_row_scope()
            .all()
            .await
    }

    /// 按渠道取一行(启用判断由调用方做)。
    pub async fn select_by_channel(
        &self,
        ctx: &DataContext<'_>,
        channel: &str,
    ) -> Result<Option<Model>, DataError> {
        use app_data::sea_orm::{ColumnTrait, QueryFilter};
        ctx.select(Entity::find().filter(Column::Channel.eq(channel)))
            .without_row_scope()
            .one()
            .await
    }

    /// 更新配置(按 channel 定位;密钥字段由 service 层加密后传入)。
    /// 漏斗无 update_many 出口,故先读主键再按主键 update。
    pub async fn update_by_channel(
        &self,
        ctx: &DataContext<'_>,
        mut am: ActiveModel,
    ) -> Result<(), DataError> {
        let channel = match am.channel.clone() {
            Set(v) => v,
            _ => String::new(),
        };
        match self.select_by_channel(ctx, &channel).await? {
            Some(existing) => {
                am.channel_id = Set(existing.channel_id);
                ctx.update(am).await?;
            }
            None => {
                // 行不存在(全新库未跑种子/被清空):插入一行
                let name = match channel.as_str() {
                    "DINGTALK" => "钉钉",
                    "WECHAT" => "企业微信",
                    "FEISHU" => "飞书",
                    _ => channel.as_str(),
                };
                am.channel_name = Set(name.to_string());
                am.del_flag = Set("0".to_string());
                ctx.insert(am).await?;
            }
        }
        Ok(())
    }
}

impl Default for ChannelRepository {
    fn default() -> Self {
        Self::new()
    }
}

/// 供 update_by_channel 组装的窄更新模型构造器。
#[allow(clippy::too_many_arguments)]
pub fn narrow_update(
    channel: &str,
    enabled: bool,
    client_id: Option<String>,
    client_secret_enc: Option<String>,
    agent_id: Option<String>,
    agent_secret_enc: Option<String>,
    base_url: Option<String>,
    domain: Option<String>,
    remark: Option<String>,
) -> ActiveModel {
    ActiveModel {
        channel: Set(channel.to_string()),
        enabled: Set(if enabled { "1" } else { "0" }.to_string()),
        client_id: Set(client_id),
        client_secret: Set(client_secret_enc),
        agent_id: Set(agent_id),
        agent_secret: Set(agent_secret_enc),
        base_url: Set(base_url),
        domain: Set(domain),
        remark: Set(remark),
        ..Default::default()
    }
}
