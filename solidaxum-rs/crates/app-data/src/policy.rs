//! per-entity 策略声明(设计 6.3)。手写 impl,漏声明 = 编译错误。

use crate::sea_orm::{DateTimeUtc, EntityTrait};

/// 审计列指针:插入/更新时由漏斗统一盖章。Option<fn> 而非泛型列枚举,
/// 因为盖章要写 ActiveModel 的具体字段,只有实体自己知道映射。
pub struct AuditColumns<A> {
    /// RuoYi-Plus 的 create_dept(部分表有)
    pub create_dept: Option<fn(&mut A, i64)>,
    pub create_by: Option<fn(&mut A, i64)>,
    pub create_time: Option<fn(&mut A, DateTimeUtc)>,
    pub update_by: Option<fn(&mut A, i64)>,
    pub update_time: Option<fn(&mut A, DateTimeUtc)>,
}

impl<A> Default for AuditColumns<A> {
    fn default() -> Self {
        Self {
            create_dept: None,
            create_by: None,
            create_time: None,
            update_by: None,
            update_time: None,
        }
    }
}

/// 列枚举按值喂给 `crate::principal::scope_condition`(逐分支消费
/// `Option<Column>`),派生宏生成的 Column 一律 Copy —— 钉进 trait,
/// 免得 38 个实体的每个泛型调用点重复写 `E::Column: Copy`。
pub trait DataPolicy: EntityTrait
where
    Self::Column: Copy,
{
    /// 是否受行级数据权限约束。默认 true —— 忘了想落在安全的那一侧。
    /// 字典/参数配置/关联路径表显式写 false。
    const CONSTRAINED: bool = true;

    /// 行级范围的部门列(sys_user.dept_id 这类)
    fn dept_column() -> Option<Self::Column>;
    /// 行级范围的归属人列
    fn owner_column() -> Option<Self::Column>;
    /// 软删除列 + "未删除"的值(RuoYi del_flag,'0' 存活)
    fn soft_delete() -> Option<(Self::Column, &'static str)>;
    /// 审计列(全 None 表示无审计)
    fn audit_columns() -> Option<AuditColumns<Self::ActiveModel>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{dict_entry, sandbox, sys_user};

    #[test]
    // 本测试断言的就是常量默认值;clippy 建议改成 const 块会把运行期
    // RED/GREEN 语义变成编译期检查,故定点豁免 assertions_on_constants。
    #[allow(clippy::assertions_on_constants)]
    fn constrained_defaults_to_true_fail_closed() {
        // 不显式覆盖 CONSTRAINED → true。忘了想落在安全的一侧(设计 6.3)。
        assert!(<sandbox::Entity as DataPolicy>::CONSTRAINED);
        assert!(<sys_user::Entity as DataPolicy>::CONSTRAINED);
        // 字典表是显式豁免的那一档
        assert!(!<dict_entry::Entity as DataPolicy>::CONSTRAINED);
    }

    #[test]
    fn soft_delete_carries_alive_value() {
        let (col, alive) =
            <sandbox::Entity as DataPolicy>::soft_delete().unwrap();
        // 派生宏生成的 Column 枚举不带 PartialEq,只能模式匹配比较
        assert!(matches!(col, sandbox::Column::DelFlag));
        assert_eq!(alive, "0");
        // 真实实体走同一形状
        let (col, alive) =
            <sys_user::Entity as DataPolicy>::soft_delete().unwrap();
        assert!(matches!(col, sys_user::Column::DelFlag));
        assert_eq!(alive, "0");
    }

    #[test]
    fn unconstrained_entity_declares_no_policy_columns() {
        assert!(<dict_entry::Entity as DataPolicy>::dept_column().is_none());
        assert!(<dict_entry::Entity as DataPolicy>::owner_column().is_none());
        assert!(<dict_entry::Entity as DataPolicy>::soft_delete().is_none());
        assert!(<dict_entry::Entity as DataPolicy>::audit_columns().is_none());
    }

    #[test]
    fn create_dept_is_wired_only_where_the_column_exists() {
        // sandbox 无 create_dept 列 → None;sys_user 有 → Some。
        // 盖章逻辑靠这个 Option 决定盖不盖,不能想当然全表都有。
        assert!(
            <sandbox::Entity as DataPolicy>::audit_columns()
                .unwrap()
                .create_dept
                .is_none()
        );
        assert!(
            <sys_user::Entity as DataPolicy>::audit_columns()
                .unwrap()
                .create_dept
                .is_some()
        );
    }
}
