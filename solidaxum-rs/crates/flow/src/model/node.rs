use serde::{Deserialize, Serialize};

use crate::repository::entity::flow_node;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: Option<i64>,
    pub node_type: i8,
    pub definition_id: i64,
    pub node_code: String,
    pub node_name: Option<String>,
    pub permission_flag: Option<String>,
    pub node_ratio: Option<String>,
    pub coordinate: Option<String>,
    pub any_node_skip: Option<String>,
    pub listener_type: Option<String>,
    pub listener_path: Option<String>,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    pub version: String,
    pub create_time: Option<chrono::NaiveDateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<chrono::NaiveDateTime>,
    pub update_by: Option<String>,
    pub ext: Option<String>,
    pub del_flag: Option<String>,
    pub tenant_id: Option<String>,
}

impl From<&flow_node::Model> for Node {
    fn from(m: &flow_node::Model) -> Self {
        Self {
            id: Some(m.id),
            node_type: m.node_type,
            definition_id: m.definition_id,
            node_code: m.node_code.clone(),
            node_name: m.node_name.clone(),
            permission_flag: m.permission_flag.clone(),
            node_ratio: m.node_ratio.clone(),
            coordinate: m.coordinate.clone(),
            any_node_skip: m.any_node_skip.clone(),
            listener_type: m.listener_type.clone(),
            listener_path: m.listener_path.clone(),
            form_custom: m.form_custom.clone(),
            form_path: m.form_path.clone(),
            version: m.version.clone(),
            create_time: m.create_time,
            create_by: m.create_by.clone(),
            update_time: m.update_time,
            update_by: m.update_by.clone(),
            ext: m.ext.clone(),
            del_flag: m.del_flag.clone(),
            tenant_id: m.tenant_id.clone(),
        }
    }
}
