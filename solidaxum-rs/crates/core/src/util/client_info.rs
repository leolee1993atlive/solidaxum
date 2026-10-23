//! 客户端信息提取(IP / UA 解析),用于登录日志完整性。
//!
//! 设计原则:
//! - 纯函数,不依赖外部网络/数据库,可在脚手架内单测。
//! - 不引入 woothee/ip2region 等重依赖,采用简单的字符串启发式;
//!   复杂 UA 解析与归属地查询可作为后续 P2 改进。

use axum::http::HeaderMap;

/// 从请求头提取客户端 IP:
/// 1. `X-Forwarded-For` 第一项(经代理转发时的原始 IP)
/// 2. `X-Real-IP`(部分代理设置的直连 IP)
/// 3. 空字符串(调用方可在 axum ConnectInfo 中补 socket addr)
pub fn extract_client_ip(headers: &HeaderMap) -> String {
    if let Some(first) = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|xff| xff.split(',').map(|s| s.trim()).next())
        .filter(|s| !s.is_empty())
    {
        return first.to_string();
    }
    if let Some(real) = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        return real.to_string();
    }
    String::new()
}

/// 简易 UA 解析,返回 (browser, os)。
///
/// 识别常见浏览器与操作系统关键词;未知 UA 返回空字符串。
/// 这是 woothee 的轻量替代,覆盖主流场景即可,复杂的 UA 分类留待后续。
pub fn parse_user_agent(ua: &str) -> (String, String) {
    (parse_browser(ua), parse_os(ua))
}

fn parse_browser(ua: &str) -> String {
    let ua_lower = ua.to_lowercase();
    // 顺序敏感:更具体的先匹配(Edg/OPR 等基于 Chrome 的标识)
    if ua_lower.contains("edg/") {
        "Edge".to_string()
    } else if ua_lower.contains("opr/") || ua_lower.contains("opera") {
        "Opera".to_string()
    } else if ua_lower.contains("chrome/") {
        "Chrome".to_string()
    } else if ua_lower.contains("firefox/") {
        "Firefox".to_string()
    } else if ua_lower.contains("safari/") {
        "Safari".to_string()
    } else if ua_lower.contains("msie") || ua_lower.contains("trident/") {
        "IE".to_string()
    } else {
        String::new()
    }
}

fn parse_os(ua: &str) -> String {
    let ua_lower = ua.to_lowercase();
    if ua_lower.contains("windows nt 10") {
        "Windows 10".to_string()
    } else if ua_lower.contains("windows nt") {
        "Windows".to_string()
    } else if ua_lower.contains("mac os x") || ua_lower.contains("iphone") {
        "macOS".to_string()
    } else if ua_lower.contains("android") {
        "Android".to_string()
    } else if ua_lower.contains("linux") {
        "Linux".to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn make_headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        h
    }

    #[test]
    fn xff_first_ip_wins() {
        let h = make_headers(&[("x-forwarded-for", "1.2.3.4, 5.6.7.8")]);
        assert_eq!(extract_client_ip(&h), "1.2.3.4");
    }

    #[test]
    fn x_real_ip_fallback() {
        let h = make_headers(&[("x-real-ip", "9.9.9.9")]);
        assert_eq!(extract_client_ip(&h), "9.9.9.9");
    }

    #[test]
    fn empty_when_no_headers() {
        let h = HeaderMap::new();
        assert_eq!(extract_client_ip(&h), "");
    }

    #[test]
    fn xff_takes_precedence_over_x_real_ip() {
        let h = make_headers(&[
            ("x-forwarded-for", "1.1.1.1"),
            ("x-real-ip", "2.2.2.2"),
        ]);
        assert_eq!(extract_client_ip(&h), "1.1.1.1");
    }

    #[test]
    fn parse_chrome_on_windows() {
        let (b, o) = parse_user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/120.0.0.0 Safari/537.36",
        );
        assert_eq!(b, "Chrome");
        assert_eq!(o, "Windows 10");
    }

    #[test]
    fn parse_edge_variant() {
        let (b, _) = parse_user_agent("Mozilla/5.0 Edg/120.0.0.0");
        assert_eq!(b, "Edge");
    }

    #[test]
    fn parse_firefox_on_mac() {
        let (b, o) = parse_user_agent(
            "Mozilla/5.0 (Mac OS X 14.0) Gecko/20100101 Firefox/121.0",
        );
        assert_eq!(b, "Firefox");
        assert_eq!(o, "macOS");
    }

    #[test]
    fn parse_unknown_returns_empty() {
        let (b, o) = parse_user_agent("curl/8.0");
        assert_eq!(b, "");
        assert_eq!(o, "");
    }
}
