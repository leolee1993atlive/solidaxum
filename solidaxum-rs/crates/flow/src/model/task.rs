use serde::{Deserialize, Serialize};

use crate::repository::entity::flow_task;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: Option<i64>,
    pub definition_id: i64,
    pub instance_id: i64,
    pub node_code: String,
    pub node_name: Option<String>,
    pub node_type: i8,
    pub flow_status: String,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    pub create_time: Option<chrono::NaiveDateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<chrono::NaiveDateTime>,
    pub update_by: Option<String>,
    pub del_flag: Option<String>,
    pub tenant_id: Option<String>,
    /// 权限列表（从 permission_flag 解析，不持久化到 flow_task 表）
    pub permission_list: Vec<String>,
}

impl From<&flow_task::Model> for Task {
    fn from(m: &flow_task::Model) -> Self {
        Self {
            id: Some(m.id),
            definition_id: m.definition_id,
            instance_id: m.instance_id,
            node_code: m.node_code.clone(),
            node_name: m.node_name.clone(),
            node_type: m.node_type,
            flow_status: m.flow_status.clone(),
            form_custom: m.form_custom.clone(),
            form_path: m.form_path.clone(),
            create_time: m.create_time,
            create_by: m.create_by.clone(),
            update_time: m.update_time,
            update_by: m.update_by.clone(),
            del_flag: m.del_flag.clone(),
            tenant_id: m.tenant_id.clone(),
            permission_list: vec![],
        }
    }
}
