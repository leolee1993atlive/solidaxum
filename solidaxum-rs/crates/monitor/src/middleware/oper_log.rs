use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Instant,
};

use app_core::model::login_user::LoginUser;
use axum::{
    body::{Body, Bytes},
    extract::Request,
    http::Method,
    response::Response,
};
use tower::{Layer, Service};

use crate::operlog::{
    model::{OperLog, OperatorType, business_type_from_method},
    service::OperLogService,
};

#[derive(Clone)]
pub struct OperLogLayer {
    pub oper_log_service: OperLogService,
    pub title_rules: TitleRules,
}

/// 路径前缀 → 操作模块名的补充规则。装配层注册,供下游产品加自己的业务路径。
/// 内置规则先匹配,未命中再查本表,仍未命中回落「其他操作」。
pub type TitleRules = &'static [(&'static str, &'static str)];

impl OperLogLayer {
    pub fn new(
        oper_log_service: OperLogService,
        title_rules: TitleRules,
    ) -> Self {
        Self {
            oper_log_service,
            title_rules,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OperLogMeta {
    pub title: String,
    pub business_type: i32,
    pub operator_type: i32,
}

impl OperLogMeta {
    pub fn new(title: impl Into<String>, business_type: i32) -> Self {
        Self {
            title: title.into(),
            business_type,
            operator_type: OperatorType::MANAGE,
        }
    }
}

impl<S> Layer<S> for OperLogLayer {
    type Service = OperLogMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        OperLogMiddleware {
            inner,
            oper_log_service: self.oper_log_service.clone(),
            title_rules: self.title_rules,
        }
    }
}

#[derive(Clone)]
pub struct OperLogMiddleware<S> {
    inner: S,
    oper_log_service: OperLogService,
    title_rules: TitleRules,
}

impl<S> Service<Request> for OperLogMiddleware<S>
where
    S: Service<Request, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<
        Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let start = Instant::now();
        let method = req.method().clone();
        let uri = req.uri().clone();
        let oper_url = uri.path().to_string();

        // 跳过 GET 请求——绝大多数 @Log 标注在写操作上（POST/PUT/DELETE）。
        // 导出类 GET 的日志后续可通过路由元数据补充（W6 idiom 改进）。
        if method == Method::GET {
            let inner = self.inner.clone();
            let mut inner = std::mem::replace(&mut self.inner, inner);
            return Box::pin(async move { inner.call(req).await });
        }

        let user_agent = req
            .headers()
            .get(axum::http::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let (browser, os) = parse_user_agent(&user_agent);
        let oper_ip = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(',')
            .next()
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        // 从 REQUEST extensions 读取 LoginUser（auth 中间件已注入）
        let login_user = req.extensions().get::<LoginUser>().cloned();
        let oper_name = login_user
            .as_ref()
            .map(|u| u.username.clone())
            .unwrap_or_default();
        let user_id = login_user.as_ref().map(|u| u.user_id);
        let dept_id = login_user.as_ref().and_then(|u| u.dept_id);
        let dept_name: Option<String> = None;
        let client_key = login_user.as_ref().and_then(|u| u.client_key.clone());
        let device_type =
            login_user.as_ref().and_then(|u| u.device_type.clone());

        let meta = req.extensions().get::<OperLogMeta>().cloned();

        let oper_log_service = self.oper_log_service.clone();
        let inner = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, inner);

        // multipart/form-data 上传体可能数 MB 且为二进制内容:
        // extract_body_bytes 的 64KB 上限会触发 unwrap_or_default() 把 body 截成空,
        // 导致下游 Multipart 解析失败("读取上传文件失败")。故对 multipart 请求
        // 跳过 body 读取/重建,oper_param 留空;响应体(R<T> JSON,体积小)仍照常记录。
        let skip_body = is_multipart_request(req.headers());
        let title_rules = self.title_rules;

        Box::pin(async move {
            // (oper_param, response): multipart 路径不碰 body 原样透传,
            // 其它路径读取+脱敏+重建 body 再交给下游 handler。
            let (oper_param, response) = if skip_body {
                (String::new(), inner.call(req).await?)
            } else {
                let (req_parts, req_body_bytes) = extract_body_bytes(req).await;
                let raw_param =
                    String::from_utf8_lossy(&req_body_bytes).to_string();
                let oper_param = desensitize_param(&raw_param)
                    .chars()
                    .take(4000)
                    .collect::<String>();
                let new_req =
                    Request::from_parts(req_parts, Body::from(req_body_bytes));
                (oper_param, inner.call(new_req).await?)
            };

            let cost_time = start.elapsed().as_millis() as i64;
            let status_code = response.status();
            let (resp_parts, resp_body_bytes) =
                extract_response_body(response).await;
            let raw_json =
                String::from_utf8_lossy(&resp_body_bytes).to_string();
            let json_result = desensitize_param(&raw_json)
                .chars()
                .take(4000)
                .collect::<String>();

            let (log_status, error_msg) =
                determine_log_status(&json_result, status_code);

            let (title, business_type, operator_type) = resolve_log_meta(
                meta.as_ref(),
                &oper_url,
                &method,
                title_rules,
            );

            let log = OperLog::new(
                title,
                business_type,
                operator_type,
                oper_name,
                oper_url,
                method.to_string(),
                oper_ip,
                oper_param,
                json_result,
                log_status,
                error_msg,
                cost_time,
                user_id,
                dept_id,
                dept_name,
                client_key,
                device_type,
                Some(browser),
                Some(os),
            );

            tokio::spawn(async move {
                // 无会话路径(响应成形后后台异步落库):system_context,
                // 审计人=系统;sys_oper_log 无审计列/软删列/范围列,
                // 与用户 ctx 行为逐点一致(生产
                // system_context 消费)。
                let ctx = oper_log_service.system_context();
                if let Err(e) = oper_log_service.record(&ctx, &log).await {
                    tracing::warn!(error = %e, "异步写入操作日志失败");
                }
            });

            let new_resp =
                Response::from_parts(resp_parts, Body::from(resp_body_bytes));
            Ok(new_resp)
        })
    }
}

async fn extract_body_bytes(
    req: Request,
) -> (axum::http::request::Parts, Bytes) {
    let (parts, body) = req.into_parts();
    let bytes = axum::body::to_bytes(body, 65536).await.unwrap_or_default();
    (parts, bytes)
}

async fn extract_response_body(
    resp: Response,
) -> (axum::http::response::Parts, Bytes) {
    let (parts, body) = resp.into_parts();
    let bytes = axum::body::to_bytes(body, 65536).await.unwrap_or_default();
    (parts, bytes)
}

/// 判断是否为 `multipart/form-data` 请求(对照 xss_filter.rs 的 is_json_request 写法)。
/// oper_log/repeat_submit 均需据此跳过二进制上传体,避免缓冲截断破坏 Multipart 解析。
fn is_multipart_request(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| {
            let ct = ct.split(';').next().unwrap_or(ct).trim().to_lowercase();
            ct == "multipart/form-data"
        })
        .unwrap_or(false)
}

fn resolve_log_meta(
    meta: Option<&OperLogMeta>,
    oper_url: &str,
    method: &Method,
    extra: TitleRules,
) -> (String, i32, i32) {
    match meta {
        Some(m) => (m.title.clone(), m.business_type, m.operator_type),
        None => (
            derive_title(oper_url, method, extra),
            business_type_from_method(method.as_str()),
            OperatorType::MANAGE,
        ),
    }
}

pub fn derive_title(path: &str, _method: &Method, extra: TitleRules) -> String {
    if path.starts_with("/system/user") {
        "用户管理".to_string()
    } else if path.starts_with("/system/role") {
        "角色管理".to_string()
    } else if path.starts_with("/system/menu") {
        "菜单管理".to_string()
    } else if path.starts_with("/system/dept") {
        "部门管理".to_string()
    } else if path.starts_with("/system/post") {
        "岗位管理".to_string()
    } else if path.starts_with("/system/dict") {
        "字典管理".to_string()
    } else if path.starts_with("/system/config") {
        "参数管理".to_string()
    } else if path.starts_with("/system/notice") {
        "通知公告".to_string()
    } else if path.starts_with("/monitor/operlog") {
        "操作日志".to_string()
    } else if path.starts_with("/monitor/loginInfo") {
        "登录日志".to_string()
    } else if path.starts_with("/monitor/online") {
        "在线用户".to_string()
    } else if path.starts_with("/monitor/cache") {
        "缓存监控".to_string()
    } else if path.starts_with("/auth/login") {
        "登录".to_string()
    } else if path.starts_with("/auth/logout") {
        "登出".to_string()
    } else if path.starts_with("/auth/register") {
        "注册".to_string()
    } else {
        // 内置未命中 → 查注册表(最长前缀优先,规则顺序无关)
        extra
            .iter()
            .filter(|(prefix, _)| path.starts_with(prefix))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, name)| name.to_string())
            .unwrap_or_else(|| "其他操作".to_string())
    }
}

fn parse_user_agent(ua: &str) -> (String, String) {
    let ua_lower = ua.to_lowercase();
    let browser = if ua_lower.contains("edg/") || ua_lower.contains("edge/") {
        "Edge"
    } else if ua_lower.contains("chrome/") && !ua_lower.contains("edg/") {
        "Chrome"
    } else if ua_lower.contains("firefox/") {
        "Firefox"
    } else if ua_lower.contains("safari/") && !ua_lower.contains("chrome/") {
        "Safari"
    } else {
        "Unknown"
    };
    let os = if ua_lower.contains("windows nt 10") {
        "Windows 10"
    } else if ua_lower.contains("windows") {
        "Windows"
    } else if ua_lower.contains("mac os x") || ua_lower.contains("macintosh") {
        "Mac OS"
    } else if ua_lower.contains("linux") {
        "Linux"
    } else if ua_lower.contains("android") {
        "Android"
    } else if ua_lower.contains("iphone") || ua_lower.contains("ipad") {
        "iOS"
    } else {
        "Unknown"
    };
    (browser.to_string(), os.to_string())
}

/// 从请求体 JSON 中移除敏感字段（password 等），返回安全可入库的字符串。
fn desensitize_param(raw: &str) -> String {
    if raw.is_empty() {
        return raw.to_string();
    }
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(mut v) => {
            strip_sensitive(&mut v);
            serde_json::to_string(&v).unwrap_or_else(|_| raw.to_string())
        }
        Err(_) => raw.to_string(),
    }
}

const SENSITIVE_KEYS: [&str; 10] = [
    "password",
    "oldPassword",
    "newPassword",
    "confirmPassword",
    "secret",
    "secretKey",
    "accessKey",
    "token",
    "access_token",
    "refresh_token",
];

fn strip_sensitive(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            map.retain(|k, _| {
                !SENSITIVE_KEYS.iter().any(|s| k.eq_ignore_ascii_case(s))
            });
            for (_, child) in map.iter_mut() {
                strip_sensitive(child);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                strip_sensitive(item);
            }
        }
        _ => {}
    }
}

fn parse_response_code(v: &serde_json::Value) -> Option<i64> {
    v.get("code").and_then(|c| {
        c.as_i64()
            .or_else(|| c.as_str().and_then(|s| s.parse().ok()))
            .or_else(|| c.as_f64().map(|f| f as i64))
    })
}

/// 从响应体 JSON 中解析 R<T>.code，判断业务是否成功。
/// 返回 (status: i32, error_msg: String)。status 0=成功, 1=失败。
fn determine_log_status(
    resp_body: &str,
    http_status: axum::http::StatusCode,
) -> (i32, String) {
    if !http_status.is_success() {
        return (1, resp_body.chars().take(2000).collect());
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(resp_body)
        && let Some(code) = parse_response_code(&v)
        && code != 200
    {
        let msg = v
            .get("msg")
            .or_else(|| v.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .chars()
            .take(2000)
            .collect::<String>();
        return (1, msg);
    }
    (0, String::new())
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;
    use crate::operlog::model::BusinessType;

    #[test]
    fn desensitize_removes_password_fields() {
        let input = r#"{"username":"admin","password":"secret123"}"#;
        let output = desensitize_param(input);
        let v: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(v.get("password").is_none());
        assert_eq!(v["username"], "admin");
    }

    #[test]
    fn desensitize_handles_nested_password() {
        let input =
            r#"{"user":{"name":"a","password":"x"},"list":[{"password":"y"}]}"#;
        let output = desensitize_param(input);
        let v: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(v["user"].get("password").is_none());
        assert!(v["list"][0].get("password").is_none());
    }

    #[test]
    fn desensitize_preserves_non_json() {
        let input = "not json at all";
        let output = desensitize_param(input);
        assert_eq!(output, input);
    }

    #[test]
    fn determine_status_http_error() {
        let (status, _) =
            determine_log_status("error", StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(status, 1);
    }

    #[test]
    fn determine_status_business_error() {
        let body = r#"{"code":500,"msg":"参数错误"}"#;
        let (status, msg) = determine_log_status(body, StatusCode::OK);
        assert_eq!(status, 1);
        assert_eq!(msg, "参数错误");
    }

    #[test]
    fn determine_status_success() {
        let body = r#"{"code":200,"msg":"ok","data":{}}"#;
        let (status, _) = determine_log_status(body, StatusCode::OK);
        assert_eq!(status, 0);
    }

    #[test]
    fn desensitize_is_case_insensitive() {
        let input = r#"{"Password":"secret","ACCESS_TOKEN":"tok"}"#;
        let output = desensitize_param(input);
        let v: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(v.get("Password").is_none());
        assert!(v.get("ACCESS_TOKEN").is_none());
    }

    #[test]
    fn desensitize_response_body_removes_tokens() {
        let input = r#"{"code":200,"data":{"access_token":"abc","refresh_token":"xyz"}}"#;
        let output = desensitize_param(input);
        let v: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(v["data"].get("access_token").is_none());
        assert!(v["data"].get("refresh_token").is_none());
    }

    #[test]
    fn determine_status_string_code() {
        let body = r#"{"code":"500","msg":"error"}"#;
        let (status, _) = determine_log_status(body, StatusCode::OK);
        assert_eq!(status, 1);
    }

    #[test]
    fn determine_status_non_json_http_200() {
        let (status, _) = determine_log_status("plain text", StatusCode::OK);
        assert_eq!(status, 0);
    }

    #[test]
    fn resolve_log_meta_prefers_explicit_meta() {
        let meta = OperLogMeta::new("清空操作日志", BusinessType::CLEAN);
        let (title, bt, ot) = resolve_log_meta(
            Some(&meta),
            "/monitor/operlog/clean",
            &Method::POST,
            &[],
        );
        assert_eq!(title, "清空操作日志");
        assert_eq!(bt, BusinessType::CLEAN);
        assert_eq!(ot, OperatorType::MANAGE);
    }

    #[test]
    fn resolve_log_meta_falls_back_to_heuristics() {
        let (title, bt, ot) = resolve_log_meta(
            None,
            "/monitor/operlog/clean",
            &Method::POST,
            &[],
        );
        assert_eq!(title, "操作日志");
        assert_eq!(bt, BusinessType::INSERT);
        assert_eq!(ot, OperatorType::MANAGE);
    }
}
