use serde::{Deserialize, Serialize};

use crate::repository::entity::flow_instance;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instance {
    pub id: Option<i64>,
    pub definition_id: i64,
    pub business_id: String,
    pub node_type: i8,
    pub node_code: String,
    pub node_name: Option<String>,
    pub variable: Option<String>,
    pub flow_status: String,
    pub activity_status: i8,
    pub def_json: Option<String>,
    pub create_time: Option<chrono::NaiveDateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<chrono::NaiveDateTime>,
    pub update_by: Option<String>,
    pub ext: Option<String>,
    pub del_flag: Option<String>,
    pub tenant_id: Option<String>,
}

impl From<&flow_instance::Model> for Instance {
    fn from(m: &flow_instance::Model) -> Self {
        Self {
            id: Some(m.id),
            definition_id: m.definition_id,
            business_id: m.business_id.clone(),
            node_type: m.node_type,
            node_code: m.node_code.clone(),
            node_name: m.node_name.clone(),
            variable: m.variable.clone(),
            flow_status: m.flow_status.clone(),
            activity_status: m.activity_status,
            def_json: m.def_json.clone(),
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
