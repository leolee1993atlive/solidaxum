//! dept 仓储。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:
//!
//! | 本文件方法 | Java 锚(SysDeptServiceImpl → SysDeptMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `count_dept_by_id` | `countDeptById`(Mapper:63-68;checkDeptDataScope ServiceImpl:299 调) | ✅ dept_id | `ctx.select().page()` 自动 |
//! | `select_list`(含 select_all 合并) | `selectDeptList(Wrapper)`(Mapper:36-41) | ✅ dept_id | `ctx.select` 自动施加 |
//! | `select_by_id` | `selectDeptById` → `selectVoById`(ServiceImpl:163) | ❌ | `without_row_scope()` |
//! | `select_by_name_and_parent` | `checkDeptNameUnique` → `lambda().exists()`(ServiceImpl:277-284) | ❌ | `without_row_scope()`(唯一性须全量) |
//! | `insert` | `insertDept` → mapper.insert(写路径) | ❌ | `ctx.insert`(审计五列漏斗盖) |
//! | `update_by_id` | `updateDept` → `updateById`(ServiceImpl:357,写路径) | ❌ | `ctx.update_without_row_scope`(见表下段) |
//! | `update_children_ancestors`(及 `_in_txn` 变体) | `updateDeptChildren`(ServiceImpl:389-403):findInSet 读 + updateBatchById 写;`update_by_id_in_txn` 同表 `update_by_id` | 读 ❌ / 写 ❌ | 读 `without_row_scope()`;写同样不受辖(对齐 updateBatchById 无注解,跨范围子部门不让换父整单失败)。`_in_txn` 变体是生产路径:dept_service 换父分支把自身 update + 级联放同一 `ctx.transaction`(Java updateDept @Transactional ServiceImpl:337 平价) |
//! | `find_descendant_ids` | `selectDeptAndChildById` → `selectListByParentId`(Mapper:76-94,find_in_set) | ❌ | `without_row_scope()`;保持内存算法(见方法注释) |
//! | `count_normal_children` | `selectNormalChildrenDeptById`(ServiceImpl:241-246)lambda count | ❌ | `without_row_scope()`(守卫需真实计数) |
//! | `find_child_ids` | `selectListByParentId`(Mapper:76-81)—— permission resolver 的 DeptAndBelow 展开也走它 | ❌ | `without_row_scope()`(ctx 由 resolver 的自有 unscoped 通道传入,设计 7.2) |
//!
//! **selectDeptList 是 dept 的范围主通道**:list/excludeChild/optionselect
//! (`selectDeptByIds` ServiceImpl:182-188)/ 三个 deptTree(
//! SysPostController:164、SysUserController:341、SysRoleController:277 →
//! `selectDeptTreeList` ServiceImpl:85-89)/ `selectDeptsByList`
//! (ServiceImpl:226)全部经它 —— 受辖。Mapper:50 的
//! `selectPageDeptList` 现网 Java 基线无 controller 调用(Rust 无部门分页
//! 端点,Vue 契约部门列表不分页),死代码锚,不迁移。
//!
//! **变体合并**:`select_all()` 并入 `select_list(ctx, None, None, None)`
//! —— Java excludeChild(controller:58)走的就是 `selectDeptList(new
//! SysDeptBo())`,同一受辖基方法;排序统一 (order_num, dept_id)
//! (Java buildQueryWrapper:107 排 order_num 在其间,DeptId 兜底并列序,
//! 超集不构成行为回归)。
//!
//! **写路径统一不受辖**:Java 写方法无注解(拦截器只改写查询),本仓
//! 全部 update 走 `*_without_row_scope`,与 user/role 同口径;越权拦截
//! 由 `check_dept_data_scope` 探针承担。受辖 update 会让换父级联打到
//! 跨范围子部门时 0 行 → NotFound → 整笔回滚,操作恒失败。
//!
//! **SelfOnly × sys_dept 的语义**:sys_dept 无 owner
//! 列(create_by 不是部门行属主语义,Java 注解也只声明 deptName)。Java
//! 对仅本人角色(self scope)在缺 userName 键时落 elseSql `1 = 0`
//! (PlusDataPermissionHandler.java:151-172 → DataScopeType.java:51)
//! —— **空集**;漏斗 `scope_condition` 对 SelfOnly × dept-only 实体同样
//! 合成恒假 `0 = 1`(principal.rs),双列皆缺
//! 才保留 fail-closed `Unsupported`。受辖 dept 查询面对 SelfOnly 用户
//! 返回空数组,与 Java 生产形态一致。
//!
//! **resolver 的 unscoped 通道**:permission resolver 持**自有 unscoped
//! 通道**(`CachedScopeResolver.ds`,设计 7.2);请求路径全传真实 ctx,
//! 测试直接用 `ds.system_context()`。

use std::collections::HashMap;

use app_core::model::{
    entity::BaseEntity,
    status::{DelFlag, Status},
};
use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{
        self, ActiveModelBehavior, ActiveValue::Set, ColumnTrait, DateTimeUtc,
        DeriveEntityModel, DerivePrimaryKey, DeriveRelation, EntityTrait,
        EnumIter, Order, PrimaryKeyTrait, QueryFilter, QueryOrder, Select,
    },
};

use crate::dept::model::dept::Dept;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_dept")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub dept_id: i64,
    pub parent_id: i64,
    pub ancestors: Option<String>,
    pub dept_name: String,
    pub dept_category: Option<String>,
    pub order_num: i32,
    pub leader: Option<i64>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub status: String,
    pub del_flag: String,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTimeUtc>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<&Model> for Dept {
    fn from(m: &Model) -> Self {
        Dept {
            dept_id: m.dept_id,
            parent_id: m.parent_id,
            ancestors: m.ancestors.clone(),
            dept_name: m.dept_name.clone(),
            dept_category: m.dept_category.clone(),
            order_num: m.order_num,
            leader: m.leader,
            phone: m.phone.clone(),
            email: m.email.clone(),
            status: m.status.clone(),
            del_flag: Some(m.del_flag.clone()),
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

/// Dept → ActiveModel。审计五列(create_dept/create_by/create_time/
/// update_by/update_time)只搬运领域层已持有的值;**写入时由漏斗统一盖章**
/// (insert 全盖、update 盖 update_*),领域层不再手工盖章。
fn dept_to_active_model(dept: &Dept) -> ActiveModel {
    ActiveModel {
        dept_id: Set(dept.dept_id),
        parent_id: Set(dept.parent_id),
        ancestors: Set(dept.ancestors.clone()),
        dept_name: Set(dept.dept_name.clone()),
        dept_category: Set(dept.dept_category.clone()),
        order_num: Set(dept.order_num),
        leader: Set(dept.leader),
        phone: Set(dept.phone.clone()),
        email: Set(dept.email.clone()),
        status: Set(dept.status.clone()),
        del_flag: Set(dept
            .del_flag
            .clone()
            .unwrap_or_else(|| DelFlag::Alive.as_str().to_string())),
        create_dept: Set(dept.base_entity.create_dept),
        create_by: Set(dept.base_entity.create_by),
        create_time: Set(dept.base_entity.create_time),
        update_by: Set(dept.base_entity.update_by),
        update_time: Set(dept.base_entity.update_time),
    }
}

#[derive(Clone, Default)]
pub struct DeptRepository {
    // 无状态仓储:执行面全在调用方传入的 ctx(用户请求由 handler 请求期
    // 从 `infra.data_source()` 构造,那里才带 resolver,见 app.rs 装配
    // 顺序注释;resolver 路径见 scope_resolver.rs 的 ds 字段注释)。
}

impl DeptRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// 越权可见性探针:对齐 Java `checkDeptDataScope` →
    /// `countDeptById`(SysDeptMapper.java:63-68,**带 @DataPermission**:
    /// `@DataColumn(value = "dept_id")` —— 范围列即本表主键,部门行可见
    /// ⟺ 自身 id ∈ 可见部门集)。走受辖 `ctx.select`(范围 + 软删过滤
    /// 都施加),count==0 ⟺ 目标部门不存在**或**不在调用方数据范围内
    /// —— user add/edit 的部门维度越权判定由它承担(Java
    /// SysUserController:177/:202)。
    /// 本方法不取 self.ds,执行面全在调用方传入的受辖 ctx(与 user 的
    /// `count_user_by_id` 同型)。
    pub async fn count_dept_by_id(
        &self,
        ctx: &DataContext<'_>,
        dept_id: i64,
    ) -> Result<u64, DataError> {
        ctx.select(Entity::find_by_id(dept_id)).count().await
    }

    /// 按过滤条件查询部门列表(dept 的范围主通道)。
    ///
    /// **范围施加点**:Java `selectDeptList`(SysDeptMapper.java:36-41 带
    /// @DataPermission:deptName=dept_id)→ 本方法走 `ctx.select`,漏斗按
    /// sys_dept 的 DataPolicy 自动施加;排序在 Select 构件上保持。
    /// **变体合并注**:原 `select_all()` 并入本方法(全 None 过滤)——
    /// Java excludeChild(controller:58)走 `selectDeptList(new SysDeptBo())`,
    /// 同一受辖基方法。
    pub async fn select_list(
        &self,
        ctx: &DataContext<'_>,
        dept_name: &Option<String>,
        status: &Option<String>,
        dept_category: &Option<String>,
    ) -> Result<Vec<Dept>, DataError> {
        let mut select: Select<Entity> = Entity::find();

        if let Some(name) = dept_name
            && !name.is_empty()
        {
            select = select.filter(Column::DeptName.contains(name));
        }
        if let Some(s) = status
            && !s.is_empty()
        {
            select = select.filter(Column::Status.eq(s));
        }
        // 类别编码精确匹配(对齐 Java SysDeptBo.deptCategory)
        if let Some(c) = dept_category
            && !c.is_empty()
        {
            select = select.filter(Column::DeptCategory.eq(c));
        }

        let models = ctx
            .select(
                select
                    .order_by(Column::OrderNum, Order::Asc)
                    .order_by(Column::DeptId, Order::Asc),
            )
            .all()
            .await?;
        Ok(models.iter().map(Dept::from).collect())
    }

    /// 按主键查详情。
    ///
    /// 豁免裁定:Java `selectDeptById` → `selectVoById`(ServiceImpl:163),
    /// 无 @DataPermission → `without_row_scope()`;软删过滤照施加。
    /// 越权可见性由调用方探针承担(getInfo 的 checkDeptDataScope,
    /// SysDeptController:73)。
    pub async fn select_by_id(
        &self,
        ctx: &DataContext<'_>,
        dept_id: &i64,
    ) -> Result<Option<Dept>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(*dept_id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Dept::from))
    }

    /// 按部门名 + 父部门查(唯一性校验用)。
    ///
    /// 豁免裁定:Java `checkDeptNameUnique` → `lambda().exists()`
    /// (ServiceImpl:277-284),无 @DataPermission(唯一性校验必须全量,
    /// 否则同名部门在越权父部门下会被漏判)→ `without_row_scope()`。
    pub async fn select_by_name_and_parent(
        &self,
        ctx: &DataContext<'_>,
        dept_name: &str,
        parent_id: i64,
    ) -> Result<Option<Dept>, DataError> {
        let model = ctx
            .select(
                Entity::find()
                    .filter(Column::DeptName.eq(dept_name))
                    .filter(Column::ParentId.eq(parent_id)),
            )
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Dept::from))
    }

    /// 新增部门。审计五列由漏斗 insert 统一盖章(principal 取自 ctx),
    /// 领域层传入的审计值会被覆盖。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        dept: &Dept,
    ) -> Result<(), DataError> {
        let active = dept_to_active_model(dept);
        ctx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// 按主键更新。**写路径不受行级范围**:Java `updateDept` →
    /// `updateById`(ServiceImpl:357)无
    /// @DataPermission,越权拦截由 handler/service 的
    /// `check_dept_data_scope` 探针承担。漏斗 update:审计盖 update_*
    /// (create_* 搬运领域层原值),WHERE = 主键 + 软删存活,
    /// 已删 0 行 → NotFound。
    pub async fn update_by_id(
        &self,
        ctx: &DataContext<'_>,
        dept: &Dept,
        dept_id: &i64,
    ) -> Result<(), DataError> {
        let mut active = dept_to_active_model(dept);
        active.dept_id = Set(*dept_id);
        ctx.update_without_row_scope(active).await?;
        Ok(())
    }

    /// [`Self::update_by_id`] 的事务内变体(关联函数:执行面全在 `tx`,
    /// 无仓储状态参与)。漏斗 update 语义不变 —— 审计盖 update_*、
    /// 写路径不受行级范围(同 `update_by_id`)、
    /// 0 行 → NotFound。供 dept_service 换父分支的
    /// `ctx.transaction` 闭包使用:自身 update 与子孙级联必须同事务
    /// (Java `updateDept` 带 @Transactional,ServiceImpl:337)。
    /// `pub(crate)`:调用面只有 crate 内的
    /// dept_service,不做跨 crate 工具面。
    pub(crate) async fn update_by_id_in_txn(
        tx: &TxnContext<'_>,
        dept: &Dept,
        dept_id: &i64,
    ) -> Result<(), DataError> {
        let mut active = dept_to_active_model(dept);
        active.dept_id = Set(*dept_id);
        tx.update_without_row_scope(active).await?;
        Ok(())
    }

    /// 级联更新子部门的 ancestors 前缀
    ///
    /// 仅匹配 moved_dept 的实际后代（ancestors 以 "old_ancestors,dept_id" 开头），
    /// 将前缀从 "old_ancestors,dept_id" 替换为 "new_ancestors,dept_id"。
    /// 不会触及 moved_dept 本身（已在调用前持久化）或新父部门/兄弟部门。
    ///
    /// 漏斗裁定:读侧对齐 Java `updateDeptChildren` 的 findInSet 读
    /// (ServiceImpl:390,无 @DataPermission)→ `without_row_scope()`;
    /// 写侧同样不受辖(详见
    /// [`Self::update_children_ancestors_in_txn`] 的说明)。
    /// 逃生舱无 into_tuple 终端,整行读(部门表量级可忽略)。
    ///
    /// 关联函数(执行面全在 ctx,无仓储状态参与);
    /// 生产路径走 [`Self::update_children_ancestors_in_txn`](dept_service
    /// 换父分支的 `ctx.transaction` 闭包,与自身 update 同事务);本方法
    /// 保留独立工具面(自带单语句事务,读写同一连接原子),repo 测试
    /// 直接钉它的行语义。
    pub async fn update_children_ancestors(
        ctx: &DataContext<'_>,
        dept_id: i64,
        old_ancestors: &str,
        new_ancestors: &str,
    ) -> Result<(), DataError> {
        let old = old_ancestors.to_string();
        let new = new_ancestors.to_string();
        ctx.transaction(|tx| {
            Box::pin(async move {
                Self::update_children_ancestors_in_txn(tx, dept_id, &old, &new)
                    .await
            })
        })
        .await
    }

    /// [`Self::update_children_ancestors`] 的事务内变体(关联函数:执行面
    /// 全在 `tx`)。
    /// **级联写不受行级范围**:读半边取全部
    /// 后代(Java selectListByParentId 无注解),写半边同样不受辖 ——
    /// 对齐 Java `updateDeptChildren`(ServiceImpl:389-403)的
    /// `updateBatchById`(无 @DataPermission)。此前写半边受辖会让任何
    /// 数据范围未覆盖整棵子树的管理员在换父时命中 0 行 → NotFound →
    /// 整笔回滚,操作恒失败。事务包裹保留(Java @Transactional
    /// ServiceImpl:337 平价):自身 update 与级联仍同事务,真出错时不留
    /// 「父已移、子未级联」的撕裂祖先链。
    /// `pub(crate)` 同 `update_by_id_in_txn`(crate 内工具面)。
    pub(crate) async fn update_children_ancestors_in_txn(
        tx: &TxnContext<'_>,
        dept_id: i64,
        old_ancestors: &str,
        new_ancestors: &str,
    ) -> Result<(), DataError> {
        let old_prefix = format!("{},{}", old_ancestors, dept_id);
        let new_prefix = format!("{},{}", new_ancestors, dept_id);

        let like_pattern = format!("{}%", old_prefix);
        let rows = tx
            .select(
                Entity::find().filter(Column::Ancestors.like(&like_pattern)),
            )
            .without_row_scope()
            .all()
            .await?;

        for row in rows {
            if let Some(new_value) = row
                .ancestors
                .as_deref()
                .map(|a| a.replacen(&old_prefix, &new_prefix, 1))
            {
                let active = ActiveModel {
                    dept_id: Set(row.dept_id),
                    ancestors: Set(Some(new_value)),
                    ..Default::default()
                };
                tx.update_without_row_scope(active).await?;
            }
        }

        Ok(())
    }

    /// 查询某部门及其所有后代 id(含自身)。
    ///
    /// 豁免裁定:Java `selectDeptAndChildById`(Mapper:89-94)→
    /// `selectListByParentId`(Mapper:76-81,find_in_set)无
    /// @DataPermission → `without_row_scope()` —— 子树展开是业务搜索
    /// (belongDeptId)与守卫(循环检测)语义,不是权限。
    ///
    /// **保持内存算法**:Java 用 find_in_set(parentId, ancestors) 的
    /// 字符串匹配,无结构化等价;内存 DFS 语义已测对齐(含自身)。
    /// SQL 化归入 dialect 议题 backlog(达梦/MySQL 的 FIND_IN_SET 方言)。
    pub async fn find_descendant_ids(
        &self,
        ctx: &DataContext<'_>,
        parent_id: i64,
    ) -> Result<Vec<i64>, DataError> {
        self.find_child_ids(ctx, parent_id).await
    }

    /// 统计父部门的正常状态子部门数(停用/删除守卫用)。
    ///
    /// 豁免裁定:Java `selectNormalChildrenDeptById`(ServiceImpl:241-246)
    /// lambda count,无 @DataPermission → `without_row_scope()`
    /// (守卫需要真实计数,不能被范围砍小)。
    pub async fn count_normal_children(
        &self,
        ctx: &DataContext<'_>,
        parent_id: i64,
    ) -> Result<i64, DataError> {
        let n = ctx
            .select(
                Entity::find()
                    .filter(Column::ParentId.eq(parent_id))
                    .filter(Column::Status.eq(Status::Normal.as_str())),
            )
            .without_row_scope()
            .count()
            .await?;
        Ok(n as i64)
    }

    /// 加载存活部门全集并在内存展开 parent_id 的子树(含自身)。
    ///
    /// 豁免裁定:Java `selectListByParentId`(Mapper:76-81)无
    /// @DataPermission → `without_row_scope()`;软删过滤由漏斗照施。
    /// permission resolver 的 DeptAndBelow
    /// 展开也走本方法(ctx 由 resolver 的自有 unscoped 通道传入,
    /// 设计 7.2,见 scope_resolver.rs)。
    pub async fn find_child_ids(
        &self,
        ctx: &DataContext<'_>,
        parent_id: i64,
    ) -> Result<Vec<i64>, DataError> {
        let all = ctx.select(Entity::find()).without_row_scope().all().await?;

        let mut children: HashMap<i64, Vec<i64>> = HashMap::new();
        for m in &all {
            children.entry(m.parent_id).or_default().push(m.dept_id);
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
}
