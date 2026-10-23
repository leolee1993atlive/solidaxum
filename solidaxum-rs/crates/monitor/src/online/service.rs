use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
    model::login_user::LoginUser,
    vo::page::PageResult,
};
use deadpool_redis::Pool;

use super::model::UserOnline;

const TOKEN_PREFIX: &str = "global:token:";

/// 将 Redis token key 与 LoginUser 转换为 UserOnline(纯函数,便于单测)。
/// token_key 形如 `global:token:xxx`,会去除前缀得到 token_id。
/// dept_name/login_location/browser/os 暂无数据源,留空。
pub fn parse_online_user(
    token_key: &str,
    login_user: &LoginUser,
) -> UserOnline {
    let token_id = token_key.strip_prefix(TOKEN_PREFIX).unwrap_or(token_key);
    UserOnline {
        token_id: token_id.to_string(),
        user_name: login_user.username.clone(),
        nick_name: login_user.nick_name.clone().unwrap_or_default(),
        dept_name: String::new(),
        ipaddr: login_user.login_ip.clone().unwrap_or_default(),
        login_location: String::new(),
        browser: String::new(),
        os: String::new(),
        login_time: login_user.login_date,
        client_key: login_user.client_key.clone(),
        device_type: login_user.device_type.clone(),
        user_id: login_user.user_id,
    }
}

/// 判断 UserOnline 是否匹配用户名/IP 过滤条件(纯函数,便于单测)。
/// None 或空字符串表示不过滤该维度(视为匹配)。
pub fn matches_online_filter(
    user: &UserOnline,
    user_name: Option<&str>,
    ipaddr: Option<&str>,
) -> bool {
    let matches_name =
        user_name.is_none_or(|n| n.is_empty() || user.user_name.contains(n));
    let matches_ip =
        ipaddr.is_none_or(|i| i.is_empty() || user.ipaddr.contains(i));
    matches_name && matches_ip
}

#[derive(Clone)]
pub struct OnlineUserService {
    pub redis_pool: Pool,
}

impl OnlineUserService {
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            redis_pool: infra.pool.redis_pool.clone(),
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn list_online_users(
        &self,
        user_name: Option<String>,
        ipaddr: Option<String>,
    ) -> AppResult<PageResult<UserOnline>> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis 连接获取失败: {}", e))
        })?;

        let pattern = format!("{}*", TOKEN_PREFIX);
        let mut cursor: u64 = 0;
        let mut keys: Vec<String> = Vec::new();

        loop {
            let (next_cursor, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut *conn)
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Redis SCAN 失败: {}", e))
                })?;
            keys.extend(batch);
            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        let mut users = Vec::new();
        for key in &keys {
            let json: Option<String> = redis::cmd("GET")
                .arg(key)
                .query_async(&mut *conn)
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Redis GET {} 失败: {}", key, e))
                })?;

            if let Some(json) = json
                && let Ok(login_user) = serde_json::from_str::<LoginUser>(&json)
            {
                let user = parse_online_user(key, &login_user);

                if matches_online_filter(
                    &user,
                    user_name.as_deref(),
                    ipaddr.as_deref(),
                ) {
                    users.push(user);
                }
            }
        }

        let total = users.len() as i64;
        Ok(PageResult::new(users, total))
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_current_user_online(
        &self,
        current_user_name: String,
    ) -> AppResult<Vec<UserOnline>> {
        let result = self
            .list_online_users(Some(current_user_name), None)
            .await?;
        Ok(result.rows)
    }

    #[tracing::instrument(skip(self))]
    pub async fn force_logout(&self, token_id: &str) -> AppResult<()> {
        let key = format!("{}{}", TOKEN_PREFIX, token_id);
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis 连接获取失败: {}", e))
        })?;

        redis::cmd("DEL")
            .arg(&key)
            .exec_async(&mut *conn)
            .await
            .map_err(|e| {
                AppError::Internal(format!("Redis 删除token失败: {}", e))
            })?;

        tracing::info!(token_id = %token_id, "强制下线成功");
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn batch_force_logout(
        &self,
        token_ids: &[String],
    ) -> AppResult<()> {
        for token_id in token_ids {
            if let Err(e) = self.force_logout(token_id).await {
                tracing::warn!(
                    token_id = %token_id,
                    error = %e,
                    "批量强退: 跳过失败的 token"
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::TimeZone;

    use super::*;

    fn sample_login_user() -> LoginUser {
        LoginUser {
            user_id: 1,
            dept_id: Some(103),
            username: "admin".to_string(),
            nick_name: Some("管理员".to_string()),
            user_type: Some("sys_user".to_string()),
            client_id: "e5cd7e4891bf95d1d19206ce24a7b32e".to_string(),
            client_key: Some("pc".to_string()),
            device_type: Some("pc".to_string()),
            permissions: HashSet::new(),
            roles: HashSet::new(),
            login_ip: Some("192.168.1.100".to_string()),
            login_date: Some(
                chrono::Utc.with_ymd_and_hms(2026, 6, 14, 8, 30, 0).unwrap(),
            ),
        }
    }

    #[test]
    fn parse_strips_token_prefix() {
        let user =
            parse_online_user("global:token:abc-123", &sample_login_user());
        assert_eq!(user.token_id, "abc-123");
    }

    #[test]
    fn parse_keeps_full_key_when_no_prefix() {
        let user = parse_online_user("raw-key", &sample_login_user());
        assert_eq!(user.token_id, "raw-key");
    }

    #[test]
    fn parse_maps_core_fields() {
        let src = sample_login_user();
        let user = parse_online_user("global:token:t1", &src);
        assert_eq!(user.user_id, 1);
        assert_eq!(user.user_name, "admin");
        assert_eq!(user.nick_name, "管理员");
        assert_eq!(user.ipaddr, "192.168.1.100");
        assert_eq!(user.client_key.as_deref(), Some("pc"));
        assert_eq!(user.device_type.as_deref(), Some("pc"));
        assert!(user.login_time.is_some());
    }

    #[test]
    fn parse_uses_empty_string_when_option_fields_none() {
        let mut src = sample_login_user();
        src.nick_name = None;
        src.login_ip = None;
        src.client_key = None;
        src.device_type = None;
        let user = parse_online_user("global:token:t2", &src);
        assert_eq!(user.nick_name, "");
        assert_eq!(user.ipaddr, "");
        assert_eq!(user.client_key, None);
        assert_eq!(user.device_type, None);
        assert_eq!(user.login_time, src.login_date);
    }

    #[test]
    fn parse_leaves_unset_metadata_fields_empty() {
        // dept_name/login_location/browser/os 暂无数据源,应为空串
        let user = parse_online_user("global:token:t3", &sample_login_user());
        assert_eq!(user.dept_name, "");
        assert_eq!(user.login_location, "");
        assert_eq!(user.browser, "");
        assert_eq!(user.os, "");
    }

    #[test]
    fn filter_matches_when_no_constraints() {
        let user = parse_online_user("global:token:t", &sample_login_user());
        assert!(matches_online_filter(&user, None, None));
    }

    #[test]
    fn filter_matches_empty_string_constraints() {
        let user = parse_online_user("global:token:t", &sample_login_user());
        assert!(matches_online_filter(&user, Some(""), Some("")));
    }

    #[test]
    fn filter_matches_by_username_substring() {
        let user = parse_online_user("global:token:t", &sample_login_user());
        assert!(matches_online_filter(&user, Some("dmi"), None));
    }

    #[test]
    fn filter_rejects_non_matching_username() {
        let user = parse_online_user("global:token:t", &sample_login_user());
        assert!(!matches_online_filter(&user, Some("root"), None));
    }

    #[test]
    fn filter_matches_by_ip_substring() {
        let user = parse_online_user("global:token:t", &sample_login_user());
        assert!(matches_online_filter(&user, None, Some("192.168")));
    }

    #[test]
    fn filter_rejects_non_matching_ip() {
        let user = parse_online_user("global:token:t", &sample_login_user());
        assert!(!matches_online_filter(&user, None, Some("10.0.0")));
    }

    #[test]
    fn filter_requires_both_dimensions_to_match() {
        let user = parse_online_user("global:token:t", &sample_login_user());
        // 用户名匹配但 IP 不匹配 → 拒绝
        assert!(!matches_online_filter(&user, Some("admin"), Some("10.0.0")));
        // IP 匹配但用户名不匹配 → 拒绝
        assert!(!matches_online_filter(&user, Some("root"), Some("192.168")));
        // 两者都匹配 → 通过
        assert!(matches_online_filter(&user, Some("admin"), Some("192.168")));
    }
}
