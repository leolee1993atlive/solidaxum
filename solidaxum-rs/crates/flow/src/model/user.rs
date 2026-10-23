use serde::{Deserialize, Serialize};

use crate::repository::entity::flow_user;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: Option<i64>,
    pub type_: i8,
    pub processed_by: Option<String>,
    pub associated: i64,
    pub create_time: Option<chrono::NaiveDateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<chrono::NaiveDateTime>,
    pub update_by: Option<String>,
    pub del_flag: Option<String>,
    pub tenant_id: Option<String>,
}

impl From<&flow_user::Model> for User {
    fn from(m: &flow_user::Model) -> Self {
        Self {
            id: Some(m.id),
            type_: m.type_.trim().parse::<i8>().unwrap_or(0),
            processed_by: m.processed_by.clone(),
            associated: m.associated,
            create_time: m.create_time,
            create_by: m.create_by.clone(),
            update_time: m.update_time,
            update_by: m.update_by.clone(),
            del_flag: m.del_flag.clone(),
            tenant_id: m.tenant_id.clone(),
        }
    }
}
