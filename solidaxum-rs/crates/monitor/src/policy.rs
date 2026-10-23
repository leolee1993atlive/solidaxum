//! monitor 实体的 DataPolicy 声明(设计 6.3,system/policy.rs 同款集中式)。
//!
//! 判定基线(2026-08-17 实读):SysOperLogMapper.java 与
//! SysLoginInfoMapper.java 均为空接口(extends BaseMapperPlus,
//! 零 @DataPermission)→ 双实体全豁免。两实体均无审计列、无软删列
//! (日志表只增不改,Java 锚即物理 delete/clean)。sys_oper_log 的
//! user_id/dept_id 是数据列(记录"谁操作的"),不是归属列 —— 勿"见列
//! 加回"(sys_social 的 user_id 同款裁定)。
//!
//! 统一锚:SysOperLogMapper/SysLoginInfoMapper 无 @DataPermission
//! —— 审计日志非行级数据。

use app_data::DataPolicy;

use crate::{logininfo::repo, operlog::repo as oper_repo};

impl DataPolicy for repo::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        None
    }
    fn audit_columns() -> Option<app_data::AuditColumns<Self::ActiveModel>> {
        None
    }
}

impl DataPolicy for oper_repo::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        None
    }
    fn audit_columns() -> Option<app_data::AuditColumns<Self::ActiveModel>> {
        None
    }
}
