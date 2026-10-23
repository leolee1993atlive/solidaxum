use std::collections::HashMap;

use serde_json::Value;

use crate::{
    dto::flow_params::FlowParams,
    model::{
        definition::Definition, instance::Instance, node::Node, task::Task,
    },
};

/// 监听器类型标识（与 warm-flow 原格式一致）
pub const LISTENER_START: &str = "start";
pub const LISTENER_ASSIGNMENT: &str = "assignment";
pub const LISTENER_FINISH: &str = "finish";
pub const LISTENER_CREATE: &str = "create";

/// 传给监听器的上下文变量
#[derive(Clone, Debug)]
pub struct ListenerVariable {
    pub definition: Definition,
    pub instance: Instance,
    pub node: Option<Node>,
    pub task: Option<Task>,
    pub next_nodes: Vec<Node>,
    pub next_tasks: Vec<Task>,
    pub variable: HashMap<String, Value>,
    pub flow_params: FlowParams,
}

impl ListenerVariable {
    pub fn new(
        definition: Definition,
        instance: Instance,
        variable: HashMap<String, Value>,
        flow_params: FlowParams,
    ) -> Self {
        Self {
            definition,
            instance,
            node: None,
            task: None,
            next_nodes: vec![],
            next_tasks: vec![],
            variable,
            flow_params,
        }
    }
}
