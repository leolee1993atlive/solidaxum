use serde::{Deserialize, Serialize};

use crate::repository::entity::flow_his_task;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HisTask {
    pub id: Option<i64>,
    pub definition_id: i64,
    pub instance_id: i64,
    pub task_id: i64,
    pub node_code: Option<String>,
    pub node_name: Option<String>,
    pub node_type: Option<i8>,
    pub target_node_code: Option<String>,
    pub target_node_name: Option<String>,
    pub approver: Option<String>,
    pub cooperate_type: i8,
    pub collaborator: Option<String>,
    pub skip_type: String,
    pub flow_status: String,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    pub message: Option<String>,
    pub variable: Option<String>,
    pub ext: Option<String>,
    pub create_time: Option<chrono::NaiveDateTime>,
    pub update_time: Option<chrono::NaiveDateTime>,
    pub del_flag: Option<String>,
    pub tenant_id: Option<String>,
}

impl From<&flow_his_task::Model> for HisTask {
    fn from(m: &flow_his_task::Model) -> Self {
        Self {
            id: Some(m.id),
            definition_id: m.definition_id,
            instance_id: m.instance_id,
            task_id: m.task_id,
            node_code: m.node_code.clone(),
            node_name: m.node_name.clone(),
            node_type: m.node_type,
            target_node_code: m.target_node_code.clone(),
            target_node_name: m.target_node_name.clone(),
            approver: m.approver.clone(),
            cooperate_type: m.cooperate_type,
            collaborator: m.collaborator.clone(),
            skip_type: m.skip_type.clone(),
            flow_status: m.flow_status.clone(),
            form_custom: m.form_custom.clone(),
            form_path: m.form_path.clone(),
            message: m.message.clone(),
            variable: m.variable.clone(),
            ext: m.ext.clone(),
            create_time: m.create_time,
            update_time: m.update_time,
            del_flag: m.del_flag.clone(),
            tenant_id: m.tenant_id.clone(),
        }
    }
}
