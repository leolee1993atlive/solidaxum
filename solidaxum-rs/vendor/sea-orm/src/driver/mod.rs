#[cfg(feature = "mock")]
mod mock;
#[cfg(feature = "proxy")]
mod proxy;
#[cfg(feature = "rusqlite")]
pub(crate) mod rusqlite;
#[cfg(any(feature = "sqlx-sqlite", feature = "rusqlite"))]
mod sqlite;
#[cfg(feature = "sqlx-dep")]
mod sqlx_common;
#[cfg(feature = "sqlx-mysql")]
pub(crate) mod sqlx_mysql;
#[cfg(feature = "sqlx-postgres")]
pub(crate) mod sqlx_postgres;
#[cfg(feature = "sqlx-sqlite")]
pub(crate) mod sqlx_sqlite;
// 达梦 driver(fork 新增,无 feature 门控)
pub(crate) mod dm;

#[cfg(feature = "mock")]
pub use mock::*;
#[cfg(feature = "proxy")]
pub use proxy::*;
#[cfg(feature = "sqlx-dep")]
pub(crate) use sqlx_common::*;
#[cfg(feature = "sqlx-mysql")]
pub use sqlx_mysql::*;
#[cfg(feature = "sqlx-postgres")]
pub use sqlx_postgres::*;
#[cfg(feature = "sqlx-sqlite")]
pub use sqlx_sqlite::*;
// self:: 限定,避免与 dm crate 同名歧义
pub use self::dm::*;
