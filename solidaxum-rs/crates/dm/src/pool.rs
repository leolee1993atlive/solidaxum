//! RAII 连接池与同步 ODBC 到 async 的桥接。
//!
//! `odbc-api` 是同步阻塞的,所有调用必须经 `spawn_blocking` 移出 async 运行时,
//! 否则会阻塞 tokio 工作线程。`Connection<'static>: Send` 已在 Phase 0 编译期验证。
//!
//! 采用 checkout/release 而非闭包借用:`acquire()` 交出连接所有权,
//! 使其可被 move 进 `DatabaseTransaction` 并跨多次调用存活 —— 这是事务隔离的结构前提。

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use odbc_api::Connection;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{DmError, connect_raw};

#[derive(Clone)]
pub struct DmPool {
    inner: Arc<PoolInner>,
}

struct PoolInner {
    conn_str: String,
    idle: Mutex<Vec<Connection<'static>>>,
    permits: Arc<Semaphore>,
    acquire_timeout: Duration,
    /// 新建连接后立即切换到的 schema。`None` = 用登录用户自己的 schema(达梦默认)。
    schema: Option<String>,
}

/// 拥有式连接句柄。非事务句柄 `Drop` 时归还池;事务句柄(见 [`mark_txn`])
/// `Drop` 时**丢弃连接、仅释放名额**,绝不归还 —— 见事务收尾说明。
pub struct PooledConn {
    /// 用 `Option` 是为了在 `exec` / `finish_txn` 中把连接临时移出,以及 `Drop` 时取出。
    /// 正常态恒为 `Some`。只有两种情况会变成 `None`:事务收尾(`finish_txn`)之后,
    /// 以及 `exec` 遇到**连接级**故障(见 [`is_connection_fatal`])之后 ——
    /// 语句级错误不会清空它,事务可以继续。
    conn: Option<Connection<'static>>,
    inner: Arc<PoolInner>,
    _permit: OwnedSemaphorePermit,
    /// 是否服务于事务。事务句柄不论正常收尾还是被遗弃都不得归还池 ——
    /// 否则 `set_autocommit(false)` 的状态会泄漏给下一个使用者。
    txn: bool,
    /// 事务是否已开启。用于拒绝嵌套 `begin()`,见 [`PooledConn::begin_txn`]。
    begun: bool,
}

impl DmPool {
    /// `acquire_timeout` 是等待空闲名额的上限,对应 `ConnectOptions::acquire_timeout`。
    /// `schema` 对应 `ConnectOptions::schema_search_path`,`None` 时用登录用户自己的
    /// schema(达梦默认行为),见 [`set_schema_stmt`]。
    pub fn new(
        conn_str: String,
        size: usize,
        acquire_timeout: Duration,
        schema: Option<String>,
    ) -> Self {
        Self {
            inner: Arc::new(PoolInner {
                conn_str,
                idle: Mutex::new(Vec::new()),
                permits: Arc::new(Semaphore::new(size)),
                acquire_timeout,
                schema,
            }),
        }
    }

    /// 取一条连接,所有权交给调用方。池空且已达上限时等待。
    pub async fn acquire(&self) -> Result<PooledConn, DmError> {
        // 等名额必须有上限。无限等待时,池耗尽会让请求静默挂住、连一条日志都没有;
        // 一条泄漏的 `PooledConn` 就足以拖垮整个服务,且现场极难定位。
        let permit = tokio::time::timeout(
            self.inner.acquire_timeout,
            Arc::clone(&self.inner.permits).acquire_owned(),
        )
        .await
        .map_err(|_| DmError::AcquireTimeout(self.inner.acquire_timeout))?
        .expect("Semaphore 不会被关闭");
        let inner = Arc::clone(&self.inner);

        let conn =
            tokio::task::spawn_blocking(move || -> Result<_, DmError> {
                loop {
                    let candidate = inner.idle.lock().unwrap().pop();
                    match candidate {
                        // 复用前做一次廉价健康检查,避免把坏连接发出去
                        Some(c) if !c.is_dead().unwrap_or(true) => {
                            return Ok(c);
                        }
                        Some(_) => continue, // 坏连接丢弃,取下一条
                        // 新建连接:立即切到配置的 schema。只在**新建**时做即可 ——
                        // schema 是会话状态,归还池的连接仍带着它;事务连接用完即弃、
                        // 也总是新建。配置在启动时固定,不会中途变。
                        None => {
                            let conn = connect_raw(&inner.conn_str)?;
                            if let Some(schema) = inner.schema.as_deref() {
                                conn.execute(
                                    &set_schema_stmt(schema)?,
                                    (),
                                    None,
                                )?;
                            }
                            return Ok(conn);
                        }
                    }
                }
            })
            .await
            .expect("spawn_blocking 任务 panic")?;

        Ok(PooledConn {
            conn: Some(conn),
            inner: Arc::clone(&self.inner),
            _permit: permit,
            txn: false,
            begun: false,
        })
    }
}

impl std::fmt::Debug for PooledConn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledConn")
            .field("txn", &self.txn)
            .field("has_conn", &self.conn.is_some())
            .finish()
    }
}

impl PooledConn {
    /// 在阻塞线程池上执行闭包。连接临时移出再移回,故取 `&mut self`。
    pub async fn exec<T, F>(&mut self, f: F) -> Result<T, DmError>
    where
        F: FnOnce(&Connection<'static>) -> Result<T, DmError> + Send + 'static,
        T: Send + 'static,
    {
        let Some(conn) = self.conn.take() else {
            return Err(DmError::ConnectionLost);
        };
        let (result, conn) = tokio::task::spawn_blocking(move || {
            let r = f(&conn);
            (r, conn)
        })
        .await
        .expect("spawn_blocking 任务 panic");

        // **只有连接级故障才弃用连接。** 语句级错误(唯一键冲突、语法错误、
        // 字符串截断……)之后连接仍然完好,而事务里「捕获错误再继续」是常见写法 ——
        // 若在这里一并丢掉连接,后续语句就无连接可用,且 commit 会误报成功。
        // 被留下的坏连接也不会扩散:非事务句柄归还池后,`acquire` 的 is_dead
        // 健康检查会把它筛掉;事务句柄本就不归还。
        match &result {
            Err(e) if is_connection_fatal(e) => {}
            _ => self.conn = Some(conn),
        }
        result
    }

    /// 标记本句柄为事务句柄:`Drop` 时丢弃连接而非归还。由 driver 的 `begin()` 在
    /// `set_autocommit(false)` 之后调用。见 [`Self::finish_txn`] 与事务收尾说明。
    pub fn mark_txn(&mut self) {
        self.txn = true;
    }

    /// 登记「事务已开启」。第二次调用即嵌套事务,**报错而非静默摊平**。
    ///
    /// ODBC 路径没有 SAVEPOINT(sqlx 后端靠连接内的 savepoint 深度计数支持嵌套)。
    /// 若放行嵌套,内层 `commit()` 提交的会是整个外层事务,外层随后的回滚也回滚不掉
    /// 已提交的部分 —— 与「事务隔离由结构保证」的前提直接冲突。基座不用嵌套事务;
    /// 要引入得先实现 SAVEPOINT。
    pub fn begin_txn(&mut self) -> Result<(), DmError> {
        if self.begun {
            return Err(DmError::NestedTransaction);
        }
        self.begun = true;
        Ok(())
    }

    /// 事务收尾:在 `spawn_blocking` 里执行 commit/rollback 并丢弃连接,**不归还池**。
    /// 之所以连"丢弃"也放进 `spawn_blocking`,是为了遵守"所有 ODBC 调用不得跑在
    /// async 工作线程上"的原则 —— 连接 `Drop` 会触发 `SQLDisconnect`,是阻塞调用。
    /// 丢弃后 `conn == None`,本句柄随后的 `Drop` 见 `None` 即空操作。
    ///
    /// 若 `conn` 已因先前的连接级故障而丢失,两种收尾的语义并不对称:
    /// **回滚**已由达梦在断连时隐式完成,语义达成,返回 `Ok(())`;
    /// **提交**则无从提交,必须返回错误 —— 返回 `Ok(())` 会让调用方以为数据已落盘,
    /// 而它其实已随断连被回滚,是最难排查的一类静默丢数据。
    pub async fn finish_txn(&mut self, commit: bool) -> Result<(), DmError> {
        let Some(conn) = self.conn.take() else {
            return if commit {
                Err(DmError::ConnectionLost)
            } else {
                Ok(())
            };
        };
        tokio::task::spawn_blocking(move || -> Result<(), DmError> {
            let r = if commit {
                conn.commit()
            } else {
                conn.rollback()
            };
            // conn 在此 drop:ODBC 断开,残余事务由达梦侧隐式回滚
            r.map_err(Into::into)
        })
        .await
        .expect("spawn_blocking 任务 panic")
    }
}

/// 构造 `SET SCHEMA` 语句。
///
/// 达梦是单实例单库架构:MySQL 的 database 对应达梦的 **schema**,而 schema 即用户。
/// 连接后的默认 schema 是登录用户自己的那个;若表建在别的 schema 下(交付时的常态 ——
/// 建 `RUOYI` 用户装表、用另一个账号跑应用),裸表名解析不到,报
/// `无效的表或视图名[sys_user]`(SQLSTATE 42S02)。
///
/// 标识符用双引号包裹:实例上确实存在带连字符的 schema 名,不加引号解析不了;
/// 加引号同时保住大小写(建库参数 `CASE_SENSITIVE=N` 下不加引号也不会被折叠,
/// 但不能指望所有实例都这么建)。
fn set_schema_stmt(schema: &str) -> Result<String, DmError> {
    let name = schema.trim();
    if name.is_empty() {
        return Err(DmError::InvalidSchema("schema 名为空".to_owned()));
    }
    // 这是唯一被拼进 SQL 的配置项。内嵌双引号会截断标识符、把后续内容变成语句。
    if name.contains('"') {
        return Err(DmError::InvalidSchema(format!(
            "schema 名不得含双引号: {schema}"
        )));
    }
    Ok(format!(r#"SET SCHEMA "{name}""#))
}

/// 这个错误是否意味着连接本身已不可用。
///
/// 判据是 ODBC 的 SQLSTATE 类别:`08xxx` 是 connection exception,`HYT01` 是连接
/// 超时,二者之后连接都不能再用。其余类别 —— `23xxx` 完整性约束、`42xxx` 语法与
/// 权限、`22xxx` 数据异常 —— 都是**语句级**的,连接完好,事务可以继续。
///
/// 判错的代价是不对称的,所以此处从严定义「致命」:把坏连接误判为可用,最坏是下一次
/// 调用报错、并被 `acquire` 的健康检查筛掉,可自愈;把好连接误判为坏,则会白白终止
/// 一个本可继续的事务。
fn is_connection_fatal(err: &DmError) -> bool {
    let DmError::Odbc(odbc_api::Error::Diagnostics { record, .. }) = err else {
        return false;
    };
    let state = &record.state.0;
    state.starts_with(b"08") || state == b"HYT01"
}

impl Drop for PooledConn {
    fn drop(&mut self) {
        // `take()` 对两种句柄都执行 —— 事务句柄(已 finish_txn 或被遗弃)的连接
        // 就此丢弃、绝不归还;被遗弃的事务句柄此时仍有未提交事务,连接 Drop 时
        // 达梦侧隐式回滚。只有非事务句柄归还池复用:
        if let (false, Some(conn)) = (self.txn, self.conn.take()) {
            self.inner.idle.lock().unwrap().push(conn);
        }
        // 名额随 `_permit` 一并释放,池可按需新建连接补充
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一个「连接已失效」的事务句柄。不需要真实达梦:`PoolInner` 与信号量
    /// 许可都不依赖 ODBC,`conn: None` 正是 `exec` 报错后留下的状态。
    async fn orphaned_txn_handle() -> PooledConn {
        let inner = Arc::new(PoolInner {
            conn_str: String::new(),
            idle: Mutex::new(Vec::new()),
            permits: Arc::new(Semaphore::new(1)),
            acquire_timeout: Duration::from_millis(50),
            schema: None,
        });
        let permit = Arc::clone(&inner.permits).acquire_owned().await.unwrap();
        PooledConn {
            conn: None,
            inner,
            _permit: permit,
            txn: true,
            begun: false,
        }
    }

    /// 池耗尽时 `acquire` 必须在配置的超时内报错,而不是永久阻塞。
    ///
    /// 外层的 `tokio::time::timeout` 是回归护栏:一旦退回「无限等待」的行为,
    /// 这里会以明确的信息失败,而不是把整个测试套件挂死。生产上的对应故障是
    /// 一条泄漏的 `PooledConn` 让全部请求静默卡死,且没有任何诊断信息。
    #[tokio::test]
    async fn acquire_times_out_when_pool_is_exhausted() {
        let pool = DmPool::new(
            "DSN=unused".into(),
            1,
            Duration::from_millis(50),
            None,
        );
        // 占住唯一的名额来模拟「池已耗尽」—— 不需要真实达梦
        let _hog = Arc::clone(&pool.inner.permits)
            .acquire_owned()
            .await
            .unwrap();

        let outcome =
            tokio::time::timeout(Duration::from_secs(2), pool.acquire()).await;
        let Ok(result) = outcome else {
            panic!(
                "acquire 在池耗尽时永久阻塞 —— 应在配置的 acquire_timeout 内报错"
            );
        };
        let err = result.expect_err("池耗尽时应当报错");
        assert!(
            err.to_string().contains("超时"),
            "错误信息应指明是获取连接超时: {err}"
        );
    }

    /// 首次 `begin_txn` 成功,再次调用即嵌套,必须报错。
    ///
    /// ODBC 路径没有 SAVEPOINT,嵌套若放行会**静默摊平**:内层 `commit()` 提交的
    /// 是整个外层事务,外层随后的回滚也回滚不掉已提交的部分。基座有 92 处
    /// `C: ConnectionTrait` 泛型函数,任一处对已是事务的连接调 `begin()` 都会踩到。
    #[tokio::test]
    async fn rejects_nested_begin() {
        let mut c = orphaned_txn_handle().await;
        c.begin_txn().expect("首次开启事务应成功");
        let err = c.begin_txn().expect_err("嵌套 begin 必须报错");
        assert!(
            err.to_string().contains("嵌套"),
            "错误信息应指明是嵌套事务: {err}"
        );
    }

    /// schema 名用双引号包裹 —— 实例上确实存在带连字符的 schema
    /// (`solidaxum-demo`),不加引号解析不了;加引号同时保住大小写。
    #[test]
    fn quotes_schema_identifier() {
        assert_eq!(
            set_schema_stmt("RUOYI_DM").unwrap(),
            r#"SET SCHEMA "RUOYI_DM""#
        );
        assert_eq!(
            set_schema_stmt("solidaxum-demo").unwrap(),
            r#"SET SCHEMA "solidaxum-demo""#
        );
    }

    /// schema 名来自配置文件而非用户输入,但它是**唯一**被拼进 SQL 的配置项。
    /// 内嵌双引号会截断标识符、把后续内容变成语句,必须拒绝。
    #[test]
    fn rejects_schema_with_embedded_quote() {
        let err = set_schema_stmt(r#"a" ; DROP TABLE sys_user; --"#)
            .expect_err("含双引号的 schema 名必须拒绝");
        assert!(err.to_string().contains("schema"), "错误信息应指明: {err}");
    }

    #[test]
    fn rejects_empty_schema() {
        assert!(set_schema_stmt("").is_err(), "空 schema 名必须拒绝");
        assert!(set_schema_stmt("   ").is_err(), "空白 schema 名必须拒绝");
    }

    fn odbc_err(state: &[u8; 5]) -> DmError {
        DmError::Odbc(odbc_api::Error::Diagnostics {
            record: odbc_api::handles::Record {
                state: odbc_api::handles::State(*state),
                ..Default::default()
            },
            function: "SQLExecute",
        })
    }

    /// 连接已失效时提交,**必须报错**。返回 Ok 会让调用方以为提交成功,
    /// 而数据其实已随连接断开被达梦隐式回滚 —— 静默丢数据。
    #[tokio::test]
    async fn commit_without_connection_reports_error() {
        let mut c = orphaned_txn_handle().await;
        let err = c
            .finish_txn(true)
            .await
            .expect_err("无连接可提交时必须报错");
        assert!(
            err.to_string().contains("连接"),
            "错误信息应说明连接已失效: {err}"
        );
    }

    /// 回滚则相反:连接断开时达梦已隐式回滚,语义上已达成,报成功是正确的。
    #[tokio::test]
    async fn rollback_without_connection_succeeds() {
        let mut c = orphaned_txn_handle().await;
        c.finish_txn(false)
            .await
            .expect("连接断开已隐式回滚,rollback 应视为成功");
    }

    /// 连接已失效时再执行语句,要返回错误而不是 panic —— panic 会打穿整个请求任务。
    #[tokio::test]
    async fn exec_without_connection_reports_error_instead_of_panicking() {
        let mut c = orphaned_txn_handle().await;
        let err = c.exec(|_| Ok(())).await.expect_err("无连接时必须报错");
        assert!(
            err.to_string().contains("连接"),
            "错误信息应说明连接已失效: {err}"
        );
    }

    /// 语句级错误(唯一键冲突 23000、语法错误 42000)不该报废连接 ——
    /// 事务里业务代码捕获这类错误后继续用同一个事务是常见写法。
    #[test]
    fn statement_level_errors_are_not_connection_fatal() {
        assert!(!is_connection_fatal(&odbc_err(b"23000")), "唯一键冲突");
        assert!(!is_connection_fatal(&odbc_err(b"42000")), "语法错误");
        assert!(!is_connection_fatal(&odbc_err(b"22001")), "字符串截断");
    }

    /// 08 类(连接异常)与 HYT01(连接超时)是连接级故障,连接必须弃用。
    #[test]
    fn connection_class_errors_are_fatal() {
        assert!(is_connection_fatal(&odbc_err(b"08S01")), "通信链路故障");
        assert!(is_connection_fatal(&odbc_err(b"08003")), "连接未打开");
        assert!(is_connection_fatal(&odbc_err(b"HYT01")), "连接超时");
    }

    /// 非 ODBC 诊断类的错误(值映射失败等)与连接无关,连接可继续使用。
    #[test]
    fn non_odbc_errors_are_not_connection_fatal() {
        assert!(!is_connection_fatal(&DmError::UnsupportedValue("x".into())));
    }
}
