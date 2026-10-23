//! admin crate library 入口。
//!
//! 同时提供二进制(`src/main.rs`)与库目标:库目标导出 [`build_router`],
//! 使完整的应用路由可在集成测试中被复用(见 `tests/`)。

pub mod app;
pub mod assignee_sync_worker;
pub mod auth;
pub mod flow_notifier;
pub mod flow_resolver;
pub mod installer;
pub mod router;

#[cfg(feature = "test-util")]
pub mod testkit;

pub use router::{AuthMode, build_router};
