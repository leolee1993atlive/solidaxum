use crate::model::{node::Node, skip::Skip};

/// 流转路径数据：记录流转过程中经过的节点、跳转和最终目标节点
#[derive(Clone, Debug, Default)]
pub struct PathWayData {
    pub def_id: i64,
    pub ins_id: Option<i64>,
    pub skip_type: Option<String>,
    pub pathway_nodes: Vec<Node>,
    pub pathway_skips: Vec<Skip>,
    pub target_nodes: Vec<Node>,
}
