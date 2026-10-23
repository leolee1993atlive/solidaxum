//! 自定义请求提取器。

use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
    http::header::CONTENT_TYPE,
};

use crate::{error::AppError, serde::coerce::Coerce};

/// JSON 请求体提取器,行为同 [`axum::Json`],但额外做"字符串→数字"强制转换:
/// 前端把超出 JS 安全整数范围的 `i64`(雪花 ID)以字符串形式回传时,仍能正确
/// 反序列化为整数字段(对齐 Java/Jackson 默认行为,见 [`crate::serde::coerce`])。
///
/// 用法:把 handler 里的 `axum::Json(payload)` 换成 `AppJson(payload)` 即可,
/// 必须作为最后一个(消费 body 的)提取器参数。
#[derive(Debug, Clone, Copy, Default)]
pub struct AppJson<T>(pub T);

impl<T, S> FromRequest<S> for AppJson<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(
        req: Request,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        // 校验 Content-Type 为 JSON(缺省时也放行,兼容部分客户端)。
        if let Some(ct) = req.headers().get(CONTENT_TYPE) {
            let is_json = ct
                .to_str()
                .ok()
                .map(|s| {
                    let s = s.to_ascii_lowercase();
                    s.starts_with("application/json")
                        || (s.starts_with("application/")
                            && s.contains("+json"))
                })
                .unwrap_or(false);
            if !is_json {
                return Err(AppError::BadRequest(
                    "请求体必须为 JSON".to_string(),
                ));
            }
        }

        let bytes = Bytes::from_request(req, state).await.map_err(|e| {
            AppError::BadRequest(format!("读取请求体失败: {e}"))
        })?;

        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::BadRequest(format!("JSON 解析失败: {e}")))?;

        let data = T::deserialize(Coerce(value))
            .map_err(|e| AppError::BadRequest(format!("JSON 解析失败: {e}")))?;

        Ok(AppJson(data))
    }
}
