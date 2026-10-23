use app_core::util::snowflake;
use chrono::Local;

use crate::{
    dto::flow_params::FlowParams,
    enums::{CooperateType, FlowStatus, SkipType},
    model::{his_task::HisTask, node::Node, task::Task},
};

/// 历史任务服务（无状态工具类）
pub struct HisTaskService;

impl HisTaskService {
    /// 构建流转历史记录（不写库）
    pub fn build_skip_his(
        completed_task: &Task,
        next_nodes: &[Node],
        flow_params: &FlowParams,
        merged_variable: &str,
    ) -> HisTask {
        let skip_type = flow_params
            .skip_type
            .clone()
            .unwrap_or_else(|| SkipType::Pass.as_str().to_string());

        let flow_status = flow_params
            .his_status
            .clone()
            .or(flow_params.flow_status.clone())
            .unwrap_or_else(|| {
                if skip_type == SkipType::Reject.as_str() {
                    FlowStatus::Reject.as_str().to_string()
                } else {
                    FlowStatus::Pass.as_str().to_string()
                }
            });

        let now = Local::now().naive_local();

        HisTask {
            id: Some(snowflake::next_id()),
            task_id: completed_task.id.unwrap_or(0),
            instance_id: completed_task.instance_id,
            definition_id: completed_task.definition_id,
            node_code: Some(completed_task.node_code.clone()),
            node_name: completed_task.node_name.clone(),
            node_type: Some(completed_task.node_type),
            target_node_code: Some(
                next_nodes
                    .iter()
                    .map(|n| n.node_code.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            target_node_name: Some(
                next_nodes
                    .iter()
                    .filter_map(|n| n.node_name.as_deref())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            approver: flow_params.handler.clone(),
            cooperate_type: CooperateType::Approval as i8,
            collaborator: None,
            skip_type,
            flow_status,
            form_custom: completed_task.form_custom.clone(),
            form_path: completed_task.form_path.clone(),
            message: flow_params.message.clone(),
            variable: Some(merged_variable.to_string()),
            ext: flow_params.his_task_ext.clone(),
            create_time: Some(now),
            update_time: Some(now),
            del_flag: Some("0".to_string()),
            tenant_id: None,
        }
    }
}
