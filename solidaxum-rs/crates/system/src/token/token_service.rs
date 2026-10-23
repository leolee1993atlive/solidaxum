use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
    model::login_user::LoginUser,
};

const TOKEN_PREFIX: &str = "global:token:";

/// 纯函数: 判断给定会话是否拥有指定角色 key(用于多角色 OR 失效判定)。
/// 抽出以便在没有 Redis 的脚手架内单测。
pub fn session_has_role(login_user: &LoginUser, role_key: &str) -> bool {
    login_user.roles.contains(role_key)
}

#[derive(Clone)]
pub struct TokenService {
    redis_pool: deadpool_redis::Pool,
}

impl TokenService {
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            redis_pool: infra.pool.redis_pool.clone(),
        }
    }

    /// 清理拥有指定角色 key 的所有在线会话(对齐 Java `OnlineUserCleanEvent`)。
    ///
    /// 实现:SCAN `global:token:*` → GET 反序列化 LoginUser →
    /// 若 `session_has_role(login_user, role_key)` 为 true 则 DEL。
    ///
    /// 用于角色 edit / changeStatus / permission 变更后,使受影响用户的下次请求重新加载权限。
    #[tracing::instrument(skip(self), fields(role_key = %role_key))]
    pub async fn clean_sessions_by_role(
        &self,
        role_key: &str,
    ) -> AppResult<usize> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis 连接获取失败: {}", e))
        })?;

        let pattern = format!("{}*", TOKEN_PREFIX);
        let mut cursor: u64 = 0;
        let mut removed: usize = 0;

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

            for key in batch {
                let json: Option<String> = redis::cmd("GET")
                    .arg(&key)
                    .query_async(&mut *conn)
                    .await
                    .map_err(|e| {
                        AppError::Internal(format!(
                            "Redis GET {} 失败: {}",
                            key, e
                        ))
                    })?;

                let Some(json) = json else { continue };
                let Ok(login_user) = serde_json::from_str::<LoginUser>(&json)
                else {
                    continue;
                };

                if session_has_role(&login_user, role_key) {
                    redis::cmd("DEL")
                        .arg(&key)
                        .exec_async(&mut *conn)
                        .await
                        .map_err(|e| {
                            AppError::Internal(format!(
                                "Redis DEL {} 失败: {}",
                                key, e
                            ))
                        })?;
                    removed += 1;
                }
            }

            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        tracing::info!(
            role_key = %role_key,
            removed = removed,
            "按角色清理会话完成"
        );
        Ok(removed)
    }

    pub async fn create_session(
        &self,
        login_user: &LoginUser,
        ttl_secs: u64,
    ) -> AppResult<String> {
        let token_value = uuid::Uuid::now_v7().to_string();
        let key = format!("{}{}", TOKEN_PREFIX, token_value);
        let json = serde_json::to_string(login_user).map_err(|e| {
            AppError::Internal(format!("序列化 LoginUser 失败: {}", e))
        })?;

        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis 连接获取失败: {}", e))
        })?;

        redis::cmd("SET")
            .arg(&key)
            .arg(&json)
            .arg("EX")
            .arg(ttl_secs)
            .exec_async(&mut *conn)
            .await
            .map_err(|e| {
                AppError::Internal(format!("Redis 存储token失败: {}", e))
            })?;

        Ok(token_value)
    }

    pub async fn get_session(
        &self,
        token_value: &str,
    ) -> AppResult<Option<LoginUser>> {
        let key = format!("{}{}", TOKEN_PREFIX, token_value);
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis 连接获取失败: {}", e))
        })?;

        let val: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut *conn)
            .await
            .map_err(|e| {
                AppError::Internal(format!("Redis 查询token失败: {}", e))
            })?;

        match val {
            Some(json) => {
                let login_user: LoginUser = serde_json::from_str(&json)
                    .map_err(|e| {
                        AppError::Internal(format!(
                            "反序列化 LoginUser 失败: {}",
                            e
                        ))
                    })?;
                Ok(Some(login_user))
            }
            None => Ok(None),
        }
    }

    /// 取出 session 并**同时**滑动续期,一次往返完成。
    ///
    /// 鉴权中间件原先是 `get_session()` + `refresh_session_ttl()` 两次调用,
    /// 各自 `redis_pool.get()` 一次 —— 而 deadpool 每次取连接都会发
    /// `UNWATCH` + `PING` 做健康检查。于是一个读请求要发 6 条 Redis 命令,
    /// 其中 **4 条是纯开销**(2 次取连接 × 2 条检查),真正的业务命令只有
    /// `GET` 和 `EXPIRE`。
    ///
    /// `GETEX key EX ttl` 把两条业务命令合成一条,加上只取一次连接,
    /// 单请求 Redis 命令数从 6 降到 3(UNWATCH + PING + GETEX)。
    ///
    /// 需要 Redis 6.2+;本项目 Redis 依赖已要求更高版本,故直接使用。
    pub async fn get_session_and_refresh(
        &self,
        token_value: &str,
        ttl_secs: u64,
    ) -> AppResult<Option<LoginUser>> {
        let key = format!("{}{}", TOKEN_PREFIX, token_value);
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis 连接获取失败: {}", e))
        })?;

        let val: Option<String> = redis::cmd("GETEX")
            .arg(&key)
            .arg("EX")
            .arg(ttl_secs)
            .query_async(&mut *conn)
            .await
            .map_err(|e| {
                AppError::Internal(format!("Redis 查询token失败: {}", e))
            })?;

        match val {
            Some(json) => {
                let login_user: LoginUser = serde_json::from_str(&json)
                    .map_err(|e| {
                        AppError::Internal(format!(
                            "反序列化 LoginUser 失败: {}",
                            e
                        ))
                    })?;
                Ok(Some(login_user))
            }
            None => Ok(None),
        }
    }

    pub async fn refresh_session_ttl(
        &self,
        token_value: &str,
        ttl_secs: u64,
    ) -> AppResult<()> {
        let key = format!("{}{}", TOKEN_PREFIX, token_value);
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis 连接获取失败: {}", e))
        })?;

        redis::cmd("EXPIRE")
            .arg(&key)
            .arg(ttl_secs)
            .exec_async(&mut *conn)
            .await
            .map_err(|e| {
                AppError::Internal(format!("Redis 续期token失败: {}", e))
            })?;

        Ok(())
    }

    pub async fn revoke_session(&self, token_value: &str) -> AppResult<()> {
        let key = format!("{}{}", TOKEN_PREFIX, token_value);
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

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn make_user(roles: &[&str]) -> LoginUser {
        LoginUser {
            user_id: 1,
            dept_id: None,
            username: "u".to_string(),
            nick_name: None,
            user_type: None,
            client_id: "c".to_string(),
            client_key: None,
            device_type: None,
            permissions: HashSet::new(),
            roles: roles.iter().map(|s| s.to_string()).collect(),
            login_ip: None,
            login_date: None,
        }
    }

    #[test]
    fn session_has_role_matches_when_present() {
        let user = make_user(&["admin", "common"]);
        assert!(session_has_role(&user, "admin"));
        assert!(session_has_role(&user, "common"));
    }

    #[test]
    fn session_has_role_misses_when_absent() {
        let user = make_user(&["admin"]);
        assert!(!session_has_role(&user, "common"));
    }

    #[test]
    fn session_has_role_empty_roles() {
        let user = make_user(&[]);
        assert!(!session_has_role(&user, "admin"));
    }
}
