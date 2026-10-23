use crate::model::{
    def_json::{DefJson, NodeJson, SkipJson},
    definition::Definition,
    flow_combine::FlowCombine,
    node::Node,
    skip::Skip,
};

/// DefJson -> FlowCombine（不生成 ID，ID 由 service 层统一分配）
pub fn def_json_to_entities(def: &DefJson) -> FlowCombine {
    let definition = Definition {
        id: None,
        flow_code: def.flow_code.clone(),
        flow_name: def.flow_name.clone(),
        model_value: if def.model_value.is_empty() {
            "CLASSICS".to_string()
        } else {
            def.model_value.clone()
        },
        category: def.category.clone(),
        version: def.version.clone(),
        is_publish: def.is_publish.unwrap_or(0),
        form_custom: def.form_custom.clone(),
        form_path: def.form_path.clone(),
        activity_status: 1,
        listener_type: def.listener_type.clone(),
        listener_path: def.listener_path.clone(),
        ext: def.ext.clone(),
        create_time: None,
        create_by: None,
        update_time: None,
        update_by: None,
        del_flag: Some("0".to_string()),
        tenant_id: None,
    };

    let mut all_nodes: Vec<Node> = Vec::new();
    for node_json in &def.node_list {
        let node = Node {
            id: None,
            node_type: node_json.node_type,
            definition_id: 0, // service 层填充
            node_code: node_json.node_code.clone(),
            node_name: node_json.node_name.clone(),
            permission_flag: node_json.permission_flag.clone(),
            node_ratio: Some(
                node_json
                    .node_ratio
                    .clone()
                    .unwrap_or_else(|| "0".to_string()),
            ),
            coordinate: node_json.coordinate.clone(),
            any_node_skip: node_json.any_node_skip.clone(),
            listener_type: node_json.listener_type.clone(),
            listener_path: node_json.listener_path.clone(),
            form_custom: node_json.form_custom.clone(),
            form_path: node_json.form_path.clone(),
            version: node_json
                .version
                .clone()
                .unwrap_or_else(|| def.version.clone()),
            create_time: None,
            create_by: None,
            update_time: None,
            update_by: None,
            ext: node_json.ext.clone(),
            del_flag: Some("0".to_string()),
            tenant_id: None,
        };
        all_nodes.push(node);
    }

    // 构建跳转，并填充 next_node_type
    let node_type_map: std::collections::HashMap<&str, i8> = all_nodes
        .iter()
        .map(|n| (n.node_code.as_str(), n.node_type))
        .collect();

    let mut all_skips: Vec<Skip> = Vec::new();
    for node_json in &def.node_list {
        for skip_json in &node_json.skip_list {
            let next_type = node_type_map
                .get(skip_json.next_node_code.as_str())
                .copied();
            let skip = Skip {
                id: None,
                definition_id: 0, // service 层填充
                now_node_code: skip_json.now_node_code.clone(),
                now_node_type: Some(node_json.node_type),
                next_node_code: skip_json.next_node_code.clone(),
                next_node_type: next_type,
                node_id: None,
                skip_name: skip_json.skip_name.clone(),
                skip_type: skip_json.skip_type.clone(),
                skip_condition: skip_json.skip_condition.clone(),
                coordinate: skip_json.coordinate.clone(),
                create_time: None,
                create_by: None,
                update_time: None,
                update_by: None,
                del_flag: Some("0".to_string()),
                tenant_id: None,
            };
            all_skips.push(skip);
        }
    }

    FlowCombine {
        definition,
        all_nodes,
        all_skips,
    }
}

/// Definition + Node 列表 + Skip 列表 -> DefJson
pub fn entities_to_def_json(
    def: &Definition,
    nodes: &[Node],
    skips: &[Skip],
) -> DefJson {
    let node_list: Vec<NodeJson> = nodes
        .iter()
        .map(|n| {
            let skip_list: Vec<SkipJson> = skips
                .iter()
                .filter(|s| s.now_node_code == n.node_code)
                .map(|s| SkipJson {
                    now_node_code: s.now_node_code.clone(),
                    next_node_code: s.next_node_code.clone(),
                    skip_name: s.skip_name.clone(),
                    skip_type: s.skip_type.clone(),
                    skip_condition: s.skip_condition.clone(),
                    coordinate: s.coordinate.clone(),
                    create_by: s.create_by.clone(),
                    update_by: s.update_by.clone(),
                    status: None,
                })
                .collect();

            NodeJson {
                node_type: n.node_type,
                node_code: n.node_code.clone(),
                node_name: n.node_name.clone(),
                version: Some(n.version.clone()),
                permission_flag: n.permission_flag.clone(),
                node_ratio: n.node_ratio.clone(),
                coordinate: n.coordinate.clone(),
                any_node_skip: n.any_node_skip.clone(),
                listener_type: n.listener_type.clone(),
                listener_path: n.listener_path.clone(),
                form_custom: n.form_custom.clone(),
                form_path: n.form_path.clone(),
                ext: n.ext.clone(),
                create_by: n.create_by.clone(),
                update_by: n.update_by.clone(),
                status: None,
                skip_list,
            }
        })
        .collect();

    DefJson {
        id: def.id,
        flow_code: def.flow_code.clone(),
        flow_name: def.flow_name.clone(),
        model_value: def.model_value.clone(),
        category: def.category.clone(),
        version: def.version.clone(),
        is_publish: Some(def.is_publish),
        form_custom: def.form_custom.clone(),
        form_path: def.form_path.clone(),
        listener_type: def.listener_type.clone(),
        listener_path: def.listener_path.clone(),
        ext: def.ext.clone(),
        create_by: def.create_by.clone(),
        update_by: def.update_by.clone(),
        node_list,
        chart_status_color: None,
        top_text: None,
        top_text_show: false,
    }
}
