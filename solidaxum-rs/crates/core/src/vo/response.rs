use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum R<T> {
    NoDataSuccess(ApiResponse<()>),
    Success(ApiResponse<T>),
    Error(ApiResponse<()>),
}

impl<T> IntoResponse for R<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        match self {
            R::NoDataSuccess(r) => r.into_response(),
            R::Success(r) => r.into_response(),
            R::Error(r) => r.into_response(),
        }
    }
}

impl<T> R<T> {
    pub fn ok() -> Self {
        R::NoDataSuccess(ApiResponse::<()>::ok())
    }

    pub fn ok_with_data(data: T) -> Self {
        R::Success(ApiResponse::ok_with_data(data))
    }

    #[allow(dead_code)]
    pub fn ok_with_data_and_message(data: T, message: String) -> Self {
        R::Success(ApiResponse::ok_with_data_and_message(data, message))
    }

    pub fn error_with_code_and_message(code: i32, message: String) -> Self {
        R::Error(ApiResponse::<()>::error_with_code_and_message(
            code, message,
        ))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub msg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn ok() -> ApiResponse<()> {
        ApiResponse {
            code: 200,
            msg: "操作成功".to_string(),
            data: None,
        }
    }

    pub fn ok_with_data(data: T) -> ApiResponse<T> {
        ApiResponse {
            code: 200,
            msg: "操作成功".to_string(),
            data: Some(data),
        }
    }

    pub fn ok_with_data_and_message(data: T, msg: String) -> ApiResponse<T> {
        ApiResponse {
            code: 200,
            msg,
            data: Some(data),
        }
    }

    pub fn error_with_code_and_message(
        code: i32,
        msg: String,
    ) -> ApiResponse<()> {
        ApiResponse {
            code,
            msg,
            data: None,
        }
    }
}

/// JS `Number.MAX_SAFE_INTEGER` —— 超出此范围的整数在前端会丢失精度。
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
/// JS `Number.MIN_SAFE_INTEGER`。
const MIN_SAFE_INTEGER: i64 = -9_007_199_254_740_991;

/// 判定 JSON 键是否为 ID 语义 —— 命中则该键下的整数一律序列化为字符串。
///
/// 输出主体是 camelCase，但 job 模块实体未加 `rename_all`，故 snake_case
/// 后缀也要覆盖。不带 `Id` 后缀的语义 ID 键(实测自各 VO 的 i64 字段)显式列出。
fn is_id_key(key: &str) -> bool {
    key.ends_with("Id")
        || key.ends_with("Ids")
        || key.ends_with("_id")
        || key.ends_with("_ids")
        || matches!(
            key,
            "id"
                | "leader"        // sys_dept.leader = user_id
                | "avatar"        // sys_user.avatar = oss_id
                | "associated"    // flow_user.associated = 关联业务 id
                | "dictCode"      // sys_dict_data 主键
                | "dict_code"
                | "createBy"      // 创建者 user_id
                | "create_by"
                | "updateBy"      // 更新者 user_id
                | "update_by"
                | "createDept"    // 创建部门 dept_id
                | "create_dept"
                | "checkedKeys"   // 角色菜单树回显的 menu_id 数组
                | "checked_keys"
        )
}

/// 递归遍历 JSON,把 **ID 字段** 与 **超出 JS 安全整数范围的整数** 序列化为字符串。
///
/// 公开是为了让 SSE 等非 HTTP-响应场景复用同一规则;HTTP 响应无需直接调用,
/// `R<T>` / `ApiResponse<T>` / [`IdJson`] 已内置。
pub fn stringify_ids(value: &mut serde_json::Value, key: Option<&str>) {
    use serde_json::Value;
    let is_id = key.is_some_and(is_id_key);
    match value {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if is_id || !(MIN_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&i)
                {
                    *value = Value::String(i.to_string());
                }
            } else if let Some(u) = n.as_u64()
                && (is_id || u > MAX_SAFE_INTEGER as u64)
            {
                *value = Value::String(u.to_string());
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                stringify_ids(v, key);
            }
        }
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                stringify_ids(v, Some(k.as_str()));
            }
        }
        _ => {}
    }
}

/// 与 [`axum::Json`] 等价,但对 ID 字段套用与 `R<T>` 信封一致的字符串化规则。
///
/// 用于少数**不走 `R<T>` 信封**的裸 JSON 响应(warm-flow UI 兼容层)——
/// 那些端点的响应形状由前端第三方组件规定,套不进 `ApiResponse`,
/// 但 ID 精度约束一视同仁。
pub struct IdJson(pub serde_json::Value);

impl IntoResponse for IdJson {
    fn into_response(self) -> Response {
        let mut value = self.0;
        stringify_ids(&mut value, None);
        Json(value).into_response()
    }
}

impl<T> IntoResponse for ApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        let status_code = StatusCode::from_u16(self.code as u16)
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        match serde_json::to_value(&self) {
            Ok(mut value) => {
                stringify_ids(&mut value, None);
                (status_code, Json(value)).into_response()
            }
            Err(_) => (status_code, Json(self)).into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, response::IntoResponse};

    use super::*;

    #[tokio::test]
    async fn test_ok_serializes_with_msg_field() {
        let resp = R::<()>::ok().into_response();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["code"], 200);
        assert!(
            json["msg"].is_string(),
            "field should be 'msg', got: {}",
            json
        );
        assert!(json.get("data").is_none() || json["data"].is_null());
    }

    #[tokio::test]
    async fn test_ok_with_data_has_data() {
        let resp = R::ok_with_data(vec!["a", "b"]).into_response();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["code"], 200);
        assert_eq!(json["data"], serde_json::json!(["a", "b"]));
    }

    #[tokio::test]
    async fn test_error_has_correct_code_and_msg() {
        let resp =
            R::<()>::error_with_code_and_message(500, "服务器错误".to_string())
                .into_response();
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(status, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(json["code"], 500);
        assert_eq!(json["msg"], "服务器错误");
    }

    #[tokio::test]
    async fn test_id_fields_always_stringified() {
        // 新规则:ID 键无论大小一律字符串,非 ID 键保持数字。
        let big: i64 = 1_761_100_000_000_000_000;
        let resp = R::ok_with_data(serde_json::json!({
            "userId": big,
            "deptId": 100_i64,
            "parentId": 0_i64,
            "id": 7_i64,
            "total": 25_i64,
            "pageNum": 1_i64,
            "orderNum": 3_i64,
            "costTime": 128_i64,
        }))
        .into_response();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let d = &json["data"];
        assert_eq!(d["userId"], big.to_string());
        assert_eq!(d["deptId"], "100", "小 ID 也必须是字符串: {d}");
        assert_eq!(d["parentId"], "0", "根节点 0 也必须是字符串: {d}");
        assert_eq!(d["id"], "7");
        assert!(d["total"].is_number(), "total 不是 ID: {d}");
        assert!(d["pageNum"].is_number(), "pageNum 不是 ID: {d}");
        assert!(d["orderNum"].is_number(), "orderNum 不是 ID: {d}");
        assert!(d["costTime"].is_number(), "costTime 不是 ID: {d}");
    }

    #[tokio::test]
    async fn test_id_arrays_inherit_parent_key() {
        // 数组元素没有自己的键,须继承父键才能命中 *Ids 规则。
        let resp = R::ok_with_data(serde_json::json!({
            "menuIds": [1_i64, 2_i64],
            "checkedKeys": [100_i64],
            "rows": [{ "roleId": 200_i64, "roleSort": 0_i64 }],
        }))
        .into_response();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let d = &json["data"];
        assert_eq!(d["menuIds"], serde_json::json!(["1", "2"]));
        assert_eq!(d["checkedKeys"], serde_json::json!(["100"]));
        assert_eq!(d["rows"][0]["roleId"], "200");
        assert!(d["rows"][0]["roleSort"].is_number());
    }

    #[tokio::test]
    async fn test_semantic_id_keys_without_id_suffix() {
        // 这些键不带 Id 后缀但语义是 ID(实测自各 VO 的 i64 字段)。
        let resp = R::ok_with_data(serde_json::json!({
            "dictCode": 1_i64,
            "createBy": 1_i64,
            "updateBy": 2_i64,
            "createDept": 100_i64,
            "leader": 3_i64,
            "avatar": 9001_i64,
            "associated": 5_i64,
            "job_id": 42_i64,
            "job_log_id": 7_i64,
        }))
        .into_response();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let d = &json["data"];
        for k in [
            "dictCode",
            "createBy",
            "updateBy",
            "createDept",
            "leader",
            "avatar",
            "associated",
            "job_id",
            "job_log_id",
        ] {
            assert!(d[k].is_string(), "{k} 应为字符串: {d}");
        }
    }

    #[tokio::test]
    async fn test_non_id_large_number_still_stringified() {
        // 超范围兜底不能因新规则丢失:非 ID 的大整数仍转字符串。
        let big: i64 = 9_007_199_254_740_993;
        let resp = R::ok_with_data(serde_json::json!({ "costTime": big }))
            .into_response();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["data"]["costTime"], big.to_string());
    }

    #[tokio::test]
    async fn test_id_json_applies_same_rule() {
        // 不走 R<T> 信封的裸 JSON 响应,也要拿到同一套 ID 规则。
        let resp = IdJson(serde_json::json!({
            "code": 200,
            "data": { "id": 1_i64, "nodeName": "开始", "weight": 5_i64 }
        }))
        .into_response();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["data"]["id"], "1");
        assert_eq!(json["data"]["nodeName"], "开始");
        assert!(json["data"]["weight"].is_number(), "weight 不是 ID: {json}");
    }
}
