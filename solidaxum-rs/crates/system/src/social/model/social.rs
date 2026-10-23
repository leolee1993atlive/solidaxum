// sea_orm 经 app-data 受限 re-export(model 层不直接依赖 sea-orm)
use app_data::sea_orm::DateTimeUtc;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Social {
    pub id: Option<i64>,
    pub user_id: i64,
    pub auth_id: Option<String>,
    pub source: Option<String>,
    pub open_id: Option<String>,
    pub user_name: Option<String>,
    pub nick_name: Option<String>,
    pub avatar: Option<String>,
    pub access_token: Option<String>,
    pub expire_in: Option<i32>,
    pub refresh_token: Option<String>,
    pub create_time: Option<DateTimeUtc>,
    pub del_flag: Option<String>,
}

impl Social {
    pub fn to_vo(&self) -> SocialVo {
        SocialVo {
            id: self.id.unwrap_or_default(),
            user_id: self.user_id,
            auth_id: self.auth_id.clone(),
            source: self.source.clone(),
            open_id: self.open_id.clone(),
            user_name: self.user_name.clone(),
            nick_name: self.nick_name.clone(),
            avatar: self.avatar.clone(),
            create_time: self.create_time,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialVo {
    pub id: i64,
    pub user_id: i64,
    pub auth_id: Option<String>,
    pub source: Option<String>,
    pub open_id: Option<String>,
    pub user_name: Option<String>,
    pub nick_name: Option<String>,
    pub avatar: Option<String>,
    pub create_time: Option<DateTimeUtc>,
}
