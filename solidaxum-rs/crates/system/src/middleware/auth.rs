use std::sync::Arc;

use app_core::{util::metrics, vo::response::R};
use axum::{
    extract::{MatchedPath, Request, State},
    http,
    http::StatusCode,
    middleware::Next,
    response,
    response::IntoResponse,
};

use crate::app::AppState;

#[tracing::instrument(skip(state, req, next))]
pub async fn auth(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<response::Response, StatusCode> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    tracing::debug!(method = %method, uri = %uri, "收到请求");

    let metrics_path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|mp| mp.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());

    // 1. 提取 token
    let token = match extract_token(&req) {
        Some(t) => t,
        None => {
            tracing::warn!(uri = %uri, "请求缺少认证凭据");
            metrics::record_http_request(method.as_str(), &metrics_path, 401);
            return Ok(R::<()>::error_with_code_and_message(
                401,
                "请求缺少认证凭据".to_string(),
            )
            .into_response());
        }
    };

    // 2. 从 Redis 获取 session,并**同一次往返**完成滑动续期
    //
    // 原先是 get_session() 取值、再单独 refresh_session_ttl() 续期,两次调用
    // 各取一次连接;而 deadpool 每次取连接都会发 UNWATCH + PING 做健康检查,
    // 于是一个读请求要发 6 条 Redis 命令,其中 4 条是纯开销。
    // GETEX 把取值与续期合成一条,连接也只取一次 → 6 条降到 3 条。
    let login_user = match state
        .services
        .token_service
        .get_session_and_refresh(&token, state.infra.token_ttl)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::warn!(uri = %uri, "token 不存在或已过期");
            metrics::record_http_request(method.as_str(), &metrics_path, 401);
            return Ok(R::<()>::error_with_code_and_message(
                401,
                "登录状态已过期，请重新登录".to_string(),
            )
            .into_response());
        }
        Err(e) => {
            tracing::error!(uri = %uri, error = %e, "Redis 查询 session 失败");
            metrics::record_http_request(method.as_str(), &metrics_path, 500);
            return Ok(R::<()>::error_with_code_and_message(
                500,
                "服务器内部错误".to_string(),
            )
            .into_response());
        }
    };

    // 3. 校验 clientid(优先 header,回退 URL query —— 供 SSE/EventSource 使用)
    let header_client_id = req
        .headers()
        .get("clientid")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| query_param(&req, "clientid"))
        .unwrap_or_default();

    if !header_client_id.is_empty() && header_client_id != login_user.client_id
    {
        tracing::warn!(
            uri = %uri,
            header_client_id = %header_client_id,
            token_client_id = %login_user.client_id,
            "clientid 与 token 不匹配"
        );
        metrics::record_http_request(method.as_str(), &metrics_path, 401);
        return Ok(R::<()>::error_with_code_and_message(
            401,
            "客户端ID与Token不匹配".to_string(),
        )
        .into_response());
    }

    // 4. 续期已在步骤 2 的 GETEX 里一并完成,此处不再单独发一次 EXPIRE。

    // 5. 注入 LoginUser 到 request extensions
    if let Ok(val) = login_user.user_id.to_string().parse() {
        req.headers_mut().insert("user_id", val);
    }
    req.extensions_mut().insert(login_user.clone());

    let _timer =
        metrics::RequestTimer::new(method.to_string(), metrics_path.clone());
    let mut resp = next.run(req).await;
    // 注入 LoginUser 到 response extensions (供 OperLogLayer 读取)
    resp.extensions_mut().insert(login_user);
    metrics::record_http_request(
        method.as_str(),
        &metrics_path,
        resp.status().as_u16(),
    );
    Ok(resp)
}

fn extract_token(req: &Request) -> Option<String> {
    // 优先从 Authorization header 取(标准方式)
    let from_header = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .and_then(strip_bearer);
    if let Some(t) = from_header {
        return Some(t);
    }

    // 回退:从 URL query 取。SSE/EventSource 无法设置自定义请求头,
    // 只能把 Authorization=Bearer xxx 放到 query 中(如 /resource/message)。
    let raw = query_param(req, "Authorization")?;
    strip_bearer(&raw)
}

/// 去掉 `Bearer ` 前缀;无前缀时按裸 token 处理。
fn strip_bearer(auth: &str) -> Option<String> {
    let token = auth.strip_prefix("Bearer ").unwrap_or(auth).trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

/// 从 URL query 中取出指定参数并做 percent 解码。
fn query_param(req: &Request, key: &str) -> Option<String> {
    let query = req.uri().query()?;
    form_urlencoded::parse(query.as_bytes())
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}
