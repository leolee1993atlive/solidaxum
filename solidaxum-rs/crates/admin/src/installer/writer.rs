//! `config.toml` 模板渲染 + 升级外科手术式更新。
//!
//! ## 两种模式
//!
//! - **首次渲染**([`render_fresh`]):文件不存在时,从内嵌模板
//!   `templates/config.toml.tmpl` 单遍扫描 `@PLACEHOLDER@` 替换 + CSPRNG 生成
//!   `jwt.secret` / `security.aes_key`。
//! - **升级更新**([`update_existing`]):文件已存在时,用 `toml_edit` 只覆盖
//!   `[server].port`、整段 `[db]`、整段 `[redis]`。**`[jwt].secret` 与
//!   `[security].aes_key` 永不触碰** —— 轮换 `aes_key` 会让已加密的 OSS 配置
//!   不可读。注释与其他段原样保留。
//!
//! ## 单遍扫描(而非链式 `.replace()`)
//!
//! 模板里的占位符用单遍字节扫描替换,而不是对每个占位符调一次 `str::replace`。
//! 链式替换有二次替换 bug:若用户密码值恰好等于某个占位符字面量(如
//! `@JWT_SECRET@`),第一轮写入后,第二轮会把密码里的字面量也替换掉。
//! 单遍扫描不会回看已输出的字节,天然免疫。

use std::{collections::HashMap, path::Path, str::FromStr};

use anyhow::Context;

use crate::installer::answers::Answers;

/// 内嵌的配置模板(编译期注入,文件移走会编译失败而非运行时静默读空)。
const TEMPLATE: &str = include_str!("../../templates/config.toml.tmpl");

/// JWT secret 长度(字符)。当前 token 已改 Redis session,此字段仅用于启动
/// 校验(≥16),生成 64 足够且与来源项目一致。
const JWT_SECRET_LEN: usize = 64;

/// AES-256 密钥的字节长度(`security.aes_key` 解码后须 32 字节,见 main.rs 校验)。
const AES_KEY_BYTES: usize = 32;

/// 渲染并写入 config.toml。
///
/// 文件不存在 → 首次渲染;存在 → 升级更新(保留密钥与手改段)。
/// 两个模式共用 [`resolve_https`] 物化出来的证书落地参数。
pub fn write_config(path: &Path, answers: &Answers) -> anyhow::Result<()> {
    let https = resolve_https(path, answers)?;
    if path.exists() {
        update_existing(path, answers, &https)
    } else {
        let rendered = render_fresh(answers, &https)?;
        // 自检:渲染结果必须是合法 TOML,否则密码里的特殊字符可能已破坏文件
        toml_edit::DocumentMut::from_str(&rendered)
            .with_context(|| "渲染出的 config.toml 不是合法 TOML(内部错误)")?;
        std::fs::write(path, rendered)
            .with_context(|| format!("写入 config.toml 失败: {:?}", path))?;
        Ok(())
    }
}

// ────────── HTTPS 物化 ──────────

/// 证书/私钥落在安装目录下的相对路径(config.toml 同级的 `certs/`)。
/// 用相对路径而非绝对路径:整目录搬迁后依然有效(服务 CWD = 安装目录)。
const HTTPS_CERT_REL: &str = "certs/server-cert.pem";
const HTTPS_KEY_REL: &str = "certs/server-key.pem";

/// answers 的 HTTPS 选择物化到磁盘后的渲染参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EffectiveHttps {
    pub enabled: bool,
    pub cert: String,
    pub key: String,
}

/// 把 `[server.https]` 选择落成证书文件,返回渲染/更新用的生效参数。
///
/// - `off`/缺节:不动磁盘,enabled=false(路径仍写默认值,占位无害)。
/// - `self-signed`:证书文件已存在则**复用不重生成**(重生成会让所有
///   已信任旧证书的浏览器再次告警);缺失才生成,有效期 3650 天。
/// - `import`:把用户提供的 PEM 复制进 `certs/`(保持安装目录自包含,
///   U 盘拔走后服务不受影响)。
fn resolve_https(
    config_path: &Path,
    answers: &Answers,
) -> anyhow::Result<EffectiveHttps> {
    let https = &answers.server.https;
    if https.is_off() {
        return Ok(EffectiveHttps {
            enabled: false,
            cert: HTTPS_CERT_REL.to_string(),
            key: HTTPS_KEY_REL.to_string(),
        });
    }

    let cert_dir = config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("certs");
    std::fs::create_dir_all(&cert_dir)
        .with_context(|| format!("创建证书目录失败: {:?}", cert_dir))?;
    let cert_p = cert_dir.join("server-cert.pem");
    let key_p = cert_dir.join("server-key.pem");

    match https.mode.as_str() {
        "self-signed" => {
            if !(cert_p.exists() && key_p.exists()) {
                let cn = https
                    .cn
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| "localhost".to_string());
                let (cert, key) = crate::tls::generate_self_signed(&cn)?;
                std::fs::write(&cert_p, cert).with_context(|| {
                    format!("写入自签证书失败: {:?}", cert_p)
                })?;
                std::fs::write(&key_p, key)
                    .with_context(|| format!("写入私钥失败: {:?}", key_p))?;
                restrict_key_permissions(&key_p);
            }
        }
        "import" => {
            let src_cert = https
                .cert
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "https.mode=import 需要在 [server.https].cert 提供证书路径"
                    )
                })?;
            let src_key = https
                .key
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "https.mode=import 需要在 [server.https].key 提供私钥路径"
                    )
                })?;
            copy_if_changed(Path::new(src_cert), &cert_p)?;
            copy_if_changed(Path::new(src_key), &key_p)?;
            restrict_key_permissions(&key_p);
        }
        other => anyhow::bail!(
            "未知的 [server.https].mode: {other}(支持 off / self-signed / import)"
        ),
    }

    Ok(EffectiveHttps {
        enabled: true,
        cert: HTTPS_CERT_REL.to_string(),
        key: HTTPS_KEY_REL.to_string(),
    })
}

/// 内容一致则跳过复制(升级重跑幂等,不动 mtime)。
fn copy_if_changed(src: &Path, dst: &Path) -> anyhow::Result<()> {
    let content = std::fs::read(src)
        .with_context(|| format!("读取证书/私钥失败: {:?}", src))?;
    if std::fs::read(dst).is_ok_and(|old| old == content) {
        return Ok(());
    }
    std::fs::write(dst, &content)
        .with_context(|| format!("写入失败: {:?}", dst))
}

/// 私钥权限收紧到仅属主可读写(unix;Windows 走 ACL,不动)。
fn restrict_key_permissions(key: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(
            key,
            std::fs::Permissions::from_mode(0o600),
        );
    }
    #[cfg(not(unix))]
    let _ = key;
}

// ────────── 首次渲染 ──────────

/// 从内嵌模板渲染出完整 config.toml(含随机生成的密钥)。
pub fn render_fresh(
    answers: &Answers,
    https: &EffectiveHttps,
) -> anyhow::Result<String> {
    let jwt_secret = gen_jwt_secret();
    let aes_key = gen_aes_key();
    let vars = build_vars(answers, &jwt_secret, &aes_key, https);
    Ok(render_template(TEMPLATE, &vars))
}

/// 组装占位符 → 值映射。字符串值先做 TOML 转义(密码含引号/反斜杠会破坏文件)。
fn build_vars(
    answers: &Answers,
    jwt_secret: &str,
    aes_key: &str,
    https: &EffectiveHttps,
) -> HashMap<&'static str, String> {
    let mut vars = HashMap::new();
    // 数字占位符:直接转字符串,不加引号
    vars.insert("PORT", answers.server.port.to_string());
    vars.insert("DB_PORT", answers.db.port.to_string());
    vars.insert("REDIS_PORT", answers.redis.port.to_string());
    vars.insert("REDIS_DB", answers.redis.db.to_string());
    // HTTPS(首装增强)
    vars.insert("HTTPS_ENABLED", https.enabled.to_string());
    vars.insert("HTTPS_CERT", toml_escape(&https.cert));
    vars.insert("HTTPS_KEY", toml_escape(&https.key));
    // 字符串占位符:TOML 转义后填入(模板里已带引号)
    vars.insert("DB_TYPE", toml_escape(&answers.db.db_type));
    vars.insert("DB_URL", toml_escape(&answers.db.url));
    vars.insert("DB_NAME", toml_escape(&answers.db.db_name));
    vars.insert("DB_USERNAME", toml_escape(&answers.db.username));
    vars.insert("DB_PASSWORD", toml_escape(&answers.db.password));
    vars.insert("DB_SCHEMA", toml_escape(&answers.db.schema));
    vars.insert("REDIS_URL", toml_escape(&answers.redis.url));
    vars.insert("REDIS_PASSWORD", toml_escape(&answers.redis.password));
    vars.insert("JWT_SECRET", toml_escape(jwt_secret));
    vars.insert("AES_KEY", toml_escape(aes_key));
    vars
}

/// 单遍扫描模板,替换 `@NAME@` 占位符。
///
/// 占位符语法:`@` 后跟 `[A-Z0-9_]+` 再跟 `@`。未命中的 `@X@` 原样输出
/// (避免误伤非占位符的 `@`)。逐 `char` 推进,保护多字节 UTF-8(中文注释)。
fn render_template(tmpl: &str, vars: &HashMap<&str, String>) -> String {
    let chars: Vec<char> = tmpl.chars().collect();
    let mut out = String::with_capacity(tmpl.len());
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c != '@' {
            out.push(c);
            i += 1;
            continue;
        }

        // 尝试匹配占位符:@[A-Z0-9_]+@
        let mut j = i + 1;
        while j < chars.len()
            && (chars[j].is_ascii_uppercase()
                || chars[j].is_ascii_digit()
                || chars[j] == '_')
        {
            j += 1;
        }

        if j > i + 1 && j < chars.len() && chars[j] == '@' {
            // 命中占位符结构,取出 name
            let name: String = chars[i + 1..j].iter().collect();
            if let Some(val) = vars.get(name.as_str()) {
                // 已知占位符 → 替换
                out.push_str(val);
            } else {
                // 未知占位符 → 原样输出(保留 @NAME@)
                out.extend(&chars[i..=j]);
            }
            i = j + 1;
        } else {
            // 不是占位符(裸 @,如 user@host),原样输出
            out.push(c);
            i += 1;
        }
    }

    out
}

/// TOML 字符串值转义:反斜杠和双引号。模板里占位符已带引号(`"@X@"`),
/// 所以这里只转义值内部的特殊字符。
fn toml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(c),
        }
    }
    out
}

// ────────── 密钥生成 ──────────

/// 生成 JWT secret:64 个随机字母数字字符。
fn gen_jwt_secret() -> String {
    use rand::distr::{Alphanumeric, Distribution};
    let mut rng = rand::rng();
    (0..JWT_SECRET_LEN)
        .map(|_| Alphanumeric.sample(&mut rng) as char)
        .collect()
}

/// 生成 AES-256 密钥:32 个随机字节 → base64 编码。
///
/// 对齐 `main.rs` 的 `base64::engine::general_purpose::STANDARD.decode` +
/// `len == 32` 校验。base64 编码后约 44 字符。
fn gen_aes_key() -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut bytes = [0u8; AES_KEY_BYTES];
    rand::fill(&mut bytes);
    STANDARD.encode(bytes)
}

// ────────── 升级外科手术式更新 ──────────

/// 已有 config.toml 的升级更新:只覆盖脚本收集到的 `[server]/[db]/[redis]`,
/// 永不碰 `[jwt].secret` 与 `[security].aes_key`,保留所有注释与其他段。
fn update_existing(
    path: &Path,
    answers: &Answers,
    https: &EffectiveHttps,
) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("读取现有 config.toml 失败: {:?}", path))?;
    let mut doc = toml_edit::DocumentMut::from_str(&raw)
        .with_context(|| format!("解析现有 config.toml 失败: {:?}", path))?;

    // [server].port
    if let Some(server) = doc.get_mut("server").and_then(|v| v.as_table_mut())
        && let Some(port) =
            server.get_mut("port").and_then(|v| v.as_value_mut())
    {
        *port = toml_edit::Value::from(i64::from(answers.server.port));
    }

    // 整段 [db]
    if let Some(db) = doc.get_mut("db").and_then(|v| v.as_table_mut()) {
        set_str(db, "db_type", &answers.db.db_type);
        set_str(db, "url", &answers.db.url);
        set_int(db, "port", answers.db.port);
        set_str(db, "db_name", &answers.db.db_name);
        set_str(db, "username", &answers.db.username);
        set_str(db, "password", &answers.db.password);
        // 与 username/password 同级:总是写。空串 = 未设置,故切换库类型时
        // 不会残留上一个类型的 schema。key 不存在时 set_str 会插入。
        set_str(db, "schema", &answers.db.schema);
    }

    // 整段 [redis]
    if let Some(redis) = doc.get_mut("redis").and_then(|v| v.as_table_mut()) {
        set_str(redis, "url", &answers.redis.url);
        set_int(redis, "port", answers.redis.port);
        // RedisConfig.password 是 Option<String>,空串与 None 等价
        set_str(redis, "password", &answers.redis.password);
        set_int(redis, "db", answers.redis.db);
    }

    // [server.https]:answers 选了 HTTPS 才写/更新;off/缺节**不动已有
    // 配置 —— 升级不能把客户手工配好的 HTTPS 关掉。
    if https.enabled
        && let Some(server) =
            doc.get_mut("server").and_then(|v| v.as_table_mut())
    {
        let mut t = toml_edit::Table::new();
        t.insert(
            "enabled",
            toml_edit::Item::Value(toml_edit::Value::from(true)),
        );
        set_str(&mut t, "cert", &https.cert);
        set_str(&mut t, "key", &https.key);
        server.insert("https", toml_edit::Item::Table(t));
    }

    // 注意:不触碰 [jwt].secret 与 [security].aes_key

    std::fs::write(path, doc.to_string())
        .with_context(|| format!("写回 config.toml 失败: {:?}", path))?;
    Ok(())
}

/// 设置 table 中某个 key 为字符串值。key 不存在时插入。
fn set_str(table: &mut toml_edit::Table, key: &str, val: &str) {
    table.insert(key, toml_edit::Item::Value(toml_edit::Value::from(val)));
}

/// 设置 table 中某个 key 为整数值。key 不存在时插入。
fn set_int(table: &mut toml_edit::Table, key: &str, val: i32) {
    table.insert(
        key,
        toml_edit::Item::Value(toml_edit::Value::from(i64::from(val))),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// off 模式的渲染参数(默认路径占位)
    fn https_off() -> EffectiveHttps {
        EffectiveHttps {
            enabled: false,
            cert: HTTPS_CERT_REL.to_string(),
            key: HTTPS_KEY_REL.to_string(),
        }
    }

    fn sample_answers() -> Answers {
        let raw = r#"
[server]
port = 3000
[db]
db_type = "mysql"
url = "127.0.0.1"
port = 3306
db_name = "solidaxum"
username = "root"
password = "secret"
[redis]
url = "localhost"
port = 6379
password = ""
db = 10
"#;
        toml::from_str(raw).unwrap()
    }

    #[test]
    fn fresh_render_with_https_enabled() {
        let answers = sample_answers();
        let https = EffectiveHttps {
            enabled: true,
            cert: HTTPS_CERT_REL.to_string(),
            key: HTTPS_KEY_REL.to_string(),
        };
        let rendered = render_fresh(&answers, &https).unwrap();
        let doc = toml_edit::DocumentMut::from_str(&rendered).unwrap();
        assert_eq!(
            doc["server"]["https"]["enabled"].as_bool(),
            Some(true),
            "enabled 必须是布尔值而不是字符串"
        );
        assert_eq!(
            doc["server"]["https"]["cert"].as_str(),
            Some("certs/server-cert.pem")
        );
    }

    #[test]
    fn fresh_render_with_https_off_keeps_defaults() {
        let answers = sample_answers();
        let rendered = render_fresh(&answers, &https_off()).unwrap();
        let doc = toml_edit::DocumentMut::from_str(&rendered).unwrap();
        assert_eq!(doc["server"]["https"]["enabled"].as_bool(), Some(false));
    }

    /// 自签闭环:write_config → certs/ 两文件落盘 → 装载器可读 →
    /// config 的 [server.https] 指向它们(即 --write-config 的真实产物)。
    #[test]
    fn write_config_self_signed_materializes_certs() {
        let dir = std::env::temp_dir().join(format!(
            "solidaxum-writer-https-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg_path = dir.join("config.toml");
        let answers: Answers = toml::from_str(
            r#"
[server]
port = 8443
[server.https]
mode = "self-signed"
cn = "10.0.0.8"
[db]
db_type = "mysql"
db_name = "solidaxum"
[redis]
url = "localhost"
port = 6379
db = 0
"#,
        )
        .unwrap();

        write_config(&cfg_path, &answers).unwrap();

        let cert_p = dir.join("certs/server-cert.pem");
        let key_p = dir.join("certs/server-key.pem");
        assert!(cert_p.exists() && key_p.exists(), "证书必须物化到 certs/");
        // 产物可被运行时装载器接受
        assert!(crate::tls::build_server_config(&cert_p, &key_p).is_ok());
        let doc = toml_edit::DocumentMut::from_str(
            &std::fs::read_to_string(&cfg_path).unwrap(),
        )
        .unwrap();
        assert_eq!(doc["server"]["https"]["enabled"].as_bool(), Some(true));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 升级语义:answers 未选 HTTPS(off/缺节)时,已有 [server.https]
    /// 原样保留 —— 升级不能把客户配好的 HTTPS 关掉。
    #[test]
    fn update_existing_does_not_touch_https_when_off() {
        let dir = std::env::temp_dir().join(format!(
            "solidaxum-writer-upd-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg_path = dir.join("config.toml");
        std::fs::write(
            &cfg_path,
            r#"
[server]
port = 3000
[server.https]
enabled = true
cert = "my/cert.pem"
key = "my/key.pem"
[db]
db_type = "mysql"
db_name = "solidaxum"
[redis]
url = "localhost"
port = 6379
db = 0
"#,
        )
        .unwrap();
        let answers = sample_answers(); // 无 https 节

        update_existing(&cfg_path, &answers, &https_off()).unwrap();

        let doc = toml_edit::DocumentMut::from_str(
            &std::fs::read_to_string(&cfg_path).unwrap(),
        )
        .unwrap();
        assert_eq!(doc["server"]["https"]["enabled"].as_bool(), Some(true));
        assert_eq!(
            doc["server"]["https"]["cert"].as_str(),
            Some("my/cert.pem")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_fresh_produces_valid_toml() {
        let answers = sample_answers();
        let rendered = render_fresh(&answers, &https_off()).unwrap();
        let doc = toml_edit::DocumentMut::from_str(&rendered).unwrap();
        assert_eq!(doc["server"]["port"].as_integer(), Some(3000));
        assert_eq!(doc["db"]["db_type"].as_str(), Some("mysql"));
        assert_eq!(doc["db"]["password"].as_str(), Some("secret"));
        assert_eq!(doc["redis"]["db"].as_integer(), Some(10));
        // 密钥已生成
        assert!(doc["jwt"]["secret"].as_str().unwrap().len() >= 16);
        let aes = doc["security"]["aes_key"].as_str().unwrap();
        // base64 解码后须 32 字节
        use base64::{Engine, engine::general_purpose::STANDARD};
        assert_eq!(STANDARD.decode(aes).unwrap().len(), 32);
    }

    #[test]
    fn render_fresh_does_not_depend_on_filesystem() {
        // TEMPLATE 是 include_str!,常量,不读文件
        let answers = sample_answers();
        let _ = render_fresh(&answers, &https_off()).unwrap();
    }

    #[test]
    fn password_with_quotes_backslashes_preserved() {
        // 密码含反斜杠和双引号 —— 渲染后必须仍是合法 TOML 且值正确
        let answers = Answers {
            server: crate::installer::answers::AnswersServer {
                port: 3000,
                https: Default::default(),
            },
            db: crate::installer::answers::AnswersDb {
                db_type: "mysql".into(),
                url: "h".into(),
                port: 3306,
                db_name: "db".into(),
                username: "u".into(),
                password: r#"p@\s"word"#.into(),
                schema: String::new(),
            },
            redis: crate::installer::answers::AnswersRedis {
                url: "h".into(),
                port: 6379,
                password: "".into(),
                db: 0,
            },
        };
        let rendered = render_fresh(&answers, &https_off()).unwrap();
        let doc = toml_edit::DocumentMut::from_str(&rendered).unwrap();
        assert_eq!(doc["db"]["password"].as_str(), Some(r#"p@\s"word"#));
    }

    #[test]
    fn password_equal_to_placeholder_literal_not_re_substituted() {
        // 密码恰好是占位符字面量 —— 单遍扫描不应二次替换
        let raw = r#"
[server]
port = 3000
[db]
db_type = "mysql"
url = "h"
port = 3306
db_name = "db"
username = "u"
password = "@JWT_SECRET@"
[redis]
url = "h"
port = 6379
password = ""
db = 0
"#;
        let answers: Answers = toml::from_str(raw).unwrap();
        let rendered = render_fresh(&answers, &https_off()).unwrap();
        let doc = toml_edit::DocumentMut::from_str(&rendered).unwrap();
        // password 必须原样保留,不被替换成实际的 jwt secret
        assert_eq!(doc["db"]["password"].as_str(), Some("@JWT_SECRET@"));
    }

    #[test]
    fn update_existing_preserves_jwt_secret_and_aes_key() {
        // 模拟已有 config(含手填的密钥与额外的 [oss] 段)
        let existing = r#"
[server]
port = 8080

[db]
db_type = "sqlite"
db_name = "old.db"

[redis]
url = "old-host"
port = 6380
password = "oldpass"
db = 5

[jwt]
secret = "OLD-JWT-SECRET-16chars"

[security]
aes_key = "OLD-AES-KEY"

[oss]
upload_body_limit = "20MB"
"#;
        let dir = std::env::temp_dir()
            .join(format!("axum-installer-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, existing).unwrap();

        let answers = sample_answers();
        update_existing(&path, &answers, &https_off()).unwrap();

        let updated = std::fs::read_to_string(&path).unwrap();
        let doc = toml_edit::DocumentMut::from_str(&updated).unwrap();

        // [server]/[db]/[redis] 已更新
        assert_eq!(doc["server"]["port"].as_integer(), Some(3000));
        assert_eq!(doc["db"]["db_type"].as_str(), Some("mysql"));
        assert_eq!(doc["db"]["url"].as_str(), Some("127.0.0.1"));
        assert_eq!(doc["redis"]["url"].as_str(), Some("localhost"));
        assert_eq!(doc["redis"]["db"].as_integer(), Some(10));

        // 密钥保留(未被覆盖)
        assert_eq!(
            doc["jwt"]["secret"].as_str(),
            Some("OLD-JWT-SECRET-16chars")
        );
        assert_eq!(doc["security"]["aes_key"].as_str(), Some("OLD-AES-KEY"));

        // 手改段保留
        assert_eq!(doc["oss"]["upload_body_limit"].as_str(), Some("20MB"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_template_leaves_bare_at_sign() {
        let vars: HashMap<&str, String> =
            vec![("X", "val".to_string())].into_iter().collect();
        let out = render_template("user@host @X@ @@ @UNKNOWN@", &vars);
        assert_eq!(out, "user@host val @@ @UNKNOWN@");
    }

    #[test]
    fn toml_escape_handles_backslash_and_quote() {
        assert_eq!(toml_escape(r#"a\b"c"#), r#"a\\b\"c"#);
    }

    #[test]
    fn gen_jwt_secret_is_alphanumeric_and_correct_len() {
        let s = gen_jwt_secret();
        assert_eq!(s.len(), JWT_SECRET_LEN);
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn gen_aes_key_decodes_to_32_bytes() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let k = gen_aes_key();
        let bytes = STANDARD.decode(k).unwrap();
        assert_eq!(bytes.len(), AES_KEY_BYTES);
    }

    #[test]
    fn gen_jwt_secret_is_random() {
        let a = gen_jwt_secret();
        let b = gen_jwt_secret();
        assert_ne!(a, b, "两次生成的 secret 不应相同");
    }

    fn dm_answers() -> Answers {
        let raw = r#"
[server]
port = 3000
[db]
db_type = "dm"
url = ""
port = 0
db_name = "DM8"
username = "SYSDBA"
password = "secret"
schema = "RUOYI"
[redis]
url = "localhost"
port = 6379
password = ""
db = 10
"#;
        toml::from_str(raw).unwrap()
    }

    #[test]
    fn render_fresh_writes_dm_and_schema() {
        let rendered = render_fresh(&dm_answers(), &https_off()).unwrap();
        let doc = toml_edit::DocumentMut::from_str(&rendered).unwrap();
        assert_eq!(doc["db"]["db_type"].as_str(), Some("dm"));
        assert_eq!(doc["db"]["db_name"].as_str(), Some("DM8"));
        assert_eq!(doc["db"]["schema"].as_str(), Some("RUOYI"));
    }

    /// mysql 场景:安装器总是写这个键,空串必须仍是合法 TOML 且语义 = 未设置。
    #[test]
    fn render_fresh_empty_schema_is_valid_toml() {
        let rendered = render_fresh(&sample_answers(), &https_off()).unwrap();
        let doc = toml_edit::DocumentMut::from_str(&rendered).unwrap();
        assert_eq!(doc["db"]["schema"].as_str(), Some(""));
    }

    /// 旧 config(达梦适配之前装的)没有 schema 键 —— 升级必须补上。
    #[test]
    fn update_existing_inserts_schema_when_absent() {
        let existing = r#"
[server]
port = 8080

[db]
db_type = "mysql"
url = "old-host"
port = 3306
db_name = "old"
username = "root"
password = "old"

[redis]
url = "old-host"
port = 6380
password = ""
db = 5

[jwt]
secret = "OLD-JWT-SECRET-16chars"

[security]
aes_key = "OLD-AES-KEY"
"#;
        let dir = std::env::temp_dir()
            .join(format!("axum-writer-insert-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, existing).unwrap();

        update_existing(&path, &dm_answers(), &https_off()).unwrap();

        let doc = toml_edit::DocumentMut::from_str(
            &std::fs::read_to_string(&path).unwrap(),
        )
        .unwrap();
        assert_eq!(doc["db"]["schema"].as_str(), Some("RUOYI"));
        assert_eq!(
            doc["security"]["aes_key"].as_str(),
            Some("OLD-AES-KEY"),
            "密钥仍不得被触碰"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// dm → mysql 切换:残留的 schema 必须被清掉,否则切库后语义错乱。
    #[test]
    fn update_existing_clears_stale_schema_on_backend_switch() {
        let existing = r#"
[server]
port = 8080

[db]
db_type = "dm"
url = ""
port = 0
db_name = "DM8"
username = "SYSDBA"
password = "x"
schema = "RUOYI"

[redis]
url = "h"
port = 6379
password = ""
db = 0

[jwt]
secret = "OLD-JWT-SECRET-16chars"
"#;
        let dir = std::env::temp_dir()
            .join(format!("axum-writer-switch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, existing).unwrap();

        // sample_answers() 是 mysql,schema 为空串
        update_existing(&path, &sample_answers(), &https_off()).unwrap();

        let doc = toml_edit::DocumentMut::from_str(
            &std::fs::read_to_string(&path).unwrap(),
        )
        .unwrap();
        assert_eq!(doc["db"]["db_type"].as_str(), Some("mysql"));
        assert_eq!(doc["db"]["schema"].as_str(), Some(""));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
