use std::collections::HashMap;

use app_core::error::AppResult;
use serde_json::Value;

use crate::{
    engine::transition::PathWayData,
    enums::NodeType,
    expression::condition::{SpelContext, eval_condition_with},
    model::{node::Node, skip::Skip},
};

/// 获取下一个节点，并记录路径信息
///
/// 1. 将当前节点记录到 pathway_nodes
/// 2. 若指定了任意跳转目标 any_node_code，直接定位该节点（不能是网关）
/// 3. 否则按 skip_type 匹配出边跳转，解析目标节点（不能是开始节点）
pub fn get_next_node(
    now_node: &Node,
    any_node_code: Option<&str>,
    skip_type: &str,
    path_way: &mut PathWayData,
    all_skips: &[Skip],
    all_nodes: &[Node],
) -> Option<Node> {
    // 1. 记录当前节点
    path_way.pathway_nodes.push(now_node.clone());

    // 2. 任意跳转
    if let Some(code) = any_node_code {
        let target = all_nodes.iter().find(|n| n.node_code == code)?;

        // 目标不能是网关
        let is_gateway = NodeType::from_i8(target.node_type)
            .map(|t| t.is_gate_way())
            .unwrap_or(false);
        if is_gateway {
            return None;
        }

        // 记录连接跳转
        if let Some(skip) = all_skips.iter().find(|s| {
            s.now_node_code == now_node.node_code && s.next_node_code == code
        }) {
            path_way.pathway_skips.push(skip.clone());
        }

        path_way.pathway_nodes.push(target.clone());
        return Some(target.clone());
    }

    // 3. 常规跳转：按 skip_type 匹配
    let matching_skip = all_skips
        .iter()
        .filter(|s| s.now_node_code == now_node.node_code)
        .find(|s| match &s.skip_type {
            Some(t) if !t.is_empty() => t == skip_type,
            _ => true,
        });

    let skip = matching_skip?;

    // 解析目标节点
    let target = all_nodes
        .iter()
        .find(|n| n.node_code == skip.next_node_code)?;

    // 目标不能是开始节点
    let is_start = NodeType::from_i8(target.node_type)
        .map(|t| t.is_start())
        .unwrap_or(false);
    if is_start {
        return None;
    }

    path_way.pathway_skips.push(skip.clone());
    path_way.pathway_nodes.push(target.clone());
    Some(target.clone())
}

/// 递归检查下一节点是否为网关，若是则按条件展开分支
///
/// `spel` 为调用层预热的角色快照（hasRole 求值用）。
/// 返回所有非网关目标节点的列表；条件求值失败时返回 Err
pub fn get_next_by_check_gateway(
    next_node: &Node,
    variable: &HashMap<String, Value>,
    path_way: &mut PathWayData,
    all_skips: &[Skip],
    all_nodes: &[Node],
    spel: &SpelContext,
) -> AppResult<Vec<Node>> {
    let node_type = match NodeType::from_i8(next_node.node_type) {
        Some(nt) => nt,
        None => return Ok(vec![]),
    };

    // 1. 非网关：撤销 get_next_node 的投机添加，作为目标节点返回
    if !node_type.is_gate_way() {
        if path_way
            .pathway_nodes
            .last()
            .map(|n| n.node_code == next_node.node_code)
            .unwrap_or(false)
        {
            path_way.pathway_nodes.pop();
        }
        return Ok(vec![next_node.clone()]);
    }

    // 2. 获取所有出边跳转
    let outgoing: Vec<&Skip> = all_skips
        .iter()
        .filter(|s| s.now_node_code == next_node.node_code)
        .collect();

    // 3. 按网关类型过滤分支
    let filtered: Vec<&Skip> = match node_type {
        NodeType::Serial => {
            // 串行网关：取第一个条件为 true 的分支；若无则取第一个无条件分支
            let mut chosen = None;
            for skip in &outgoing {
                let cond = skip.skip_condition.as_deref().unwrap_or("");
                if !cond.is_empty()
                    && eval_condition_with(cond, variable, spel)?
                {
                    chosen = Some(*skip);
                    break;
                }
            }
            if chosen.is_none() {
                chosen = outgoing
                    .iter()
                    .find(|s| {
                        s.skip_condition.as_deref().unwrap_or("").is_empty()
                    })
                    .copied();
            }
            chosen.into_iter().collect()
        }
        NodeType::Inclusive => {
            // 包容网关：保留所有空条件（默认分支）或条件求值为 true 的分支
            let mut kept: Vec<&Skip> = Vec::new();
            for skip in &outgoing {
                let cond = skip.skip_condition.as_deref().unwrap_or("");
                if cond.is_empty() || eval_condition_with(cond, variable, spel)?
                {
                    kept.push(*skip);
                }
            }
            kept
        }
        NodeType::Parallel => {
            // 并行网关：保留所有出边
            outgoing
        }
        _ => vec![],
    };

    // 4. 至少有一条分支存活
    if filtered.is_empty() {
        return Ok(vec![]);
    }

    // 5. 递归展开每个分支
    let mut results = Vec::new();
    for skip in &filtered {
        let target = match all_nodes
            .iter()
            .find(|n| n.node_code == skip.next_node_code)
        {
            Some(t) => t,
            None => continue,
        };
        path_way.pathway_nodes.push(target.clone());
        path_way.pathway_skips.push((*skip).clone());
        let sub = get_next_by_check_gateway(
            target, variable, path_way, all_skips, all_nodes, spel,
        )?;
        results.extend(sub);
    }

    Ok(results)
}
