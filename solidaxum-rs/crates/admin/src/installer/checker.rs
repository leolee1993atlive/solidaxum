//! `--check-config` 实现:用生产同款驱动真实连接 DB + Redis 探活。
//!
//! 设计原则:测的就是服务器启动时实际会用的连接串。因此用 `installer::dsn`
//! 构建连接串(sea-orm + deadpool-redis),与 `app::init_app` 同源,而非脚本侧
//! 的 `nc`/`telnet`(协议层有差异)。
//!
//! 对 aes_key 长度的校验在这里用友好 `Err`,而不是触发 `main.rs` 的 `assert!`
//! panic —— 安装向导需要对用户展示「密钥长度不对」而不是崩进程。

use std::{path::Path, time::Duration};

use anyhow::Context;
use app_core::config::AppConfig;
// 连接探活走 app-data 漏斗(connect 后即 drop);错误是
// DataError,达梦 hint 的文本匹配经 Backend(e) 提取 DbErr 原文 ——
// Display 文本依赖不变,不可吞不可包。
use app_data::DataSource;
use app_data::sea_orm::ConnectOptions;

use crate::installer::dsn::{
    build_db_dsn, build_redis_url, schema_search_path,
};

/// 校验 config.toml:解析 → aes_key 长度 → DB 连通 → Redis 连通(PING)。
///
/// 任一失败返回 `anyhow::Error`,stderr 供安装脚本展示并循环重问。
pub async fn check_config(path: &Path) -> anyhow::Result<()> {
    // 注意:config crate 的 File::from(&Path) 会按扩展名推断格式。
    // 用 path 的字符串形式喂给 File::with_name 可能在 Windows 反斜杠上出问题,
    // 故这里用 File::from(&Path) + 显式 format 更可靠。
    let config = ::config::Config::builder()
        .add_source(::config::File::from(path))
        .build()
        .with_context(|| format!("读取 config.toml 失败: {:?}", path))?
        .try_deserialize::<AppConfig>()
        .with_context(|| format!("解析 config.toml 失败: {:?}", path))?;

    // 1. aes_key 长度校验(友好报错,不 panic)
    check_aes_key(&config)?;

    // 2. DB 连通性
    check_db(&config).await?;

    // 3. Redis 连通性(PING)
    check_redis(&config).await?;

    Ok(())
}

/// 校验 `security.aes_key`:若存在且非空,必须是 base64 编码、解码后 32 字节。
fn check_aes_key(config: &AppConfig) -> anyhow::Result<()> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    if let Some(ref key) = config.security.aes_key
        && !key.is_empty()
    {
        let bytes = STANDARD
            .decode(key)
            .with_context(|| "security.aes_key 不是有效的 base64 字符串")?;
        if bytes.len() != 32 {
            anyhow::bail!(
                "security.aes_key 解码后为 {} 字节,必须是 32 字节(AES-256)",
                bytes.len()
            );
        }
    }
    Ok(())
}

/// 把达梦 ODBC 的原始报错翻译成运维能照着做的提示。
///
/// 放在这里、而不是 install.sh 与 .iss 各写一份:两个安装器都只是把本进程的
/// stderr 原样展示给用户,所以一份实现两处受益,而且可以单测。
///
/// 匹配以 SQLSTATE / native error 优先 —— 驱动管理器的消息文本受语言环境影响,
/// 错误码不受。中文文本作兜底。
fn dm_hint(err: &str) -> Option<&'static str> {
    if err.contains("IM002")
        || err.contains("数据源名未找到")
        || err.contains("Data source name not found")
    {
        return Some(
            "ODBC 数据源未注册。注意 [db].db_name 填的是**数据源名**,不是库名。\n\
             Linux:见 /etc/odbc.ini,已注册的数据源可用 `odbcinst -q -s` 列出;\n\
             Windows:见「ODBC 数据源管理器(64 位)」。",
        );
    }
    if err.contains("-71008") || err.contains("需要更多信息") {
        return Some(
            "达梦驱动要求补全登录信息,通常是 [db].username / [db].password 为空。\n\
             达梦的 ODBC 配置对话框不保存密码,凭据必须由 config.toml 提供。",
        );
    }
    if err.contains("无效的模式名") {
        return Some(
            "[db].schema 指向的 schema 不存在。达梦的 schema 即建表账号名\n\
             (如 RUOYI),不是库名;建表步骤见源码仓库 sql/dm/Readme.md\n\
             (该目录不随安装包交付)。",
        );
    }
    if err.contains("libdodbc")
        || err.contains("Can't open lib")
        || err.contains("找不到驱动")
    {
        return Some(
            "达梦 ODBC 驱动未安装,或 odbcinst.ini 里的驱动路径不对。\n\
             驱动文件是达梦安装包里的 libdodbc.so(Windows 为 dodbc.dll)。",
        );
    }
    None
}

/// 只在达梦后端附加提示 —— 其余后端的报错不该被达梦的文案污染。
fn dm_hint_for(
    db: &app_core::config::DbConfig,
    err: &str,
) -> Option<&'static str> {
    if db.db_type != "dm" {
        return None;
    }
    dm_hint(err)
}

/// 抹掉 DSN 里的密码。连接失败的报错会被安装脚本原样打到终端,
/// 重定向进日志就会明文落盘。
///
/// 按形态分派,不做全文子串搜索 —— 用户名/数据源名/密码里含字面 `PWD=`
/// 时,`find("PWD=")` 会命中错位置,真正的密码反而留在输出里(有单测钉死)。
fn redact_dsn(dsn: &str) -> String {
    // URL 形态:scheme://user:pass@host。userinfo 里的 `@:/;=` 都被
    // dsn::enc percent-encode 过,首个 `@` 必是 userinfo 分隔符。
    // 但 `://` 前必须是合法 scheme —— ODBC 字段值里可能恰好含
    // `://user:pass`,只凭子串就进 URL 分支会把真密码漏出(有单测钉死)。
    if let Some(scheme_end) = dsn.find("://")
        && is_valid_scheme(&dsn[..scheme_end])
    {
        let (scheme, rest) = dsn.split_at(scheme_end + 3);
        if let Some(at) = rest.find('@') {
            let userinfo = &rest[..at];
            if let Some(colon) = userinfo.find(':') {
                return format!(
                    "{scheme}{}:***{}",
                    &userinfo[..colon],
                    &rest[at..]
                );
            }
        }
        // 合法 scheme 但抹不了 userinfo(无 @ 或无密码):可能只是值里
        // 恰好含 `://` 的 ODBC 串,继续走 ODBC 分支判断
    }
    redact_odbc_pwd(dsn).unwrap_or_else(|| dsn.to_string())
}

/// RFC 3986 scheme:首字符字母,其余为字母数字 / `+` / `-` / `.`。
/// ODBC 串里 `://` 前必然混着 `;` / `=` / `:`,过不了这道门。
fn is_valid_scheme(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic())
        && chars
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// ODBC 形态(`键=值;` 结构):段首为 `PWD=` 的那一段,值抹为 `***`。
///
/// 切段必须花括号感知 —— 含 `;` 的值由 `dsn::odbc_value` 包上 `{}`,
/// 括号内的 `;` 是值的一部分,不是分隔符。花括号不闭合(字段值中间
/// 有裸 `{`,`build_db_dsn` 产不出但用户输入能到这)时解析不可靠,
/// **整体脱敏**而不是回显原文 —— 本函数存在的前提就是错误信息可能进日志。
fn redact_odbc_pwd(dsn: &str) -> Option<String> {
    let mut segs: Vec<&str> = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (i, c) in dsn.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ';' if depth == 0 => {
                segs.push(&dsn[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if depth > 0 {
        return Some("<格式异常,DSN 已整体脱敏>".to_string());
    }
    segs.push(&dsn[start..]);

    let hit = segs.iter().position(|s| s.starts_with("PWD="))?;
    Some(
        segs.iter()
            .enumerate()
            .map(|(i, s)| if i == hit { "PWD=***" } else { *s })
            .collect::<Vec<_>>()
            .join(";"),
    )
}

/// 经 app-data 漏斗连 DB 探活(min_connections=1 强制真实握手,与
/// app.rs 同源;连上即 drop,不留句柄)。
async fn check_db(config: &AppConfig) -> anyhow::Result<()> {
    let dsn = build_db_dsn(&config.db)?;
    let mut opt = ConnectOptions::new(dsn.clone());
    opt.max_connections(1)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(10))
        .acquire_timeout(Duration::from_secs(10))
        .sqlx_logging(false);

    // 与 app.rs 同源:检测用的连接必须和启动时一致,否则会出现「检测通过、
    // 启动报 无效的表或视图名」。达梦据此真发 SET SCHEMA,schema 不存在即
    // 在这里报错 —— 这一步是有检验价值的,不是装饰。
    if let Some(schema) = schema_search_path(&config.db) {
        opt.set_schema_search_path(schema);
    }

    // 显式连接并立即丢弃 —— min_connections=1 保证不是惰性建连
    // (DataSource 无显式 close,drop 即弃;探活语义只关握手成败)
    match DataSource::connect(opt).await {
        Ok(ds) => {
            drop(ds);
        }
        Err(e) => {
            // 达梦 hint 匹配依赖 DbErr 的 Display 原文:Backend 变体原样
            // 提取;其余变体(连接路径理论不可达)整体字符串兜底。
            let raw = match e {
                app_data::DataError::Backend(err) => err.to_string(),
                other => other.to_string(),
            };
            let err = anyhow::anyhow!(
                "数据库连接失败 (dsn={}): {raw}",
                redact_dsn(&dsn)
            );
            return Err(match dm_hint_for(&config.db, &raw) {
                Some(hint) => err.context(hint),
                None => err,
            });
        }
    }
    Ok(())
}

/// 用 deadpool-redis 连接并发 PING(pool size 1,三项 timeout 各 5s)。
async fn check_redis(config: &AppConfig) -> anyhow::Result<()> {
    let url = build_redis_url(&config.redis);
    let mut cfg = deadpool_redis::Config::from_url(url.clone());
    cfg.pool = Some(deadpool_redis::PoolConfig {
        max_size: 1,
        timeouts: deadpool_redis::Timeouts {
            wait: Some(Duration::from_secs(5)),
            create: Some(Duration::from_secs(5)),
            recycle: Some(Duration::from_secs(5)),
        },
        ..Default::default()
    });
    let pool = cfg
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .map_err(|e| {
            anyhow::anyhow!(
                "创建 Redis 连接池失败 (url={}): {e:?}",
                redact_dsn(&url)
            )
        })?;

    let mut conn = pool.get().await.map_err(|e| {
        anyhow::anyhow!("Redis 连接超时或失败 (url={}): {e}", redact_dsn(&url))
    })?;

    let pong: String = redis::cmd("PING")
        .query_async(&mut *conn)
        .await
        .map_err(|e| {
            anyhow::anyhow!("Redis PING 失败 (url={}): {e}", redact_dsn(&url))
        })?;
    if pong != "PONG" {
        anyhow::bail!("Redis PING 返回异常: {pong}(期望 PONG)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个唯一的临时目录(并行测试安全)。
    fn unique_tmp(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "axum-checker-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn write_config(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
        let path = dir.join("config.toml");
        std::fs::write(&path, body).unwrap();
        path
    }

    /// 从 TOML 文本解析出 AppConfig,与 check_config 内部同一条路径。
    fn parse_config(name: &str, body: &str) -> AppConfig {
        let dir = unique_tmp(name);
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_config(&dir, body);
        let c = ::config::Config::builder()
            .add_source(::config::File::from(path.as_path()))
            .build()
            .unwrap()
            .try_deserialize::<AppConfig>()
            .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        c
    }

    /// 新增的 schema 下发分支不能波及其他后端 —— sqlite 不认
    /// schema_search_path,但必须照常连通。
    ///
    /// 达梦真实下发 `SET SCHEMA` 的行为需实机验收(见 spec §9),这里守的是
    /// 「加了这个分支不会把别的后端弄坏」。
    #[tokio::test]
    async fn schema_does_not_break_other_backends() {
        let body = r#"
[server]
port = 3000
host = "localhost"
[db]
db_type = "sqlite"
db_name = ":memory:"
schema = "RUOYI"
[redis]
url = "localhost"
port = 6379
db = 0
[jwt]
secret = "at-least-16-chars"
access_ttl = 3600
refresh_ttl = 604800
"#;
        let config = parse_config("schema-sqlite", body);
        check_db(&config)
            .await
            .expect("sqlite 配了 schema 也应照常连通");
    }

    #[tokio::test]
    async fn sqlite_file_database_connects() {
        // 2026-10-22 修复回归:文件型 sqlite 的 DSN 曾产出 `file:xxx`,
        // sea-orm 只认 `sqlite:` 起头,--check-config 与运行时双双
        // "no supporting driver"。这里用真实驱动连通 + 落盘文件兜住。
        let dir = unique_tmp("sqlite-file");
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("check.db");
        // Windows 反斜杠在 DSN 里也走得通,但统一正斜杠免生枝节
        let db_name = db_path.to_string_lossy().replace('\\', "/");
        let body = format!(
            r#"
[server]
port = 3000
host = "localhost"
[db]
db_type = "sqlite"
db_name = "{db_name}"
[redis]
url = "localhost"
port = 6379
db = 0
[jwt]
secret = "at-least-16-chars"
access_ttl = 3600
refresh_ttl = 604800
"#
        );
        let config = parse_config("sqlite-file", &body);
        check_db(&config)
            .await
            .expect("文件型 sqlite 应能连通(file: DSN 修复后)");
        assert!(db_path.exists(), "连接应在磁盘上创建数据库文件");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn bad_aes_key_length_fails_friendly() {
        let dir = unique_tmp("bad-aes");
        std::fs::create_dir_all(&dir).unwrap();
        use base64::{Engine, engine::general_purpose::STANDARD};
        // 10 字节,不是 32
        let bad_key = STANDARD.encode(b"0123456789");
        let body = format!(
            r#"
[server]
port = 3000
host = "localhost"
[db]
db_type = "sqlite"
db_name = ":memory:"
[redis]
url = "localhost"
port = 6379
db = 0
[jwt]
secret = "at-least-16-chars"
access_ttl = 3600
refresh_ttl = 604800
[security]
aes_key = "{bad_key}"
"#
        );
        let path = write_config(&dir, &body);

        let result = check_config(&path).await;
        assert!(result.is_err());
        let full = format!("{:?}", result.unwrap_err());
        assert!(
            full.contains("32 字节"),
            "应报 aes_key 长度错误,实际: {full}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn empty_aes_key_passes_length_check() {
        // aes_key 为空 → 跳过长度校验(DB/Redis 连不上会在后续步骤失败,这里
        // 只验证空串不触发 aes_key 报错)
        let dir = unique_tmp("empty-aes");
        std::fs::create_dir_all(&dir).unwrap();
        let body = r#"
[server]
port = 3000
host = "localhost"
[db]
db_type = "sqlite"
db_name = ":memory:"
[redis]
url = "localhost"
port = 6379
db = 0
[jwt]
secret = "at-least-16-chars"
access_ttl = 3600
refresh_ttl = 604800
[security]
aes_key = ""
"#;
        let path = write_config(&dir, body);
        let result = check_config(&path).await;
        // Redis 可能连不上(本地无 Redis),但 aes_key 校验应通过
        // 错误信息里不应出现 "32 字节"
        if let Err(ref e) = result {
            let msg = format!("{e:?}");
            assert!(
                !msg.contains("32 字节"),
                "空 aes_key 不应触发长度错误,实际: {msg}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn db_wrong_port_fails() {
        let dir = unique_tmp("db-wrong");
        std::fs::create_dir_all(&dir).unwrap();
        // mysql 连不上的端口
        let body = r#"
[server]
port = 3000
host = "localhost"
[db]
db_type = "mysql"
url = "127.0.0.1"
port = 1
db_name = "db"
username = "u"
password = "p"
[redis]
url = "127.0.0.1"
port = 1
db = 0
[jwt]
secret = "at-least-16-chars"
access_ttl = 3600
refresh_ttl = 604800
"#;
        let path = write_config(&dir, body);
        let result = check_config(&path).await;
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn missing_server_host_fails() {
        // AppConfig 的 ServerConfig.host 是必填(无 serde default)
        let dir = unique_tmp("no-host");
        std::fs::create_dir_all(&dir).unwrap();
        let body = r#"
[server]
port = 3000
[db]
db_type = "sqlite"
db_name = ":memory:"
[redis]
url = "localhost"
port = 6379
db = 0
[jwt]
secret = "at-least-16-chars"
access_ttl = 3600
refresh_ttl = 604800
"#;
        let path = write_config(&dir, body);
        let result = check_config(&path).await;
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn dm_db(db_type: &str) -> app_core::config::DbConfig {
        app_core::config::DbConfig {
            db_type: db_type.to_string(),
            url: String::new(),
            port: 0,
            db_name: "DM8".to_string(),
            username: "SYSDBA".to_string(),
            password: "p".to_string(),
            schema: None,
        }
    }

    /// IM002 是 ODBC 标准码,与驱动管理器的语言环境无关。
    #[test]
    fn dm_hint_recognizes_missing_data_source() {
        let raw = "State: IM002, Native error: 0, Message: [unixODBC][Driver Manager]\
                   Data source name not found and no default driver specified";
        let hint = dm_hint(raw).expect("IM002 应当被识别");
        assert!(hint.contains("数据源"), "提示应指向 ODBC 数据源: {hint}");
    }

    /// 实测记录见 sql/dm/Readme.md:这条不是连不上服务器,而是驱动想弹窗补信息。
    #[test]
    fn dm_hint_recognizes_missing_credentials() {
        let raw = "State: HY000, Native error: -71008, Message: 需要更多信息 (SQLDriverConnect)";
        let hint = dm_hint(raw).expect("-71008 应当被识别");
        assert!(hint.contains("password"), "提示应指向凭据字段: {hint}");
    }

    #[test]
    fn dm_hint_recognizes_missing_schema() {
        let raw =
            "State: 42S02, Native error: -2007, Message: 无效的模式名[RUOYI]";
        let hint = dm_hint(raw).expect("无效的模式名应当被识别");
        assert!(hint.contains("schema"), "提示应指向 schema 配置: {hint}");
    }

    #[test]
    fn dm_hint_recognizes_missing_driver() {
        let raw = "[unixODBC][Driver Manager]Can't open lib '/opt/dmdbms/bin/libdodbc.so'";
        let hint = dm_hint(raw).expect("驱动加载失败应当被识别");
        assert!(hint.contains("驱动"), "提示应指向 ODBC 驱动: {hint}");
    }

    #[test]
    fn dm_hint_returns_none_for_unknown_error() {
        assert_eq!(dm_hint("connection reset by peer"), None);
    }

    /// mysql 的报错不该被达梦的提示污染 —— 哪怕文本恰好撞上签名。
    #[test]
    fn dm_hint_not_attached_for_other_backends() {
        let raw = "State: IM002, Message: Data source name not found";
        assert!(dm_hint_for(&dm_db("dm"), raw).is_some());
        assert_eq!(dm_hint_for(&dm_db("mysql"), raw), None);
        assert_eq!(dm_hint_for(&dm_db("postgres"), raw), None);
    }

    /// 连接失败的报错会被打到终端、甚至进日志 —— DSN 里的密码必须先抹掉。
    #[test]
    fn redact_dsn_hides_password() {
        // URL 形态:userinfo 里的 @:/;= 均被 percent-encode,首个 @ 必是分隔符
        assert_eq!(
            redact_dsn(
                "mysql://root:p%40ss@127.0.0.1:3306/db?tcp_keepalive=30000"
            ),
            "mysql://root:***@127.0.0.1:3306/db?tcp_keepalive=30000"
        );
        // ODBC 明文值
        assert_eq!(
            redact_dsn("dm:DSN=DM8;UID=SYSDBA;PWD=secret"),
            "dm:DSN=DM8;UID=SYSDBA;PWD=***"
        );
        // ODBC 花括号值(密码含分号,由 dsn::odbc_value 包裹)
        assert_eq!(
            redact_dsn("dm:DSN=DM8;UID=SYSDBA;PWD={pa;ss}"),
            "dm:DSN=DM8;UID=SYSDBA;PWD=***"
        );
        // sqlite 没有凭据,原样返回
        assert_eq!(redact_dsn("sqlite::memory:"), "sqlite::memory:");
    }

    /// Redis URL 的密码在 userinfo 里 —— 与 DB DSN 同一条脱敏规则。
    #[test]
    fn redact_dsn_hides_redis_password() {
        assert_eq!(
            redact_dsn("redis://:p%40ss@h:6379/10"),
            "redis://:***@h:6379/10"
        );
        // 无密码的 URL 原样返回
        assert_eq!(redact_dsn("redis://h:6379/10"), "redis://h:6379/10");
    }

    /// 字段值里含字面 `PWD=` 时,全文子串匹配会命中错位置、把真密码漏在
    /// 输出里 —— 必须按「段首为 PWD=」匹配。这是实现不用 find("PWD=") 的原因。
    #[test]
    fn redact_dsn_pwd_literal_in_field_value_does_not_leak() {
        // 密码本身含字面 PWD=(无分号,odbc_value 不会上花括号)
        assert_eq!(
            redact_dsn("dm:DSN=DM8;UID=x;PWD=sePWD=cret"),
            "dm:DSN=DM8;UID=x;PWD=***"
        );
        // 用户名含 `;PWD=`(odbc_value 会给它上花括号)—— 切段必须括号感知,
        // 否则真正的 PWD 段会原样留在输出里
        assert_eq!(
            redact_dsn("dm:DSN=DM8;UID={x;PWD=y};PWD=real"),
            "dm:DSN=DM8;UID={x;PWD=y};PWD=***"
        );
        // URL 形态优先:库名里含 `;PWD=` 时不得误入 ODBC 分支、漏掉 userinfo
        assert_eq!(
            redact_dsn("mysql://root:secret@h/db;PWD=x"),
            "mysql://root:***@h/db;PWD=x"
        );
    }

    /// ODBC 字段值里含 `://user:pass` 时不能误入 URL 分支 —— `://` 前
    /// 必须是合法 scheme,否则真密码会漏出。
    #[test]
    fn redact_dsn_url_like_odbc_value_takes_odbc_branch() {
        // 密码本身长得像 URL userinfo
        assert_eq!(
            redact_dsn("dm:DSN=DM8;UID=u;PWD=http://admin:p@ss"),
            "dm:DSN=DM8;UID=u;PWD=***"
        );
        // 数据源名含 `://a:b@c`,真密码在后面的 PWD 段
        assert_eq!(
            redact_dsn("dm:DSN=foo://a:b@c;UID=u;PWD=real"),
            "dm:DSN=foo://a:b@c;UID=u;PWD=***"
        );
    }

    /// 字段值中间的裸 `{`(odbc_value 只在含 `;`/`=` 或以 `{` 开头时才包裹)
    /// 会打破括号配对 —— 解析不可靠时必须整体脱敏,绝不回显原文。
    #[test]
    fn redact_dsn_unbalanced_brace_fails_closed() {
        assert_eq!(
            redact_dsn("dm:DSN=DM{8;UID=SYSDBA;PWD=secret"),
            "<格式异常,DSN 已整体脱敏>"
        );
    }
}
