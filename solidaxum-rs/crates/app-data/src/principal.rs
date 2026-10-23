//! 身份与已解析的行级范围(设计 6.3 / 7.2)。
//! ScopeFilter 与业务无关:策略的计算在业务层(ScopeResolver 实现),
//! 施加在数据层(本 crate)。对 RuoYi 六档 DataScopeType 精确覆盖。
use crate::{
    error::DataError,
    sea_orm::{ColumnTrait, Condition, Expr, ExprTrait},
};

/// 请求身份。app-data 不知道 LoginUser/DataScope —— 那是上层概念。
#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: i64,
    pub dept_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScopeFilter {
    All,
    DeptIn(Vec<i64>),
    SelfOnly(i64),
    DeptInOrSelf { depts: Vec<i64>, user_id: i64 },
}

/// 范围解析器。由 system crate 实现(内部即现有 get_data_scope);
/// app-data 经 Arc<dyn ScopeResolver> 持有它 —— 这是依赖反转点(设计 7.2)。
#[async_trait::async_trait]
pub trait ScopeResolver: Send + Sync {
    async fn resolve(&self, p: &Principal) -> Result<ScopeFilter, DataError>;
}

/// 把已解析的范围合成附加 Condition。
/// 泛型 C 同时服务两种落点:驱动实体的列,与 constrain_via 声明的 join 实体列
/// (sea-orm 会对 join 上下文里的列自动加限定前缀)。
pub(crate) fn scope_condition<C: ColumnTrait + Copy>(
    dept: Option<C>,
    owner: Option<C>,
    sf: &ScopeFilter,
) -> Result<Option<Condition>, DataError> {
    let need = |what: &str| {
        Err(DataError::Unsupported(format!(
            "ScopeFilter 需要实体声明 {what} 列(fail-closed)"
        )))
    };
    Ok(match sf {
        ScopeFilter::All => None,
        ScopeFilter::SelfOnly(uid) => match (owner, dept) {
            (Some(o), _) => Some(Condition::all().add(o.eq(*uid))),
            // Java 平价:SELF 模板只含 `#userName`
            // (PlusDataPermissionHandler.java:151-172);实体只声明 dept 列
            // (生产形态:SysDeptMapper 仅 deptName)→ 模板缺 userName 键 →
            // elseSql `1 = 0`(DataScopeType.java:51)→ **空集**而非报错。
            // 此前此处返回 Unsupported(fail-closed 拒答 → 500),对 6 个受辖
            // dept 查询面的 SelfOnly 用户是行为回归;现按 Java 恒假空集平价。
            (None, Some(_)) => Some(Condition::all().add(Expr::value(0).eq(1))),
            // 双列皆缺:无 owner 列定位本人,也无 dept 列可锚定 —— 这不是
            // Java 会遇到的形态(注解至少声明一列),是本侧的配置错误,
            // fail-closed Unsupported 意图不变。
            (None, None) => return need("owner"),
        },
        ScopeFilter::DeptIn(depts) => match dept {
            // IN 是 ColumnTrait::is_in(entity/column.rs:387)。**不是 eq_any** ——
            // 那个带 #[cfg(feature = "postgres-array")] 且生成 `= ANY(ARRAY[…])`。
            Some(d) => Some(Condition::all().add(d.is_in(depts.clone()))),
            None => return need("dept"),
        },
        ScopeFilter::DeptInOrSelf { depts, user_id } => match (dept, owner) {
            (Some(d), Some(o)) => Some(
                Condition::any()
                    .add(d.is_in(depts.clone()))
                    .add(o.eq(*user_id)),
            ),
            // 对齐 Java:userName 未声明时退化为 dept-only
            // (PlusDataPermissionHandler.java:151-172 渲染缺 `#userName` 键的
            // 分支值为 `-1`,生成 `dept_name IN (...) OR -1 = user_id` 的
            // 恒假自支 —— 与下方 dept-only 退化精确平价:
            // 语义同为"可见集 = dept 集",保留现状不加恒假支。)
            (Some(d), None) => {
                Some(Condition::all().add(d.is_in(depts.clone())))
            }
            (None, _) => return need("dept"),
        },
    })
}

/// 业务 crate SQL 断言的公共面:门控在 test-util,
/// 生产面不出现(不是执行能力,只是合成函数的类型擦除包装)。
#[cfg(any(test, feature = "test-util"))]
pub fn scope_condition_for_test<C: sea_orm::ColumnTrait + Copy>(
    dept: Option<C>,
    owner: Option<C>,
    sf: &ScopeFilter,
) -> Result<Option<sea_orm::Condition>, DataError> {
    scope_condition(dept, owner, sf)
}

#[cfg(test)]
mod scope_condition_tests {
    use sea_orm::DbBackend;

    use super::*;
    use crate::{
        sea_orm::{EntityTrait, QueryFilter, QueryTrait},
        testing::sandbox,
    }; // crate 内部测试,不走受限模块

    fn build_sql(cond: Condition) -> String {
        sandbox::Entity::find()
            .filter(cond)
            .build(DbBackend::Sqlite)
            .to_string()
    }

    #[test]
    fn dept_in_generates_in_clause() {
        let c = scope_condition(
            Some(sandbox::Column::DeptId),
            Some(sandbox::Column::OwnerId),
            &ScopeFilter::DeptIn(vec![100, 200]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            build_sql(c),
            "SELECT \"sandbox\".\"id\", \"sandbox\".\"dept_id\", \"sandbox\".\"owner_id\", \
             \"sandbox\".\"del_flag\", \"sandbox\".\"create_by\", \"sandbox\".\"create_time\", \
             \"sandbox\".\"update_by\", \"sandbox\".\"update_time\" FROM \"sandbox\" \
             WHERE \"sandbox\".\"dept_id\" IN (100, 200)"
        );
    }

    #[test]
    fn self_only_generates_eq_and_dept_only_entity_is_empty_set() {
        let c = scope_condition(
            Some(sandbox::Column::DeptId),
            Some(sandbox::Column::OwnerId),
            &ScopeFilter::SelfOnly(7),
        )
        .unwrap()
        .unwrap();
        assert!(build_sql(c).contains("\"sandbox\".\"owner_id\" = 7"));

        // Java 平价:SELF 模板只含 #userName,实体仅声明 dept 列
        // (SysDeptMapper 生产形态)→ elseSql `1 = 0` → 空集,不再 Unsupported。
        let c = scope_condition(
            Some(sandbox::Column::DeptId),
            None,
            &ScopeFilter::SelfOnly(7),
        )
        .unwrap()
        .unwrap();
        assert!(build_sql(c).contains("0 = 1"));

        // 双列皆缺:仍是配置错误,保留 fail-closed Unsupported。
        let err = scope_condition::<sandbox::Column>(
            None,
            None,
            &ScopeFilter::SelfOnly(7),
        )
        .unwrap_err();
        assert!(matches!(err, DataError::Unsupported(_)));
    }

    #[test]
    fn self_only_dept_only_empty_set_filters_everything_out() {
        // 恒假条件挂在真实查询上:SELECT ... WHERE 0 = 1 —— 空集的 SQL 形态。
        let c = scope_condition(
            Some(sandbox::Column::DeptId),
            None,
            &ScopeFilter::SelfOnly(7),
        )
        .unwrap()
        .unwrap();
        let sql = build_sql(c);
        // WHERE 子句整体就是恒假式 `0 = 1`,不引用任何实体列
        let where_clause = sql.split(" WHERE ").nth(1).unwrap();
        assert_eq!(where_clause, "0 = 1");
    }

    #[test]
    fn dept_in_or_self_generates_or() {
        let c = scope_condition(
            Some(sandbox::Column::DeptId),
            Some(sandbox::Column::OwnerId),
            &ScopeFilter::DeptInOrSelf {
                depts: vec![1, 2],
                user_id: 9,
            },
        )
        .unwrap()
        .unwrap();
        let sql = build_sql(c);
        assert!(sql.contains("\"sandbox\".\"dept_id\" IN (1, 2)"));
        assert!(sql.contains("OR"));
        assert!(sql.contains("\"sandbox\".\"owner_id\" = 9"));
    }

    #[test]
    fn dept_in_or_self_without_owner_column_degrades_to_dept_only() {
        // 对齐 Java:userName 未声明时退化为 dept-only,不报错也不放行到全量
        let c = scope_condition(
            Some(sandbox::Column::DeptId),
            None,
            &ScopeFilter::DeptInOrSelf {
                depts: vec![1, 2],
                user_id: 9,
            },
        )
        .unwrap()
        .unwrap();
        let sql = build_sql(c);
        assert!(sql.contains("\"sandbox\".\"dept_id\" IN (1, 2)"));
        assert!(!sql.contains("OR"));
    }

    #[test]
    fn dept_in_without_dept_column_is_unsupported() {
        let err = scope_condition(
            None,
            Some(sandbox::Column::OwnerId),
            &ScopeFilter::DeptIn(vec![1]),
        )
        .unwrap_err();
        assert!(matches!(err, DataError::Unsupported(_))); // fail-closed
    }

    #[test]
    fn all_generates_none() {
        let c = scope_condition(
            Some(sandbox::Column::DeptId),
            Some(sandbox::Column::OwnerId),
            &ScopeFilter::All,
        )
        .unwrap();
        assert!(c.is_none());
    }
}
