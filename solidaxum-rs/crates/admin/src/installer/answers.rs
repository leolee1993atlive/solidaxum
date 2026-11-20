//! `answers.toml` 反序列化结构。
//!
//! 这是安装脚本层写给二进制的中间文件(用文件而非 CLI 参数,因为 Windows
//! 进程列表任何进程可读,密码不该进 argv)。只含脚本收集到的三段:
//! `[server]`(含可选 `[server.https]` 子节)/ `[db]` / `[redis]`。

use serde::Deserialize;

/// 脚本层收集的安装参数。
#[derive(Debug, Clone, Deserialize)]
pub struct Answers {
    pub server: AnswersServer,
    pub db: AnswersDb,
    pub redis: AnswersRedis,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnswersServer {
    pub port: i32,
    /// HTTPS 选择(首装增强)。旧脚本不写这一节 → mode 默认 "off",
    /// 行为与从前完全一致(升级兼容)。
    #[serde(default)]
    pub https: AnswersHttps,
}

/// `[server.https]` 安装参数。
///
/// 脚本只在用户选择 HTTPS 时写这一节;`mode = "off"` 与缺节等价
/// (writer 对两者都不触碰已有配置)。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AnswersHttps {
    /// "off" | "self-signed" | "import"
    pub mode: String,
    /// self-signed:服务器 IP 或主机名(写入 SAN/CN)。
    pub cn: Option<String>,
    /// import:已有证书 PEM 路径(安装时复制进安装目录)。
    pub cert: Option<String>,
    /// import:已有私钥 PEM 路径。
    pub key: Option<String>,
}

impl AnswersHttps {
    pub fn is_off(&self) -> bool {
        self.mode.is_empty() || self.mode == "off"
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnswersDb {
    pub db_type: String,
    /// 主机地址(mysql/postgres 用;sqlite 省略)。
    #[serde(default)]
    pub url: String,
    /// 端口(mysql/postgres 用;sqlite 省略,反序列化为 0)。
    #[serde(default)]
    pub port: i32,
    pub db_name: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    /// 达梦 / PostgreSQL:裸表名解析所在的 schema。空串 = 不设置,其余后端忽略。
    ///
    /// 安装器总是写这个键,所以空串必须是合法输入。注意 `Answers` 未启用
    /// `deny_unknown_fields` —— 旧二进制读到本字段会静默忽略。脚本与二进制同包
    /// 发布,这个错配不会在交付路径上出现。
    #[serde(default)]
    pub schema: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnswersRedis {
    pub url: String,
    pub port: i32,
    #[serde(default)]
    pub password: String,
    pub db: i32,
}

impl Answers {
    /// 从 `answers.toml` 文件加载。
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|e| {
            anyhow::anyhow!("读取 answers 文件失败 {:?}: {e}", path)
        })?;
        let answers: Answers = toml::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("解析 answers.toml 失败: {e}"))?;
        Ok(answers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mysql_answers() {
        let raw = r#"
[server]
port = 3000

[db]
db_type = "mysql"
url = "127.0.0.1"
port = 3306
db_name = "solidaxum"
username = "root"
password = "p@ss"

[redis]
url = "localhost"
port = 6379
password = ""
db = 10
"#;
        let a: Answers = toml::from_str(raw).unwrap();
        assert_eq!(a.server.port, 3000);
        assert_eq!(a.db.db_type, "mysql");
        assert_eq!(a.db.password, "p@ss");
        assert_eq!(a.redis.db, 10);
    }

    #[test]
    fn parse_sqlite_answers_omits_db_fields() {
        // sqlite 合法地省略 url/port/username/password
        let raw = r#"
[server]
port = 8080

[db]
db_type = "sqlite"
db_name = ":memory:"

[redis]
url = "localhost"
port = 6379
db = 0
"#;
        let a: Answers = toml::from_str(raw).unwrap();
        assert_eq!(a.db.db_type, "sqlite");
        assert_eq!(a.db.url, "");
        assert_eq!(a.db.port, 0);
        assert_eq!(a.db.username, "");
        assert_eq!(a.db.password, "");
    }

    /// 达梦:db_name 承载 ODBC 数据源名,url/port 无意义
    /// (SERVER/TCP_PORT 由 ODBC 数据源提供)。
    #[test]
    fn parse_dm_answers_with_schema() {
        let raw = r#"
[server]
port = 3000

[db]
db_type = "dm"
url = ""
port = 0
db_name = "DM8"
username = "SYSDBA"
password = "p@ss"
schema = "RUOYI"

[redis]
url = "localhost"
port = 6379
db = 0
"#;
        let a: Answers = toml::from_str(raw).unwrap();
        assert_eq!(a.db.db_type, "dm");
        assert_eq!(a.db.db_name, "DM8", "达梦的 db_name 是 ODBC 数据源名");
        assert_eq!(a.db.schema, "RUOYI");
    }

    /// HTTPS 三模式(首装增强):self-signed 段解析。
    #[test]
    fn parse_https_self_signed_answers() {
        let raw = r#"
[server]
port = 3000
[server.https]
mode = "self-signed"
cn = "192.168.1.10"
[db]
db_type = "mysql"
db_name = "solidaxum"
[redis]
url = "localhost"
port = 6379
db = 0
"#;
        let a: Answers = toml::from_str(raw).unwrap();
        assert!(!a.server.https.is_off());
        assert_eq!(a.server.https.mode, "self-signed");
        assert_eq!(a.server.https.cn.as_deref(), Some("192.168.1.10"));
        assert_eq!(a.server.https.cert, None);
    }

    /// 旧脚本(不写 https 节)配新二进制:mode 默认 off,升级零迁移。
    #[test]
    fn https_section_absent_means_off() {
        let a: Answers = toml::from_str(
            r#"
[server]
port = 3000
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
        assert!(a.server.https.is_off());
    }

    /// 旧脚本(不写 schema)配新二进制:必须得空串,而不是解析失败。
    #[test]
    fn schema_defaults_to_empty_when_absent() {
        let raw = r#"
[server]
port = 3000

[db]
db_type = "mysql"
url = "127.0.0.1"
port = 3306
db_name = "solidaxum"
username = "root"
password = ""

[redis]
url = "localhost"
port = 6379
db = 0
"#;
        let a: Answers = toml::from_str(raw).unwrap();
        assert_eq!(a.db.schema, "");
    }
}
