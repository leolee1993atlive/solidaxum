use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    extract::State,
    http::{Method, Request, StatusCode, header::CONTENT_TYPE},
    middleware::Next,
    response::{IntoResponse, Response},
};
use md5::{Digest, Md5};

use crate::{
    error::AppError, middleware::state::MiddlewareState, vo::response::R,
};

/// 防重复提交中间件
///
/// 仅拦截 POST/PUT 请求，通过 Redis SETNX 在指定窗口内保证同一请求只执行一次。
/// key 格式为 `repeat_submit:{method}:{path}:{md5(token + body)}`。
pub async fn repeat_submit_middleware(
    State(state): State<Arc<MiddlewareState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let method = request.method().clone();

    if method != Method::POST && method != Method::PUT {
        return Ok(next.run(request).await);
    }

    let config = &state.repeat_submit_config;
    let path = request.uri().path().to_string();
    let token = extract_token(&request);

    // multipart/form-data 上传体为二进制内容,可能数 MB:
    // 1) 全量缓冲(to_bytes)浪费内存;2) 二进制 body MD5 去重意义不大且易误杀。
    // 故直接透传,不做防重放校验。
    if is_multipart_request(&request) {
        return Ok(next.run(request).await);
    }

    // 读取请求体并重建请求
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, usize::MAX)
        .await
        .map_err(|e| AppError::Internal(format!("读取请求体失败: {}", e)))?;

    let digest = md5_token_body(&token, &bytes);
    let key = format!("repeat_submit:{}:{}:{}", method, path, digest);

    let pool = &state.redis;
    let mut conn = pool.redis_pool.get().await.map_err(|e| {
        AppError::Internal(format!("Redis 连接获取失败: {}", e))
    })?;

    let set: Option<String> = redis::cmd("SET")
        .arg(&key)
        .arg("")
        .arg("NX")
        .arg("EX")
        .arg(config.ttl_seconds as i64)
        .query_async(&mut conn)
        .await
        .map_err(|e| {
            AppError::Internal(format!("重复提交锁设置失败: {}", e))
        })?;

    if set.is_none() {
        return Ok(R::<()>::error_with_code_and_message(
            StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
            "不允许重复提交，请稍候再试".to_string(),
        )
        .into_response());
    }

    // **必须在跑下游之前把 Redis 连接还回池子。**
    //
    // 不显式 drop 的话,`conn` 的作用域会一直延续到函数结尾,也就是覆盖整个
    // `next.run()` —— 包括登录时约 45ms 的 bcrypt、以及写请求的全部数据库
    // 操作。一条连接被这样占着的时长与**下游延迟成正比**,池子很快见底。
    //
    // 实测(写场景,池大小 32):
    //   axum + MySQL   25 迭代/秒 业务成功率 63%,40 迭代/秒 39.8%
    //                  失败全是「Redis 连接获取失败: Timeout waiting for a slot」
    //   axum + PG(更快)同速率 100%
    // 同一份代码,下游换快就正常 —— 这正是「持有时长 ∝ 下游延迟」的证据。
    drop(conn);

    // 用原 body 重建请求，确保下游 handler 能再次读取
    let new_request = Request::from_parts(parts, Body::from(bytes));
    Ok(next.run(new_request).await)
}

fn extract_token(request: &Request<Body>) -> String {
    request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.strip_prefix("Bearer ").unwrap_or(s).to_string())
        .unwrap_or_else(|| "anonymous".to_string())
}

fn md5_token_body(token: &str, body: &axum::body::Bytes) -> String {
    let mut hasher = Md5::new();
    hasher.update(token.as_bytes());
    hasher.update(body.as_ref());
    format!("{:x}", hasher.finalize())
}

/// 判断是否为 `multipart/form-data` 请求(对照 xss_filter.rs 的 is_json_request 写法)。
/// 二进制上传体无需防重放校验,直接透传。
fn is_multipart_request(request: &Request<Body>) -> bool {
    request
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| {
            let ct = ct.split(';').next().unwrap_or(ct).trim().to_lowercase();
            ct == "multipart/form-data"
        })
        .unwrap_or(false)
}
