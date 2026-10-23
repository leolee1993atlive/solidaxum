//! 达梦后端:把 SeaORM 的 Statement 翻译成 ODBC 调用。
//!
//! SQL 由 SeaORM 以 `DbBackend::MySql` 生成 —— 达梦接受 MySQL 方言的反引号标识符、
//! `LIMIT ? OFFSET ?` 与 `?` 占位符(实机实测)。唯一例外是 upsert,见 [`rewrite`]。

pub mod env;
pub mod pool;
pub mod rewrite;
pub mod row;
pub mod value;

use odbc_api::{Connection, ConnectionOptions};

#[derive(Debug, thiserror::Error)]
pub enum DmError {
    #[error("ODBC 错误: {0}")]
    Odbc(#[from] odbc_api::Error),
    /// 连接已因连接级故障(SQLSTATE 08xxx / HYT01)被弃用。持有该句柄的事务
    /// 无法继续,也无法提交 —— 达梦已在断连时隐式回滚。
    #[error("连接已失效:事务中途发生连接级故障,无法继续执行或提交")]
    ConnectionLost,
    #[error("不支持的 Value 类型: {0}")]
    UnsupportedValue(String),
    #[error("值超出达梦可表示范围: {0}")]
    ValueOutOfRange(String),
    /// 连接池已耗尽,在 `acquire_timeout` 内没等到空闲名额。
    #[error("获取达梦连接超时({0:?}):连接池已耗尽")]
    AcquireTimeout(std::time::Duration),
    /// 在已有事务上再次 `begin()`。ODBC 路径无 SAVEPOINT,放行会静默摊平事务。
    #[error("达梦后端不支持嵌套事务(ODBC 路径无 SAVEPOINT)")]
    NestedTransaction,
    /// 配置的 schema 名不合法。
    #[error("非法的 schema 配置: {0}")]
    InvalidSchema(String),
    #[error("不支持的 upsert 形态: {0}")]
    UnsupportedUpsert(String),
}

/// 建立一条裸 ODBC 连接。连接池见 `pool` 模块。
pub fn connect_raw(conn_str: &str) -> Result<Connection<'static>, DmError> {
    Ok(env::environment().connect_with_connection_string(
        conn_str,
        ConnectionOptions::default(),
    )?)
}
