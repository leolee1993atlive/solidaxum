use std::sync::Arc;

use crate::infra::ConnectionPool;

/// 限流配置
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    /// 窗口内允许的最大请求数，为 0 时关闭限流
    pub capacity: u64,
    /// 窗口大小（秒）
    pub window_secs: u64,
    /// 跳过的路径前缀列表
    pub exclude_paths: Vec<String>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            capacity: 100,
            window_secs: 60,
            exclude_paths: vec![],
        }
    }
}

/// 防重复提交配置
#[derive(Clone, Debug)]
pub struct RepeatSubmitConfig {
    /// 锁的过期时间（秒）
    pub ttl_seconds: u64,
}

impl Default for RepeatSubmitConfig {
    fn default() -> Self {
        Self { ttl_seconds: 5 }
    }
}

/// XSS 过滤配置
#[derive(Clone, Debug, Default)]
pub struct XssConfig {
    /// 跳过的路径前缀列表
    pub exclude_paths: Vec<String>,
}

/// 中间件共享状态
#[derive(Clone)]
pub struct MiddlewareState {
    pub redis: Arc<ConnectionPool>,
    pub rate_limit_config: RateLimitConfig,
    pub repeat_submit_config: RepeatSubmitConfig,
    pub xss_config: XssConfig,
}

impl MiddlewareState {
    pub fn new(redis: Arc<ConnectionPool>) -> Self {
        Self {
            redis,
            rate_limit_config: RateLimitConfig::default(),
            repeat_submit_config: RepeatSubmitConfig::default(),
            xss_config: XssConfig::default(),
        }
    }
}
