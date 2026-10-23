//! role 仓储。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级豁免裁定(Java 基线逐方法核对,现网
//! SysRoleMapper.java 的 @DataPermission 在 4 个方法上都带双列:
//! deptName=create_dept + userName=create_by,行号 :33/:47/:61/:75)**:
//!
//! | 本文件方法 | Java 锚(SysRoleController/ServiceImpl → SysRoleMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_page` | Controller:56 `list` → ServiceImpl:64 `selectPageRoleList` → Mapper:33 | ✅ 双列 | `ctx.select(..).page()` 自动施加 |
//! | `select_list`(导出) | Controller:69 `export` → ServiceImpl:76 `selectRoleList` → Mapper:47 | ✅ 双列 | `ctx.select` 自动(**与 post 相反**:role 的导出在 Java 带注解) |
//! | `select_all` | ServiceImpl:153 `selectRoleAll` → `selectRoleList` → Mapper:47;optionselect(:194)→ `selectRoleByIds`(:187)→ 同 | ✅ 双列 | `ctx.select` 自动 |
//! | `select_by_id` | Controller:82 `getInfo` → ServiceImpl:176 `selectRoleById` → Mapper:75 | ✅ 双列 | `ctx.select` 自动(详情读也受辖) |
//! | `select_by_role_key` | ServiceImpl:216 `checkRoleKeyUnique` → `lambda().exists()` | ❌ | `without_row_scope()`(唯一性须跨部门全量) |
//! | `select_by_role_name` | ServiceImpl:201 `checkRoleNameUnique` → `lambda().exists()` | ❌ | `without_row_scope()`(同上) |
//! | `count_role_by_id` | `selectRoleCount`(Mapper:61,带注解)→ `checkRoleDataScope`(ServiceImpl:260-281)越权探针 | ✅ 双列 | `ctx.select(..).count()` 自动施加(见表下段) |
//! | `select_roles_by_user_id` | Mapper:89 `selectRolesByUserId`(join,无注解) | ❌ | `without_row_scope()`("我的角色"语义) |
//! | `select_dept_ids_by_role_ids` | roleDeptMapper 关联读(SysRoleDeptMapper 无注解) | ❌ | `without_row_scope()`(关联表) |
//! | `select_role_dept_pairs_by_role_ids` | 同上(范围解析的自定义部门集回填) | ❌ | `without_row_scope()`(关联表) |
//! | `insert` | ServiceImpl:302 `insertRole` → mapper.insert(写路径) | — | `ctx.insert`(审计五列漏斗盖) |
//! | `update_by_id` | ServiceImpl:318 `updateRoleBaseInfo` → updateById(写路径) | — | `update_without_row_scope`(越权由 controller 探针拦) |
//! | `update_status` | ServiceImpl:357 `updateRoleStatus` → `lambda().updateCount()` | ❌(写) | `update_without_row_scope` 单行(同 user update_status) |
//! | `select_allocated_user_page` | Controller:207 → SysUserServiceImpl `selectAllocatedList` → SysUserMapper.java:93 | ✅(user 实体:d.dept_id/u.create_by) | `ctx.select(..).page()` 按查询根 sys_user 的 DataPolicy 施加 |
//! | `select_unallocated_user_page` | Controller:220 → `selectUnallocatedList` → SysUserMapper.java:114 | ✅(同上) | 同上 |
//! | `insert_role_menus` | ServiceImpl:373 `insertRoleMenu` → roleMenuMapper(无注解) | ❌ | `ctx.insert_many`(关联表全 None) |
//! | `delete_role_menus_by_role_id` | ServiceImpl:342 roleMenuMapper.delete | ❌ | `ctx.delete_where`(豁免实体;事务内 delete-then-insert) |
//! | `insert_role_depts` | ServiceImpl:394 `insertRoleDept` → roleDeptMapper | ❌ | `ctx.insert_many`(关联表全 None) |
//! | `delete_role_depts_by_role_id` | ServiceImpl:345 roleDeptMapper.delete | ❌ | `ctx.delete_where`(同 role_menu) |
//! | `insert_user_roles` | ServiceImpl:503 `insertAuthUsers` → userRoleMapper(无注解) | ❌ | `ctx.insert_many`(关联表全 None) |
//! | `delete_user_roles` | ServiceImpl:459/:481 `deleteAuthUser(s)` → userRoleMapper.delete | ❌ | `ctx.delete_where`(user_role 豁免实体) |
//!
//! **范围语义说明(Java 对照)**:Java 的 `selectRoleCount`(:61,带注解)是
//! `checkRoleDataScope`(ServiceImpl:260-281)的越权探针,现网调用面在
//! SysRoleController 的 getInfo/edit/editPermission/changeStatus 四处。
//! Rust 侧对位为 `count_role_by_id` + service 的 `check_role_data_scope`:
//! 读路径另有受辖 `select_by_id` 兜底,写路径的
//! 越权拦截全部由 handler 探针承担。
//!
//! **写路径**:Java 写方法无注解(拦截器只改写
//! 查询),update 统一走 `update_without_row_scope`,WHERE 只剩主键 +
//! 软删存活,0 行 → NotFound;越权拦截在 controller 的
//! `checkRoleDataScope` 探针(与 user 模块同裁定)。
//!
//! **`select_all` 受辖的影响面**:本方法服务 Rust 侧 `list()`(GET
//! /system/role 不分页变体)与 optionselect —— Java 两者都汇入带注解的
//! `selectRoleList`,受辖即对齐;permission/profile 服务走的
//! `select_roles_by_user_id` 是豁免方法,不受影响。

use app_core::{
    model::{
        entity::BaseEntity,
        status::{DelFlag, Status},
    },
    repo::str_trim_or_empty,
    vo::page::{TableDataInfo, normalize_paging},
};
use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{
        self, ActiveModelBehavior,
        ActiveValue::{NotSet, Set},
        ColumnTrait, Condition, DateTimeUtc, DeriveEntityModel,
        DerivePrimaryKey, DeriveRelation, EntityTrait, EnumIter, JoinType,
        Order, PrimaryKeyTrait, QueryFilter, QueryOrder, QuerySelect, Select,
        sea_query::Query,
    },
};

use crate::{
    relation::{role_dept, role_menu, user_role},
    role::{
        handler::role_handler::RolePageQuery,
        model::role::{Role, RoleListVo},
    },
    user::{
        handler::user_handler::UserPageQuery,
        model::user::User,
        repository::user_repo::{Column as UserColumn, Entity as UserEntity},
    },
};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_role")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub role_id: i64,
    pub role_name: String,
    pub role_key: String,
    /// 上级角色ID(参考实现定制: 上级办理人解析)。
    pub parent_id: Option<i64>,
    pub role_sort: i32,
    pub data_scope: String,
    pub menu_check_strictly: bool,
    pub dept_check_strictly: bool,
    pub status: String,
    pub del_flag: String,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTimeUtc>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTimeUtc>,
    pub remark: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<&Model> for Role {
    fn from(m: &Model) -> Self {
        Role {
            role_id: Some(m.role_id),
            role_name: m.role_name.clone(),
            role_key: m.role_key.clone(),
            parent_id: m.parent_id,
            role_sort: m.role_sort,
            data_scope: m.data_scope.clone(),
            menu_check_strictly: m.menu_check_strictly,
            dept_check_strictly: m.dept_check_strictly,
            status: m.status.clone(),
            del_flag: Some(m.del_flag.clone()),
            remark: m.remark.clone(),
            base_entity: BaseEntity {
                create_dept: m.create_dept,
                create_by: m.create_by,
                create_time: m.create_time,
                update_by: m.update_by,
                update_time: m.update_time,
            },
        }
    }
}

/// Role → ActiveModel。审计五列(create_dept/create_by/create_time/
/// update_by/update_time)只搬运领域层已持有的值;**写入时由漏斗统一盖章**
/// (insert 全盖、update 盖 update_*),领域层不再手工盖章。
fn role_to_active_model(role: &Role) -> ActiveModel {
    ActiveModel {
        role_id: match role.role_id {
            Some(id) => Set(id),
            None => NotSet,
        },
        role_name: Set(role.role_name.clone()),
        role_key: Set(role.role_key.clone()),
        parent_id: Set(role.parent_id),
        role_sort: Set(role.role_sort),
        data_scope: Set(role.data_scope.clone()),
        menu_check_strictly: Set(role.menu_check_strictly),
        dept_check_strictly: Set(role.dept_check_strictly),
        status: Set(role.status.clone()),
        del_flag: Set(role
            .del_flag
            .clone()
            .unwrap_or_else(|| DelFlag::Alive.as_str().to_string())),
        create_dept: Set(role.base_entity.create_dept),
        create_by: Set(role.base_entity.create_by),
        create_time: Set(role.base_entity.create_time),
        update_by: Set(role.base_entity.update_by),
        update_time: Set(role.base_entity.update_time),
        remark: Set(role.remark.clone()),
    }
}

#[derive(Clone, Default)]
pub struct RoleRepository {
    // 无状态仓储:执行面全在调用方传入的 ctx(用户请求
    // 由 handler 请求期从 `infra.data_source()` 构造,那里才带 resolver,
    // 见 app.rs 装配顺序注释;测试经自有 `ds.system_context()` 构造 ctx)。
}

impl RoleRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// 全部角色(下拉/不分页列表用)。`status` 非空时按状态过滤
    /// (optionselect 传 `Some("0")` 对齐 Java `selectRoleByIds`
    /// ServiceImpl:189 `.eq(SysRole::getStatus, NORMAL)`;不分页列表
    /// 传 None,对齐 `selectRoleAll` 的空 bo 无 status 条件)。
    ///
    /// 范围施加点:Java `selectRoleAll`(ServiceImpl:153)→ 带注解的
    /// `selectRoleList`(Mapper:47)→ `ctx.select` 自动施加。
    pub async fn select_all(
        &self,
        ctx: &DataContext<'_>,
        status: &Option<String>,
    ) -> Result<Vec<Role>, DataError> {
        let mut select = Entity::find();
        if let Some(s) = status.as_deref().filter(|s| !s.is_empty()) {
            select = select.filter(Column::Status.eq(s));
        }
        let models = ctx
            .select(
                select
                    .order_by(Column::RoleSort, Order::Asc)
                    .order_by(Column::RoleId, Order::Asc),
            )
            .all()
            .await?;
        Ok(models.iter().map(Role::from).collect())
    }

    /// 按主键查详情。
    ///
    /// 范围施加点:Java `selectRoleById`(ServiceImpl:176)→ Mapper:75 带
    /// @DataPermission → `ctx.select` 自动施加(越权行与不存在行同为 None);
    /// 软删过滤由漏斗照施。
    pub async fn select_by_id(
        &self,
        ctx: &DataContext<'_>,
        role_id: &i64,
    ) -> Result<Option<Role>, DataError> {
        let model = ctx.select(Entity::find_by_id(*role_id)).one().await?;
        Ok(model.as_ref().map(Role::from))
    }

    /// 按角色标识查(唯一性校验用)。
    ///
    /// 豁免裁定:Java `checkRoleKeyUnique`(ServiceImpl:216)→ `lambda()
    /// .exists()`,无 @DataPermission(唯一性校验必须跨部门全量)→
    /// `without_row_scope()`;软删过滤照施加。
    pub async fn select_by_role_key(
        &self,
        ctx: &DataContext<'_>,
        role_key: &str,
    ) -> Result<Option<Role>, DataError> {
        let model = ctx
            .select(Entity::find().filter(Column::RoleKey.eq(role_key)))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Role::from))
    }

    /// 按角色名查询(排除指定 id 用于唯一性校验)。
    ///
    /// 豁免裁定:Java `checkRoleNameUnique`(ServiceImpl:201)→ `lambda()
    /// .exists()`,无 @DataPermission → `without_row_scope()`(理由同上)。
    pub async fn select_by_role_name(
        &self,
        ctx: &DataContext<'_>,
        role_name: &str,
        exclude_id: Option<i64>,
    ) -> Result<Option<Role>, DataError> {
        let mut q = Entity::find().filter(Column::RoleName.eq(role_name));
        if let Some(rid) = exclude_id {
            q = q.filter(Column::RoleId.ne(rid));
        }
        let model = ctx.select(q).without_row_scope().one().await?;
        Ok(model.as_ref().map(Role::from))
    }

    /// 新增角色。审计五列由漏斗 insert 统一盖章(principal 取自 ctx),
    /// 领域层传入的审计值会被覆盖。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        role: &Role,
    ) -> Result<(), DataError> {
        let active = role_to_active_model(role);
        ctx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// 新增角色 + 菜单关联,单事务(对齐 Java
    /// SysRoleServiceImpl.insertRole:302-308 → insertRoleMenu;此前
    /// create 路径把 menuIds 整个丢弃,新建角色零权限)。
    pub async fn insert_with_menus(
        &self,
        ctx: &DataContext<'_>,
        role: &Role,
        menu_ids: &[i64],
    ) -> Result<(), DataError> {
        let active = role_to_active_model(role);
        let menu_models: Vec<role_menu::ActiveModel> = menu_ids
            .iter()
            .map(|&mid| role_menu::ActiveModel {
                role_id: Set(role.role_id.unwrap_or_default()),
                menu_id: Set(mid),
            })
            .collect();
        ctx.transaction(|tx| {
            Box::pin(async move {
                tx.insert_returning_nothing(active).await?;
                if !menu_models.is_empty() {
                    tx.insert_many(menu_models).await?;
                }
                Ok(())
            })
        })
        .await
    }

    /// 按主键更新。
    ///
    /// **写路径不受行级范围**:Java 写方法无
    /// @DataPermission,越权拦截在 controller 的 checkRoleDataScope。
    /// WHERE = 主键 + 软删存活,0 行 → NotFound;审计盖 update_*。
    pub async fn update_by_id(
        &self,
        ctx: &DataContext<'_>,
        role: &Role,
        role_id: &i64,
    ) -> Result<(), DataError> {
        let mut active = role_to_active_model(role);
        active.role_id = Set(*role_id);
        ctx.update_without_row_scope(active).await?;
        Ok(())
    }

    /// 角色列表的过滤条件。**`select_page`(列表)与 `select_list`(导出)共用**,
    /// 保证导出结果与页面列表始终一致 —— 各写一份是导出/列表口径漂移的老来源。
    /// 软删/行级范围不在其中 —— 那两样由漏斗施加,SQL 里 AND 叠加,正交。
    fn apply_filters(
        select: Select<Entity>,
        query: &RolePageQuery,
    ) -> Select<Entity> {
        let keyword = str_trim_or_empty(&query.keyword);
        let role_name = str_trim_or_empty(&query.role_name);
        let role_key = str_trim_or_empty(&query.role_key);
        let status = str_trim_or_empty(&query.status);

        let mut select = select;
        if !role_name.is_empty() {
            select = select.filter(Column::RoleName.contains(&role_name));
        }
        // roleKey 模糊匹配(对齐 SysRoleMapper selectPageRoleList 的 role_key like)。
        if !role_key.is_empty() {
            select = select.filter(Column::RoleKey.contains(&role_key));
        }
        if !status.is_empty() {
            select = select.filter(Column::Status.eq(&status));
        }
        // params[beginTime]/[endTime] → create_time 区间(对齐 user 列表)。
        if let Some(begin) = query.begin_time.as_deref().map(str::trim)
            && !begin.is_empty()
            && let Ok(dt) = chrono::NaiveDateTime::parse_from_str(
                begin,
                "%Y-%m-%d %H:%M:%S",
            )
        {
            select = select.filter(Column::CreateTime.gte(dt.and_utc()));
        }
        if let Some(end) = query.end_time.as_deref().map(str::trim)
            && !end.is_empty()
            && let Ok(dt) =
                chrono::NaiveDateTime::parse_from_str(end, "%Y-%m-%d %H:%M:%S")
        {
            select = select.filter(Column::CreateTime.lte(dt.and_utc()));
        }
        if !keyword.is_empty() {
            select = select.filter(
                Condition::any()
                    .add(Column::RoleName.contains(&keyword))
                    .add(Column::RoleKey.contains(&keyword)),
            );
        }

        select
    }

    /// 分页查询角色列表。
    ///
    /// 范围施加点:Java `selectPageRoleList`(SysRoleMapper.java:33-39 带
    /// @DataPermission:deptName=create_dept,userName=create_by)→ 本方法走
    /// `ctx.select(..).page()`,漏斗按 sys_role 的 DataPolicy 自动施加;
    /// 排序在 Select 构件上,逃生舱分页保持 ORDER BY(设计 6.4 裁定)。
    pub async fn select_page(
        &self,
        ctx: &DataContext<'_>,
        query: &RolePageQuery,
        page_no: u64,
        page_size: u64,
        order_by: &str,
    ) -> Result<TableDataInfo<RoleListVo>, DataError> {
        let mut select = Self::apply_filters(Entity::find(), query);
        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let page = ctx
            .select(select)
            .page(page_no as i64, page_size as i64)
            .await?;

        let rows: Vec<RoleListVo> = page
            .rows
            .iter()
            .map(Role::from)
            .map(|r: Role| r.to_list_vo())
            .collect();

        Ok(TableDataInfo::from_page_rows(&page, rows))
    }

    /// 按过滤条件查询全部角色(不分页,导出用,与 select_page 共用 filter)。
    ///
    /// 范围施加点:Java 导出(Controller:69)→ `selectRoleList`(Mapper:47
    /// **带 @DataPermission**)→ `ctx.select` 自动施加。
    /// **与 post 的导出豁免相反** —— role 的导出在 Java 基线就是受辖的。
    pub async fn select_list(
        &self,
        ctx: &DataContext<'_>,
        query: &RolePageQuery,
        order_by: &str,
    ) -> Result<Vec<Role>, DataError> {
        let mut select = Self::apply_filters(Entity::find(), query);
        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let models = ctx.select(select).all().await?;
        Ok(models.iter().map(Role::from).collect())
    }

    /// 查询用户关联的角色列表 (JOIN sys_user_role)。
    ///
    /// 豁免裁定:Java `selectRolesByUserId`(Mapper:89-96,join)无
    /// @DataPermission("我的角色"语义)→ `without_row_scope()`;软删过滤
    /// 由漏斗照施,status='0' 是业务过滤
    /// (范围计算只认启用角色,保留)。
    ///
    /// 单跳 INNER JOIN,无显式外键,沿用调用点构造法:
    /// `Entity::belongs_to(..).from(..).to(..).into()` 生成 RelationDef,
    /// JOIN 类型由 `.join(JoinType::InnerJoin, ..)` 显式指定。
    pub async fn select_roles_by_user_id(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> Result<Vec<Role>, DataError> {
        let select = Entity::find()
            .join(
                JoinType::InnerJoin,
                // role ⟕ user_role:ON role.role_id = user_role.role_id
                Entity::belongs_to(user_role::Entity)
                    .from(Column::RoleId)
                    .to(user_role::Column::RoleId)
                    .into(),
            )
            .filter(user_role::Column::UserId.eq(user_id))
            .filter(Column::Status.eq(Status::Normal.as_str()))
            .order_by(Column::RoleSort, Order::Asc);
        let models = ctx.select(select).without_row_scope().all().await?;
        Ok(models.iter().map(Role::from).collect())
    }

    /// 插入角色-菜单关联(原 INSERT INTO sys_role_menu)。
    ///
    /// 裁定:关联表 DataPolicy 全 None(CONSTRAINED=false,无软删/审计)
    /// → `ctx.insert_many`;空列表 no-op 保留。
    pub async fn insert_role_menus(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
        menu_ids: &[i64],
    ) -> Result<(), DataError> {
        if menu_ids.is_empty() {
            return Ok(());
        }
        ctx.insert_many(
            menu_ids
                .iter()
                .map(|&mid| role_menu::ActiveModel {
                    role_id: Set(role_id),
                    menu_id: Set(mid),
                })
                .collect(),
        )
        .await?;
        Ok(())
    }

    /// 删除角色的所有菜单关联(原 DELETE FROM sys_role_menu WHERE role_id = ?)。
    ///
    /// 裁定:role_menu 豁免实体(全 None,无软删列)→ `ctx.delete_where`
    /// 承担 delete-then-insert 的 DELETE 半边(生产调用点在
    /// update_role_permission 的事务闭包内,经 TxnContext 同名方法)。
    pub async fn delete_role_menus_by_role_id(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
    ) -> Result<(), DataError> {
        ctx.delete_where::<role_menu::Entity>(
            Condition::all().add(role_menu::Column::RoleId.eq(role_id)),
        )
        .await?;
        Ok(())
    }

    /// 插入角色-部门关联(原 INSERT INTO sys_role_dept)。
    /// 裁定同 [`Self::insert_role_menus`](关联表全 None → `ctx.insert_many`)。
    pub async fn insert_role_depts(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
        dept_ids: &[i64],
    ) -> Result<(), DataError> {
        if dept_ids.is_empty() {
            return Ok(());
        }
        ctx.insert_many(
            dept_ids
                .iter()
                .map(|&did| role_dept::ActiveModel {
                    role_id: Set(role_id),
                    dept_id: Set(did),
                })
                .collect(),
        )
        .await?;
        Ok(())
    }

    /// 删除角色的所有部门关联(原 DELETE FROM sys_role_dept WHERE role_id = ?)。
    /// 裁定同 [`Self::delete_role_menus_by_role_id`](豁免实体 → `delete_where`)。
    pub async fn delete_role_depts_by_role_id(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
    ) -> Result<(), DataError> {
        ctx.delete_where::<role_dept::Entity>(
            Condition::all().add(role_dept::Column::RoleId.eq(role_id)),
        )
        .await?;
        Ok(())
    }

    /// 查询角色集合关联的部门 ID(去重,原 SELECT DISTINCT dept_id)。
    ///
    /// 豁免裁定:role_dept 关联表读(全 None)→ `without_row_scope`;
    /// 投影/去重在 Rust 侧完成(实体查询后 dedup,DISTINCT 的 SQL 形态
    /// 不再必要,行集等价)。
    pub async fn select_dept_ids_by_role_ids(
        &self,
        ctx: &DataContext<'_>,
        role_ids: &[i64],
    ) -> Result<Vec<i64>, DataError> {
        if role_ids.is_empty() {
            return Ok(Vec::new());
        }
        let models = ctx
            .select(
                role_dept::Entity::find()
                    .filter(role_dept::Column::RoleId.is_in(role_ids.to_vec())),
            )
            .without_row_scope()
            .all()
            .await?;
        let mut dept_ids: Vec<i64> =
            models.into_iter().map(|m| m.dept_id).collect();
        dept_ids.sort_unstable();
        dept_ids.dedup();
        Ok(dept_ids)
    }

    /// 查询给定角色集合的 (role_id, dept_id) 对(用于多角色数据权限并集时
    /// 按 role 分组填充各自的自定义部门集)。结果不去重,保持原始关联。
    ///
    /// 豁免裁定:同 [`Self::select_dept_ids_by_role_ids`](关联表读,全 None)。
    pub async fn select_role_dept_pairs_by_role_ids(
        &self,
        ctx: &DataContext<'_>,
        role_ids: &[i64],
    ) -> Result<Vec<(i64, i64)>, DataError> {
        if role_ids.is_empty() {
            return Ok(Vec::new());
        }
        let models = ctx
            .select(
                role_dept::Entity::find()
                    .filter(role_dept::Column::RoleId.is_in(role_ids.to_vec())),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.into_iter().map(|m| (m.role_id, m.dept_id)).collect())
    }

    /// 更新角色状态。
    ///
    /// **写路径不受行级范围**:Java 写方法无
    /// @DataPermission(`updateRoleStatus` ServiceImpl:357 → `lambda()
    /// .updateCount()`),越权拦截在 controller 的 checkRoleDataScope。
    /// WHERE = 主键 + 软删存活,0 行 → NotFound;审计盖 update_*。
    pub async fn update_status(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
        status: &str,
    ) -> Result<(), DataError> {
        let active = ActiveModel {
            role_id: Set(role_id),
            status: Set(status.to_string()),
            ..Default::default()
        };
        ctx.update_without_row_scope(active).await?;
        Ok(())
    }

    /// 越权可见性探针(对齐 Java `SysRoleMapper.selectRoleCount`,
    /// Mapper.java:61-66,**带 @DataPermission 双列**:create_dept /
    /// create_by)。走受辖 `ctx.select`,count==0 ⟺ 角色不存在**或**不在
    /// 调用方数据范围内 —— 读/写路径的越权判定都由它承担。
    pub async fn count_role_by_id(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
    ) -> Result<u64, DataError> {
        ctx.select(Entity::find_by_id(role_id)).count().await
    }

    /// 查询某角色已分配的用户分页(JOIN sys_user_role)。
    ///
    /// 范围施加点:Java `selectAllocatedList`(SysUserMapper.java:93-108 带
    /// @DataPermission:d.dept_id/u.create_by,锚在查询根 sys_user)→
    /// `ctx.select(..).page()` 按 sys_user 的 DataPolicy 自动施加(本查询
    /// 根即 UserEntity,join 的 user_role 无范围列,无歧义)。
    ///
    /// 单跳 INNER JOIN 用调用点构造法;count/分页由逃生舱 `.page()` 一体出
    /// (vendored fork 的子查询 count 形态)。
    ///
    /// 无需 DISTINCT:sys_user_role 主键为 (user_id, role_id),按 role_id
    /// 过滤后每个用户至多出现一次,INNER JOIN 不会产生重复行。
    pub async fn select_allocated_user_page(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
        q: &UserPageQuery,
    ) -> Result<(Vec<User>, u64), DataError> {
        // 动态 WHERE:必选 role_id;软删/行级范围由漏斗施加;
        // 可选 user_name/phone LIKE。
        let mut cond =
            Condition::all().add(user_role::Column::RoleId.eq(role_id));
        if let Some(user_name) = q.user_name.as_ref().filter(|s| !s.is_empty())
        {
            cond =
                cond.add(UserColumn::UserName.like(format!("%{}%", user_name)));
        }
        if let Some(phone) = q.phone_number.as_ref().filter(|s| !s.is_empty()) {
            cond =
                cond.add(UserColumn::PhoneNumber.like(format!("%{}%", phone)));
        }

        // user ⟕ user_role:ON user.user_id = user_role.user_id
        let select = UserEntity::find()
            .join(
                JoinType::InnerJoin,
                UserEntity::belongs_to(user_role::Entity)
                    .from(UserColumn::UserId)
                    .to(user_role::Column::UserId)
                    .into(),
            )
            .filter(cond)
            .order_by(UserColumn::UserId, Order::Asc);

        let (page_no, page_size, _offset) = normalize_paging(&q.page);
        let page = ctx.select(select).page(page_no, page_size).await?;
        let users: Vec<User> = page.rows.iter().map(User::from).collect();

        Ok((users, page.total.max(0) as u64))
    }

    /// 查询某角色未分配的用户分页(反连接)。
    ///
    /// 范围施加点:Java `selectUnallocatedList`(SysUserMapper.java:114-129
    /// 带 @DataPermission,同 allocated)→ `ctx.select(..).page()` 自动施加。
    ///
    /// 反连接子查询:SELECT user_id FROM sys_user_role WHERE role_id = ?
    /// 主查询的 user_id NOT IN (子查询) ⇒ "没有该 role 的用户"。
    /// 注意:`role_id=?` 必须进子查询的 WHERE(此处),不能放主查询外层,
    /// 否则会把"无关联行(本该 NOT IN 命中)"错误过滤掉,导致语义反转
    /// (spec §8 最高风险)。
    pub async fn select_unallocated_user_page(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
        q: &UserPageQuery,
    ) -> Result<(Vec<User>, u64), DataError> {
        let sub = Query::select()
            .column(user_role::Column::UserId)
            .from(user_role::Entity)
            .and_where(user_role::Column::RoleId.eq(role_id))
            .to_owned();

        // 动态 WHERE:NOT IN 子查询;软删/行级范围由漏斗施加;
        // 可选 user_name/phone LIKE。
        let mut cond =
            Condition::all().add(UserColumn::UserId.not_in_subquery(sub));
        if let Some(user_name) = q.user_name.as_ref().filter(|s| !s.is_empty())
        {
            cond =
                cond.add(UserColumn::UserName.like(format!("%{}%", user_name)));
        }
        if let Some(phone) = q.phone_number.as_ref().filter(|s| !s.is_empty()) {
            cond =
                cond.add(UserColumn::PhoneNumber.like(format!("%{}%", phone)));
        }

        // 结构化 SELECT + 逃生舱分页一体出 count/行(同 allocated 模式)。
        let select = UserEntity::find()
            .filter(cond)
            .order_by(UserColumn::UserId, Order::Asc);

        let (page_no, page_size, _offset) = normalize_paging(&q.page);
        let page = ctx.select(select).page(page_no, page_size).await?;
        let users: Vec<User> = page.rows.iter().map(User::from).collect();

        Ok((users, page.total.max(0) as u64))
    }

    /// 插入用户-角色关联(原 INSERT INTO sys_user_role,role 维度批量授权)。
    ///
    /// 裁定:关联表全 None → `ctx.insert_many`;排序去重逻辑
    /// 按 spec §5.2 红线保留。
    pub async fn insert_user_roles(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
        user_ids: &[i64],
    ) -> Result<(), DataError> {
        if user_ids.is_empty() {
            return Ok(());
        }
        let mut ids: Vec<i64> = user_ids.to_vec();
        ids.sort_unstable();
        ids.dedup();

        ctx.insert_many(
            ids.into_iter()
                .map(|uid| user_role::ActiveModel {
                    role_id: Set(role_id),
                    user_id: Set(uid),
                })
                .collect(),
        )
        .await?;
        Ok(())
    }

    /// [`Self::insert_user_roles`] 的事务内变体(select_auth_users 外层
    /// 事务闭包内使用 —— 补偿任务与授权变更同事务)。
    pub async fn insert_user_roles_in_txn(
        &self,
        tx: &TxnContext<'_>,
        role_id: i64,
        user_ids: &[i64],
    ) -> Result<(), DataError> {
        if user_ids.is_empty() {
            return Ok(());
        }
        let mut ids: Vec<i64> = user_ids.to_vec();
        ids.sort_unstable();
        ids.dedup();

        tx.insert_many(
            ids.into_iter()
                .map(|uid| user_role::ActiveModel {
                    role_id: Set(role_id),
                    user_id: Set(uid),
                })
                .collect(),
        )
        .await?;
        Ok(())
    }

    /// 取消用户的指定角色授权(原 DELETE FROM sys_user_role
    /// WHERE role_id = ? AND user_id IN (...))。
    ///
    /// 裁定:user_role 豁免实体(全 None)→ `ctx.delete_where`;
    /// 0 行返回 Ok(取消授权无 NotFound 约定,Java deleteCount 同语义)。
    pub async fn delete_user_roles(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
        user_ids: &[i64],
    ) -> Result<(), DataError> {
        if user_ids.is_empty() {
            return Ok(());
        }
        ctx.delete_where::<user_role::Entity>(
            Condition::all()
                .add(user_role::Column::RoleId.eq(role_id))
                .add(user_role::Column::UserId.is_in(user_ids.iter().copied())),
        )
        .await?;
        Ok(())
    }

    /// [`Self::delete_user_roles`] 的事务内变体(cancel_auth_user(s)
    /// 外层事务闭包内使用 —— 补偿任务与取消授权同事务)。
    pub async fn delete_user_roles_in_txn(
        &self,
        tx: &TxnContext<'_>,
        role_id: i64,
        user_ids: &[i64],
    ) -> Result<(), DataError> {
        if user_ids.is_empty() {
            return Ok(());
        }
        tx.delete_where::<user_role::Entity>(
            Condition::all()
                .add(user_role::Column::RoleId.eq(role_id))
                .add(user_role::Column::UserId.is_in(user_ids.iter().copied())),
        )
        .await?;
        Ok(())
    }

    /// 按主键取 role_key(办理人补偿挂钩的 auth_code 用)。
    ///
    /// 豁免裁定:挂钩只需标识字符串,与操作者的数据范围无关(受辖读会把
    /// 越权行读成 None,使补偿行丢掉标识)—— `select_by_role_key`
    /// 唯一性校验的同款豁免;软删过滤照施加。
    pub async fn select_role_key_by_id(
        &self,
        ctx: &DataContext<'_>,
        role_id: &i64,
    ) -> Result<Option<String>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(*role_id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.map(|m| m.role_key))
    }
}

fn parse_order(order_by: &str) -> (Column, Order) {
    let mut parts = order_by.split_whitespace();
    let col = match parts.next().unwrap_or("role_sort") {
        "role_id" => Column::RoleId,
        "role_name" => Column::RoleName,
        "role_sort" => Column::RoleSort,
        "create_time" => Column::CreateTime,
        _ => Column::RoleSort,
    };
    let desc = !parts.next().is_some_and(|d| d.eq_ignore_ascii_case("asc"));
    if desc {
        (col, Order::Desc)
    } else {
        (col, Order::Asc)
    }
}
