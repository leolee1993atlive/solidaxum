use std::collections::{HashMap, HashSet};

use crate::{
    engine::transition::PathWayData,
    enums::NodeType,
    model::def_json::{DefJson, NodeJson},
};

pub const NOT_DONE: i8 = 0;
pub const TO_DO: i8 = 1;
pub const DONE: i8 = 2;

/// 构建 node_code -> node_type 的查找表（借用源数据，不影响后续克隆体的可变操作）
fn build_type_map(node_list: &[NodeJson]) -> HashMap<&str, i8> {
    node_list
        .iter()
        .map(|n| (n.node_code.as_str(), n.node_type))
        .collect()
}

/// 判断 node_code 对应的节点是否为结束节点
fn is_end_node(code: &str, type_map: &HashMap<&str, i8>) -> bool {
    type_map
        .get(code)
        .and_then(|&t| NodeType::from_i8(t))
        .map(|t| t.is_end())
        .unwrap_or(false)
}

/// 启动时生成初始流程图状态 JSON
///
/// - 所有节点/跳转初始化为 NOT_DONE
/// - pathway 中的节点/跳转标记为 DONE
/// - target 节点标记为 TO_DO（结束节点为 DONE）
pub fn start_metadata(
    design_json: &DefJson,
    path_way: &PathWayData,
) -> DefJson {
    // 先从源数据构建查找表，避免与克隆体的可变借用冲突
    let type_map = build_type_map(&design_json.node_list);
    let mut json = design_json.clone();

    let pathway_codes: HashSet<&str> = path_way
        .pathway_nodes
        .iter()
        .map(|n| n.node_code.as_str())
        .collect();

    let target_codes: HashSet<&str> = path_way
        .target_nodes
        .iter()
        .map(|n| n.node_code.as_str())
        .collect();

    let pathway_skip_pairs: HashSet<(&str, &str)> = path_way
        .pathway_skips
        .iter()
        .map(|s| (s.now_node_code.as_str(), s.next_node_code.as_str()))
        .collect();

    // 初始化所有节点和跳转为 NOT_DONE，然后按 pathway/target 覆盖
    for node in &mut json.node_list {
        node.status = Some(NOT_DONE);
        for skip in &mut node.skip_list {
            skip.status = Some(NOT_DONE);
        }

        let code = node.node_code.as_str();
        if pathway_codes.contains(code) {
            node.status = Some(DONE);
        } else if target_codes.contains(code) {
            node.status = Some(if is_end_node(code, &type_map) {
                DONE
            } else {
                TO_DO
            });
        }

        for skip in &mut node.skip_list {
            let pair =
                (skip.now_node_code.as_str(), skip.next_node_code.as_str());
            if pathway_skip_pairs.contains(&pair) {
                skip.status = Some(DONE);
            } else if target_codes.contains(skip.next_node_code.as_str()) {
                skip.status = Some(
                    if is_end_node(skip.next_node_code.as_str(), &type_map) {
                        DONE
                    } else {
                        TO_DO
                    },
                );
            }
        }
    }

    json
}

/// 每次流转时更新流程图状态 JSON
///
/// - 保留 instance_json 中已有的状态
/// - pathway 节点/跳转标记为 DONE
/// - target 节点标记为 TO_DO（结束节点为 DONE，并清除残余 TO_DO）
pub fn skip_metadata(
    instance_json: &DefJson,
    path_way: &PathWayData,
) -> DefJson {
    let type_map = build_type_map(&instance_json.node_list);
    let mut json = instance_json.clone();

    let pathway_codes: HashSet<&str> = path_way
        .pathway_nodes
        .iter()
        .map(|n| n.node_code.as_str())
        .collect();

    let target_codes: HashSet<&str> = path_way
        .target_nodes
        .iter()
        .map(|n| n.node_code.as_str())
        .collect();

    let pathway_skip_pairs: HashSet<(&str, &str)> = path_way
        .pathway_skips
        .iter()
        .map(|s| (s.now_node_code.as_str(), s.next_node_code.as_str()))
        .collect();

    // 第一轮：标记节点状态，检测是否到达结束节点
    let mut reached_end = false;
    for node in &mut json.node_list {
        let code = node.node_code.as_str();
        if pathway_codes.contains(code) {
            node.status = Some(DONE);
        }
        if target_codes.contains(code) {
            if is_end_node(code, &type_map) {
                node.status = Some(DONE);
                reached_end = true;
            } else {
                node.status = Some(TO_DO);
            }
        }
    }

    // 到达结束节点时清除残余 TO_DO
    if reached_end {
        for node in &mut json.node_list {
            if node.status == Some(TO_DO) {
                node.status = Some(NOT_DONE);
            }
        }
    }

    // 第二轮：标记跳转状态
    for node in &mut json.node_list {
        for skip in &mut node.skip_list {
            let pair =
                (skip.now_node_code.as_str(), skip.next_node_code.as_str());
            if pathway_skip_pairs.contains(&pair) {
                skip.status = Some(DONE);
            } else if target_codes.contains(skip.next_node_code.as_str()) {
                skip.status = Some(
                    if is_end_node(skip.next_node_code.as_str(), &type_map) {
                        DONE
                    } else {
                        TO_DO
                    },
                );
            }
        }
    }

    json
}
