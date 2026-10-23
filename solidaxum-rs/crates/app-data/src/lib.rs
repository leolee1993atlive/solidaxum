// async_trait 展开后的方法自带 `#[must_use]`,与返回类型的 must_use 重复,
// 新版 clippy 的 double_must_use 会因此对每个 async trait 方法报错。
#![allow(clippy::double_must_use)]

//! app-data:数据访问层唯一漏斗。
//!
//! workspace 里唯一依赖 sea-orm 的业务侧 crate(`crates/dm` 在 fork 之下,
//! 不计)。业务 crate 的 sea-orm 直连已全部移除 ——
//! `scripts/check-sea-orm-uniqueness.sh` 守着这条不变量。业务 crate 经
//! [`sea_orm`](受限 re-export)拼查询,经 `DataContext`/`TxnContext` 执行。
//!
//! ## 业务仓储/服务的公共契约
//!
//! 各业务仓储/服务的执行面统一经 `&DataContext<'_>`(连接路径)或
//! `&TxnContext<'_>`(事务路径):软删除、行级范围、审计盖章由漏斗按实体
//! `DataPolicy` 施加。错误统一 `DataError`(个别历史模块例外 —— job 的
//! 两个 service 返回保持 `anyhow::Result`、translation 服务统一
//! `AppError`,见各自文件头)。各仓储文件头只保留本文件独有的方法级
//! 裁定表(Java 锚逐方法核对),公共契约不再逐文件复制。

pub mod context;
pub mod count;
mod dialect;
pub mod error;
pub mod meta;
pub mod page;
pub mod policy;
pub mod principal;
pub mod sea_orm;
pub mod select_builder;

#[cfg(any(test, feature = "test-util"))]
pub mod testing;
#[cfg(any(test, feature = "test-util"))]
pub mod testkit;

pub use context::{DataContext, DataSource, TxnContext};
pub use count::{CountMode, DEFAULT_COUNT_CAP};
pub use error::DataError;
pub use page::{Page, Paging};
pub use policy::{AuditColumns, DataPolicy};
// 业务 crate SQL 断言用的合成函数(仅 test-util 面,与 testing 同门)。
#[cfg(any(test, feature = "test-util"))]
pub use principal::scope_condition_for_test;
pub use principal::{Principal, ScopeFilter, ScopeResolver};
pub use select_builder::SelectQuery;
