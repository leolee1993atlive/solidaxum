use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    extract::State,
    http::{Method, Request, Uri, header::CONTENT_TYPE},
    middleware::Next,
    response::Response,
};
use serde_json::Value;

use crate::{error::AppError, middleware::state::MiddlewareState};

/// XSS 过滤中间件
///
/// 跳过 GET/DELETE 请求以及排除路径，对 JSON 请求体和查询参数中的字符串值进行 HTML/JS 清理。
pub async fn xss_filter_middleware(
    State(state): State<Arc<MiddlewareState>>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let method = request.method().clone();

    if method == Method::GET || method == Method::DELETE {
        return Ok(next.run(request).await);
    }

    let path = request.uri().path().to_string();
    if state
        .xss_config
        .exclude_paths
        .iter()
        .any(|p| path.starts_with(p))
    {
        return Ok(next.run(request).await);
    }

    // 清理查询参数
    if let Some(query) = request.uri().query() {
        let sanitized = sanitize_query_string(query);
        if sanitized != query {
            let new_uri = build_uri(request.uri().path(), &sanitized);
            *request.uri_mut() = new_uri;
        }
    }

    // 仅处理 JSON 请求体
    if is_json_request(request.headers()) {
        let (parts, body) = request.into_parts();
        let bytes = to_bytes(body, usize::MAX).await.map_err(|e| {
            AppError::Internal(format!("读取请求体失败: {}", e))
        })?;

        if !bytes.is_empty() {
            let mut json: Value =
                serde_json::from_slice(&bytes).map_err(|e| {
                    AppError::BadRequest(format!("请求体 JSON 格式错误: {}", e))
                })?;
            sanitize_json_value(&mut json);
            let new_bytes = serde_json::to_vec(&json).map_err(|e| {
                AppError::Internal(format!("请求体序列化失败: {}", e))
            })?;
            request = Request::from_parts(parts, Body::from(new_bytes));
        } else {
            request = Request::from_parts(parts, Body::from(bytes));
        }
    }

    Ok(next.run(request).await)
}

fn is_json_request(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| {
            let ct = ct.split(';').next().unwrap_or(ct).trim().to_lowercase();
            ct == "application/json"
        })
        .unwrap_or(false)
}

fn sanitize_json_value(value: &mut Value) {
    match value {
        Value::String(s) => *s = ammonia::clean(s),
        Value::Array(arr) => {
            for item in arr {
                sanitize_json_value(item);
            }
        }
        Value::Object(map) => {
            for v in map.values_mut() {
                sanitize_json_value(v);
            }
        }
        _ => {}
    }
}

fn sanitize_query_string(query: &str) -> String {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            if let Some((key, value)) = pair.split_once('=') {
                let sanitized_value = ammonia::clean(&percent_decode(value));
                format!("{}={}", key, percent_encode(&sanitized_value))
            } else {
                // 没有值的键直接返回
                pair.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn build_uri(path: &str, query: &str) -> Uri {
    let uri = if query.is_empty() {
        path.to_string()
    } else {
        format!("{}?{}", path, query)
    };
    uri.parse().unwrap_or_else(|_| Uri::from_static("/"))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) =
                (hex_value(bytes[i + 1]), hex_value(bytes[i + 2]))
            {
                out.push(h * 16 + l);
                i += 3;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn percent_encode(s: &str) -> String {
    const UNRESERVED: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~";
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if UNRESERVED.contains(&b) {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{:02X}", b));
        }
    }
    out
}

fn hex_value(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}
