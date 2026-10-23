use std::collections::HashSet;

#[derive(Clone, Debug)]
pub enum DataScope {
    All,
    Custom(Vec<i64>),
    SelfOnly(i64),
    CustomOrSelf { dept_ids: Vec<i64>, user_id: i64 },
}

impl DataScope {
    pub fn is_all(&self) -> bool {
        matches!(self, DataScope::All)
    }
}

/// 单个角色的数据范围原始描述,对应 `sys_role.data_scope` 取值。
///
/// 取值对照(Java/RuoYi 标准):
/// - `All`           ↔ data_scope = 1(全部)
/// - `Custom(_)`     ↔ data_scope = 2(自定义);内部承载已预查询的角色部门集
/// - `Dept`          ↔ data_scope = 3(本部门)
/// - `DeptAndBelow`  ↔ data_scope = 4(本部门及以下)
/// - `SelfOnly`      ↔ data_scope = 5(仅本人)
///
/// 注:Java 历史上 data_scope = 6 用于"本部门及以下或本人",Rust 端用
/// `DataScope::CustomOrSelf` 表达其合并结果,但其**不作为 sys_role.data_scope
/// 的合法配置值**(仅为内部组合类型);sys_role 配 6 时服务层归一为 4+5 合并。
#[derive(Clone, Debug, PartialEq)]
pub enum RoleScope {
    All,
    Custom(Vec<i64>),
    Dept,
    DeptAndBelow,
    SelfOnly,
}

/// 合并多角色数据范围为单一 `DataScope`(对齐 Java 多角色 OR 并集语义)。
///
/// 规则:
/// 1. 任一角色为 `All` → 返回 `DataScope::All`(全部优先)。
/// 2. 否则收集所有角色贡献的部门 id 并去重:
///    - `Custom(ids)` → 加 ids
///    - `Dept`        → 加 self_dept_id(若存在)
///    - `DeptAndBelow`→ 加 child_dept_ids + self_dept_id
///    - `SelfOnly`    → 仅标记 include_self_user,不加部门
/// 3. 部门集空 + 仅本人贡献 → `SelfOnly(self_user_id)`
///    部门集非空 + 仅本人贡献 → `CustomOrSelf { dept_ids, user_id }`
///    部门集非空 + 无仅本人 → `Custom(dept_ids)`
///
/// `child_dept_ids`:本部门及以下时,要并入的子部门 id 列表(调用方需自行包含 self_dept_id)。
pub fn combine_scopes(
    scopes: &[RoleScope],
    self_user_id: i64,
    self_dept_id: Option<i64>,
    child_dept_ids: &[i64],
) -> DataScope {
    if scopes.is_empty() {
        return DataScope::SelfOnly(self_user_id);
    }

    // 任一 All → All
    if scopes.iter().any(|s| matches!(s, RoleScope::All)) {
        return DataScope::All;
    }

    let mut dept_set: HashSet<i64> = HashSet::new();
    let mut include_self_user = false;

    for scope in scopes {
        match scope {
            RoleScope::All => unreachable!("All 已在前面提前返回"),
            RoleScope::Custom(ids) => {
                dept_set.extend(ids.iter().copied());
            }
            RoleScope::Dept => {
                if let Some(d) = self_dept_id {
                    dept_set.insert(d);
                }
            }
            RoleScope::DeptAndBelow => {
                dept_set.extend(child_dept_ids.iter().copied());
                if let Some(d) = self_dept_id {
                    dept_set.insert(d);
                }
            }
            RoleScope::SelfOnly => {
                include_self_user = true;
            }
        }
    }

    let mut dept_ids: Vec<i64> = dept_set.into_iter().collect();
    dept_ids.sort_unstable();

    if dept_ids.is_empty() {
        // 没有任何部门贡献:无论是否含仅本人,结果都是 SelfOnly
        DataScope::SelfOnly(self_user_id)
    } else if include_self_user {
        DataScope::CustomOrSelf {
            dept_ids,
            user_id: self_user_id,
        }
    } else {
        DataScope::Custom(dept_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_all_true() {
        assert!(DataScope::All.is_all());
    }

    #[test]
    fn test_is_all_custom() {
        assert!(!DataScope::Custom(vec![1, 2]).is_all());
    }

    #[test]
    fn test_is_all_custom_empty() {
        assert!(!DataScope::Custom(vec![]).is_all());
    }

    #[test]
    fn test_is_all_self_only() {
        assert!(!DataScope::SelfOnly(42).is_all());
    }

    #[test]
    fn test_is_all_custom_or_self() {
        assert!(
            !DataScope::CustomOrSelf {
                dept_ids: vec![1, 2],
                user_id: 42,
            }
            .is_all()
        );
    }

    // ============================================================
    // combine_scopes — 多角色 OR 并集
    // ============================================================

    #[test]
    fn combine_empty_returns_self_only() {
        let combined = combine_scopes(&[], 5, Some(100), &[]);
        assert!(matches!(combined, DataScope::SelfOnly(5)));
    }

    #[test]
    fn all_scope_dominates() {
        // 角色 A=全部, 角色 B=自定义[101] → 合并应为 All
        let combined = combine_scopes(
            &[RoleScope::All, RoleScope::Custom(vec![101])],
            5,
            Some(100),
            &[],
        );
        assert!(matches!(combined, DataScope::All));
    }

    #[test]
    fn custom_scopes_union() {
        // 角色 A=自定义[101], 角色 B=自定义[103] → 部门集 {101,103} 的并集
        let combined = combine_scopes(
            &[RoleScope::Custom(vec![101]), RoleScope::Custom(vec![103])],
            5,
            Some(100),
            &[],
        );
        match combined {
            DataScope::Custom(d) => {
                let set: HashSet<_> = d.into_iter().collect();
                assert!(set.contains(&101) && set.contains(&103));
            }
            other => panic!("应为 Custom 并集, got {other:?}"),
        }
    }

    #[test]
    fn custom_union_dept_returns_custom() {
        // 角色 A=自定义[101], 角色 B=本部门(dept=100) → Custom({100,101})
        let combined = combine_scopes(
            &[RoleScope::Custom(vec![101]), RoleScope::Dept],
            5,
            Some(100),
            &[],
        );
        match combined {
            DataScope::Custom(d) => {
                let set: HashSet<_> = d.into_iter().collect();
                assert_eq!(set, [100, 101].into_iter().collect());
            }
            other => panic!("应为 Custom, got {other:?}"),
        }
    }

    #[test]
    fn custom_union_dept_and_below_includes_children() {
        // 角色 A=自定义[101], 角色 B=本部门及以下 → 含 child_dept_ids
        let combined = combine_scopes(
            &[RoleScope::Custom(vec![101]), RoleScope::DeptAndBelow],
            5,
            Some(100),
            &[100, 103, 104],
        );
        match combined {
            DataScope::Custom(d) => {
                let set: HashSet<_> = d.into_iter().collect();
                assert!(
                    set.contains(&101)
                        && set.contains(&100)
                        && set.contains(&103)
                );
            }
            other => panic!("应为 Custom, got {other:?}"),
        }
    }

    #[test]
    fn custom_union_self_returns_custom_or_self() {
        // 角色 A=自定义[101], 角色 B=仅本人 → CustomOrSelf({101}, user=5)
        let combined = combine_scopes(
            &[RoleScope::Custom(vec![101]), RoleScope::SelfOnly],
            5,
            Some(100),
            &[],
        );
        match combined {
            DataScope::CustomOrSelf { dept_ids, user_id } => {
                assert_eq!(dept_ids, vec![101]);
                assert_eq!(user_id, 5);
            }
            other => panic!("应为 CustomOrSelf, got {other:?}"),
        }
    }

    #[test]
    fn only_self_returns_self_only() {
        // 仅"仅本人" → SelfOnly
        let combined =
            combine_scopes(&[RoleScope::SelfOnly], 5, Some(100), &[]);
        assert!(matches!(combined, DataScope::SelfOnly(5)));
    }

    #[test]
    fn only_dept_without_self_dept_falls_back_to_self_only() {
        // 只有本部门角色但 self_dept=None → 无任何部门贡献 → SelfOnly
        let combined = combine_scopes(&[RoleScope::Dept], 5, None, &[]);
        assert!(matches!(combined, DataScope::SelfOnly(5)));
    }
}
