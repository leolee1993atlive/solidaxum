use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserOnline {
    pub token_id: String,
    pub user_name: String,
    pub nick_name: String,
    pub dept_name: String,
    pub ipaddr: String,
    pub login_location: String,
    pub browser: String,
    pub os: String,
    pub login_time: Option<DateTime<Utc>>,
    pub client_key: Option<String>,
    pub device_type: Option<String>,
    pub user_id: i64,
}
