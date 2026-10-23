use std::{collections::HashMap, sync::Arc};

use app_core::{
    error::AppError, middleware::extractors::CurrentUser, vo::response::R,
};
use axum::extract::State;
use serde::Serialize;
use system::permission::guard;

use crate::OperLogState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    pub info: HashMap<String, String>,
    pub db_size: i64,
    pub command_stats: Vec<HashMap<String, String>>,
}

/// 解析 Redis INFO 命令返回的字符串为 (info_map, command_stats)。
/// 抽出为纯函数便于单测。`# Section` 行划分段落,仅 Commandstats 段
/// 的 cmdstat_xxx 行进入 command_stats,其余键值进入 info_map。
pub fn parse_cache_info(
    info_str: &str,
) -> (HashMap<String, String>, Vec<HashMap<String, String>>) {
    let mut info_map = HashMap::new();
    let mut command_stats = Vec::new();
    let mut current_section = String::new();

    for line in info_str.lines() {
        if line.starts_with('#') {
            current_section = line.trim_start_matches("# ").to_string();
        } else if !line.is_empty()
            && !current_section.is_empty()
            && let Some((key, value)) = line.split_once(':')
        {
            if current_section == "Commandstats" {
                let name = key.trim_start_matches("cmdstat_").to_string();
                let calls = value
                    .split(',')
                    .find_map(|kv| kv.strip_prefix("calls="))
                    .unwrap_or("0")
                    .to_string();
                let mut cmd = HashMap::new();
                cmd.insert("name".to_string(), name);
                cmd.insert("value".to_string(), calls);
                command_stats.push(cmd);
            } else {
                info_map.insert(key.to_string(), value.to_string());
            }
        }
    }
    (info_map, command_stats)
}

#[tracing::instrument(skip(state))]
pub async fn get_cache_info(
    State(state): State<Arc<OperLogState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<CacheInfo>, AppError> {
    guard::require_permission(&login_user, "monitor:cache:list")?;
    let mut conn = state
        .services
        .online_user_service
        .redis_pool
        .get()
        .await
        .map_err(|e| {
            AppError::Internal(format!("Redis 连接获取失败: {}", e))
        })?;

    let info_str: String = redis::cmd("INFO")
        .query_async(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(format!("Redis INFO 失败: {}", e)))?;

    let (info_map, command_stats) = parse_cache_info(&info_str);

    let db_size: i64 = redis::cmd("DBSIZE")
        .query_async(&mut *conn)
        .await
        .map_err(|e| AppError::Internal(format!("Redis DBSIZE 失败: {}", e)))?;

    Ok(R::ok_with_data(CacheInfo {
        info: info_map,
        db_size,
        command_stats,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_server_section_into_info_map() {
        let raw = "# Server\n\
                   redis_version:7.0.0\n\
                   redis_mode:standalone\n\
                   \n\
                   # Clients\n\
                   connected_clients:1\n";
        let (info, cmds) = parse_cache_info(raw);
        assert_eq!(info.get("redis_version").unwrap(), "7.0.0");
        assert_eq!(info.get("redis_mode").unwrap(), "standalone");
        assert_eq!(info.get("connected_clients").unwrap(), "1");
        assert!(cmds.is_empty(), "无 Commandstats 段: {cmds:?}");
    }

    #[test]
    fn parses_commandstats_section() {
        let raw = "# Commandstats\n\
                   cmdstat_get:calls=10,usec=500,usec_per_call=50.0\n\
                   cmdstat_set:calls=3,usec=120\n";
        let (info, cmds) = parse_cache_info(raw);
        assert!(info.is_empty(), "Commandstats 段不应进入 info_map");
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].get("name").unwrap(), "get");
        assert_eq!(cmds[0].get("value").unwrap(), "10");
        assert_eq!(cmds[1].get("name").unwrap(), "set");
        assert_eq!(cmds[1].get("value").unwrap(), "3");
    }

    #[test]
    fn commandstats_without_calls_defaults_to_zero() {
        let raw = "# Commandstats\n\
                   cmdstat_info:usec=10\n";
        let (_info, cmds) = parse_cache_info(raw);
        assert_eq!(cmds[0].get("value").unwrap(), "0");
    }

    #[test]
    fn empty_string_returns_empty_maps() {
        let (info, cmds) = parse_cache_info("");
        assert!(info.is_empty());
        assert!(cmds.is_empty());
    }

    #[test]
    fn lines_without_section_are_skipped() {
        // 没有任何 # Section 头 → 所有键值应被忽略
        let raw = "redis_version:7.0.0\nos:Linux\n";
        let (info, cmds) = parse_cache_info(raw);
        assert!(info.is_empty());
        assert!(cmds.is_empty());
    }
}
