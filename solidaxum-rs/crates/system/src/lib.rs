// async_trait 展开后的方法自带 `#[must_use]`,与返回类型的 must_use 重复,
// 新版 clippy 的 double_must_use 会因此对每个 async trait 方法报错。
#![allow(clippy::double_must_use)]

pub mod app;
pub mod assignee_sync;
pub mod client;
pub mod codegen;
pub mod config;
pub mod dept;
pub mod dict;
pub mod menu;
pub mod message;
pub mod middleware;
pub mod notice;
pub mod oss;
pub mod permission;
pub mod policy;
pub mod post;
pub mod profile;
pub mod push;
pub mod relation;
pub mod role;
pub mod social;
pub mod token;
pub mod translation;
pub mod user;

pub use crate::{
    oss::route::oss_route::oss_route, push::route::push_route::push_route,
};
