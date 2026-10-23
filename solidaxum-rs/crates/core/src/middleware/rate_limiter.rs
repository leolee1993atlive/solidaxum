use std::{net::SocketAddr, sync::Arc};

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{
    error::AppError, middleware::state::MiddlewareState,
    model::login_user::LoginUser, vo::response::R,
};

/// 限流中间件
///
/// 基于 Redis 固定窗口计数实现，key 格式为 `rate_limit:{method}:{path}:{user_id_or_ip}`。
pub async fn rate_limit_middleware(
    State(state): State<Arc<MiddlewareState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let config = &state.rate_limit_config;

    // capacity 为 0 或路径在排除列表中时跳过
    if config.capacity == 0 {
        return Ok(next.run(request).await);
    }

    let path = request.uri().path().to_string();
    if config.exclude_paths.iter().any(|p| path.starts_with(p)) {
        return Ok(next.run(request).await);
    }

    let method = request.method().as_str().to_string();
    let user_id_or_ip = get_user_id_or_ip(&request);
    let key = format!("rate_limit:{}:{}:{}", method, path, user_id_or_ip);

    let pool = &state.redis;
    let mut conn = pool.redis_pool.get().await.map_err(|e| {
        AppError::Internal(format!("Redis 连接获取失败: {}", e))
    })?;

    let count: i64 = redis::cmd("INCR")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .map_err(|e| AppError::Internal(format!("限流计数失败: {}", e)))?;

    if count == 1 {
        let _: bool = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(config.window_secs as i64)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                AppError::Internal(format!("限流窗口设置失败: {}", e))
            })?;
    }

    if count > config.capacity as i64 {
        tracing::warn!(
            method = %method,
            path = %path,
            user = %user_id_or_ip,
            "请求过于频繁"
        );
        return Ok(R::<()>::error_with_code_and_message(
            StatusCode::TOO_MANY_REQUESTS.as_u16() as i32,
            "访问过于频繁，请稍候再试".to_string(),
        )
        .into_response());
    }

    Ok(next.run(request).await)
}

/// 生成限流 key 的第三部分：优先取登录用户 ID，否则取客户端 IP
fn get_user_id_or_ip(request: &Request<Body>) -> String {
    if let Some(login_user) = request.extensions().get::<LoginUser>() {
        return login_user.user_id.to_string();
    }

    if let Some(forwarded) = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
        return forwarded
            .split(',')
            .next()
            .unwrap_or("unknown")
            .trim()
            .to_string();
    }

    if let Some(real_ip) = request
        .headers()
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
    {
        return real_ip.to_string();
    }

    request
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
