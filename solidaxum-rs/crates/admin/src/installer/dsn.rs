//! DB / Redis 连接串构建(生产启动与 `--check-config` 共用)。
//!
//! 为什么要集中:DSN 的 user/password 必须做 URL percent-encoding —— 密码含
//! `@:/` 等字符会破坏 URL 解析,这是生产事故的真实来源。生产路径
//! (`app::init_app`)与安装检测路径(`--check-config`)必须用**同一份**逻辑,
//! 否则「检测通过但启动连不上」或反之。
//!
//! DSN 构建语义参考自 `sqlx` / `sea-orm` 的 URL 解析约定。

use app_core::config::{DbConfig, RedisConfig};

/// URL 中需要 percent-encode 的字符集:控制字符 + 所有非 unreserved 字符。
/// RFC 3986 unreserved = `A-Za-z0-9-._~`,故保留 `-._~`,其余全部编码。
const USERINFO_ENCODE_SET: &percent_encoding::AsciiSet =
    &percent_encoding::CONTROLS
        .add(b' ')
        .add(b'"')
        .add(b'#')
        .add(b'<')
        .add(b'>')
        .add(b'`')
        .add(b'?')
        .add(b'{')
        .add(b'}')
        .add(b'|')
        .add(b'\\')
        .add(b'^')
        .add(b'[')
        .add(b']')
        .add(b'/')
        .add(b':')
        .add(b';')
        .add(b'=')
        .add(b'@')
        .add(b'!')
        .add(b'$')
        .add(b'&')
        .add(b'\'')
        .add(b'(')
        .add(b')')
        .add(b'*')
        .add(b'+')
        .add(b',')
        .add(b'%');

/// 对 userinfo(用户名/密码)做 percent-encoding。
fn enc(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, USERINFO_ENCODE_SET).to_string()
}

/// 构建 DB DSN(sea-orm/sqlx 可识别的 URL)。
///
/// - mysql/postgres:`{type}://{enc(user)}:{enc(pass)}@{url}:{port}/{db}` + keepalive 参数
/// - sqlite:`sqlite:{db_name}` 文件型再补 `?mode=rwc`(`:memory:` 与自带 `?` 的直通,`file:` 前缀剥掉)
pub fn build_db_dsn(db: &DbConfig) -> anyhow::Result<String> {
    match db.db_type.as_str() {
        "mysql" => {
            let base = format!(
                "mysql://{}:{}@{}:{}/{}",
                enc(&db.username),
                enc(&db.password),
                db.url,
                db.port,
                db.db_name,
            );
            Ok(format!("{base}?tcp_keepalive=30000"))
        }
        "postgres" | "postgresql" => {
            let base = format!(
                "postgres://{}:{}@{}:{}/{}",
                enc(&db.username),
                enc(&db.password),
                db.url,
                db.port,
                db.db_name,
            );
            // keepalive 参数对齐 app.rs 原有逻辑,防止空闲连接被中间设备断掉
            Ok(format!(
                "{base}?keepalives=1&keepalives_idle=30&keepalives_interval=10&keepalives_count=5&connect_timeout=10"
            ))
        }
        "sqlite" => {
            let path = db.db_name.trim();
            // sea-orm/sqlx 只认 `sqlite:` 起头的 DSN(按前缀分派驱动),
            // `file:` 前缀会被拒以 "no supporting driver"。sqlx 0.9 的
            // 解析:剥掉 `sqlite://`/`sqlite:` 后,剩余部分即文件路径
            // (`:memory:` 特判为共享内存库):
            //   :memory:          → sqlite::memory:  (testkit 已用)
            //   sqlite:...        → 直通(逃生口,懂 DSN 的人自己负责)
            //   file:X            → sqlite:X(兼容旧文案里宣传过的写法)
            //   普通/空           → sqlite:{name} / sqlite:data.db
            //
            // 文件型还须 `?mode=rwc`:sqlx 的 SqliteConnectOptions 默认
            // create_if_missing = false,而 sea-orm 只转发 url 不做额外
            // 打开策略 —— 首次部署库文件还不存在,不补这个参数就是
            // "unable to open database file (code 14)"。调用方自带 `?`
            // 时一律尊重(可能是 mode=ro 之类的显式意图)。
            // 2026-10-22 实测修复:此前产出的 `file:data.db` 在
            // --check-config 与运行时双双连不上,文件型 sqlite 形同虚设。
            let (base, has_query) = match path {
                "" => ("sqlite:data.db".to_string(), false),
                ":memory:" => ("sqlite::memory:".to_string(), true),
                p if p.starts_with("sqlite:") => {
                    (p.to_string(), p.contains('?'))
                }
                p => {
                    let p = p.strip_prefix("file:").unwrap_or(p);
                    (format!("sqlite:{p}"), false)
                }
            };
            // 内存库与显式 query 不需要(也不该)被补写
            let is_memory = base == "sqlite::memory:";
            Ok(if has_query || is_memory {
                base
            } else {
                format!("{base}?mode=rwc")
            })
        }
        // 达梦走 ODBC,不是 URL —— `db_name` 承载 ODBC 数据源名(不是 schema 名),
        // schema 另由 config 的 `[db].schema` 经 ConnectOptions 下发。
        //
        // **这里刻意不做 percent-encoding。** 上面几支编码是因为 DSN 是 URL,
        // 密码里的 `@:/` 会破坏 URL 解析;而 ODBC 连接串是 `键=值;` 结构,
        // 对它做 URL 编码反而会把密码改错(`@` 变成 `%40` 直接连不上)。
        //
        // ODBC 有自己的转义规则:值含 `;` 会截断连接串,须用 `{}` 包裹。
        // `}` 无法在各家驱动间可靠转义,故直接拒绝。
        "dm" => {
            let dsn_name = odbc_value("db_name", &db.db_name)?;
            let uid = odbc_value("username", &db.username)?;
            let pwd = odbc_value("password", &db.password)?;
            Ok(format!("dm:DSN={dsn_name};UID={uid};PWD={pwd}"))
        }
        unsupported => anyhow::bail!("不支持的数据库类型: {unsupported}"),
    }
}

/// `[db].schema` 的取用规则:非空才下发。
///
/// 安装器**总是**写这个键(空串 = 未设置),故 `Some("")` 必须与 `None` 等价。
/// 生产启动(`app.rs`)与 `--check-config`(`checker.rs`)共用本函数 —— 理由与
/// `build_db_dsn` 被集中的理由相同:两条路径分叉,就会出现「检测通过但启动
/// 连不上」或反之。
///
/// 达梦据此下发 `SET SCHEMA "x"`(见 `dm` crate 的 pool 模块),PostgreSQL 据此
/// 设 `search_path`,mysql / sqlite 的驱动忽略它。
///
/// **必须 trim 后再判空,也必须返回 trim 过的值。** 纯空白不是一个 schema 名,
/// 但它是个非空字符串 —— 不 trim 的话 `schema = " "` 会被当成有效配置下发,
/// 而 PostgreSQL 的 `SET search_path = " "` **不报错**(PG 只在解析对象时才报),
/// 于是 `--check-config` 通过、服务起得来,之后每条裸表名查询才炸
/// (`relation "sys_user" does not exist`)—— 正是本函数存在要防的那种分叉。
/// 达梦侧 `dm` crate 的 `set_schema_stmt` 自己也 trim,但不能指望下游兜底。
pub fn schema_search_path(db: &DbConfig) -> Option<&str> {
    db.schema
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// ODBC 连接串的值转义:含分隔符的值用 `{}` 包裹。
///
/// ODBC 连接串是 `键=值;键=值` 结构,值里出现 `;` 会截断整个串 —— 密码含分号
/// 是很常见的。规范做法是 `PWD={带;的密码}`。但 `}` 本身在各家驱动间没有统一
/// 的转义写法,遇到就直接报错,好过拼出一个语义错乱的连接串。
fn odbc_value(field: &str, v: &str) -> anyhow::Result<String> {
    if v.contains('}') {
        anyhow::bail!(
            "[db].{field} 不能包含 `}}` —— ODBC 连接串无法可靠转义该字符"
        );
    }
    if v.contains(';') || v.contains('=') || v.starts_with('{') {
        Ok(format!("{{{v}}}"))
    } else {
        Ok(v.to_string())
    }
}

/// 构建 Redis URL(deadpool-redis 可识别)。
///
/// 密码非空 → `redis://:{enc(pass)}@{url}:{port}/{db}`,否则无密码段。
/// `Some("")` 与 `None` 等价(都视为无密码)。
pub fn build_redis_url(redis: &RedisConfig) -> String {
    match redis.password.as_deref() {
        Some(p) if !p.is_empty() => {
            format!(
                "redis://:{}@{}:{}/{}",
                enc(p),
                redis.url,
                redis.port,
                redis.db
            )
        }
        _ => format!("redis://{}:{}/{}", redis.url, redis.port, redis.db),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(
        db_type: &str,
        url: &str,
        port: i32,
        db_name: &str,
        user: &str,
        pass: &str,
    ) -> DbConfig {
        DbConfig {
            db_type: db_type.to_string(),
            url: url.to_string(),
            port,
            db_name: db_name.to_string(),
            username: user.to_string(),
            password: pass.to_string(),
            schema: None,
        }
    }

    /// 达梦是 ODBC 连接串而非 URL:`db_name` 承载数据源名,且**不做 percent-encoding**
    /// —— 对 ODBC 串做 URL 编码会把密码改错(`@` → `%40` 直接连不上)。
    #[test]
    fn dm_builds_odbc_connection_string() {
        let dsn = build_db_dsn(&db("dm", "", 0, "DM8", "SYSDBA", "p@ss/word"))
            .unwrap();
        assert_eq!(dsn, "dm:DSN=DM8;UID=SYSDBA;PWD=p@ss/word");
    }

    /// 值含 `;` 会截断 ODBC 连接串 —— 必须用 `{}` 包裹。密码带分号很常见。
    #[test]
    fn dm_braces_values_containing_separators() {
        let dsn =
            build_db_dsn(&db("dm", "", 0, "DM8", "SYSDBA", "pa;ss")).unwrap();
        assert_eq!(dsn, "dm:DSN=DM8;UID=SYSDBA;PWD={pa;ss}");
    }

    /// `}` 在各家 ODBC 驱动间没有统一转义写法,直接报错好过拼出错乱的连接串。
    #[test]
    fn dm_rejects_unescapable_brace() {
        let err = build_db_dsn(&db("dm", "", 0, "DM8", "SYSDBA", "pa}ss"))
            .expect_err("含 `}` 的密码必须报错");
        assert!(
            err.to_string().contains("password"),
            "错误信息应指明是哪个字段: {err}"
        );
    }

    #[test]
    fn mysql_basic() {
        let dsn = build_db_dsn(&db(
            "mysql",
            "127.0.0.1",
            3306,
            "solidaxum",
            "root",
            "pass",
        ))
        .unwrap();
        assert_eq!(
            dsn,
            "mysql://root:pass@127.0.0.1:3306/solidaxum?tcp_keepalive=30000"
        );
    }

    #[test]
    fn postgres_basic() {
        let dsn =
            build_db_dsn(&db("postgres", "h", 5432, "db", "u", "p")).unwrap();
        assert!(dsn.starts_with("postgres://u:p@h:5432/db?"));
        assert!(dsn.contains("keepalives=1"));
        assert!(dsn.contains("connect_timeout=10"));
    }

    #[test]
    fn password_with_special_chars_encoded() {
        // 密码含 @:/ 等特殊字符,必须被编码,否则破坏 URL
        let dsn =
            build_db_dsn(&db("mysql", "h", 3306, "db", "u", "p@ss:/word"))
                .unwrap();
        assert_eq!(
            dsn,
            "mysql://u:p%40ss%3A%2Fword@h:3306/db?tcp_keepalive=30000"
        );
    }

    #[test]
    fn username_with_special_chars_encoded() {
        let dsn = build_db_dnl_check("user@host", "p");
        assert!(dsn.contains("user%40host"));
    }

    #[test]
    fn sqlite_memory() {
        let dsn =
            build_db_dsn(&db("sqlite", "", 0, ":memory:", "", "")).unwrap();
        assert_eq!(dsn, "sqlite::memory:");
    }

    #[test]
    fn sqlite_file_path() {
        // `file:` 前缀曾被原样透传,sea-orm 不认(只认 `sqlite:` 起头),
        // 2026-10-22 实测 "no supporting driver" 后修正;另补 mode=rwc,
        // 否则首次部署(库文件还不存在)报 unable to open database file
        let dsn =
            build_db_dsn(&db("sqlite", "", 0, "data.db", "", "")).unwrap();
        assert_eq!(dsn, "sqlite:data.db?mode=rwc");
    }

    #[test]
    fn sqlite_file_prefix_is_stripped() {
        // 兼容旧文案宣传过的 file: 写法
        let dsn =
            build_db_dsn(&db("sqlite", "", 0, "file:data.db", "", "")).unwrap();
        assert_eq!(dsn, "sqlite:data.db?mode=rwc");
    }

    #[test]
    fn sqlite_explicit_query_is_respected() {
        // 显式带 query 说明调用方有明确意图(如只读挂载),不补 mode=rwc
        let dsn =
            build_db_dsn(&db("sqlite", "", 0, "sqlite:a.db?mode=ro", "", ""))
                .unwrap();
        assert_eq!(dsn, "sqlite:a.db?mode=ro");
    }

    #[test]
    fn sqlite_empty_defaults() {
        let dsn = build_db_dsn(&db("sqlite", "", 0, "", "", "")).unwrap();
        assert_eq!(dsn, "sqlite:data.db?mode=rwc");
    }

    #[test]
    fn unsupported_type_errors() {
        assert!(build_db_dsn(&db("oracle", "h", 1, "d", "u", "p")).is_err());
    }

    /// schema 的取用规则只有一处实现 —— 生产启动(app.rs)与 --check-config
    /// 共用它。两条路径分叉会导致「检测通过但启动连不上」,这正是 build_db_dsn
    /// 当初被集中的原因。
    #[test]
    fn schema_search_path_only_when_non_empty() {
        let mut c = db("dm", "", 0, "DM8", "SYSDBA", "p");

        assert_eq!(schema_search_path(&c), None, "未配置 → 不下发");

        c.schema = Some(String::new());
        assert_eq!(
            schema_search_path(&c),
            None,
            "空串必须与 None 等价 —— 安装器总是写这个键"
        );

        c.schema = Some("RUOYI".to_string());
        assert_eq!(schema_search_path(&c), Some("RUOYI"));

        c.schema = Some("solidaxum-demo".to_string());
        assert_eq!(
            schema_search_path(&c),
            Some("solidaxum-demo"),
            "带连字符的 schema 名实机存在,不能在这一层被过滤掉"
        );
    }

    /// 纯空白必须与未配置等价。它是个**非空**字符串,只判 `is_empty()` 会放它
    /// 过去:PG 的 `SET search_path = " "` 不报错,于是检测通过、服务起得来,
    /// 之后每条裸表名查询才炸。安装脚本也可能产出这个值(用户拿空格当「清空」)。
    #[test]
    fn schema_search_path_treats_whitespace_as_unset() {
        let mut c = db("postgres", "h", 5432, "db", "u", "p");

        c.schema = Some(" ".to_string());
        assert_eq!(schema_search_path(&c), None, "单个空格 = 未配置");

        c.schema = Some("\t \n".to_string());
        assert_eq!(schema_search_path(&c), None, "任意空白组合 = 未配置");
    }

    /// 首尾空白要剥掉再下发:`SET SCHEMA " RUOYI "` 里带空格的标识符与
    /// `RUOYI` 不是同一个 schema,会解析不到表。
    #[test]
    fn schema_search_path_trims_surrounding_whitespace() {
        let mut c = db("dm", "", 0, "DM8", "SYSDBA", "p");
        c.schema = Some("  RUOYI  ".to_string());
        assert_eq!(schema_search_path(&c), Some("RUOYI"));
    }

    // 辅助:只测用户名编码(不关心完整 DSN)
    fn build_db_dnl_check(user: &str, pass: &str) -> String {
        build_db_dsn(&db("mysql", "h", 3306, "db", user, pass)).unwrap()
    }

    fn redis(
        url: &str,
        port: i32,
        password: Option<&str>,
        db: i32,
    ) -> RedisConfig {
        RedisConfig {
            url: url.to_string(),
            port,
            password: password.map(|s| s.to_string()),
            db,
        }
    }

    #[test]
    fn redis_with_password() {
        let u = build_redis_url(&redis("localhost", 6379, Some("secret"), 10));
        assert_eq!(u, "redis://:secret@localhost:6379/10");
    }

    #[test]
    fn redis_empty_password_treated_as_none() {
        let u = build_redis_url(&redis("localhost", 6379, Some(""), 10));
        assert_eq!(u, "redis://localhost:6379/10");
    }

    #[test]
    fn redis_none_password() {
        let u = build_redis_url(&redis("localhost", 6379, None, 10));
        assert_eq!(u, "redis://localhost:6379/10");
    }

    #[test]
    fn redis_password_special_chars_encoded() {
        let u = build_redis_url(&redis("h", 6379, Some("p@ss"), 0));
        assert_eq!(u, "redis://:p%40ss@h:6379/0");
    }
}
