use std::time::Duration;

use redis::Script;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    infra::ConnectionPool,
};

/// 分布式锁
pub struct DistributedLock {
    key: String,
    value: String,
    pool: ConnectionPool,
}

impl DistributedLock {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

/// 尝试获取分布式锁
///
/// 在 `acquire_timeout_ms` 时间内轮询获取锁，锁持有者标识为 UUID，
/// 只有持有相同 value 的客户端才能释放锁。
pub async fn try_lock(
    pool: &ConnectionPool,
    lock_key: &str,
    acquire_timeout_ms: u64,
    hold_time_ms: u64,
) -> AppResult<Option<DistributedLock>> {
    let value = Uuid::now_v7().to_string();
    let deadline =
        tokio::time::Instant::now() + Duration::from_millis(acquire_timeout_ms);

    loop {
        let mut conn = pool.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis 连接获取失败: {}", e))
        })?;

        let result: Option<String> = redis::cmd("SET")
            .arg(lock_key)
            .arg(&value)
            .arg("NX")
            .arg("PX")
            .arg(hold_time_ms as i64)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                AppError::Internal(format!("分布式锁获取失败: {}", e))
            })?;

        if result.is_some() {
            return Ok(Some(DistributedLock {
                key: lock_key.to_string(),
                value,
                pool: pool.clone(),
            }));
        }

        if tokio::time::Instant::now() >= deadline {
            return Ok(None);
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

impl Drop for DistributedLock {
    fn drop(&mut self) {
        let key = self.key.clone();
        let value = self.value.clone();
        let pool = self.pool.clone();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let script = Script::new(
                    "if redis.call('get', KEYS[1]) == ARGV[1] then return redis.call('del', KEYS[1]) else return 0 end"
                );
                if let Ok(mut conn) = pool.redis_pool.get().await {
                    let _: Result<(), _> = script
                        .key(&key)
                        .arg(&value)
                        .invoke_async(&mut conn)
                        .await;
                }
            });
        }
    }
}
