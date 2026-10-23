// async_trait 展开后的方法自带 `#[must_use]`,与返回类型的 must_use 重复,
// 新版 clippy 的 double_must_use 会因此对每个 async trait 方法报错。
#![allow(clippy::double_must_use)]

pub mod dto;
pub mod engine;
pub mod enums;
pub mod expression;
pub mod handler;
pub mod listener;
pub mod model;
pub mod notifier;
pub mod permission;
pub mod policy;
pub mod repository;
pub mod resolver;
pub mod route;
pub mod service;
