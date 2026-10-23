//! flow 实体的 DataPolicy 声明(设计 6.3,monitor/policy.rs 同款集中式)。
//!
//! 判定总锚:warm-flow / ruoyi-workflow 的全部 Mapper
//! **零 @DataPermission** —— 工作流数据非行级数据 → 12 实体全豁免
//! (CONSTRAINED=false),每个读方法显式 `without_row_scope()`。
//!
//! 软删除:除 test_leave(DDL 无 del_flag,硬删)与 flow_variable
//! (参考实现 DDL 无 del_flag,硬删)外 10 实体均有 del_flag(Java
//! `@TableLogic` 语义)
//! → 声明 `soft_delete() = Some((DelFlag, "0"))`,存活过滤由漏斗接管。
//! test_leave 无软删列(硬删),是 ruoyi-workflow 的业务测试表。
//!
//! 审计列**双组类型裁定**(sys_job 同款逻辑,见 job/policy.rs):
//! - A 组(warm-flow 引擎表:flow_definition / flow_instance / flow_task /
//!   flow_his_task / flow_node / flow_skip / flow_user):create_by/update_by
//!   是 `Option<String>`、时间列是 `Option<DateTime>`(= NaiveDateTime),
//!   与漏斗盖章签名(`i64` / `DateTimeUtc`)**不匹配** → 手工盖章保留在
//!   service 层;**不为漏斗改列类型**(改类型 = schema/序列化涟漪,超范围)。
//!   flow_his_task 连 create_by/update_by 都没有。
//! - B 组(ruoyi 自有表:flow_category / flow_spel / flow_instance_biz_ext):
//!   create_by/update_by 是 `Option<i64>`(类型可盖章)—— 但时间列同为
//!   `Option<DateTime>`(NaiveDateTime)不匹配,只盖人不盖时间会造成
//!   "半盖章"分裂 → 裁定同样 None。

use app_data::DataPolicy;

use crate::repository::entity::{
    flow_category, flow_definition, flow_his_task, flow_instance,
    flow_instance_biz_ext, flow_node, flow_skip, flow_spel, flow_task,
    flow_user, flow_variable, test_leave,
};

impl DataPolicy for flow_definition::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((flow_definition::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<app_data::AuditColumns<Self::ActiveModel>> {
        None
    }
}

impl DataPolicy for flow_instance::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((flow_instance::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<app_data::AuditColumns<Self::ActiveModel>> {
        None
    }
}

impl DataPolicy for flow_task::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((flow_task::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<app_data::AuditColumns<Self::ActiveModel>> {
        None
    }
}

impl DataPolicy for flow_his_task::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((flow_his_task::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<app_data::AuditColumns<Self::ActiveModel>> {
        None
    }
}

impl DataPolicy for flow_node::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((flow_node::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<app_data::AuditColumns<Self::ActiveModel>> {
        None
    }
}

impl DataPolicy for flow_skip::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((flow_skip::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<app_data::AuditColumns<Self::ActiveModel>> {
        None
    }
}

impl DataPolicy for flow_user::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((flow_user::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<app_data::AuditColumns<Self::ActiveModel>> {
        None
    }
}

impl DataPolicy for flow_category::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((flow_category::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<app_data::AuditColumns<Self::ActiveModel>> {
        None
    }
}

impl DataPolicy for flow_spel::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((flow_spel::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<app_data::AuditColumns<Self::ActiveModel>> {
        None
    }
}

impl DataPolicy for flow_instance_biz_ext::Entity {
    const CONSTRAINED: bool = false;
    fn dept_column() -> Option<Self::Column> {
        None
    }
    fn owner_column() -> Option<Self::Column> {
        None
    }
    fn soft_delete() -> Option<(Self::Column, &'static str)> {
        Some((flow_instance_biz_ext::Column::DelFlag, "0"))
    }
    fn audit_columns() -> Option<app_data::AuditColumns<Self::ActiveModel>> {
        None
    }
}

/// flow_variable:ruoyi 自有注册表,**无 del_flag**(DDL 锚参考实现
/// update_flow_registry.sql)→ 硬删,无软删策略;审计列双组裁定的
/// B 组(create_by/update_by 为 Option<i64>)同 flow_spel 盖 None。
impl DataPolicy for flow_variable::Entity {
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

/// test_leave:ruoyi-workflow 的业务测试表,无软删列(硬删),无任何策略列。
impl DataPolicy for test_leave::Entity {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 矩阵钉死:11 实体全豁免(warm-flow/ruoyi-workflow Mapper 零
    /// @DataPermission,工作流数据非行级)。
    // 本测试断言的就是常量覆盖值;clippy 建议改成 const 块会把运行期
    // RED/GREEN 语义变成编译期检查,故定点豁免(app-data src/policy.rs 同款)。
    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn all_eleven_entities_are_unconstrained() {
        assert!(!<flow_definition::Entity as DataPolicy>::CONSTRAINED);
        assert!(!<flow_instance::Entity as DataPolicy>::CONSTRAINED);
        assert!(!<flow_task::Entity as DataPolicy>::CONSTRAINED);
        assert!(!<flow_his_task::Entity as DataPolicy>::CONSTRAINED);
        assert!(!<flow_node::Entity as DataPolicy>::CONSTRAINED);
        assert!(!<flow_skip::Entity as DataPolicy>::CONSTRAINED);
        assert!(!<flow_user::Entity as DataPolicy>::CONSTRAINED);
        assert!(!<flow_category::Entity as DataPolicy>::CONSTRAINED);
        assert!(!<flow_spel::Entity as DataPolicy>::CONSTRAINED);
        assert!(!<flow_instance_biz_ext::Entity as DataPolicy>::CONSTRAINED);
        assert!(!<test_leave::Entity as DataPolicy>::CONSTRAINED);
    }

    /// 软删除:10 实体 Some((DelFlag, "0")),test_leave 无软删列(硬删)。
    /// 派生宏生成的 Column 枚举不带 PartialEq,只能模式匹配比较
    /// (app-data src/policy.rs 判例同款)。
    #[test]
    fn soft_delete_declared_on_ten_entities_not_test_leave() {
        macro_rules! assert_alive {
            ($entity:ty, $variant:path) => {
                let (col, alive) = <$entity as DataPolicy>::soft_delete()
                    .expect(concat!(stringify!($entity), " 必须声明软删除"));
                assert!(matches!(col, $variant));
                assert_eq!(alive, "0");
            };
        }
        assert_alive!(
            flow_definition::Entity,
            flow_definition::Column::DelFlag
        );
        assert_alive!(flow_instance::Entity, flow_instance::Column::DelFlag);
        assert_alive!(flow_task::Entity, flow_task::Column::DelFlag);
        assert_alive!(flow_his_task::Entity, flow_his_task::Column::DelFlag);
        assert_alive!(flow_node::Entity, flow_node::Column::DelFlag);
        assert_alive!(flow_skip::Entity, flow_skip::Column::DelFlag);
        assert_alive!(flow_user::Entity, flow_user::Column::DelFlag);
        assert_alive!(flow_category::Entity, flow_category::Column::DelFlag);
        assert_alive!(flow_spel::Entity, flow_spel::Column::DelFlag);
        assert_alive!(
            flow_instance_biz_ext::Entity,
            flow_instance_biz_ext::Column::DelFlag
        );
        assert!(
            <test_leave::Entity as DataPolicy>::soft_delete().is_none(),
            "test_leave 无软删列(硬删)"
        );
    }

    /// 审计列双组裁定全 None:A 组 String/NaiveDateTime 类型不匹配(手工
    /// 盖章保留);B 组可盖章但防"半盖章"分裂(见文件头注释)。
    #[test]
    fn audit_columns_are_none_for_all_eleven() {
        assert!(
            <flow_definition::Entity as DataPolicy>::audit_columns().is_none()
        );
        assert!(
            <flow_instance::Entity as DataPolicy>::audit_columns().is_none()
        );
        assert!(<flow_task::Entity as DataPolicy>::audit_columns().is_none());
        assert!(
            <flow_his_task::Entity as DataPolicy>::audit_columns().is_none()
        );
        assert!(<flow_node::Entity as DataPolicy>::audit_columns().is_none());
        assert!(<flow_skip::Entity as DataPolicy>::audit_columns().is_none());
        assert!(<flow_user::Entity as DataPolicy>::audit_columns().is_none());
        assert!(
            <flow_category::Entity as DataPolicy>::audit_columns().is_none()
        );
        assert!(<flow_spel::Entity as DataPolicy>::audit_columns().is_none());
        assert!(
            <flow_instance_biz_ext::Entity as DataPolicy>::audit_columns()
                .is_none()
        );
        assert!(<test_leave::Entity as DataPolicy>::audit_columns().is_none());
    }
}
