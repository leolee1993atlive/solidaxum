//! permission 服务:四方法全收 ctx,本文件零 `system_context` 调用点。
//!
//! - **用户面**(`get_role_permission` / `get_menu_permission` /
//!   `get_menu_tree`):调用方传请求期 ctx —— getRouters 由 handler 从
//!   `infra.data_source()` 构造(带 resolver);登录路径(auth 模块)无
//!   请求身份,传 system_context(设计 7.3 列举的无身份场景)。所用读
//!   全部豁免:`SysRoleMapper.selectRolesByUserId`(Mapper:89-96)与
//!   `SysMenuMapper`(:33/:101/:167)grep 计数零 `@DataPermission` ——
//!   权限树/角色键数据非行级,ctx 形态不影响结果(Java 审计锚与
//!   role/menu 模块头一致)。
//! - **解析面**(`get_data_scope`):生产唯一调用方是
//!   [`CachedScopeResolver`](super::scope_resolver),ctx 由其**自有
//!   unscoped 通道**(`ds.system_context()`,设计 7.2)
//!   构造 —— 内部读(role 列表 / role_dept 对 / dept 子树)与用户 ctx
//!   结构上无关。直接调用(测试)应传同样的 system 语义 ctx。

use std::collections::{HashMap, HashSet};

use app_core::{error::AppResult, infra::Infrastructure};
use app_data::DataContext;

use crate::{
    dept::repository::dept_repo::DeptRepository,
    menu::{
        model::menu::SUPER_ADMIN_USER_ID, repository::menu_repo::MenuRepository,
    },
    permission::data_scope::{DataScope, RoleScope, combine_scopes},
    role::repository::role_repo::RoleRepository,
};

const SUPER_ADMIN_ROLE_KEY: &str = "superadmin";
const ALL_PERMISSION: &str = "*:*:*";

#[derive(Clone)]
pub struct PermissionService {
    role_repo: RoleRepository,
    menu_repo: MenuRepository,
    dept_repo: DeptRepository,
}

impl PermissionService {
    /// 装配期不取任何快照(三个仓储全无状态,执行面全在调用方传入的
    /// ctx),形参仅保装配调用面形状。
    pub fn new(_infra: &Infrastructure) -> Self {
        Self::for_test()
    }

    /// 测试/装配共用构造器(仓储无状态、本服务不读连接;resolver 的
    /// unscoped 通道归 CachedScopeResolver 自有)。
    pub fn for_test() -> Self {
        Self {
            role_repo: RoleRepository::new(),
            menu_repo: MenuRepository::new(),
            dept_repo: DeptRepository::new(),
        }
    }

    /// 获取用户角色 key 集合。
    ///
    /// 收 ctx(登录路径由 auth 传 system_context —— 无请求
    /// 身份,设计 7.3)。读为豁免方法(Java 锚
    /// `SysRoleMapper.selectRolesByUserId`,Mapper:89-96,无
    /// @DataPermission)→ without_row_scope,ctx 形态不影响结果。
    pub async fn get_role_permission(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> AppResult<HashSet<String>> {
        if user_id == SUPER_ADMIN_USER_ID {
            let mut set = HashSet::new();
            set.insert(SUPER_ADMIN_ROLE_KEY.to_string());
            return Ok(set);
        }

        let roles =
            self.role_repo.select_roles_by_user_id(ctx, user_id).await?;

        let role_set: HashSet<String> = roles
            .iter()
            .flat_map(|r| {
                r.role_key
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            })
            .collect();

        Ok(role_set)
    }

    /// 获取用户权限标识集合。
    ///
    /// 收 ctx(同上)。读为豁免方法(Java 锚
    /// `SysMenuMapper.selectMenuPermsByUserId`,Mapper:33,SysMenuMapper
    /// 零 @DataPermission)→ without_row_scope。
    pub async fn get_menu_permission(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> AppResult<HashSet<String>> {
        if user_id == SUPER_ADMIN_USER_ID {
            let mut set = HashSet::new();
            set.insert(ALL_PERMISSION.to_string());
            return Ok(set);
        }

        let perms = self
            .menu_repo
            .select_menu_perms_by_user_id(ctx, user_id)
            .await?;

        let perm_set: HashSet<String> = perms
            .into_iter()
            .flat_map(|p| {
                p.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .collect();

        Ok(perm_set)
    }

    /// 获取用户的菜单树 (用于 getRouters)。
    ///
    /// 收 ctx(getRouters 的 handler 请求期构造)。读为
    /// 豁免方法(Java 锚 `selectMenuTreeAll` Mapper:101 /
    /// `selectMenuTreeByUserId` Mapper:167,零 @DataPermission)。
    pub async fn get_menu_tree(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> AppResult<Vec<crate::menu::model::menu::Menu>> {
        if user_id == SUPER_ADMIN_USER_ID {
            return Ok(self.menu_repo.select_menu_tree_all(ctx).await?);
        }

        Ok(self
            .menu_repo
            .select_menu_tree_by_user_id(ctx, user_id)
            .await?)
    }

    /// 获取用户数据权限范围(多角色 OR 并集)。
    ///
    /// 对齐 Java `SysRoleServiceImpl.getDataScope`:
    /// 任一角色为"全部"→ All;否则合并所有角色贡献的部门集 ∪ 本部门(及以下)∪ 本人。
    ///
    /// 收 ctx —— 生产唯一调用方是
    /// [`CachedScopeResolver`](super::scope_resolver),ctx 经其**自有
    /// unscoped 通道**(`ds.system_context()`,设计 7.2)
    /// 构造。内部三类读(role 列表 / role_dept 对 / dept 子树)均为
    /// 豁免方法(Java 锚:SysRoleMapper:89 join 无注解;sys_role_dept
    /// 关联表读;SysDeptMapper.selectListByParentId Mapper:76-81 无
    /// 注解)→ without_row_scope,与用户 ctx 完全无关,递归不可能。
    pub async fn get_data_scope(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
        dept_id: Option<i64>,
    ) -> AppResult<DataScope> {
        if user_id == SUPER_ADMIN_USER_ID {
            return Ok(DataScope::All);
        }

        let roles =
            self.role_repo.select_roles_by_user_id(ctx, user_id).await?;

        if roles.is_empty() {
            return Ok(DataScope::SelfOnly(user_id));
        }

        // 收集每个角色的 RoleScope;data_scope=6 归一为 DeptAndBelow + SelfOnly 两条贡献。
        // 同时记录自定义角色的 id 用于批量查 dept。
        let mut role_scopes: Vec<RoleScope> =
            Vec::with_capacity(roles.len() * 2);
        let mut custom_role_ids: Vec<i64> = Vec::new();
        for r in &roles {
            match r.data_scope.as_str() {
                "1" => role_scopes.push(RoleScope::All),
                "2" => {
                    role_scopes.push(RoleScope::Custom(Vec::new())); // dept_ids 后填
                    if let Some(rid) = r.role_id {
                        custom_role_ids.push(rid);
                    }
                }
                "3" => role_scopes.push(RoleScope::Dept),
                "4" => role_scopes.push(RoleScope::DeptAndBelow),
                "5" => role_scopes.push(RoleScope::SelfOnly),
                // 6 = "本部门及以下 + 仅本人"(历史 RuoYi 扩展,非 Java 标准);
                // 拆为两条贡献,合并后自然得到 CustomOrSelf。
                "6" => {
                    role_scopes.push(RoleScope::DeptAndBelow);
                    role_scopes.push(RoleScope::SelfOnly);
                }
                _ => role_scopes.push(RoleScope::SelfOnly),
            }
        }

        // 若存在 Custom 角色,回填 dept_ids(按 role 分组)
        if !custom_role_ids.is_empty() {
            let role_dept_pairs = self
                .role_repo
                .select_role_dept_pairs_by_role_ids(ctx, &custom_role_ids)
                .await?;

            let mut by_role: HashMap<i64, Vec<i64>> = HashMap::new();
            for (rid, did) in role_dept_pairs {
                by_role.entry(rid).or_default().push(did);
            }

            // 将 dept_ids 填回对应的 Custom 变体
            let mut by_role_iter = by_role;
            for r in &roles {
                if r.data_scope.as_str() == "2"
                    && let Some(role_id) = r.role_id
                {
                    let ids = by_role_iter.remove(&role_id).unwrap_or_default();
                    // 找到 role_scopes 中第一个空 Custom 槽并填入(顺序与 roles 一致)
                    if let Some(slot) = role_scopes.iter_mut().find(
                        |s| matches!(s, RoleScope::Custom(v) if v.is_empty()),
                    ) {
                        *slot = RoleScope::Custom(ids);
                    }
                }
            }
        }

        // 若任一为 DeptAndBelow,需查询本部门及以下子部门 id 集(含 self_dept)。
        // find_child_ids 为豁免方法(Java selectListByParentId 无注解),
        // resolver 经自有 unscoped 通道传入(设计 7.2)。
        let need_child_depts = role_scopes
            .iter()
            .any(|s| matches!(s, RoleScope::DeptAndBelow));
        let child_dept_ids = if need_child_depts {
            match dept_id {
                Some(root_dept) => {
                    self.dept_repo.find_child_ids(ctx, root_dept).await?
                }
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };

        Ok(combine_scopes(
            &role_scopes,
            user_id,
            dept_id,
            &child_dept_ids,
        ))
    }
}
