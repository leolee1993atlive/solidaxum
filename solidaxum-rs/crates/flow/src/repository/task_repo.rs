//! flow_task 仓储(**全豁免模块**)。
//!
//! canonical 变体:连接路径收 `ctx: &DataContext<'_>`、事务路径收
//! `tx: &TxnContext<'_>`(`_in_txn` 后缀,dept/leave_repo 先例),错误统一
//! `DataError`。统一锚:**warm-flow/ruoyi-workflow Mapper 零 @DataPermission
//! —— 工作流数据非行级**(普查,warm-flow 全仓 grep 计数 = 0),
//! 每个读方法显式 `without_row_scope()`;DataPolicy 见 `crate::policy`
//! (CONSTRAINED=false、soft_delete=(DelFlag,"0")、audit_columns=None ——
//! A 组 create_by/update_by 是 `Option<String>`、时间列 `Option<DateTime>`,
//! 与漏斗盖章签名不匹配,审计由 service 手工盖,同 Java DataFillHandler
//! 之前的业务显式 set)。
//!
//! | 本文件方法 | Java 锚 | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_id` / `_in_txn` | warm-flow skip 前置任务加载(`TaskServiceImpl.skip` 取 task)/ ruoyi-workflow `FlwTaskMapper.getById` | ❌ | `without_row_scope()`(双路径) |
//! | `select_by_instance_id` / `_in_txn` | 取消/结束清理实例任务(warm-flow `InsService.cancel` 链 `hand_undone_task` 对位) | ❌ | `without_row_scope()`(双路径) |
//! | `select_todo_by_handler` | ruoyi-workflow `FlwTaskMapper.getTaskByPage`(经 flow_user type=1 关联待办) | ❌ | 两段皆 `without_row_scope()`;首段整行读映射 id(逃生舱无 into_tuple 终端,dept/definition 同款裁定),分页走逃生舱 `page`(ORDER BY 留构件,设计 6.4) |
//! | `select_page` | 管理员全量待办分页(ruoyi-workflow `FlwTaskServiceImpl` 管理视角) | ❌ | 逃生舱 `page` + `without_row_scope()` |
//! | `insert_batch_in_txn` | warm-flow skip 新任务落库(`TaskServiceImpl.saveBatch` 对位) | — | `tx.insert_many`(空集合 no-op) |
//! | `soft_delete_by_id_in_txn` | `FlowTask.java:74` `@TableLogic(value="0", delval="1")` —— Java skip 旧任务删除是**逻辑删** | — | `tx.soft_delete_where`(0 行 → Ok(0),批量语义无 NotFound 约定) |
//! | `soft_delete_by_instance_id` | 同上(工具面,生产零调用,保留对位) | — | `ctx.soft_delete_where`(同上) |
//! | `update_by_id_in_txn` | 顺签/多人审批更新任务(`FlwTaskMapper.updateById` 对位) | — | `tx.update`(0 行 → `DataError::NotFound`,doc 注释标明) |
//!
//! **删除一律逻辑删**(node_repo 同款裁定):FlowTask 是 `@TableLogic`
//! 实体,Java 删除走 UPDATE del_flag,`delete_many` 硬删与 Java 锚不符,
//! 故本文件只提供 `soft_delete_*`(读侧本就全滤 del_flag='0',逻辑删的
//! 行为差异仅是墓碑行不再被物理清除)。

use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{
        ActiveValue::{NotSet, Set},
        ColumnTrait, Condition, EntityTrait, JoinType, Order, QueryFilter,
        QueryOrder, QuerySelect, RelationTrait, Select,
    },
};
use flow_task::{Column, Entity, Relation};

use crate::{
    model::{dto::TaskPageFilter, task::Task},
    repository::entity::{
        flow_definition, flow_instance, flow_task, flow_user,
    },
};

fn apply_task_filters(
    mut select: Select<Entity>,
    filter: &TaskPageFilter,
) -> Select<Entity> {
    if let Some(v) = filter.node_name.as_deref().filter(|s| !s.is_empty()) {
        select = select.filter(Column::NodeName.contains(v));
    }

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
        select = select.join(JoinType::InnerJoin, Relation::Definition.def());
        if let Some(v) = filter.flow_name.as_deref().filter(|s| !s.is_empty()) {
            select =
                select.filter(flow_definition::Column::FlowName.contains(v));
        }
        if let Some(v) = filter.flow_code.as_deref().filter(|s| !s.is_empty()) {
            select =
                select.filter(flow_definition::Column::FlowCode.contains(v));
        }
    }

    // 「申请人」= flow_instance.create_by,对齐 Java FlwTaskMapper:56
    // `inIfNotEmpty("i", FlowInstance::getCreateBy, ..)`。
    // 不能用 flow_task.create_by —— 那是创建该任务行的操作者,
    // 首节点之后即上一个审批人(见 skip::persist 建新任务处)。
    let create_by_ids = filter.create_by_id_list();
    if !create_by_ids.is_empty() {
        select = select
            .join(JoinType::InnerJoin, Relation::Instance.def())
            .filter(flow_instance::Column::CreateBy.is_in(create_by_ids));
    }

    select
}

fn task_to_active_model(task: &Task) -> flow_task::ActiveModel {
    flow_task::ActiveModel {
        id: match task.id {
            Some(id) => Set(id),
            None => NotSet,
        },
        definition_id: Set(task.definition_id),
        instance_id: Set(task.instance_id),
        node_code: Set(task.node_code.clone()),
        node_name: Set(task.node_name.clone()),
        node_type: Set(task.node_type),
        flow_status: Set(task.flow_status.clone()),
        form_custom: Set(task.form_custom.clone()),
        form_path: Set(task.form_path.clone()),
        create_time: Set(task.create_time),
        create_by: Set(task.create_by.clone()),
        update_time: Set(task.update_time),
        update_by: Set(task.update_by.clone()),
        del_flag: Set(task.del_flag.clone()),
        tenant_id: Set(task.tenant_id.clone()),
    }
}

/// 流程任务数据访问(无状态关联函数;连接路径收 `&DataContext`,事务路径
/// 收 `&TxnContext`,审计手工盖 —— 见文件头审计表)
pub struct TaskRepository;

impl TaskRepository {
    pub async fn select_by_id(
        ctx: &DataContext<'_>,
        id: i64,
    ) -> Result<Option<Task>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Task::from))
    }

    /// [`Self::select_by_id`] 的事务内变体(skip 链同事务读写)。
    pub async fn select_by_id_in_txn(
        tx: &TxnContext<'_>,
        id: i64,
    ) -> Result<Option<Task>, DataError> {
        let model = tx
            .select(Entity::find_by_id(id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Task::from))
    }

    pub async fn select_by_instance_id(
        ctx: &DataContext<'_>,
        instance_id: i64,
    ) -> Result<Vec<Task>, DataError> {
        let models = ctx
            .select(Entity::find().filter(Column::InstanceId.eq(instance_id)))
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(Task::from).collect())
    }

    /// [`Self::select_by_instance_id`] 的事务内变体(取消/结束清理、
    /// 一票否决的同事务读)。
    pub async fn select_by_instance_id_in_txn(
        tx: &TxnContext<'_>,
        instance_id: i64,
    ) -> Result<Vec<Task>, DataError> {
        let models = tx
            .select(Entity::find().filter(Column::InstanceId.eq(instance_id)))
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(Task::from).collect())
    }

    /// 查询待办任务（通过 flow_user 关联查询，避免 JOIN 歧义）。
    /// 首段整行读 flow_user 再映射 associated(逃生舱无 into_tuple 终端,
    /// definition `select_def_ids_with_instances` 同款裁定;重复 id 与原
    /// select_only 口径一致,IN 查询不受影响)。
    pub async fn select_todo_by_handler(
        ctx: &DataContext<'_>,
        handler: &str,
        filter: &TaskPageFilter,
        page_no: u64,
        page_size: u64,
    ) -> Result<app_data::Page<Task>, DataError> {
        let task_ids: Vec<i64> = ctx
            .select(
                flow_user::Entity::find()
                    .filter(flow_user::Column::ProcessedBy.eq(handler))
                    // 活跃办理人口径(同 UserRepository::select_by_task_id):
                    // Approval/Depute/Collaborate 都会出现在待办列表
                    .filter(flow_user::Column::Type.is_in(["1", "3", "5"])),
            )
            .without_row_scope()
            .all()
            .await?
            .iter()
            .map(|m| m.associated)
            .collect();

        if task_ids.is_empty() {
            return Ok(app_data::Page::exact(Vec::new(), 0));
        }

        let select = Entity::find()
            .filter(Column::Id.is_in(task_ids))
            .order_by(Column::Id, Order::Desc);
        let select = apply_task_filters(select, filter);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let rows: Vec<Task> = page.rows.iter().map(Task::from).collect();
        Ok(page.with_rows(rows))
    }

    /// 管理员视角：查询所有待办任务分页
    pub async fn select_page(
        ctx: &DataContext<'_>,
        filter: &TaskPageFilter,
        page_no: u64,
        page_size: u64,
    ) -> Result<app_data::Page<Task>, DataError> {
        let select = Entity::find().order_by(Column::Id, Order::Desc);
        let select = apply_task_filters(select, filter);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let rows: Vec<Task> = page.rows.iter().map(Task::from).collect();
        Ok(page.with_rows(rows))
    }

    /// 批量插入(事务路径:skip/persist 新任务落库的写闭包内)。
    /// `tx.insert_many` 单条多值 INSERT,不回读;空集合 no-op。
    pub async fn insert_batch_in_txn(
        tx: &TxnContext<'_>,
        tasks: &[Task],
    ) -> Result<(), DataError> {
        tx.insert_many(tasks.iter().map(task_to_active_model).collect())
            .await?;
        Ok(())
    }

    /// 按 ID 逻辑删任务(事务路径:skip 旧任务清理的写闭包内)。
    /// **Java `@TableLogic` 平价 —— 逻辑删,非物理删**(裁定见文件头)。
    /// `soft_delete_where` 自带存活过滤,0 行 → Ok(0)(批量语义无 NotFound
    /// 约定);不盖审计,update_* 由调用方手工盖(同现状)。
    pub async fn soft_delete_by_id_in_txn(
        tx: &TxnContext<'_>,
        id: i64,
    ) -> Result<u64, DataError> {
        tx.soft_delete_where::<Entity>(
            Condition::all().add(Column::Id.eq(id)),
            "1",
        )
        .await
    }

    /// 按实例逻辑删全部任务(工具面:生产零调用,保留对位)。
    /// 语义同 [`Self::soft_delete_by_id_in_txn`]。
    pub async fn soft_delete_by_instance_id(
        ctx: &DataContext<'_>,
        instance_id: i64,
    ) -> Result<u64, DataError> {
        ctx.soft_delete_where::<Entity>(
            Condition::all().add(Column::InstanceId.eq(instance_id)),
            "1",
        )
        .await
    }

    /// 按 ID 更新任务(事务路径:顺签等需要保留 task_id 的场景)。
    /// 0 行(不存在或已删)→ `DataError::NotFound`(漏斗 update 语义:
    /// 无存活过滤的直更会把已删行覆盖回存活值,形同复活,此处收紧)。
    pub async fn update_by_id_in_txn(
        tx: &TxnContext<'_>,
        task: &Task,
    ) -> Result<(), DataError> {
        tx.update(task_to_active_model(task)).await?;
        Ok(())
    }
}
