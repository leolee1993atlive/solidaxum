// sea_orm 经 app-data 受限 re-export(model 层不直接依赖 sea-orm)
use app_data::sea_orm::DateTimeUtc;
use serde::Serialize;

use crate::message::repository::message_repo;

/// 消息记录展示 VO（对齐 Java SysMessageVo）。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageVo {
    pub message_id: i64,
    pub category: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub source: String,
    pub title: Option<String>,
    pub message: Option<String>,
    pub content: Option<String>,
    pub data: Option<serde_json::Value>,
    pub path: Option<String>,
    pub create_time: Option<DateTimeUtc>,
}

impl MessageVo {
    pub fn from_model(m: &message_repo::Model) -> Self {
        let data = m
            .data_json
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());
        Self {
            message_id: m.message_id,
            category: m.category.clone(),
            r#type: m.r#type.clone(),
            source: m.source.clone(),
            title: m.title.clone(),
            message: m.message.clone(),
            content: m.content.clone(),
            data,
            path: m.path.clone(),
            create_time: m.create_time,
        }
    }
}

/// 消息盒子 VO（对齐 Java SysMessageBoxVo）。
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageBoxVo {
    pub system_list: Vec<MessageVo>,
    pub notice_list: Vec<MessageVo>,
    pub workflow_list: Vec<MessageVo>,
}
