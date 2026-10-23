use serde::{Deserialize, Serialize};

use crate::repository::entity::flow_definition;

/// 流程定义领域模型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Definition {
    pub id: Option<i64>,
    pub flow_code: String,
    pub flow_name: String,
    pub model_value: String,
    pub category: Option<String>,
    pub version: String,
    pub is_publish: i8,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    pub activity_status: i8,
    pub listener_type: Option<String>,
    pub listener_path: Option<String>,
    pub ext: Option<String>,
    pub create_time: Option<chrono::NaiveDateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<chrono::NaiveDateTime>,
    pub update_by: Option<String>,
    pub del_flag: Option<String>,
    pub tenant_id: Option<String>,
}

impl From<&flow_definition::Model> for Definition {
    fn from(m: &flow_definition::Model) -> Self {
        Self {
            id: Some(m.id),
            flow_code: m.flow_code.clone(),
            flow_name: m.flow_name.clone(),
            model_value: m.model_value.clone(),
            category: m.category.clone(),
            version: m.version.clone(),
            is_publish: m.is_publish,
            form_custom: m.form_custom.clone(),
            form_path: m.form_path.clone(),
            activity_status: m.activity_status,
            listener_type: m.listener_type.clone(),
            listener_path: m.listener_path.clone(),
            ext: m.ext.clone(),
            create_time: m.create_time,
            create_by: m.create_by.clone(),
            update_time: m.update_time,
            update_by: m.update_by.clone(),
            del_flag: m.del_flag.clone(),
            tenant_id: m.tenant_id.clone(),
        }
    }
}
