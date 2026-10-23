//! system 全部实体的 DataPolicy 声明(设计 6.3)。集中一处的取舍:
//! 策略语义一眼可审,不散落在各业务模块的 repo 文件里。
//!
//! 判定规则(语义基线 = Java Mapper 的 @DataPermission 注解对齐;
//! 列名以实体文件实际 `#[sea_orm]` 定义为准):
//! - `dept_column`:Mapper 带 @DataPermission(dept) 的表才声明。sys_user 用
//!   dept_id;sys_role 用 create_dept(Java SysRoleMapper 即锚此列,表无
//!   dept_id);sys_dept 用自身主键 dept_id(部门行可见 ⟺ 自身 ∈ 可见集);
//!   sys_post 用 dept_id。
//! - `owner_column`:Mapper 带 @DataPermission(userName) 的表才声明。sys_user
//!   用 create_by(Java SysUserMapper.java:39 userName=
//!   create_by);sys_post 用 create_by(SysPostMapper.java:31 userName=create_by);
//!   sys_role 用 create_by(SysRoleMapper 的 4 个注解方法
//!   :33/:47/:61/:75 全带 userName=create_by)。
//!   sys_social 虽有 user_id 列,但 SysSocialMapper 无注解 → 豁免,勿"见列加回"。
//! - `CONSTRAINED`:上面两者任一 Some → 缺省 true;否则必须显式 false ——
//!   无范围列的表若保持缺省 true,非 All 过滤器会触发 fail-closed
//!   `Unsupported`(scope_condition 语义),豁免必须显式、可 grep。
//! - **`CONSTRAINED` 只管读路径**:四个受辖实体
//!   (sys_user / sys_role / sys_dept / sys_post)的 list/page/select 照常
//!   施加行级范围;**写路径(update/软删)一律走
//!   `update_without_row_scope`**,越权拦截由 handler 的
//!   `check_{user,dept,role}_data_scope` 探针承担 —— 对齐 Java 的实际
//!   拦截模型(controller 显式探针 + 查询方法带注解,写方法本身不带范围)。
//!   post 在 Java 侧连探针都没有,写路径完全不受辖。
//!   并发窗口(TOCTOU)说明:探针(count)与后续写执行之间存在竞态窗口
//!   —— 目标行归属若在窗口内被并发改写(移出操作者范围),写仍会落库。
//!   这是 Java 同模型的平价行为(Java 同样是 controller 探针 + 无范围
//!   写,无锁无事务包裹),属既定取舍而非遗漏。
//! - 审计五件套有什么盖什么,`create_dept` 传部门值(闭包见 user 示例);
//!   实体没有的列不写闭包,全部没有则 `audit_columns() -> None`。
//! - 关联表(relation/*)的 user_id/dept_id 是关联键不是归属,全 None。

use app_core::model::status::DelFlag;
use app_data::{AuditColumns, DataPolicy, sea_orm::ActiveValue};

use crate::{
    assignee_sync::repository::entity::flow_assignee_sync,
    client::repository::client_repo,
    codegen::repository::{gen_column_repo, gen_table_repo},
    config::repository::config_repo,
    dept::repository::dept_repo,
    dict::repository::{dict_data_repo, dict_type_repo},
    menu::repository::menu_repo,
    message::repository::message_repo,
    notice::repository::notice_repo,
    oss::repository::{oss_config_repo, oss_repo},
    post::repository::post_repo,
    relation::{role_dept, role_menu, user_post, user_role},
    role::repository::role_repo,
    social::repository::social_repo,
    user::repository::user_repo,
};

// ---------------------------------------------------------------------------
// 受行级数据权限约束:有业务 dept/owner 列,CONSTRAINED 走缺省 true。
// ---------------------------------------------------------------------------

/// sys_user:数据权限的主战场 —— dept_id 定部门范围,create_by 定"仅本人"
/// (Java 基线:SysUserMapper.java:37-43 的
/// @DataColumn userName=create_by,"仅本人"是"我创建的用户"而非"我本人",
/// 与 SysPostMapper.java:31 同锚;勿锚 user_id,SelfOnly 会把建单语义
/// 误变自见)。DeptInOrSelf 的 OR 语义两者都成立。审计五件套齐全(含
/// create_dept)。
impl DataPolicy for user_repo::Entity {
    fn dept_column() -> Option<Self::Column> {
        Some(Self::Column::DeptId)
    }
    fn owner_column() -> Option<Self::Column> {
        Some(Self::Column::CreateBy)
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((Self::Column::DelFlag, DelFlag::Alive.as_str()))
    }
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

/// sys_role:受辖 —— Java 基线 SysRoleMapper 的 4 个查询方法(:33/:47/:61/:75)
/// 都带 @DataPermission 双列:部门范围锚 create_dept(角色表无 dept_id,Java 侧
/// 即用此审计列当部门归属),"仅本人"锚 create_by(与
/// sys_user/sys_post 同锚)。软删 + 审计五件套齐全(含 create_dept)。
impl DataPolicy for role_repo::Entity {
    fn dept_column() -> Option<Self::Column> {
        Some(Self::Column::CreateDept)
    }
    fn owner_column() -> Option<Self::Column> {
        Some(Self::Column::CreateBy)
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((Self::Column::DelFlag, DelFlag::Alive.as_str()))
    }
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

/// sys_dept:受辖 —— Java 基线 SysDeptMapper 带 @DataPermission,范围列是
/// 自身主键 dept_id(部门行可见 ⟺ 自身 id ∈ 可见部门集)。软删 + 审计五件套齐全。
impl DataPolicy for dept_repo::Entity {
    fn dept_column() -> Option<Self::Column> {
        Some(Self::Column::DeptId)
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((Self::Column::DelFlag, DelFlag::Alive.as_str()))
    }
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

/// sys_post:岗位归属部门(dept_id 是业务列);"仅本人"锚 create_by ——
/// 对齐 Java 基线 SysPostMapper.java:31 的 userName=create_by(建岗人即归属人,
/// 表无 user_id)。软删 + 审计五件套齐全。
impl DataPolicy for post_repo::Entity {
    fn dept_column() -> Option<Self::Column> {
        Some(Self::Column::DeptId)
    }
    fn owner_column() -> Option<Self::Column> {
        Some(Self::Column::CreateBy)
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((Self::Column::DelFlag, DelFlag::Alive.as_str()))
    }
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

// sys_social → 见下方"显式豁免"一节(Java SysSocialMapper 无 @DataPermission)。

// ---------------------------------------------------------------------------
// 显式豁免(CONSTRAINED=false)但保留软删与审计的主数据表:
// 无业务 dept/owner 列 —— create_dept 只是审计,数据权限过滤对它们无业务含义。
// ---------------------------------------------------------------------------

/// sys_social:三方账号绑定。**Java 基线:SysSocialMapper 无 @DataPermission
/// 注解**,三方绑定在 Java 侧本就不受数据权限约束。表虽有 user_id 列,但那是
/// 绑定目标不是权限归属 —— 勿以"它有 user_id"为由加回 owner_column(会把
/// Java 里全量可见的绑定数据误砍到"仅本人")。无部门列;审计列只有
/// create_time,没有的不盖。
impl DataPolicy for social_repo::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((Self::Column::DelFlag, DelFlag::Alive.as_str()))
    }
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: None,
            create_by: None,
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: None,
            update_time: None,
        })
    }
}

/// sys_client:认证客户端是全局配置。软删 + 审计五件套照常,范围列全 None。
impl DataPolicy for client_repo::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((Self::Column::DelFlag, DelFlag::Alive.as_str()))
    }
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

// ---------------------------------------------------------------------------
// 显式豁免的全局配置/内容表:无业务范围列、无软删列(del_flag),
// 但审计五件套实际存在 —— 有什么盖什么。
// ---------------------------------------------------------------------------

/// sys_dict_type:字典/配置类全局表:显式豁免(忘了写就是 CONSTRAINED=true,
/// fail-closed 会拒掉非 All 的一切查询,豁免必须显式、可 grep)。
impl DataPolicy for dict_type_repo::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

/// sys_dict_data:同 sys_dict_type —— 全局字典数据,显式豁免。
impl DataPolicy for dict_data_repo::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

/// sys_config:系统参数配置,全局生效。显式豁免。
impl DataPolicy for config_repo::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

/// sys_menu:菜单/权限点是全局功能定义,按角色分配而非按部门。显式豁免。
impl DataPolicy for menu_repo::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

/// sys_oss_config:存储桶配置,全局生效。显式豁免。
impl DataPolicy for oss_config_repo::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

/// sys_notice:通知公告 —— 实体只有审计列,无业务范围列(按列判定 → false)。
impl DataPolicy for notice_repo::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

/// sys_message:站内消息 —— send_user_ids 是 String 目标用户串("0" 表示全局),
/// 不是 i64 归属列,不能当 owner;仅有审计列 → 显式豁免。
impl DataPolicy for message_repo::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

/// sys_oss:文件记录 —— 实体只有审计列,无业务范围列(按列判定 → false)。
impl DataPolicy for oss_repo::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

// ---------------------------------------------------------------------------
// 代码生成元数据(开发者工具表):无范围列。裁定表 audit 标 "—",但实体
// 实际带审计五件套 —— 冲突以实体实际列为准(盖,不漏)。
// ---------------------------------------------------------------------------

/// gen_table:代码生成业务表注册信息。显式豁免。
impl DataPolicy for gen_table_repo::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

/// gen_table_column:代码生成字段元信息。显式豁免。
impl DataPolicy for gen_column_repo::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        Some(AuditColumns {
            create_dept: Some(|m, v| m.create_dept = ActiveValue::Set(Some(v))),
            create_by: Some(|m, v| m.create_by = ActiveValue::Set(Some(v))),
            create_time: Some(|m, v| m.create_time = ActiveValue::Set(Some(v))),
            update_by: Some(|m, v| m.update_by = ActiveValue::Set(Some(v))),
            update_time: Some(|m, v| m.update_time = ActiveValue::Set(Some(v))),
        })
    }
}

// ---------------------------------------------------------------------------
// flow_assignee_sync:参考实现自有 outbox 表(办理人重解析补偿任务,
// 无 Java 对应 —— 替代旧系统脆弱的 @Async 事件链)。系统级队列数据
// 非行级;无软删列(DDL 无 del_flag,硬删),
// 时间列为 `Option<NaiveDateTime>` 与漏斗盖章签名不匹配,由
// assignee_sync 模块手工盖(flow 引擎表 A 组同款裁定)。
// ---------------------------------------------------------------------------
impl DataPolicy for flow_assignee_sync::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        None
    }
}

/// sys_user_role:用户-角色关联路径表(设计 6.3:关联表全 None)。user_id 是关联键。
impl DataPolicy for user_role::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        None
    }
}

/// sys_user_post:用户-岗位关联路径表。user_id 是关联键,不是归属。
impl DataPolicy for user_post::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        None
    }
}

/// sys_role_dept:角色-部门(自定义数据权限)关联路径表。dept_id 是关联目标,
/// 不是行的归属部门。
impl DataPolicy for role_dept::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        None
    }
}

/// sys_role_menu:角色-菜单关联路径表。
impl DataPolicy for role_menu::Entity {
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
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use app_data::{
        DataPolicy, ScopeFilter,
        sea_orm::{DbBackend, EntityTrait, QueryFilter, QueryTrait},
    };

    use crate::{
        dept::repository::dept_repo,
        dict::repository::dict_type_repo as sys_dict_type,
        post::repository::post_repo, role::repository::role_repo,
        social::repository::social_repo,
        user::repository::user_repo as sys_user,
    }; // 经 app_data 受限 re-export(test-util 面)

    #[test]
    fn sys_user_policy_generates_dept_in() {
        let (d, o) = (
            <sys_user::Entity as DataPolicy>::dept_column(),
            <sys_user::Entity as DataPolicy>::owner_column(),
        );
        let c = app_data::scope_condition_for_test(
            d,
            o,
            &ScopeFilter::DeptIn(vec![100, 200]),
        )
        .unwrap()
        .unwrap();
        let sql = sys_user::Entity::find()
            .filter(c)
            .build(DbBackend::Sqlite)
            .to_string();
        assert!(sql.contains("IN (100, 200)"), "{sql}");
    }

    // 断言的就是常量默认值本身(锚定 dict=false 的豁免极性);clippy 建议改成
    // const 块会把运行期 RED/GREEN 语义变成编译期检查,故定点豁免。
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn dict_tables_opt_out_explicitly() {
        assert!(!<sys_dict_type::Entity as DataPolicy>::CONSTRAINED);
        assert!(
            <sys_dict_type::Entity as DataPolicy>::soft_delete().is_none()
                || <sys_dict_type::Entity as DataPolicy>::dept_column()
                    .is_none()
        );
    }

    // ---- 四条 Java 基线平价裁定(SQL 断言锚定语义)----

    /// 裁定 1:sys_role 受辖,部门范围走 create_dept(Java SysRoleMapper
    /// 的 @DataPermission 即锚在 create_dept,而非业务 dept_id —— 表没有)。
    #[test]
    fn parity_sys_role_scopes_via_create_dept() {
        let d = <role_repo::Entity as DataPolicy>::dept_column();
        let c = app_data::scope_condition_for_test(
            d,
            <role_repo::Entity as DataPolicy>::owner_column(),
            &ScopeFilter::DeptIn(vec![103]),
        )
        .unwrap()
        .unwrap();
        let sql = role_repo::Entity::find()
            .filter(c)
            .build(DbBackend::Sqlite)
            .to_string();
        assert!(
            sql.contains("create_dept") && sql.contains("IN (103)"),
            "{sql}"
        );
    }

    /// 裁定:sys_role 的"仅本人"锚 create_by —— SysRoleMapper 的
    /// 4 个 @DataPermission 方法(:33/:47/:61/:75)全部带
    /// `@DataColumn(key="userName", value="create_by")`,与 sys_user/sys_post
    /// 同锚(建单人即归属人,表无独立 user 列)。SelfOnly 生成
    /// `create_by = :uid`;owner 缺失会让
    /// SelfOnly 在 sys_role 上 fail-closed(Unsupported)。
    #[test]
    fn parity_sys_role_owner_is_create_by() {
        let o = <role_repo::Entity as DataPolicy>::owner_column();
        assert!(o.is_some(), "sys_role 必须声明 owner_column(create_by)");
        let c = app_data::scope_condition_for_test(
            <role_repo::Entity as DataPolicy>::dept_column(),
            o,
            &ScopeFilter::SelfOnly(7),
        )
        .unwrap()
        .unwrap();
        let sql = role_repo::Entity::find()
            .filter(c)
            .build(DbBackend::Sqlite)
            .to_string();
        // SELECT 列清单里恒有 create_by(实体列),故只核 WHERE 段
        let where_clause = sql.split(" WHERE ").nth(1).unwrap_or_default();
        assert!(
            where_clause.contains("create_by") && where_clause.contains("= 7"),
            "SelfOnly 的 WHERE 应落 create_by = 7:{sql}"
        );
    }

    /// 裁定 2:sys_dept 受辖,范围列是自身主键 dept_id(Java SysDeptMapper
    /// 语义:部门行可见 ⟺ 自身 id ∈ 可见部门集)。
    #[test]
    fn parity_sys_dept_scopes_via_own_pk() {
        let d = <dept_repo::Entity as DataPolicy>::dept_column();
        let c = app_data::scope_condition_for_test(
            d,
            None,
            &ScopeFilter::DeptIn(vec![100, 101]),
        )
        .unwrap()
        .unwrap();
        let sql = dept_repo::Entity::find()
            .filter(c)
            .build(DbBackend::Sqlite)
            .to_string();
        // Sqlite 方言用双引号包列名(反引号是 MySQL 方言)
        assert!(sql.contains("\"dept_id\" IN (100, 101)"), "{sql}");
    }

    /// 裁定 3:sys_social 豁免 —— Java SysSocialMapper 无 @DataPermission
    /// 注解,三方绑定在 Java 侧就不受数据权限约束。
    // 同 dict_tables_opt_out_explicitly:断言常量极性,clippy 的 const 块建议
    // 会把运行期 RED/GREEN 语义变成编译期检查,定点豁免。
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn parity_sys_social_exempt_matches_java() {
        assert!(!<social_repo::Entity as DataPolicy>::CONSTRAINED);
        assert!(<social_repo::Entity as DataPolicy>::owner_column().is_none());
    }

    /// 裁定 4:sys_post 的"仅本人"锚 create_by(SysPostMapper.java:31
    /// userName=create_by),SelfOnly 生成 create_by = :uid。
    #[test]
    fn parity_sys_post_owner_is_create_by() {
        let o = <post_repo::Entity as DataPolicy>::owner_column();
        let c = app_data::scope_condition_for_test(
            <post_repo::Entity as DataPolicy>::dept_column(),
            o,
            &ScopeFilter::SelfOnly(7),
        )
        .unwrap()
        .unwrap();
        let sql = post_repo::Entity::find()
            .filter(c)
            .build(DbBackend::Sqlite)
            .to_string();
        assert!(sql.contains("create_by") && sql.contains("= 7"), "{sql}");
    }

    /// 裁定 5:sys_user 的"仅本人"同样锚 create_by
    /// (SysUserMapper.java:37-43 @DataColumn userName=create_by ——
    /// "仅本人"是"我创建的用户",非"我本人";勿锚 user_id)。
    /// SelfOnly 生成 create_by = :uid,不得出现 user_id = :uid。
    #[test]
    fn parity_sys_user_owner_is_create_by() {
        let (d, o) = (
            <sys_user::Entity as DataPolicy>::dept_column(),
            <sys_user::Entity as DataPolicy>::owner_column(),
        );
        let c =
            app_data::scope_condition_for_test(d, o, &ScopeFilter::SelfOnly(7))
                .unwrap()
                .unwrap();
        let sql = sys_user::Entity::find()
            .filter(c)
            .build(DbBackend::Sqlite)
            .to_string();
        // SELECT 列清单里恒有 user_id(实体主键),故只核 WHERE 段
        let where_clause = sql.split(" WHERE ").nth(1).unwrap_or_default();
        assert!(
            where_clause.contains("create_by") && where_clause.contains("= 7"),
            "SelfOnly 的 WHERE 应落 create_by = 7:{sql}"
        );
        assert!(
            !where_clause.contains("user_id"),
            "WHERE 不得再锚 user_id(建单语义,非自见):{sql}"
        );
    }
}
