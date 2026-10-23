//! 关联表实体集中放置模块。
//!
//! 单设本模块的原因:关联表无单一归属方 —— `sys_user_role` 同时被
//! admin/menu/role/user 四个 feature 触及,放进任一 feature 模块都会制造
//! 对该 feature 的反向依赖,破坏分层。集中放置让所有消费方都从
//! `crate::relation::*` 平等引用,后续类别 A/B 改造(用 `Entity::insert_many`
//! /`delete_many`/`find` 替换原生 SQL)统一消费这些结构化实体。
//!
//! 各关联表实体的 `Relation` 故意保持空:跨表 JOIN 一律用调用点
//! `Entity::belongs_to(..).from(..).to(..).into()` 构造 `RelationDef`
//! (见 menu_repo/role_repo/post_repo),不在此声明 `BelongsTo` 变体。
pub mod role_dept;
pub mod role_menu;
pub mod user_post;
pub mod user_role;
