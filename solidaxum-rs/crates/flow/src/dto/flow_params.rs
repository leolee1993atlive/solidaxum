use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 流程操作入参（对应 warm-flow FlowParams）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FlowParams {
    pub flow_code: Option<String>,
    pub handler: Option<String>,
    pub node_code: Option<String>,
    pub permission_flag: Option<Vec<String>>,
    pub skip_type: Option<String>,
    pub message: Option<String>,
    #[serde(default)]
    pub variable: HashMap<String, serde_json::Value>,
    pub flow_status: Option<String>,
    pub his_status: Option<String>,
    pub activity_status: Option<i32>,
    pub cooperate_type: Option<i32>,
    pub ext: Option<String>,
    pub his_task_ext: Option<String>,
    pub add_handlers: Option<Vec<String>>,
    pub reduction_handlers: Option<Vec<String>>,
    #[serde(default)]
    pub ignore: bool,
    #[serde(default)]
    pub ignore_depute: bool,
    #[serde(default)]
    pub ignore_cooperate: bool,
    pub next_handler: Option<Vec<String>>,
    #[serde(default)]
    pub next_handler_append: bool,
    /// 弹窗选人(对齐 Java 基线 CompleteTaskBo.assigneeMap):
    /// key = 下一节点编码,value = 用户ID(逗号串或数组)。
    /// load 时并入 pass:{code}/back:{code} 变量,建任务时消费。
    #[serde(default)]
    pub assignee_map: Option<HashMap<String, serde_json::Value>>,
}
