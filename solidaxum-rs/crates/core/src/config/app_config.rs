use serde::Deserialize;

use super::size_parse::deserialize_size_bytes;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub db: DbConfig,
    pub redis: RedisConfig,
    pub jwt: JwtConfig,
    #[serde(default)]
    pub log: LogConfig,
    #[serde(default)]
    pub pool: PoolConfig,
    #[serde(default)]
    pub captcha: CaptchaConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub codegen: CodegenConfig,
    #[serde(default)]
    pub flow: FlowConfig,
    #[serde(default)]
    pub oss: OssConfig,
    #[serde(default)]
    pub app: AppSectionConfig,
}

impl AppConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.jwt.secret.len() < 16 {
            return Err(anyhow::anyhow!("jwt.secret 长度不能少于16个字符"));
        }
        self.app.validate()?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: i32,
    #[serde(default)]
    pub static_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DbConfig {
    pub db_type: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub port: i32,
    pub db_name: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    /// 达梦专用:裸表名在哪个 schema 下解析。
    ///
    /// 达梦是单实例单库架构,MySQL 的 database 对应它的 schema,而 schema 即用户。
    /// 不填则用登录用户自己的 schema(达梦默认)。当装表的账号与跑应用的账号不是
    /// 同一个时必须填 —— 否则裸表名解析不到,报 `无效的表或视图名[sys_user]`。
    /// 其余后端忽略此项。
    #[serde(default)]
    pub schema: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub port: i32,
    pub password: Option<String>,
    pub db: i32,
}

#[derive(Debug, Deserialize)]
pub struct JwtConfig {
    pub secret: String,
    pub access_ttl: i64,
    pub refresh_ttl: i64,
}

#[derive(Debug, Deserialize)]
pub struct LogConfig {
    #[serde(default = "default_log_dir")]
    pub dir: String,
    #[serde(default = "default_log_prefix")]
    pub prefix: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            dir: default_log_dir(),
            prefix: default_log_prefix(),
        }
    }
}

fn default_log_dir() -> String {
    "logs".to_string()
}

fn default_log_prefix() -> String {
    "solidaxum".to_string()
}

#[derive(Debug, Deserialize)]
pub struct PoolConfig {
    #[serde(default = "default_db_max_conn")]
    pub db_max_connections: u32,
    #[serde(default = "default_db_min_conn")]
    pub db_min_connections: u32,
    #[serde(default = "default_redis_max_conn")]
    pub redis_max_connections: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            db_max_connections: default_db_max_conn(),
            db_min_connections: default_db_min_conn(),
            redis_max_connections: default_redis_max_conn(),
        }
    }
}

fn default_db_max_conn() -> u32 {
    10
}
fn default_db_min_conn() -> u32 {
    5
}
fn default_redis_max_conn() -> usize {
    16
}

#[derive(Debug, Deserialize)]
pub struct CaptchaConfig {
    #[serde(default = "default_captcha_enable")]
    pub enable: bool,
    #[serde(default = "default_captcha_type")]
    pub captcha_type: String,
}

impl Default for CaptchaConfig {
    fn default() -> Self {
        Self {
            enable: default_captcha_enable(),
            captcha_type: default_captcha_type(),
        }
    }
}

fn default_captcha_enable() -> bool {
    true
}

fn default_captcha_type() -> String {
    "math".to_string()
}

#[derive(Debug, Deserialize)]
pub struct OssConfig {
    /// 上传单文件大小上限(字节)。支持纯数字或人类可读字符串("10MB"/"1GB")。
    /// 仅作用于 /resource/oss/upload。对齐 RuoYi `spring.servlet.multipart.max-file-size`。
    #[serde(
        default = "default_oss_upload_body_limit",
        deserialize_with = "deserialize_size_bytes"
    )]
    pub upload_body_limit: usize,
}

impl Default for OssConfig {
    fn default() -> Self {
        Self {
            upload_body_limit: default_oss_upload_body_limit(),
        }
    }
}

fn default_oss_upload_body_limit() -> usize {
    10 * 1024 * 1024
}

#[derive(Debug, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_rate_limit_capacity")]
    pub rate_limit_capacity: u64,
    #[serde(default = "default_rate_limit_window_secs")]
    pub rate_limit_window_secs: u64,
    #[serde(default = "default_repeat_submit_ttl_seconds")]
    pub repeat_submit_ttl_seconds: u64,
    #[serde(default = "default_xss_enabled")]
    pub xss_enabled: bool,
    #[serde(default)]
    pub xss_exclude_paths: Vec<String>,
    /// AES-256 字段加密密钥（32 字节，base64 编码）。
    /// 用于 OSS secret_key/access_key 加密存储。
    /// 留空则不加密（开发环境可接受，生产环境必须设置）。
    #[serde(default)]
    pub aes_key: Option<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            rate_limit_capacity: default_rate_limit_capacity(),
            rate_limit_window_secs: default_rate_limit_window_secs(),
            repeat_submit_ttl_seconds: default_repeat_submit_ttl_seconds(),
            xss_enabled: default_xss_enabled(),
            xss_exclude_paths: vec![],
            aes_key: None,
        }
    }
}

fn default_rate_limit_capacity() -> u64 {
    100
}

fn default_rate_limit_window_secs() -> u64 {
    60
}

fn default_repeat_submit_ttl_seconds() -> u64 {
    5
}

fn default_xss_enabled() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
pub struct CodegenConfig {
    #[serde(default = "default_codegen_package_name")]
    pub package_name: String,
    #[serde(default = "default_codegen_frontend_type")]
    pub frontend_type: String,
    #[serde(default = "default_codegen_author")]
    pub author: String,
}

impl Default for CodegenConfig {
    fn default() -> Self {
        Self {
            package_name: default_codegen_package_name(),
            frontend_type: default_codegen_frontend_type(),
            author: default_codegen_author(),
        }
    }
}

fn default_codegen_package_name() -> String {
    "crates.system".to_string()
}

fn default_codegen_frontend_type() -> String {
    "vue".to_string()
}

fn default_codegen_author() -> String {
    "SolidAxum".to_string()
}

/// 流程(flow)相关的**路径**配置(参考实现 办理人补偿)。
///
/// 这里只有调度节奏与重试上限 —— 补偿机制本身不可关闭:挂钩点在
/// 业务事务内写 outbox 行,worker 是唯一消费者,关掉 worker 等于
/// 让补偿队列只进不出(运行中流程的办理人逐渐与组织现状脱节)。
#[derive(Debug, Deserialize)]
pub struct FlowConfig {
    /// 补偿 worker 消费间隔(秒)。下限 5 秒(worker 内再 clamp)。
    #[serde(default = "default_assignee_sync_interval_secs")]
    pub assignee_sync_interval_secs: u64,
    /// 单条补偿任务的最大重试次数(超过后停留在 status='2' 并告警,
    /// 等待人工介入)。
    #[serde(default = "default_assignee_sync_max_retry")]
    pub assignee_sync_max_retry: u32,
}

impl Default for FlowConfig {
    fn default() -> Self {
        Self {
            assignee_sync_interval_secs: default_assignee_sync_interval_secs(),
            assignee_sync_max_retry: default_assignee_sync_max_retry(),
        }
    }
}

fn default_assignee_sync_interval_secs() -> u64 {
    30
}

fn default_assignee_sync_max_retry() -> u32 {
    5
}

/// 应用级口径开关。整节与其中每一项都可缺省 —— 已部署的 config.toml
/// 不写 `[app]` 也要能原样启动。
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct AppSectionConfig {
    /// 分页总数的默认计数口径:`"capped"`(默认)或 `"exact"`。
    ///
    /// capped 对 `LIMIT cap + 1` 的子查询计数:结果不到上限时就是精确值,
    /// 超出时返回上限并置截断标记。exact 是全量 `COUNT(*)`,代价与结果集
    /// 大小成正比。
    pub count_mode: String,
    /// capped 口径下的上限。exact 口径下此项无效。
    pub count_cap: u64,
}

impl Default for AppSectionConfig {
    fn default() -> Self {
        Self {
            count_mode: "capped".to_string(),
            count_cap: 10_000,
        }
    }
}

impl AppSectionConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if !matches!(self.count_mode.as_str(), "capped" | "exact") {
            anyhow::bail!(
                "app.count_mode 只能是 \"capped\" 或 \"exact\"(区分大小写),实际: {}",
                self.count_mode
            );
        }
        Ok(())
    }

    pub fn count_mode(&self) -> app_data::CountMode {
        match self.count_mode.as_str() {
            "exact" => app_data::CountMode::Exact,
            // validate() 已排除其它取值;这里不再重复报错,取默认口径。
            _ => app_data::CountMode::Capped(self.count_cap),
        }
    }
}

#[cfg(test)]
mod app_section_tests {
    use super::*;

    /// 整节可缺省 —— 已经部署在外的 config.toml 都没有 `[app]`,
    /// 加了新节还要能原样启动。
    #[test]
    fn app_section_is_optional_and_defaults_to_capped_ten_thousand() {
        let cfg = AppSectionConfig::default();
        assert_eq!(cfg.count_mode, "capped");
        assert_eq!(cfg.count_cap, 10_000);
        assert_eq!(cfg.count_mode(), app_data::CountMode::Capped(10_000));
    }

    #[test]
    fn exact_maps_to_exact_mode() {
        let cfg = AppSectionConfig {
            count_mode: "exact".to_string(),
            count_cap: 10_000,
        };
        assert_eq!(cfg.count_mode(), app_data::CountMode::Exact);
    }

    #[test]
    fn custom_cap_is_carried_through() {
        let cfg = AppSectionConfig {
            count_mode: "capped".to_string(),
            count_cap: 500,
        };
        assert_eq!(cfg.count_mode(), app_data::CountMode::Capped(500));
    }

    /// 拼错的口径必须在启动时炸,而不是静默退回默认值 ——
    /// 压测里配错一个字母就会让「同口径基线」悄悄变成「现状基线」,
    /// 而两组数字看起来都合理,事后无从分辨。
    #[test]
    fn unknown_count_mode_is_rejected_by_validate() {
        let cfg = AppSectionConfig {
            count_mode: "Exact".to_string(),
            count_cap: 10_000,
        };
        let err = cfg.validate().expect_err("大小写不符也必须拒绝");
        assert!(
            err.to_string().contains("count_mode"),
            "错误信息要点名是哪个配置项: {err}"
        );
    }
}
