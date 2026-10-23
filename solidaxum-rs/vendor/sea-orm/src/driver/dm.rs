//! 达梦 driver。SQL 由 `MysqlQueryBuilder` 生成(见 database/statement.rs),
//! 经 `dm::rewrite` 处理 upsert 后交给 ODBC。
//!
//! 事务隔离的结构前提:`begin()` 从池中取一条**独占**连接,move 进
//! `DatabaseTransaction` 并持有至事务结束;非事务查询从池中取其他连接。
//! 事务连接用完即弃(commit/rollback 走 `finish_txn`,连接不归还池),
//! 杜绝 `autocommit=off` 状态泄漏 —— 见 dm crate 的 pool 模块说明。
//!
//! 已知限制:不支持嵌套事务 —— 嵌套 `begin()` 是 no-op,内层 commit 会提交
//! 整个外层事务(sqlx 后端靠连接内的 savepoint 深度计数,ODBC 路径未实现)。
//! 基座不使用嵌套事务;引入前需先补 SAVEPOINT 支持。

use std::{future::Future, pin::Pin, sync::Arc};

use futures_util::lock::Mutex;
use tracing::instrument;

use crate::{
    AccessMode, ConnectOptions, DatabaseConnection, DatabaseConnectionType, DatabaseTransaction,
    IsolationLevel, QueryResult, QueryResultRow, Statement, TransactionError, debug_print, error::*,
    executor::*,
};

/// 达梦执行结果。`last_insert_id` 恒为 0:基座主键是应用侧生成的雪花 ID,
/// 不依赖数据库自增。
#[derive(Debug)]
pub(crate) struct DmExecResult {
    pub(crate) last_insert_id: u64,
    pub(crate) rows_affected: u64,
}

/// 定义达梦(DM8)ODBC 连接器
#[derive(Debug)]
pub struct DmConnector;

/// 达梦连接池(封装 dm crate 的 RAII 池)
#[derive(Clone)]
pub struct DmPoolConnection {
    pub(crate) pool: ::dm::pool::DmPool,
    metric_callback: Option<crate::metric::Callback>,
    pub(crate) record_stmt_in_spans: bool,
}

impl std::fmt::Debug for DmPoolConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DmPoolConnection")
    }
}

pub(crate) fn map_dm_conn_err(err: ::dm::DmError) -> DbErr {
    conn_err(err.to_string())
}

pub(crate) fn map_dm_err(err: ::dm::DmError) -> DbErr {
    query_err(err.to_string())
}

impl DmConnector {
    /// URL 以 `dm:` 开头即达梦,前缀之后是完整的 ODBC 连接串
    pub fn accepts(string: &str) -> bool {
        string.starts_with("dm:")
    }

    /// 建立达梦连接
    #[instrument(level = "trace")]
    pub async fn connect(options: ConnectOptions) -> Result<DatabaseConnection, DbErr> {
        let record_stmt_in_spans = options.get_record_stmt_in_spans();
        let conn_str = options
            .url
            .strip_prefix("dm:")
            .expect("accepts 已校验 dm: 前缀")
            .to_owned();
        // ConnectOptions 里只有 max_connections / acquire_timeout / schema_search_path
        // 对 ODBC 池有意义。min_connections / idle_timeout / max_lifetime 是 sqlx 池的
        // 概念,本池按需建连、`acquire` 时以 is_dead 做健康检查,**未实现**这三项 ——
        // 明确记录在此,免得下次有人对着配置文件推断行为。
        // acquire_timeout 的兜底取 30 秒,与 sqlx 一致。
        //
        // schema_search_path 借用 PG 那个字段表达「裸表名在哪个 schema 解析」——
        // 达梦是单实例单库,MySQL 的 database 对应它的 schema,语义完全对得上。
        let pool = ::dm::pool::DmPool::new(
            conn_str,
            options.max_connections.unwrap_or(8) as usize,
            options
                .acquire_timeout
                .unwrap_or(std::time::Duration::from_secs(30)),
            options.schema_search_path.clone(),
        );
        // 预热一条连接:把连接错误暴露在 connect 阶段,而不是拖到首个查询
        let warm = pool.acquire().await.map_err(map_dm_conn_err)?;
        drop(warm);

        Ok(
            DatabaseConnectionType::DmPoolConnection(DmPoolConnection {
                pool,
                metric_callback: None,
                record_stmt_in_spans,
            })
            .into(),
        )
    }
}

impl DmPoolConnection {
    /// Execute a [Statement] on a 达梦 backend
    #[instrument(level = "trace", skip(stmt))]
    pub async fn execute(&self, stmt: Statement) -> Result<ExecResult, DbErr> {
        debug_print!("{}", stmt);

        let mut conn = self.pool.acquire().await.map_err(map_dm_conn_err)?;
        execute_on(&mut conn, &stmt, &self.metric_callback).await
    }

    /// Execute an unprepared SQL statement on a 达梦 backend
    #[instrument(level = "trace", skip(sql))]
    pub async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        debug_print!("{}", sql);

        let mut conn = self.pool.acquire().await.map_err(map_dm_conn_err)?;
        execute_unprepared_on(&mut conn, sql).await
    }

    /// Get one result from a SQL query. Returns [Option::None] if no match was found
    #[instrument(level = "trace", skip(stmt))]
    pub async fn query_one(&self, stmt: Statement) -> Result<Option<QueryResult>, DbErr> {
        debug_print!("{}", stmt);

        let mut conn = self.pool.acquire().await.map_err(map_dm_conn_err)?;
        query_one_on(&mut conn, &stmt, &self.metric_callback).await
    }

    /// Get the results of a query returning them as a Vec<[QueryResult]>
    #[instrument(level = "trace", skip(stmt))]
    pub async fn query_all(&self, stmt: Statement) -> Result<Vec<QueryResult>, DbErr> {
        debug_print!("{}", stmt);

        let mut conn = self.pool.acquire().await.map_err(map_dm_conn_err)?;
        query_all_on(&mut conn, &stmt, &self.metric_callback).await
    }

    /// 事务隔离的结构前提:从池中取一条**独占**连接,move 进 DatabaseTransaction,
    /// 持有至事务结束。非事务查询从池中取其他连接,天然互不干扰。
    #[instrument(level = "trace")]
    pub async fn begin(&self) -> Result<DatabaseTransaction, DbErr> {
        let mut conn = self.pool.acquire().await.map_err(map_dm_conn_err)?;
        conn.exec(|c| c.set_autocommit(false).map_err(Into::into))
            .await
            .map_err(map_dm_conn_err)?;
        conn.mark_txn(); // 标记:用完即弃,Drop 时不归还池
        DatabaseTransaction::new_dm(
            conn,
            self.metric_callback.clone(),
            self.record_stmt_in_spans,
        )
        .await
    }

    /// 达梦不支持经 SET TRANSACTION 调整隔离级别/访问模式 —— 显式报错而非静默忽略
    #[instrument(level = "trace")]
    pub async fn begin_with_config(
        &self,
        isolation_level: Option<IsolationLevel>,
        access_mode: Option<AccessMode>,
    ) -> Result<DatabaseTransaction, DbErr> {
        if isolation_level.is_some() || access_mode.is_some() {
            return Err(DbErr::BackendNotSupported {
                db: "Dm",
                ctx: "IsolationLevel / AccessMode",
            });
        }
        self.begin().await
    }

    /// Create a 达梦 transaction
    #[instrument(level = "trace", skip(callback))]
    pub async fn transaction<F, T, E>(&self, callback: F) -> Result<T, TransactionError<E>>
    where
        F: for<'b> FnOnce(
                &'b DatabaseTransaction,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'b>>
            + Send,
        T: Send,
        E: std::fmt::Display + std::fmt::Debug + Send,
    {
        let transaction = self.begin().await.map_err(TransactionError::Connection)?;
        transaction.run(callback).await
    }

    pub(crate) fn set_metric_callback<F>(&mut self, callback: F)
    where
        F: Fn(&crate::metric::Info<'_>) + Send + Sync + 'static,
    {
        self.metric_callback = Some(Arc::new(callback));
    }

    /// Checks if a connection to the database is still valid.
    pub async fn ping(&self) -> Result<(), DbErr> {
        // acquire 内含 is_dead 健康检查,能取到即为活连接
        let conn = self.pool.acquire().await.map_err(map_dm_conn_err)?;
        drop(conn);
        Ok(())
    }

    /// Explicitly close the 达梦 connection.
    /// See [`Self::close_by_ref`] for usage with references.
    pub async fn close(self) -> Result<(), DbErr> {
        self.close_by_ref().await
    }

    /// Explicitly close the 达梦 connection
    pub async fn close_by_ref(&self) -> Result<(), DbErr> {
        // 池随 DatabaseConnection 的引用计数释放;与 mock/proxy 一致,此处无额外清理
        Ok(())
    }
}

/// 在一条(可能属于事务的)连接上执行写语句,返回受影响行数。
pub(crate) async fn execute_on(
    conn: &mut ::dm::pool::PooledConn,
    stmt: &Statement,
    metric_callback: &Option<crate::metric::Callback>,
) -> Result<ExecResult, DbErr> {
    let sql = ::dm::rewrite::rewrite(&stmt.sql)
        .map_err(|e| exec_err(e.to_string()))?
        .into_owned();
    let params = stmt
        .values
        .as_ref()
        .map(|v| ::dm::value::to_params(&v.0))
        .transpose()
        .map_err(|e| exec_err(e.to_string()))?
        .unwrap_or_default();

    crate::metric::metric!(metric_callback, stmt, {
        let affected = conn
            .exec(move |c| {
                let mut st = c.preallocate()?;
                // row_count 只在 Prepared / Preallocated 上,Connection::execute 拿不到
                if let Some(cursor) = st.execute(&sql, &params[..])? {
                    ::dm::row::drain_cursor(cursor)?;
                }
                Ok(st.row_count()?.unwrap_or(0) as u64)
            })
            .await
            .map_err(|e| exec_err(e.to_string()))?;
        Ok(ExecResult {
            result: ExecResultHolder::Dm(DmExecResult {
                last_insert_id: 0,
                rows_affected: affected,
            }),
        })
    })
}

/// 执行无参数的 SQL(execute_unprepared 路径)。
pub(crate) async fn execute_unprepared_on(
    conn: &mut ::dm::pool::PooledConn,
    sql: &str,
) -> Result<ExecResult, DbErr> {
    let sql = ::dm::rewrite::rewrite(sql)
        .map_err(|e| exec_err(e.to_string()))?
        .into_owned();
    let affected = conn
        .exec(move |c| {
            let mut st = c.preallocate()?;
            if let Some(cursor) = st.execute(&sql, ())? {
                ::dm::row::drain_cursor(cursor)?;
            }
            Ok(st.row_count()?.unwrap_or(0) as u64)
        })
        .await
        .map_err(|e| exec_err(e.to_string()))?;
    Ok(ExecResult {
        result: ExecResultHolder::Dm(DmExecResult {
            last_insert_id: 0,
            rows_affected: affected,
        }),
    })
}

/// 在一条(可能属于事务的)连接上执行查询,返回全部行。
pub(crate) async fn query_all_on(
    conn: &mut ::dm::pool::PooledConn,
    stmt: &Statement,
    metric_callback: &Option<crate::metric::Callback>,
) -> Result<Vec<QueryResult>, DbErr> {
    let sql = ::dm::rewrite::rewrite(&stmt.sql)
        .map_err(|e| query_err(e.to_string()))?
        .into_owned();
    let params = stmt
        .values
        .as_ref()
        .map(|v| ::dm::value::to_params(&v.0))
        .transpose()
        .map_err(|e| query_err(e.to_string()))?
        .unwrap_or_default();

    crate::metric::metric!(metric_callback, stmt, {
        let rows = conn
            .exec(move |c| {
                let mut st = c.preallocate()?;
                match st.execute(&sql, &params[..])? {
                    Some(cursor) => ::dm::row::cursor_to_rows(cursor),
                    None => Ok(Vec::new()),
                }
            })
            .await
            .map_err(map_dm_err)?;
        Ok(rows
            .into_iter()
            .map(|row| QueryResult {
                row: QueryResultRow::Dm(row),
            })
            .collect())
    })
}

/// 查询首行。
pub(crate) async fn query_one_on(
    conn: &mut ::dm::pool::PooledConn,
    stmt: &Statement,
    metric_callback: &Option<crate::metric::Callback>,
) -> Result<Option<QueryResult>, DbErr> {
    Ok(query_all_on(conn, stmt, metric_callback)
        .await?
        .into_iter()
        .next())
}

impl crate::DatabaseTransaction {
    /// 仿 `new_mysql`。autocommit 已在 [`DmPoolConnection::begin`] 关闭,
    /// 因此 [`Self::begin`] 里 InnerConnection::Dm 臂是 no-op。
    pub(crate) async fn new_dm(
        inner: ::dm::pool::PooledConn,
        metric_callback: Option<crate::metric::Callback>,
        record_stmt_in_spans: bool,
    ) -> Result<crate::DatabaseTransaction, DbErr> {
        Self::begin(
            Arc::new(Mutex::new(crate::InnerConnection::Dm(inner))),
            crate::DbBackend::Dm,
            metric_callback,
            record_stmt_in_spans,
            None,
            None,
            None,
        )
        .await
    }
}

// ---- 文本中间表示 → 目标类型的解码 ----
//
// 中间表示一律是 `Value::String`(dm crate 的 row 模块用 get_text 取值)。
// SeaORM 的每个 `TryGetable` 在 `QueryResultRow::Dm` 臂上调用 [`try_get`],
// 由 [`DmGetable`] 给出具体类型的文本解析。

use crate::executor::ColIdx;

/// 从达梦文本值解析出目标类型
pub(crate) trait DmGetable: Sized {
    fn dm_from_text(text: &str) -> Result<Self, TryGetError>;
}

pub(crate) fn try_get<T, I>(row: &::dm::row::DmRow, idx: I) -> Result<T, TryGetError>
where
    T: DmGetable,
    I: ColIdx,
{
    use sea_query::Value;
    let value: &Value = if let Some(name) = idx.as_str() {
        row.get(name)
            .ok_or_else(|| query_err(format!("No column for ColIdx {idx:?}")))?
    } else if let Some(i) = idx.as_usize() {
        row.get_by_index(*i)
            .ok_or_else(|| query_err(format!("Column at index {i} not found")))?
    } else {
        unreachable!("Missing ColIdx implementation for DmRow");
    };
    match value {
        Value::String(None) => Err(TryGetError::Null(format!("{idx:?}"))),
        Value::String(Some(text)) => T::dm_from_text(text.as_str()),
        other => Err(type_err(format!("达梦中间表示应为文本值,实际为 {other:?}")).into()),
    }
}

macro_rules! dm_getable_from_str {
    ($($t:ty),*) => {
        $(
            impl DmGetable for $t {
                fn dm_from_text(text: &str) -> Result<Self, TryGetError> {
                    text.parse::<$t>().map_err(|_| {
                        type_err(format!("无法把达梦文本 {text:?} 解析为 {}", stringify!($t)))
                            .into()
                    })
                }
            }
        )*
    };
}

dm_getable_from_str!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);

impl DmGetable for bool {
    fn dm_from_text(text: &str) -> Result<Self, TryGetError> {
        match text {
            "1" => Ok(true),
            "0" => Ok(false),
            other => other.parse::<bool>().map_err(|_| {
                type_err(format!("无法把达梦文本 {text:?} 解析为 bool")).into()
            }),
        }
    }
}

impl DmGetable for String {
    fn dm_from_text(text: &str) -> Result<Self, TryGetError> {
        Ok(text.to_owned())
    }
}

impl DmGetable for Vec<u8> {
    fn dm_from_text(text: &str) -> Result<Self, TryGetError> {
        Ok(text.as_bytes().to_vec())
    }
}

#[cfg(feature = "with-json")]
impl DmGetable for serde_json::Value {
    fn dm_from_text(text: &str) -> Result<Self, TryGetError> {
        serde_json::from_str(text).map_err(|e| crate::error::json_err(e).into())
    }
}

#[cfg(feature = "with-uuid")]
impl DmGetable for uuid::Uuid {
    fn dm_from_text(text: &str) -> Result<Self, TryGetError> {
        uuid::Uuid::parse_str(text)
            .map_err(|_| type_err(format!("无法把达梦文本 {text:?} 解析为 Uuid")).into())
    }
}

#[cfg(feature = "with-rust_decimal")]
impl DmGetable for rust_decimal::Decimal {
    fn dm_from_text(text: &str) -> Result<Self, TryGetError> {
        text.parse::<rust_decimal::Decimal>()
            .map_err(|_| type_err(format!("无法把达梦文本 {text:?} 解析为 Decimal")).into())
    }
}

#[cfg(feature = "with-bigdecimal")]
impl DmGetable for bigdecimal::BigDecimal {
    fn dm_from_text(text: &str) -> Result<Self, TryGetError> {
        text.parse::<bigdecimal::BigDecimal>()
            .map_err(|_| type_err(format!("无法把达梦文本 {text:?} 解析为 BigDecimal")).into())
    }
}

#[cfg(feature = "with-chrono")]
mod chrono_decode {
    use super::*;

    /// 达梦 DATETIME/TIMESTAMP 文本形如 "2026-08-12 09:28:24.212",不带时区。
    fn parse_naive_datetime(text: &str) -> Result<chrono::NaiveDateTime, TryGetError> {
        chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S%.f")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f"))
            .map_err(|_| {
                type_err(format!("无法把达梦文本 {text:?} 解析为 NaiveDateTime")).into()
            })
    }

    impl DmGetable for chrono::NaiveDate {
        fn dm_from_text(text: &str) -> Result<Self, TryGetError> {
            chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
                .map_err(|_| type_err(format!("无法把达梦文本 {text:?} 解析为 NaiveDate")).into())
        }
    }

    impl DmGetable for chrono::NaiveTime {
        fn dm_from_text(text: &str) -> Result<Self, TryGetError> {
            chrono::NaiveTime::parse_from_str(text, "%H:%M:%S%.f")
                .map_err(|_| type_err(format!("无法把达梦文本 {text:?} 解析为 NaiveTime")).into())
        }
    }

    impl DmGetable for chrono::NaiveDateTime {
        fn dm_from_text(text: &str) -> Result<Self, TryGetError> {
            parse_naive_datetime(text)
        }
    }

    impl DmGetable for chrono::DateTime<chrono::Utc> {
        fn dm_from_text(text: &str) -> Result<Self, TryGetError> {
            // 达梦不带时区,按 UTC 解释,与 sqlx mysql 的 DATETIME 语义一致
            parse_naive_datetime(text).map(|v| v.and_utc())
        }
    }

    impl DmGetable for chrono::DateTime<chrono::Local> {
        fn dm_from_text(text: &str) -> Result<Self, TryGetError> {
            parse_naive_datetime(text).map(|v| v.and_utc().into())
        }
    }
}

// time crate 的类型基座不用(用 chrono),显式报错而非静默给错值。
#[cfg(feature = "with-time")]
macro_rules! dm_getable_unsupported {
    ($($t:ty),*) => {
        $(
            impl DmGetable for $t {
                fn dm_from_text(_text: &str) -> Result<Self, TryGetError> {
                    Err(type_err(format!(
                        "达梦后端暂不支持 {} 类型的解码",
                        stringify!($t)
                    ))
                    .into())
                }
            }
        )*
    };
}

#[cfg(feature = "with-time")]
dm_getable_unsupported!(
    time::Date,
    time::Time,
    time::PrimitiveDateTime,
    time::OffsetDateTime
);
