use crate::model::{definition::Definition, node::Node, skip::Skip};

/// 运行时聚合对象：定义 + 所有节点（平铺）+ 所有跳转（平铺）
#[derive(Clone, Debug)]
pub struct FlowCombine {
    pub definition: Definition,
    pub all_nodes: Vec<Node>,
    pub all_skips: Vec<Skip>,
}
