use serde::{Deserialize, Serialize};

fn default_classics() -> String {
    "CLASSICS".to_string()
}

/// 宽松反序列化 i8：同时接受数字（0）和字符串（"0"）。
///
/// warm-flow 设计器导出的 JSON 中，nodeType/status/isPublish 等字段以字符串形式
/// 传输（如 "nodeType": "0"），而 DB 列为 tinyint、Rust 实体层用 i8。此辅助函数
/// 让 DefJson 反序列化层同时兼容两种形态，避免 "invalid type: string, expected i8"。
pub(crate) fn flexible_i8<'de, D>(deserializer: D) -> Result<i8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Null => Ok(0),
        serde_json::Value::Number(n) => n
            .as_i64()
            .and_then(|i| i8::try_from(i).ok())
            .ok_or_else(|| Error::custom(format!("数值超出 i8 范围: {}", n))),
        serde_json::Value::String(s) => s.trim().parse::<i8>().map_err(|e| {
            Error::custom(format!("无法解析为 i8 (\"{s}\"): {e}"))
        }),
        _ => Err(Error::custom(format!("期望数字或字符串，得到: {v}"))),
    }
}

/// 宽松反序列化 Option<i8>：null/缺失 -> None，其余规则同 flexible_i8。
pub(crate) fn flexible_opt_i8<'de, D>(
    deserializer: D,
) -> Result<Option<i8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let v = Option::<serde_json::Value>::deserialize(deserializer)?;
    match v {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => {
            let i = n.as_i64().and_then(|x| i8::try_from(x).ok()).ok_or_else(
                || Error::custom(format!("数值超出 i8 范围: {}", n)),
            )?;
            Ok(Some(i))
        }
        Some(serde_json::Value::String(s)) => {
            let i = s.trim().parse::<i8>().map_err(|e| {
                Error::custom(format!("无法解析为 i8 (\"{s}\"): {e}"))
            })?;
            Ok(Some(i))
        }
        Some(other) => {
            Err(Error::custom(format!("期望数字或字符串，得到: {other}")))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefJson {
    pub id: Option<i64>,
    pub flow_code: String,
    pub flow_name: String,
    #[serde(default = "default_classics")]
    pub model_value: String,
    pub category: Option<String>,
    pub version: String,
    #[serde(default, deserialize_with = "flexible_opt_i8")]
    pub is_publish: Option<i8>,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    pub listener_type: Option<String>,
    pub listener_path: Option<String>,
    pub ext: Option<String>,
    pub create_by: Option<String>,
    pub update_by: Option<String>,
    #[serde(default)]
    pub node_list: Vec<NodeJson>,
    pub chart_status_color: Option<Vec<String>>,
    pub top_text: Option<String>,
    #[serde(default)]
    pub top_text_show: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeJson {
    #[serde(deserialize_with = "flexible_i8")]
    pub node_type: i8,
    pub node_code: String,
    pub node_name: Option<String>,
    pub version: Option<String>,
    pub permission_flag: Option<String>,
    pub node_ratio: Option<String>,
    pub coordinate: Option<String>,
    pub any_node_skip: Option<String>,
    pub listener_type: Option<String>,
    pub listener_path: Option<String>,
    pub form_custom: Option<String>,
    pub form_path: Option<String>,
    pub ext: Option<String>,
    pub create_by: Option<String>,
    pub update_by: Option<String>,
    #[serde(default, deserialize_with = "flexible_opt_i8")]
    pub status: Option<i8>,
    #[serde(default)]
    pub skip_list: Vec<SkipJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkipJson {
    pub now_node_code: String,
    pub next_node_code: String,
    pub skip_name: Option<String>,
    pub skip_type: Option<String>,
    pub skip_condition: Option<String>,
    pub coordinate: Option<String>,
    pub create_by: Option<String>,
    pub update_by: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "flexible_opt_i8"
    )]
    pub status: Option<i8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_json_accepts_string_node_type() {
        // warm-flow 设计器实际输出：nodeType 为字符串
        let json = r#"{
            "nodeType": "0",
            "nodeCode": "start",
            "nodeName": "开始",
            "skipList": []
        }"#;
        let node: NodeJson = serde_json::from_str(json).unwrap();
        assert_eq!(node.node_type, 0);
        assert_eq!(node.node_code, "start");
    }

    #[test]
    fn node_json_accepts_numeric_node_type() {
        let json = r#"{
            "nodeType": 1,
            "nodeCode": "approve",
            "skipList": []
        }"#;
        let node: NodeJson = serde_json::from_str(json).unwrap();
        assert_eq!(node.node_type, 1);
    }

    #[test]
    fn def_json_accepts_mixed_string_number_types() {
        // 模拟 warm-flow 设计器完整保存请求：nodeType/status/isPublish 混用字符串
        let json = r#"{
            "flowCode": "leave1",
            "flowName": "请假流程",
            "modelValue": "CLASSICS",
            "version": "1",
            "isPublish": "0",
            "nodeList": [
                {"nodeType": "0", "nodeCode": "start", "skipList": [
                    {"nowNodeCode": "start", "nextNodeCode": "approve", "skipType": "PASS"}
                ]},
                {"nodeType": "1", "nodeCode": "approve", "status": "0", "skipList": [
                    {"nowNodeCode": "approve", "nextNodeCode": "end", "skipType": "PASS"}
                ]},
                {"nodeType": "2", "nodeCode": "end", "skipList": []}
            ]
        }"#;
        let def: DefJson = serde_json::from_str(json).unwrap();
        assert_eq!(def.is_publish, Some(0));
        assert_eq!(def.node_list.len(), 3);
        assert_eq!(def.node_list[0].node_type, 0);
        assert_eq!(def.node_list[1].node_type, 1);
        assert_eq!(def.node_list[1].status, Some(0));
        assert_eq!(def.node_list[2].node_type, 2);
        assert_eq!(
            def.node_list[0].skip_list[0].skip_type.as_deref(),
            Some("PASS")
        );
    }

    #[test]
    fn flexible_i8_rejects_garbage() {
        let json = r#"{"nodeType": "abc", "nodeCode": "x", "skipList": []}"#;
        let result: Result<NodeJson, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
