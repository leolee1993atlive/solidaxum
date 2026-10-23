//! job 实体的 DataPolicy 声明(设计 6.3,monitor/policy.rs 同款集中式)。
//!
//! 判定基线(2026-08-17 审计):Java `ruoyi-modules/ruoyi-job/` 只有
//! SnailJob 执行器,**无 SysJobMapper / SysJobLogMapper** —— sys_job /
//! sys_job_log 按"自有功能、非行级数据"豁免锚定(translation/notice
//! 同款裁定)→ 双实体 CONSTRAINED=false,每个读方法显式
//! `without_row_scope()`。
//!
//! 软删除:sys_job 有 del_flag(Java `@TableLogic` 语义,
//! `SysJobServiceImpl.deleteJobByIds` 即软删)→ 声明
//! `soft_delete() = Some((DelFlag, "0"))`,存活过滤由漏斗接管。
//! sys_job_log 是日志表(只增不改,
//! Java 锚即物理 delete/clean),无软删列。
//!
//! 审计列**类型裁定**:sys_job 的 create_by/update_by 是
//! `Option<String>`、create_time/update_time 是 `Option<NaiveDateTime>`,
//! 与漏斗盖章签名(`i64` / `DateTimeUtc`)**不匹配** →
//! `audit_columns() -> None`,手工盖章保留在 service 层;**不为漏斗改列
//! 类型**(改类型 = schema/序列化涟漪,超范围)。sys_job_log 的
//! create_time 由调度器写日志时显式 Set,同因 None。

use app_data::DataPolicy;

use crate::entity::{sys_job, sys_job_log};

impl DataPolicy for sys_job::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((sys_job::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<app_data::AuditColumns<Self::ActiveModel>> {
        None
    }
}

impl DataPolicy for sys_job_log::Entity {
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
