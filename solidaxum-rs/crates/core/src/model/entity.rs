use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BaseEntity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_dept: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_by: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_time: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_by: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseVo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_dept: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_by: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_time: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_by: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<DateTime<Utc>>,
}

impl BaseEntity {
    pub fn new(create_by: i64) -> Self {
        let now = Utc::now();
        Self {
            create_dept: None,
            create_by: Some(create_by),
            create_time: Some(now),
            update_by: Some(create_by),
            update_time: Some(now),
        }
    }

    pub fn update(&mut self, update_by: i64) {
        self.update_by = Some(update_by);
        self.update_time = Some(Utc::now());
    }
}

impl BaseVo {
    pub fn from_base(base: &BaseEntity) -> Self {
        Self {
            create_dept: base.create_dept,
            create_by: base.create_by,
            create_time: base.create_time,
            update_by: base.update_by,
            update_time: base.update_time,
        }
    }
}
