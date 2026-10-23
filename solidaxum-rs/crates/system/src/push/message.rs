use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushMessage {
    pub message_id: i64,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub source: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub timestamp: i64,
    #[serde(skip)]
    pub to_user_id: Option<i64>,
}

impl PushMessage {
    pub fn notice(
        message_id: i64,
        message: String,
        data: serde_json::Value,
        path: Option<String>,
    ) -> Self {
        Self {
            message_id,
            msg_type: "notice".to_string(),
            source: "notice".to_string(),
            message,
            data: Some(data),
            path,
            timestamp: chrono::Utc::now().timestamp_millis(),
            to_user_id: None,
        }
    }

    /// 工作流催办/待办通知（对齐 Java PushSourceEnum.WORKFLOW）。
    pub fn workflow(
        message_id: i64,
        message: String,
        path: Option<String>,
    ) -> Self {
        Self {
            message_id,
            msg_type: "message".to_string(),
            source: "workflow".to_string(),
            message,
            data: None,
            path,
            timestamp: chrono::Utc::now().timestamp_millis(),
            to_user_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_vue_push_payload_shape() {
        let msg = PushMessage::notice(
            42,
            "新公告：系统维护".into(),
            serde_json::json!({ "noticeContent": "<p>正文</p>", "noticeTitle": "系统维护" }),
            Some("/system/notice?noticeId=42".into()),
        );
        let v = serde_json::to_value(&msg).unwrap();
        assert_eq!(v["type"], "notice");
        assert_eq!(v["source"], "notice");
        assert_eq!(v["message"], "新公告：系统维护");
        assert!(v["data"]["noticeContent"].is_string());
        assert_eq!(v["messageId"], 42);
        assert!(v["timestamp"].is_i64());
    }
}
