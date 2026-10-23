//! flow_user 仓储(**全豁免模块**)。
//!
//! canonical 变体:连接路径收 `ctx: &DataContext<'_>`、事务路径收
//! `tx: &TxnContext<'_>`(`_in_txn` 后缀,dept/leave_repo 先例),错误统一
//! `DataError`。统一锚:**warm-flow/ruoyi-workflow Mapper 零 @DataPermission
//! —— 工作流数据非行级**(普查,warm-flow 全仓 grep 计数 = 0),
//! 每个读方法显式 `without_row_scope()`;DataPolicy 见 `crate::policy`
//! (CONSTRAINED=false、soft_delete=(DelFlag,"0")、audit_columns=None ——
//! A 组类型不匹配,审计由 service 手工盖)。
//!
//! | 本文件方法 | Java 锚 | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_task_id` / `_in_txn` | warm-flow 办理人列表(`FlowUser` type=1,skip 链权限校验);口径扩展为活跃办理人 type∈{1,3,5}(Approval/Depute/Collaborate) | ❌ | `without_row_scope()`(双路径) |
//! | `select_all_by_task_id` | 任务下全部有效用户(不限类型,转办/抄送视图) | ❌ | `without_row_scope()` |
//! | `insert_batch` / `_in_txn` | warm-flow skip 新办理人落库(`saveBatch` 对位);抄送批插(连接路径) | — | `ctx/tx.insert_many`(空集合 no-op) |
//! | `soft_delete_by_task_id_in_txn` | `FlowUser.java:73` `@TableLogic(value="0", delval="1")` —— skip 清理办理人是**逻辑删** | — | `tx.soft_delete_where`(0 行 → Ok(0),批量语义无 NotFound 约定) |
//! | `count_approval_by_task_id` | 会签/票签办理人计数(工具面,生产零调用,保留对位) | ❌ | 逃生舱 `count()` 原语 |
//! | `select_copy_task_ids_by_handler` | ruoyi-workflow `FlwTaskMapper.getTaskCopyByPage`(`a.type=4 AND a.processed_by=userId`) | ❌ | 两跳 join(join_rev flow_task → flow_user)+ 逃生舱 `page` + `without_row_scope()`;整行读映射 id(逃生舱无 into_tuple 终端) |
//! | `soft_delete_by_task_and_handler_in_txn` | 多人审批撤销本人办理记录(`@TableLogic` 逻辑删) | — | `tx.soft_delete_where`(0 行 → Ok(0)) |
//! | `update_processed_by_in_txn` | 转办(ruoyi-workflow `FlwTaskServiceImpl.transfer` 办理人改写) | — | 单语句 `tx.update_where`(三条件 + Set processed_by;**先写者保,Java 平价**;0 行 → Ok(0)) |
//! | `insert_in_txn` | 加签/委派单条插入 | — | `tx.insert_returning_nothing`(不回读主键) |
//!
//! **删除一律逻辑删**(node_repo 同款裁定):FlowUser 是 `@TableLogic`
//! 实体,Java 删除走 UPDATE del_flag,`delete_many` 硬删与 Java 锚不符,
//! 故本文件只提供 `soft_delete_*`。

use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{
        ActiveValue::{NotSet, Set},
        ColumnTrait, Condition, EntityTrait, JoinType, QueryFilter, QueryOrder,
        QuerySelect, RelationTrait, Select,
    },
};
use flow_user::{Column, Entity, Relation};

use crate::{
    model::{dto::TaskPageFilter, user::User},
    repository::entity::{
        flow_definition, flow_instance, flow_task, flow_user,
    },
};

fn user_to_active_model(user: &User) -> flow_user::ActiveModel {
    flow_user::ActiveModel {
        id: match user.id {
            Some(id) => Set(id),
            None => NotSet,
        },
        type_: Set(user.type_.to_string()),
        processed_by: Set(user.processed_by.clone()),
        associated: Set(user.associated),
        create_time: Set(user.create_time),
        create_by: Set(user.create_by.clone()),
        update_time: Set(user.update_time),
        update_by: Set(user.update_by.clone()),
        del_flag: Set(user.del_flag.clone()),
        tenant_id: Set(user.tenant_id.clone()),
    }
}

/// 抄送查询的 Select 构件(canonical 与 legacy 共用;执行路径各异)。
/// 以 flow_task 为主表,反向 join flow_user,避免 select_only 与多表 join
/// 时的模型 hydration 问题。
fn copy_task_select(
    handler: &str,
    filter: &TaskPageFilter,
) -> Select<flow_task::Entity> {
    let mut select = flow_task::Entity::find()
        .filter(flow_task::Column::DelFlag.eq("0"))
        .join_rev(JoinType::InnerJoin, Relation::Task.def())
        .filter(Column::Type.eq("4"))
        .filter(Column::ProcessedBy.eq(handler))
        .filter(Column::DelFlag.eq("0"))
        .order_by_desc(Column::CreateTime)
        .order_by_desc(Column::Id);

    let needs_definition_join = filter
        .flow_name
        .as_deref()
        .filter(|s| !s.is_empty())
        .is_some()
        || filter
            .flow_code
            .as_deref()
            .filter(|s| !s.is_empty())
            .is_some();
    if needs_definition_join {
        select = select
            .join(JoinType::InnerJoin, flow_task::Relation::Definition.def());
    }

    if let Some(v) = filter.node_name.as_deref().filter(|s| !s.is_empty()) {
        select = select.filter(flow_task::Column::NodeName.contains(v));
    }
    if let Some(v) = filter.flow_name.as_deref().filter(|s| !s.is_empty()) {
        select = select.filter(flow_definition::Column::FlowName.contains(v));
    }
    if let Some(v) = filter.flow_code.as_deref().filter(|s| !s.is_empty()) {
        select = select.filter(flow_definition::Column::FlowCode.contains(v));
    }
    // 「申请人」= flow_instance.create_by,对齐 Java FlwTaskMapper:94
    // `inIfNotEmpty("c", FlowInstance::getCreateBy, ..)`(抄送页同样按发起人筛)。
    let create_by_ids = filter.create_by_id_list();
    if !create_by_ids.is_empty() {
        select = select
            .join(JoinType::InnerJoin, flow_task::Relation::Instance.def())
            .filter(flow_instance::Column::CreateBy.is_in(create_by_ids));
    }

    select
}

/// 流程用户数据访问(无状态关联函数;执行面见文件头审计表)
pub struct UserRepository;

impl UserRepository {
    /// 任务下活跃办理人:Approval(1) / Depute(3) / Collaborate(5)。
    /// 委派人与协同人都是要对任务采取动作的当前办理人(权限校验、
    /// try_halt 编组检测都依赖本口径);Transfer 改写原行不新增类型,
    /// Copy(4) 是只读抄送,不收。
    pub async fn select_by_task_id(
        ctx: &DataContext<'_>,
        task_id: i64,
    ) -> Result<Vec<User>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .filter(Column::Associated.eq(task_id))
                    .filter(Column::Type.is_in(["1", "3", "5"])),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(User::from).collect())
    }

    /// 查询任务下所有有效用户（不限制类型）
    pub async fn select_all_by_task_id(
        ctx: &DataContext<'_>,
        task_id: i64,
    ) -> Result<Vec<User>, DataError> {
        let models = ctx
            .select(Entity::find().filter(Column::Associated.eq(task_id)))
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(User::from).collect())
    }

    /// 批量插入(连接路径:抄送批插)。`ctx.insert_many` 单条多值
    /// INSERT,不回读;空集合 no-op。
    pub async fn insert_batch(
        ctx: &DataContext<'_>,
        users: &[User],
    ) -> Result<(), DataError> {
        ctx.insert_many(users.iter().map(user_to_active_model).collect())
            .await?;
        Ok(())
    }

    /// 批量插入(事务路径:skip/persist 新办理人落库的写闭包内)。
    /// `tx.insert_many`,与连接路径同口径。
    pub async fn insert_batch_in_txn(
        tx: &TxnContext<'_>,
        users: &[User],
    ) -> Result<(), DataError> {
        tx.insert_many(users.iter().map(user_to_active_model).collect())
            .await?;
        Ok(())
    }

    /// 按任务逻辑删全部办理人(事务路径:skip 旧任务清理的写闭包内)。
    /// **Java `@TableLogic` 平价 —— 逻辑删,非物理删**(裁定见文件头)。
    /// `soft_delete_where` 自带存活过滤,0 行 → Ok(0);不盖审计(同现状)。
    pub async fn soft_delete_by_task_id_in_txn(
        tx: &TxnContext<'_>,
        task_id: i64,
    ) -> Result<u64, DataError> {
        tx.soft_delete_where::<Entity>(
            Condition::all().add(Column::Associated.eq(task_id)),
            "1",
        )
        .await
    }

    /// 会签/票签用：统计 task 下 Approval 类型用户数(工具面:生产零调用,
    /// 保留对位)。
    pub async fn count_approval_by_task_id(
        ctx: &DataContext<'_>,
        task_id: i64,
    ) -> Result<u64, DataError> {
        ctx.select(
            Entity::find()
                .filter(Column::Associated.eq(task_id))
                .filter(Column::Type.eq("1")),
        )
        .without_row_scope()
        .count()
        .await
    }

    /// 抄送分页：查询当前用户作为抄送人(type=4)收到的任务 id 列表。
    /// 对齐 Java FlwTaskMapper.getTaskCopyByPage 中 `a.type=4 AND a.processed_by=userId` 的过滤。
    pub async fn select_copy_task_ids_by_handler(
        ctx: &DataContext<'_>,
        handler: &str,
        filter: &TaskPageFilter,
        page_no: u64,
        page_size: u64,
    ) -> Result<app_data::Page<i64>, DataError> {
        let page = ctx
            .select(copy_task_select(handler, filter))
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let task_ids = page.rows.iter().map(|m| m.id).collect();
        Ok(page.with_rows(task_ids))
    }

    /// 多人审批用：逻辑删指定 task 下指定 handler 的活跃办理人记录
    /// (事务路径,口径同 select_by_task_id:Approval/Depute/Collaborate)。
    /// `@TableLogic` 平价,0 行 → Ok(0);不盖审计(同现状)。
    pub async fn soft_delete_by_task_and_handler_in_txn(
        tx: &TxnContext<'_>,
        task_id: i64,
        handler: &str,
    ) -> Result<u64, DataError> {
        tx.soft_delete_where::<Entity>(
            Condition::all()
                .add(Column::Associated.eq(task_id))
                .add(Column::ProcessedBy.eq(handler))
                .add(Column::Type.is_in(["1", "3", "5"])),
            "1",
        )
        .await
    }

    /// 转办用：将 task 下 old_handler 的 Approval 记录更新为 new_handler
    /// (事务路径)。**单语句条件 UPDATE**(`tx.update_where`):三条件
    /// (associated=task、processed_by=old、type=1)就是全部 WHERE,SET
    /// 仅 processed_by。**并发精确表述(Java 平价,单语句先写者保)**:
    /// 双转办重叠窗口两条 UPDATE 经行锁串行,后至者条件失配
    /// (processed_by 已被先写者改走)→ 0 行 → Ok(0),不覆盖先写者
    /// 结果;办理人恒为单一值,不产生"两人并存"的脏写。自带软删存活过滤 ——
    /// 已删办理人记录不被改写,同样 0 行 → Ok(0)。
    pub async fn update_processed_by_in_txn(
        tx: &TxnContext<'_>,
        task_id: i64,
        old_handler: &str,
        new_handler: &str,
    ) -> Result<u64, DataError> {
        tx.update_where::<Entity>(
            Condition::all()
                .add(Column::Associated.eq(task_id))
                .add(Column::ProcessedBy.eq(old_handler))
                .add(Column::Type.eq("1")),
            flow_user::ActiveModel {
                processed_by: Set(Some(new_handler.to_string())),
                ..Default::default()
            },
        )
        .await
    }

    /// 单条插入(事务路径:加签/委派的写闭包内)
    pub async fn insert_in_txn(
        tx: &TxnContext<'_>,
        user: &User,
    ) -> Result<(), DataError> {
        tx.insert_returning_nothing(user_to_active_model(user))
            .await?;
        Ok(())
    }
}
