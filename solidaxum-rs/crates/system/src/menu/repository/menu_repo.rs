//! menu 仓储(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:现网
//! `SysMenuMapper.java` **零 `@DataPermission`**(grep 计数 = 0)—— 菜单/
//! 权限点是全局功能定义,按角色分配而非按部门,数据权限过滤对它无业务
//! 含义。统一锚:**SysMenuMapper 无 @DataPermission —— 权限树数据非行级**。
//! 由此每个读方法显式 `without_row_scope()`(豁免必须写出这行字才能过审,
//! 不做静默默认);sys_menu 的 DataPolicy 是 CONSTRAINED=false,
//! 无软删列,漏斗对读只剩"显式豁免"这一个动作可做。
//!
//! | 本文件方法 | Java 锚(SysMenuController/ServiceImpl → SysMenuMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_all` | treeselect/roleMenuTreeselect(Controller:89/:102)→ `selectMenuList`(ServiceImpl:51/:63)→ `selectMenuListByUserId`(Mapper:142) | ❌ | `without_row_scope()` |
//! | `select_by_id` | getInfo(Controller:77)→ `selectMenuById`(ServiceImpl:246)→ baseMapper.selectVoById | ❌ | `without_row_scope()` |
//! | `exists_by_parent_and_name` | `checkMenuNameUnique`(ServiceImpl:340)→ `lambda().exists()` | ❌ | `without_row_scope()`(唯一性须全量) |
//! | `select_list` | list(Controller:61)→ `selectMenuList`(ServiceImpl:63)→ Mapper:142 | ❌ | `without_row_scope()` |
//! | `insert` | add(Controller:121)→ `insertMenu`(ServiceImpl:293,写路径) | — | `ctx.insert`(审计五列漏斗盖) |
//! | `update_by_id` | edit(Controller:143)→ `updateMenu`(ServiceImpl:305,写路径) | — | `ctx.update`(豁免实体无行级范围;审计盖 update_*,0 行 → NotFound) |
//! | `delete_by_id`(物理) | remove(Controller:166)→ deleteMenuById(ServiceImpl:317)→ deleteById | — | `ctx.delete`(WHERE=主键,0 行 → NotFound) |
//! | `has_child` | `hasChildByMenuId`(ServiceImpl:257)→ `lambda().exists()` | ❌ | `without_row_scope()`(守卫须真实计数) |
//! | `has_child_not_in` | `hasChildByMenuId(Collection)`(ServiceImpl:268-273)`.in(parentId,ids).notIn(menuId,ids)` | ❌ | `without_row_scope()`(排除语义原样保留) |
//! | `count_role_menu` | `checkMenuExistRole`(ServiceImpl:282)→ lambda count(sys_role_menu) | ❌ | `without_row_scope()`(守卫须真实计数) |
//! | `delete_role_menus_by_menu_ids` | deleteMenuById(Collection)(ServiceImpl:328)级联清 sys_role_menu | — | `tx.delete_where`(关联表全 None;生产调用点在 cascade 事务闭包内) |
//! | `select_menu_ids_by_role_id` | roleMenuTreeselect(Controller:102)→ `selectMenuListByRoleId`(ServiceImpl:150 → Mapper:117) | ❌ | `without_row_scope()`(role_menu 关联读) |
//! | `find_descendant_ids` | Java 无对应锚(生产零调用,保留工具面,dept 侧同名单方法才是生产路径) | ❌ | `without_row_scope()`;保持内存算法(见方法注释) |
//! | `select_menu_tree_all` | getRouters 超管分支(ServiceImpl:122)→ `selectMenuTreeAll`(Mapper:101) | ❌ | `without_row_scope()` |
//! | `select_menu_tree_by_user_id` | getRouters 普通分支(ServiceImpl:124)→ `selectMenuTreeByUserId`(Mapper:167,三跳 join) | ❌ | `without_row_scope()`(join 驱动仍是 sys_menu) |
//! | `select_menu_perms_by_user_id` | `selectMenuPermsByUserId`(Mapper:33,三跳 join) | ❌ | `without_row_scope()`(同上) |
//!
//! **写路径说明**:Java 写方法本就无注解(拦截器只改写查询),sys_menu 是
//! 豁免实体(CONSTRAINED=false),漏斗 update/delete 对它不施加行级范围
//! (WHERE = 主键),审计五列照盖(insert 全盖、update 盖 update_*)—— 与
//! Java 直连行为逐点一致,无收紧也无放松。0 行 → NotFound 的语义替代原手工
//! `RecordNotFound` 预检(delete_by_id)与"先查后改"预检(update_by_id)。
//!
//! **计数口径**:计数走逃生舱 `count()` 原语(与 page().total 同口径,
//! 只发一条 COUNT)—— `has_child_not_in` / `count_role_menu` 两处;
//! `has_child` / `exists_by_parent_and_name` 走 `.one()` 存在性探测。

use std::collections::HashMap;

use app_core::model::{
    entity::BaseEntity,
    status::{DelFlag, Status},
};
use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{
        self, ActiveModelBehavior, ActiveValue::Set, ColumnTrait, Condition,
        DateTimeUtc, DeriveEntityModel, DerivePrimaryKey, DeriveRelation,
        EntityTrait, EnumIter, JoinType, Order, PrimaryKeyTrait, QueryFilter,
        QueryOrder, QuerySelect, Select,
    },
};

use crate::{
    menu::model::menu::Menu,
    relation::{role_menu, user_role},
    role::repository::role_repo,
};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_menu")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub menu_id: i64,
    pub menu_name: String,
    pub parent_id: i64,
    pub order_num: i32,
    pub path: String,
    pub component: Option<String>,
    pub query_param: Option<String>,
    pub is_frame: String,
    pub is_cache: String,
    pub menu_type: String,
    pub visible: String,
    pub status: String,
    pub perms: Option<String>,
    pub icon: Option<String>,
    pub active_menu: Option<String>,
    pub ext: Option<String>,
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

impl From<&Model> for Menu {
    fn from(m: &Model) -> Self {
        Menu {
            menu_id: m.menu_id,
            menu_name: m.menu_name.clone(),
            parent_id: m.parent_id,
            order_num: m.order_num,
            path: m.path.clone(),
            component: m.component.clone(),
            query_param: m.query_param.clone(),
            is_frame: m.is_frame.clone(),
            is_cache: m.is_cache.clone(),
            menu_type: m.menu_type.clone(),
            visible: m.visible.clone(),
            status: m.status.clone(),
            perms: m.perms.clone(),
            icon: m.icon.clone(),
            active_menu: m.active_menu.clone(),
            ext: m.ext.clone(),
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

/// Menu → ActiveModel。审计五列(create_dept/create_by/create_time/
/// update_by/update_time)只搬运领域层已持有的值;**写入时由漏斗统一盖章**
/// (insert 全盖、update 盖 update_*),领域层不再手工盖章。
fn menu_to_active_model(menu: &Menu) -> ActiveModel {
    ActiveModel {
        menu_id: Set(menu.menu_id),
        menu_name: Set(menu.menu_name.clone()),
        parent_id: Set(menu.parent_id),
        order_num: Set(menu.order_num),
        path: Set(menu.path.clone()),
        component: Set(menu.component.clone()),
        query_param: Set(menu.query_param.clone()),
        is_frame: Set(menu.is_frame.clone()),
        is_cache: Set(menu.is_cache.clone()),
        menu_type: Set(menu.menu_type.clone()),
        visible: Set(menu.visible.clone()),
        status: Set(menu.status.clone()),
        perms: Set(menu.perms.clone()),
        icon: Set(menu.icon.clone()),
        active_menu: Set(menu.active_menu.clone()),
        ext: Set(menu.ext.clone()),
        create_dept: Set(menu.base_entity.create_dept),
        create_by: Set(menu.base_entity.create_by),
        create_time: Set(menu.base_entity.create_time),
        update_by: Set(menu.base_entity.update_by),
        update_time: Set(menu.base_entity.update_time),
        remark: Set(menu.remark.clone()),
    }
}

#[derive(Clone, Default)]
pub struct MenuRepository {
    // 无状态仓储:执行面全在调用方传入的 ctx(用户请求
    // 由 handler 请求期从 `infra.data_source()` 构造,那里才带 resolver,
    // 见 app.rs 装配顺序注释;测试经自有 `ds.system_context()` 构造 ctx)。
}

impl MenuRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// 全量菜单(treeselect / roleMenuTreeselect 用)。
    ///
    /// 豁免裁定:SysMenuMapper 无 @DataPermission —— 权限树数据非行级
    /// (Java 锚 selectMenuList → Mapper:142)。
    pub async fn select_all(
        &self,
        ctx: &DataContext<'_>,
    ) -> Result<Vec<Menu>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .order_by(Column::ParentId, Order::Asc)
                    .order_by(Column::OrderNum, Order::Asc),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(Menu::from).collect())
    }

    /// 按主键查详情。
    ///
    /// 豁免裁定:SysMenuMapper 无 @DataPermission —— 权限树数据非行级
    /// (Java 锚 getInfo → selectMenuById → baseMapper.selectVoById)。
    pub async fn select_by_id(
        &self,
        ctx: &DataContext<'_>,
        menu_id: i64,
    ) -> Result<Option<Menu>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(menu_id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Menu::from))
    }

    /// 同父节点下同名菜单是否已存在(唯一性校验)。
    ///
    /// 豁免裁定:SysMenuMapper 无 @DataPermission —— 权限树数据非行级
    /// (Java 锚 checkMenuNameUnique → lambda().exists(),唯一性须全量;
    /// exists 语义以 `.one()` 终端实现,免 COUNT)。
    pub async fn exists_by_parent_and_name(
        &self,
        ctx: &DataContext<'_>,
        parent_id: i64,
        menu_name: &str,
        exclude_id: Option<i64>,
    ) -> Result<bool, DataError> {
        let mut select = Entity::find()
            .filter(Column::ParentId.eq(parent_id))
            .filter(Column::MenuName.eq(menu_name));
        if let Some(id) = exclude_id {
            select = select.filter(Column::MenuId.ne(id));
        }
        let model = ctx.select(select).without_row_scope().one().await?;
        Ok(model.is_some())
    }

    /// 按名称/状态过滤的菜单列表(list_menu 用)。
    ///
    /// 豁免裁定:SysMenuMapper 无 @DataPermission —— 权限树数据非行级
    /// (Java 锚 list → selectMenuList ServiceImpl:63 → Mapper:142;
    /// Java 侧以 userId 参与 WHERE,但无注解)。
    pub async fn select_list(
        &self,
        ctx: &DataContext<'_>,
        menu_name: &Option<String>,
        status: &Option<String>,
    ) -> Result<Vec<Menu>, DataError> {
        let mut select: Select<Entity> = Entity::find();

        if let Some(name) = menu_name
            && !name.is_empty()
        {
            select = select.filter(Column::MenuName.contains(name));
        }
        if let Some(s) = status
            && !s.is_empty()
        {
            select = select.filter(Column::Status.eq(s));
        }

        let models = ctx
            .select(
                select
                    .order_by(Column::OrderNum, Order::Asc)
                    .order_by(Column::MenuId, Order::Asc),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(Menu::from).collect())
    }

    /// 新增菜单。审计五列由漏斗 insert 统一盖章(principal 取自 ctx),
    /// 领域层传入的审计值会被覆盖(Java 锚 insertMenu ServiceImpl:293)。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        menu: &Menu,
    ) -> Result<i64, DataError> {
        let active = menu_to_active_model(menu);
        ctx.insert_returning_nothing(active).await?;
        Ok(menu.menu_id)
    }

    /// 按主键更新。漏斗 update:审计盖 update_*(create_* 搬运领域层原值),
    /// WHERE = 主键(sys_menu 豁免实体,无软删/行级范围),0 行 → NotFound
    /// (Java 锚 updateMenu ServiceImpl:305;原"先查后改"预检随漏斗 WHERE
    /// 删除 —— handler 侧为拼领域对象仍需先读,读不到时 NotFound 语义相同)。
    pub async fn update_by_id(
        &self,
        ctx: &DataContext<'_>,
        menu: &Menu,
    ) -> Result<(), DataError> {
        let active = menu_to_active_model(menu);
        ctx.update(active).await?;
        Ok(())
    }

    /// 物理删除菜单(Java 锚 deleteMenuById ServiceImpl:317 → deleteById;
    /// sys_menu 无 @TableLogic,Java 侧同为物理删)。漏斗 delete:WHERE =
    /// 主键,0 行 → NotFound(原手工"菜单不存在或已被删除"预检随漏斗删除,
    /// 错误类不变)。
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        menu_id: i64,
    ) -> Result<(), DataError> {
        ctx.delete::<Entity>(menu_id).await
    }

    /// 是否存在子菜单(单删守卫)。
    ///
    /// 豁免裁定:SysMenuMapper 无 @DataPermission —— 权限树数据非行级
    /// (Java 锚 hasChildByMenuId ServiceImpl:257 → lambda().exists();
    /// exists 语义以 `.one()` 终端实现,免 COUNT)。
    pub async fn has_child(
        &self,
        ctx: &DataContext<'_>,
        menu_id: i64,
    ) -> Result<bool, DataError> {
        let model = ctx
            .select(Entity::find().filter(Column::ParentId.eq(menu_id)))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.is_some())
    }

    /// 是否存在「parent_id 在 menu_ids 内、但自身不在删除集合内」的子节点。
    /// 对齐 Java hasChildByMenuId(Collection) 的 .notIn(menuId, ids) 排除语义 ——
    /// 级联删除时子节点同批在集合内不算阻碍(SysMenuServiceImpl.java:268-273)。
    ///
    /// 豁免裁定:SysMenuMapper 无 @DataPermission —— 权限树数据非行级
    /// (守卫需要真实全量计数,不得被任何范围砍小)。
    pub async fn has_child_not_in(
        &self,
        ctx: &DataContext<'_>,
        menu_ids: &[i64],
    ) -> Result<bool, DataError> {
        let n = ctx
            .select(
                Entity::find()
                    .filter(Column::ParentId.is_in(menu_ids.iter().copied()))
                    .filter(Column::MenuId.is_not_in(menu_ids.iter().copied())),
            )
            .without_row_scope()
            .count()
            .await?;
        Ok(n > 0)
    }

    /// 菜单被多少角色关联(删除守卫:已分配不允许删)。
    ///
    /// 豁免裁定:SysMenuMapper 无 @DataPermission —— 权限树数据非行级;
    /// role_menu 是关联表(Java checkMenuExistRole ServiceImpl:282 →
    /// lambda count,关联读全 None)。
    pub async fn count_role_menu(
        &self,
        ctx: &DataContext<'_>,
        menu_id: i64,
    ) -> Result<u64, DataError> {
        // 原 SELECT COUNT(*) FROM sys_role_menu WHERE menu_id = ?
        ctx.select(
            role_menu::Entity::find()
                .filter(role_menu::Column::MenuId.eq(menu_id)),
        )
        .without_row_scope()
        .count()
        .await
    }

    /// 删除菜单集合的全部角色关联(原 DELETE FROM sys_role_menu
    /// WHERE menu_id IN (...))。生产调用点在 cascade 删除的
    /// `ctx.transaction` 闭包内(Java 锚 deleteMenuById(Collection)
    /// ServiceImpl:328 的级联清理),故收 `&TxnContext`(关联函数:
    /// 执行面全在 tx,无仓储状态参与,dept `update_by_id_in_txn` 同款)。
    ///
    /// 裁定:role_menu 关联表(全 None)→ `tx.delete_where`(豁免实体
    /// 专属原语);空集合 no-op 保留。
    pub async fn delete_role_menus_by_menu_ids(
        tx: &TxnContext<'_>,
        menu_ids: &[i64],
    ) -> Result<(), DataError> {
        if menu_ids.is_empty() {
            return Ok(());
        }
        // 原 DELETE FROM sys_role_menu WHERE menu_id IN (...)
        tx.delete_where::<role_menu::Entity>(
            Condition::all()
                .add(role_menu::Column::MenuId.is_in(menu_ids.iter().copied())),
        )
        .await?;
        Ok(())
    }

    /// 查询角色已分配的菜单 id 集合(roleMenuTreeselect 的 checkedKeys)。
    ///
    /// 豁免裁定:SysMenuMapper 无 @DataPermission —— 权限树数据非行级
    /// (Java 锚 selectMenuListByRoleId Mapper:117,role_menu 关联读)。
    /// 原 select_only().column(menu_id).into_tuple 投影撤销 —— 逃生舱无
    /// into_tuple 终端,整行读后取列(role 的 select_dept_ids 同款;
    /// role_menu 主键 (role_id, menu_id) 保证同 role 内 menu_id 无重复)。
    pub async fn select_menu_ids_by_role_id(
        &self,
        ctx: &DataContext<'_>,
        role_id: i64,
    ) -> Result<Vec<i64>, DataError> {
        // 原 SELECT menu_id FROM sys_role_menu WHERE role_id = ?
        let models = ctx
            .select(
                role_menu::Entity::find()
                    .filter(role_menu::Column::RoleId.eq(role_id)),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.into_iter().map(|m| m.menu_id).collect())
    }

    /// 查找 parent_id 的所有后代 menu_id(包含自身)。
    ///
    /// 豁免裁定:SysMenuMapper 无 @DataPermission —— 权限树数据非行级。
    /// **保持内存算法**(dept `find_child_ids` 同款):子树展开是树搜索
    /// 语义,无结构化等价。生产零调用,保留工具面。
    pub async fn find_descendant_ids(
        &self,
        ctx: &DataContext<'_>,
        parent_id: i64,
    ) -> Result<Vec<i64>, DataError> {
        let all = ctx.select(Entity::find()).without_row_scope().all().await?;

        let mut children: HashMap<i64, Vec<i64>> = HashMap::new();
        for m in &all {
            children.entry(m.parent_id).or_default().push(m.menu_id);
        }

        let mut result = Vec::new();
        let mut stack = vec![parent_id];
        while let Some(id) = stack.pop() {
            result.push(id);
            if let Some(kids) = children.get(&id) {
                for kid in kids {
                    stack.push(*kid);
                }
            }
        }

        Ok(result)
    }

    /// 超级管理员的完整菜单树(getRouters 超管分支)。
    ///
    /// 豁免裁定:SysMenuMapper 无 @DataPermission —— 权限树数据非行级
    /// (Java 锚 selectMenuTreeAll Mapper:101)。
    pub async fn select_menu_tree_all(
        &self,
        ctx: &DataContext<'_>,
    ) -> Result<Vec<Menu>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .filter(Column::MenuType.is_in(["M", "C"]))
                    .filter(Column::Status.eq(Status::Normal.as_str()))
                    .order_by(Column::ParentId, Order::Asc)
                    .order_by(Column::OrderNum, Order::Asc),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(Menu::from).collect())
    }

    /// 用户的菜单树(getRouters 普通分支)。
    ///
    /// 豁免裁定:SysMenuMapper 无 @DataPermission —— 权限树数据非行级
    /// (Java 锚 selectMenuTreeByUserId Mapper:167,三跳 join;join 驱动
    /// 仍是 sys_menu,豁免口径不变)。
    ///
    /// 用户的菜单树 —— 三跳 INNER JOIN 链(DISTINCT + ORDER BY)。
    /// 等价原手写 SQL:
    ///   SELECT DISTINCT m.* FROM sys_menu m
    ///     INNER JOIN sys_role_menu srm ON m.menu_id = srm.menu_id
    ///     INNER JOIN sys_user_role sur ON srm.role_id = sur.role_id
    ///     INNER JOIN sys_role sr ON sur.role_id = sr.role_id
    ///     WHERE sur.user_id = ?
    ///       AND sr.status = '0' AND sr.del_flag = '0'
    ///       AND m.menu_type IN ('M', 'C') AND m.status = '0'
    ///     ORDER BY m.parent_id ASC, m.order_num ASC
    ///
    /// 三跳均为 menu⟔role_menu⟔user_role⟔role 的列等值连接,无显式外键,
    /// 故用 `Entity::belongs_to(..).from(..).to(..).into()` 在调用点构造
    /// 各跳的 RelationDef,JOIN 类型由 `.join(JoinType::..)` 显式指定。
    pub async fn select_menu_tree_by_user_id(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> Result<Vec<Menu>, DataError> {
        let select = Entity::find()
            .distinct()
            .join(
                JoinType::InnerJoin,
                // menu ⟕ role_menu:ON menu.menu_id = role_menu.menu_id
                Entity::belongs_to(role_menu::Entity)
                    .from(Column::MenuId)
                    .to(role_menu::Column::MenuId)
                    .into(),
            )
            .join(
                JoinType::InnerJoin,
                // role_menu ⟕ user_role:ON role_menu.role_id = user_role.role_id
                role_menu::Entity::belongs_to(user_role::Entity)
                    .from(role_menu::Column::RoleId)
                    .to(user_role::Column::RoleId)
                    .into(),
            )
            .join(
                JoinType::InnerJoin,
                // user_role ⟕ role:ON user_role.role_id = role.role_id
                user_role::Entity::belongs_to(role_repo::Entity)
                    .from(user_role::Column::RoleId)
                    .to(role_repo::Column::RoleId)
                    .into(),
            )
            .filter(user_role::Column::UserId.eq(user_id))
            .filter(role_repo::Column::Status.eq(Status::Normal.as_str()))
            .filter(role_repo::Column::DelFlag.eq(DelFlag::Alive.as_str()))
            .filter(Column::MenuType.is_in(["M", "C"]))
            .filter(Column::Status.eq(Status::Normal.as_str()))
            .order_by(Column::ParentId, Order::Asc)
            .order_by(Column::OrderNum, Order::Asc);
        let models = ctx.select(select).without_row_scope().all().await?;
        Ok(models.iter().map(Menu::from).collect())
    }

    /// 用户的权限标识串(getMenuPermission / getInfo 的 permissions)。
    ///
    /// 豁免裁定:SysMenuMapper 无 @DataPermission —— 权限树数据非行级
    /// (Java 锚 selectMenuPermsByUserId Mapper:33,三跳 join;join 驱动
    /// 仍是 sys_menu,豁免口径不变)。
    ///
    /// 用户的权限标识串 —— 三跳 LEFT JOIN 链(DISTINCT)。
    /// 等价原手写 SQL:
    ///   SELECT DISTINCT m.* FROM sys_menu m
    ///     LEFT JOIN sys_role_menu srm ON srm.menu_id = m.menu_id
    ///     LEFT JOIN sys_user_role sur ON srm.role_id = sur.role_id
    ///     LEFT JOIN sys_role sr ON sr.role_id = srm.role_id
    ///     WHERE sur.user_id = ?
    ///       AND sr.status = '0' AND sr.del_flag = '0'
    ///       AND m.perms IS NOT NULL AND m.perms != ''
    ///
    /// ⚠️ 注意原 SQL 第三跳 ON 为 `sr.role_id = srm.role_id`(role_menu 侧),
    ///    而非 tree 那条的 `sur.role_id`。忠实保留:hop3 由 role_menu 出发连 role。
    pub async fn select_menu_perms_by_user_id(
        &self,
        ctx: &DataContext<'_>,
        user_id: i64,
    ) -> Result<Vec<String>, DataError> {
        let select = Entity::find()
            .distinct()
            .join(
                JoinType::LeftJoin,
                // menu ⟕ role_menu:ON role_menu.menu_id = menu.menu_id
                Entity::belongs_to(role_menu::Entity)
                    .from(Column::MenuId)
                    .to(role_menu::Column::MenuId)
                    .into(),
            )
            .join(
                JoinType::LeftJoin,
                // role_menu ⟕ user_role:ON role_menu.role_id = user_role.role_id
                role_menu::Entity::belongs_to(user_role::Entity)
                    .from(role_menu::Column::RoleId)
                    .to(user_role::Column::RoleId)
                    .into(),
            )
            .join(
                JoinType::LeftJoin,
                // role_menu ⟕ role:ON role_menu.role_id = role.role_id
                // (对齐原 SQL `sr.role_id = srm.role_id`,等值连接两侧可互换)
                role_menu::Entity::belongs_to(role_repo::Entity)
                    .from(role_menu::Column::RoleId)
                    .to(role_repo::Column::RoleId)
                    .into(),
            )
            .filter(user_role::Column::UserId.eq(user_id))
            .filter(role_repo::Column::Status.eq(Status::Normal.as_str()))
            .filter(role_repo::Column::DelFlag.eq(DelFlag::Alive.as_str()))
            .filter(Column::Perms.is_not_null())
            .filter(Column::Perms.ne(""));
        let models = ctx.select(select).without_row_scope().all().await?;
        let perms: Vec<String> = models
            .iter()
            .filter_map(|m| m.perms.clone())
            .filter(|p| !p.is_empty())
            .collect();
        Ok(perms)
    }
}
