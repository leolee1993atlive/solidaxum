use super::{registry::ListenerRegistry, types::ListenerVariable};

/// 解析 listener_type / listener_path 配置，返回 (type, name, params) 列表。
/// path 段支持 `name` 或 `name({json})` 带参形式（对齐参考实现
/// FlowStatusDecl 的声明式参数格式）；JSON 解析失败按无参处理。
/// 输入为可选字段，None 或空则返回空列表
pub fn parse_listener_config(
    listener_type: Option<&str>,
    listener_path: Option<&str>,
) -> Vec<(String, String, Option<serde_json::Value>)> {
    let types = match listener_type {
        Some(t) if !t.is_empty() => t
            .split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>(),
        _ => return vec![],
    };
    let paths = match listener_path {
        Some(p) if !p.is_empty() => p
            .split("@@")
            .map(|s| split_name_params(s.trim()))
            .collect::<Vec<_>>(),
        _ => return vec![],
    };
    types
        .into_iter()
        .zip(paths)
        .map(|(t, (n, p))| (t, n, p))
        .collect()
}

/// 拆分 `name({json})` 为 (name, Some(params));无参/非法 JSON 原样为名
fn split_name_params(path: &str) -> (String, Option<serde_json::Value>) {
    if let Some(open) = path.find('(')
        && path.ends_with(')')
        && let Ok(v) = serde_json::from_str(&path[open + 1..path.len() - 1])
    {
        return (path[..open].to_string(), Some(v));
    }
    (path.to_string(), None)
}

/// 从注册表中触发指定类型的监听器
pub async fn trigger_listeners(
    registry: &ListenerRegistry,
    listener_type_cfg: Option<&str>,
    listener_path_cfg: Option<&str>,
    event_type: &str,
    var: &mut ListenerVariable,
) -> app_core::error::AppResult<()> {
    let pairs = parse_listener_config(listener_type_cfg, listener_path_cfg);
    for (cfg_type, name, params) in &pairs {
        if cfg_type == event_type
            && let Some(listener) = registry.get(name)
        {
            listener.notify(event_type, params.as_ref(), var).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_returns_empty() {
        assert!(parse_listener_config(None, None).is_empty());
        assert!(parse_listener_config(Some(""), Some("")).is_empty());
    }

    #[test]
    fn parse_single_pair() {
        let pairs = parse_listener_config(Some("start"), Some("my_logger"));
        assert_eq!(pairs.len(), 1);
        assert_eq!(
            pairs[0],
            ("start".to_string(), "my_logger".to_string(), None)
        );
    }

    #[test]
    fn parse_multiple_pairs() {
        let pairs = parse_listener_config(
            Some("start,finish"),
            Some("logger@@notifier"),
        );
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("start".to_string(), "logger".to_string(), None));
        assert_eq!(
            pairs[1],
            ("finish".to_string(), "notifier".to_string(), None)
        );
    }

    #[test]
    fn parse_mismatched_lengths_truncates_to_shorter() {
        // 3 types, 2 paths → 2 pairs
        let pairs =
            parse_listener_config(Some("start,finish,create"), Some("a@@b"));
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn parse_path_with_json_params() {
        // 对齐参考实现 FlowStatusDecl 的 name({json}) 声明式参数格式
        let pairs = parse_listener_config(
            Some("finish"),
            Some(r#"status_sync({"repStatus":"9"})"#),
        );
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "finish");
        assert_eq!(pairs[0].1, "status_sync");
        assert_eq!(pairs[0].2, Some(serde_json::json!({"repStatus": "9"})));
    }

    #[test]
    fn parse_path_without_params_is_none() {
        let pairs = parse_listener_config(Some("start"), Some("logger"));
        assert_eq!(pairs[0].2, None);
    }
}
