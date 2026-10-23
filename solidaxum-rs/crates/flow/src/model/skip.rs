use serde::{Deserialize, Serialize};

use crate::repository::entity::flow_skip;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skip {
    pub id: Option<i64>,
    pub definition_id: i64,
    pub now_node_code: String,
    pub now_node_type: Option<i8>,
    pub next_node_code: String,
    pub next_node_type: Option<i8>,
    pub node_id: Option<i64>,
    pub skip_name: Option<String>,
    pub skip_type: Option<String>,
    pub skip_condition: Option<String>,
    pub coordinate: Option<String>,
    pub create_time: Option<chrono::NaiveDateTime>,
    pub create_by: Option<String>,
    pub update_time: Option<chrono::NaiveDateTime>,
    pub update_by: Option<String>,
    pub del_flag: Option<String>,
    pub tenant_id: Option<String>,
}

impl From<&flow_skip::Model> for Skip {
    fn from(m: &flow_skip::Model) -> Self {
        Self {
            id: Some(m.id),
            definition_id: m.definition_id,
            now_node_code: m.now_node_code.clone(),
            now_node_type: m.now_node_type,
            next_node_code: m.next_node_code.clone(),
            next_node_type: m.next_node_type,
            node_id: m.node_id,
            skip_name: m.skip_name.clone(),
            skip_type: m.skip_type.clone(),
            skip_condition: m.skip_condition.clone(),
            coordinate: m.coordinate.clone(),
            create_time: m.create_time,
            create_by: m.create_by.clone(),
            update_time: m.update_time,
            update_by: m.update_by.clone(),
            del_flag: m.del_flag.clone(),
            tenant_id: m.tenant_id.clone(),
        }
    }
}
