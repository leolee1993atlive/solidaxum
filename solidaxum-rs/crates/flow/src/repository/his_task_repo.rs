//! flow_his_task 仓储(**全豁免模块**)。
//!
//! canonical 变体:连接路径收 `ctx: &DataContext<'_>`、事务路径收
//! `tx: &TxnContext<'_>`(`_in_txn` 后缀,dept/leave_repo 先例),错误统一
//! `DataError`。统一锚:**warm-flow/ruoyi-workflow Mapper 零 @DataPermission
//! —— 工作流数据非行级**(普查,warm-flow 全仓 grep 计数 = 0),
//! 每个读方法显式 `without_row_scope()`;DataPolicy 见 `crate::policy`
//! (CONSTRAINED=false、soft_delete=(DelFlag,"0")、audit_columns=None ——
//! flow_his_task 连 create_by/update_by 都没有,A 组手工盖裁定同他表)。
//!
//! | 本文件方法 | Java 锚 | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `insert` / `_in_txn` | warm-flow skip 链历史落库(insHisTask) | — | `ctx/tx.insert` |
//! | `select_by_instance_id` | 实例历史时间线(ruoyi-workflow `FlwHisTaskServiceImpl` 列表) | ❌ | `without_row_scope()` |
//! | `select_done_page` | ruoyi-workflow `FlwHisTaskMapper` 已办分页(approver=handler) | ❌ | 逃生舱 `page` + `without_row_scope()` |
//! | `select_all_done_page` | 管理员视角全部已办分页 | ❌ | 同上 |
//! | `select_by_target_node_code` | task_back 支持(按目标节点查 PASS 历史) | ❌ | `without_row_scope()` |
//! | `select_prev_node_code` | reject_last:最近一次 PASS 历史的上游节点 | ❌ | `without_row_scope()` |
//! | `select_handler_last_pass_node` | task_back:指定办理人最近 PASS 节点 | ❌ | 同上 |
//! | `count_pass_by_task_id_in_txn` | 会签/票签通过率统计(warm-flow cooperative 对位) | ❌ | 逃生舱 `count()` 原语 |
//! | `count_reject_by_task_id_in_txn` | 同上(驳回计数) | ❌ | 同上 |
//! | `select_depute_his_by_task_id` | depute 恢复原办理人(工具面,生产零调用,保留对位) | ❌ | `without_row_scope()` |
//! | `soft_delete_by_instance_ids` | 实例删除级联清历史(ruoyi-workflow `FlwInstanceServiceImpl.deleteByIds`,FlowHisTask.java:63 `@TableLogic`) | — | `ctx.soft_delete_where`(0 行 → Ok(0),批量语义无 NotFound 约定) |
//!
//! **删除一律逻辑删**(node_repo 同款裁定):FlowHisTask 是 `@TableLogic`
//! 实体,Java 删除走 UPDATE del_flag,`delete_many` 硬删与 Java 锚不符,
//! 故本文件只提供 `soft_delete_by_instance_ids`。

use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{
        ActiveValue::{NotSet, Set},
        ColumnTrait, Condition, EntityTrait, JoinType, Order, QueryFilter,
        QueryOrder, QuerySelect, RelationTrait, Select,
    },
};
use flow_his_task::{Column, Entity, Relation};

use crate::{
    model::{dto::TaskPageFilter, his_task::HisTask},
    repository::entity::{flow_definition, flow_his_task, flow_instance},
};

fn apply_his_task_filters(
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

    // his_task 无 create_by 列;「申请人」= flow_instance.create_by,
    // 对齐 Java FlwHisTaskMapper:59 `inIfNotEmpty("b", FlowInstance::getCreateBy, ..)`。
    let create_by_ids = filter.create_by_id_list();
    if !create_by_ids.is_empty() {
        select = select
            .join(JoinType::InnerJoin, Relation::Instance.def())
            .filter(flow_instance::Column::CreateBy.is_in(create_by_ids));
    }

    select
}

fn his_task_to_active_model(his_task: &HisTask) -> flow_his_task::ActiveModel {
    flow_his_task::ActiveModel {
        id: match his_task.id {
            Some(id) => Set(id),
            None => NotSet,
        },
        definition_id: Set(his_task.definition_id),
        instance_id: Set(his_task.instance_id),
        task_id: Set(his_task.task_id),
        node_code: Set(his_task.node_code.clone()),
        node_name: Set(his_task.node_name.clone()),
        node_type: Set(his_task.node_type),
        target_node_code: Set(his_task.target_node_code.clone()),
        target_node_name: Set(his_task.target_node_name.clone()),
        approver: Set(his_task.approver.clone()),
        cooperate_type: Set(his_task.cooperate_type),
        collaborator: Set(his_task.collaborator.clone()),
        skip_type: Set(his_task.skip_type.clone()),
        flow_status: Set(his_task.flow_status.clone()),
        form_custom: Set(his_task.form_custom.clone()),
        form_path: Set(his_task.form_path.clone()),
        message: Set(his_task.message.clone()),
        variable: Set(his_task.variable.clone()),
        ext: Set(his_task.ext.clone()),
        create_time: Set(his_task.create_time),
        update_time: Set(his_task.update_time),
        del_flag: Set(his_task.del_flag.clone()),
        tenant_id: Set(his_task.tenant_id.clone()),
    }
}

/// 流程历史任务数据访问(无状态关联函数;执行面见文件头审计表)
pub struct HisTaskRepository;

impl HisTaskRepository {
    /// 单条插入(连接路径,通用面)。
    pub async fn insert(
        ctx: &DataContext<'_>,
        his_task: &HisTask,
    ) -> Result<(), DataError> {
        ctx.insert_returning_nothing(his_task_to_active_model(his_task))
            .await?;
        Ok(())
    }

    /// 单条插入(事务路径:skip 链历史落库的写闭包内)。
    pub async fn insert_in_txn(
        tx: &TxnContext<'_>,
        his_task: &HisTask,
    ) -> Result<(), DataError> {
        tx.insert_returning_nothing(his_task_to_active_model(his_task))
            .await?;
        Ok(())
    }

    pub async fn select_by_instance_id(
        ctx: &DataContext<'_>,
        instance_id: i64,
    ) -> Result<Vec<HisTask>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .filter(Column::InstanceId.eq(instance_id))
                    .order_by(Column::CreateTime, Order::Asc),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(HisTask::from).collect())
    }

    pub async fn select_done_page(
        ctx: &DataContext<'_>,
        handler: &str,
        filter: &TaskPageFilter,
        page_no: u64,
        page_size: u64,
    ) -> Result<app_data::Page<HisTask>, DataError> {
        let select = Entity::find()
            .filter(Column::Approver.eq(handler))
            .order_by(Column::CreateTime, Order::Desc);
        let select = apply_his_task_filters(select, filter);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let rows: Vec<HisTask> = page.rows.iter().map(HisTask::from).collect();
        Ok(page.with_rows(rows))
    }

    /// 管理员视角：所有已办任务分页
    pub async fn select_all_done_page(
        ctx: &DataContext<'_>,
        filter: &TaskPageFilter,
        page_no: u64,
        page_size: u64,
    ) -> Result<app_data::Page<HisTask>, DataError> {
        let select = Entity::find().order_by(Column::CreateTime, Order::Desc);
        let select = apply_his_task_filters(select, filter);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let rows: Vec<HisTask> = page.rows.iter().map(HisTask::from).collect();
        Ok(page.with_rows(rows))
    }

    /// 根据 target_node_code 查询历史记录
    pub async fn select_by_target_node_code(
        ctx: &DataContext<'_>,
        instance_id: i64,
        target_code: &str,
    ) -> Result<Vec<HisTask>, DataError> {
        let models = ctx
            .select(
                Entity::find()
                    .filter(Column::InstanceId.eq(instance_id))
                    .filter(Column::TargetNodeCode.contains(target_code))
                    .filter(Column::SkipType.eq("PASS"))
                    .order_by(Column::CreateTime, Order::Desc),
            )
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(HisTask::from).collect())
    }

    /// reject_last 用(连接路径:守卫提升事务外):找 instance 中
    /// 最近一次 PASS 历史,其 target_node_code 包含指定节点,返回该历史
    /// 记录的 node_code(即上一个节点)。
    pub async fn select_prev_node_code(
        ctx: &DataContext<'_>,
        instance_id: i64,
        current_node_code: &str,
    ) -> Result<Option<String>, DataError> {
        let model = ctx
            .select(
                Entity::find()
                    .filter(Column::InstanceId.eq(instance_id))
                    .filter(Column::TargetNodeCode.contains(current_node_code))
                    .filter(Column::SkipType.eq("PASS"))
                    .order_by(Column::CreateTime, Order::Desc),
            )
            .without_row_scope()
            .one()
            .await?;
        Ok(model.and_then(|m| m.node_code))
    }

    /// task_back 用(连接路径:守卫提升事务外):找指定办理人在
    /// instance 中最近一次 PASS 历史的 node_code。
    pub async fn select_handler_last_pass_node(
        ctx: &DataContext<'_>,
        instance_id: i64,
        handler: &str,
    ) -> Result<Option<String>, DataError> {
        let model = ctx
            .select(
                Entity::find()
                    .filter(Column::InstanceId.eq(instance_id))
                    .filter(Column::Approver.eq(handler))
                    .filter(Column::SkipType.eq("PASS"))
                    .order_by(Column::CreateTime, Order::Desc),
            )
            .without_row_scope()
            .one()
            .await?;
        Ok(model.and_then(|m| m.node_code))
    }

    /// 会签/票签用(事务路径)：统计指定 task_id 已通过(PASS)的历史记录数。
    pub async fn count_pass_by_task_id_in_txn(
        tx: &TxnContext<'_>,
        task_id: i64,
    ) -> Result<u64, DataError> {
        tx.select(
            Entity::find()
                .filter(Column::TaskId.eq(task_id))
                .filter(Column::SkipType.eq("PASS")),
        )
        .without_row_scope()
        .count()
        .await
    }

    /// 会签/票签用(事务路径)：统计指定 task_id 已驳回(REJECT)的历史记录数
    pub async fn count_reject_by_task_id_in_txn(
        tx: &TxnContext<'_>,
        task_id: i64,
    ) -> Result<u64, DataError> {
        tx.select(
            Entity::find()
                .filter(Column::TaskId.eq(task_id))
                .filter(Column::SkipType.eq("REJECT")),
        )
        .without_row_scope()
        .count()
        .await
    }

    /// depute 用：查找 task_id 对应的委派历史(工具面:生产零调用,保留对位)
    pub async fn select_depute_his_by_task_id(
        ctx: &DataContext<'_>,
        task_id: i64,
    ) -> Result<Option<HisTask>, DataError> {
        let model = ctx
            .select(
                Entity::find()
                    .filter(Column::TaskId.eq(task_id))
                    .filter(Column::CooperateType.eq(3_i8)), // CooperateType::Depute
            )
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(HisTask::from))
    }

    /// depute 回溯用(事务路径):委派历史,取 approver 恢复委托人。
    pub async fn select_depute_his_by_task_id_in_txn(
        tx: &TxnContext<'_>,
        task_id: i64,
    ) -> Result<Option<HisTask>, DataError> {
        let model = tx
            .select(
                Entity::find()
                    .filter(Column::TaskId.eq(task_id))
                    .filter(Column::CooperateType.eq(3_i8)), // CooperateType::Depute
            )
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(HisTask::from))
    }

    /// 按实例 ID 集逻辑删历史任务(连接路径:实例删除级联)。
    /// **Java `@TableLogic` 平价 —— 逻辑删,非物理删**(裁定见文件头)。
    /// 0 行 → Ok(0)(批量语义无 NotFound 约定);不盖审计(同现状)。
    pub async fn soft_delete_by_instance_ids(
        ctx: &DataContext<'_>,
        instance_ids: &[i64],
    ) -> Result<u64, DataError> {
        ctx.soft_delete_where::<Entity>(
            Condition::all()
                .add(Column::InstanceId.is_in(instance_ids.iter().copied())),
            "1",
        )
        .await
    }
}
