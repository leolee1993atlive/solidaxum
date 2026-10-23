//! flow_instance 仓储(**全豁免模块**)。
//!
//! canonical 变体:连接路径收 `ctx: &DataContext<'_>`、事务路径收
//! `tx: &TxnContext<'_>`(`_in_txn` 后缀,dept/definition_repo 先例),
//! 错误统一 `DataError`。统一锚:**warm-flow/ruoyi-workflow Mapper 零
//! @DataPermission —— 工作流数据非行级**(普查,warm-flow 全仓
//! grep 计数 = 0),每个读方法显式 `without_row_scope()`;DataPolicy 见
//! `crate::policy`(CONSTRAINED=false、soft_delete=(DelFlag,"0")、
//! audit_columns=None —— A 组 create_by/update_by 是 `Option<String>`、
//! 时间列 `Option<DateTime>`,与漏斗盖章签名不匹配,审计由 service 手工盖,
//! 同 Java DataFillHandler 之前的业务显式 set)。
//!
//! | 本文件方法 | Java 锚 | 漏斗路径 |
//! |---|---|---|
//! | `select_by_id` | warm-flow `InsService` 实例加载 / ruoyi-workflow `FlwInstanceMapper.selectById` | 逃生舱 `one` + `without_row_scope()` |
//! | `select_by_business_id` | ruoyi-workflow `FlwInstanceServiceImpl` 按 businessId 查实例 | 同上 |
//! | `select_page` / `select_running_page` / `select_finish_page` / `select_current_page` | `FlwInstanceServiceImpl` 各分页(runnging/finish/currentList) | 逃生舱 `page` + `without_row_scope()`(ORDER BY 留构件,设计 6.4);category 展开仍走 category_repo |
//! | `insert_in_txn` | 流程启动落库(start 写闭包内) | `tx.insert`(A 组审计列漏斗不盖,service 手工盖) |
//! | `update_by_id` / `update_by_id_in_txn` | warm-flow `InsService` 状态回写 | `ctx/tx.update`(0 行 → `DataError::NotFound`,见下) |
//! | `soft_delete_by_ids` / `soft_delete_by_business_ids` | `FlowInstance.java` `@TableLogic(value="0", delval="1")` + `FlwInstanceServiceImpl.deleteByIds` | `ctx.soft_delete_where`(0 行 → Ok(0),批量语义无 NotFound 约定) |
//!
//! **软删口径**:`soft_delete_where` 的 WHERE = cond AND 存活
//! (`DelFlag = '0'`)—— 重删已删实例返回 0
//! (tests/instance_repo_test.rs 判例钉住)。不盖审计:update_*
//! 由调用方手工盖(Java 侧同为业务代码显式 set)。
//!
//! **update 0 行语义**:漏斗 `update` 的 WHERE = 主键 + 软删存活(豁免实体
//! 无行级范围)。已删实例对写路径不可见(无存活过滤时 update 已删行会把
//! del_flag 覆盖回去,形同复活);0 行 → `DataError::NotFound`(404)——
//! 触发点都是「读后被并发删除」的竞争窗口。

use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{
        ActiveValue::{NotSet, Set},
        ColumnTrait, Condition, EntityTrait, JoinType, Order, QueryFilter,
        QueryOrder, QuerySelect, RelationTrait, Select,
    },
};
use flow_instance::{Column, Entity};

use crate::{
    enums::{FlowStatus, NodeType},
    model::{dto::InstancePageFilter, instance::Instance},
    repository::{
        category_repo,
        entity::{flow_definition, flow_instance},
    },
};

/// 应用 InstancePageFilter 中的通用过滤条件(canonical:分类展开经
/// `category_repo` canonical 变体,软删存活由逃生舱接管不再手写)。
/// category 会展开为「选中分类 + 所有子孙分类」后再按 flow_definition.category
/// 过滤,对齐 Java `FlwInstanceServiceImpl.buildQueryWrapper`。
async fn apply_instance_filters(
    ctx: &DataContext<'_>,
    mut select: Select<Entity>,
    filter: &InstancePageFilter,
) -> Result<Select<Entity>, DataError> {
    if let Some(bid) = filter.business_id.as_deref().filter(|s| !s.is_empty()) {
        select = select.filter(Column::BusinessId.contains(bid));
    }

    let needs_definition_join = filter
        .flow_code
        .as_deref()
        .filter(|s| !s.is_empty())
        .is_some()
        || filter
            .flow_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .is_some()
        || filter
            .category
            .as_deref()
            .filter(|s| !s.is_empty())
            .is_some();

    if needs_definition_join {
        select = select.join(
            JoinType::InnerJoin,
            flow_instance::Relation::Definition.def(),
        );
    }

    if let Some(code) = filter.flow_code.as_deref().filter(|s| !s.is_empty()) {
        select =
            select.filter(flow_definition::Column::FlowCode.contains(code));
    }
    if let Some(name) = filter.flow_name.as_deref().filter(|s| !s.is_empty()) {
        select =
            select.filter(flow_definition::Column::FlowName.contains(name));
    }

    if let Some(cat) = filter.category.as_deref().filter(|s| !s.is_empty())
        && let Ok(parent_id) = cat.parse::<i64>()
    {
        let category_ids =
            category_repo::select_category_ids_by_parent_id(ctx, parent_id)
                .await?;
        let id_strs: Vec<String> =
            category_ids.iter().map(|id| id.to_string()).collect();
        if !id_strs.is_empty() {
            select =
                select.filter(flow_definition::Column::Category.is_in(id_strs));
        }
    }

    if let Some(name) = filter.node_name.as_deref().filter(|s| !s.is_empty()) {
        select = select.filter(Column::NodeName.contains(name));
    }

    if let Some(ids) = filter.create_by_ids.as_deref().filter(|s| !s.is_empty())
    {
        let ids: Vec<String> = ids
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !ids.is_empty() {
            select = select.filter(Column::CreateBy.is_in(ids));
        }
    }

    Ok(select)
}

fn instance_to_active_model(instance: &Instance) -> flow_instance::ActiveModel {
    flow_instance::ActiveModel {
        id: match instance.id {
            Some(id) => Set(id),
            None => NotSet,
        },
        definition_id: Set(instance.definition_id),
        business_id: Set(instance.business_id.clone()),
        node_type: Set(instance.node_type),
        node_code: Set(instance.node_code.clone()),
        node_name: Set(instance.node_name.clone()),
        variable: Set(instance.variable.clone()),
        flow_status: Set(instance.flow_status.clone()),
        activity_status: Set(instance.activity_status),
        def_json: Set(instance.def_json.clone()),
        create_time: Set(instance.create_time),
        create_by: Set(instance.create_by.clone()),
        update_time: Set(instance.update_time),
        update_by: Set(instance.update_by.clone()),
        ext: Set(instance.ext.clone()),
        del_flag: Set(instance.del_flag.clone()),
        tenant_id: Set(instance.tenant_id.clone()),
    }
}

/// 流程实例数据访问(无状态关联函数;连接路径收 `&DataContext`,事务路径
/// 收 `&TxnContext`,审计手工盖 —— 见文件头审计表)
pub struct InstanceRepository;

impl InstanceRepository {
    pub async fn select_by_id(
        ctx: &DataContext<'_>,
        id: i64,
    ) -> Result<Option<Instance>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Instance::from))
    }

    pub async fn select_by_business_id(
        ctx: &DataContext<'_>,
        business_id: &str,
    ) -> Result<Option<Instance>, DataError> {
        let model = ctx
            .select(Entity::find().filter(Column::BusinessId.eq(business_id)))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Instance::from))
    }

    /// 运行中实例全量(办理人补偿重解析用,参考实现):
    /// `node_type BETWEEN 0 AND 1`(起始节点与第一个审批节点之间)
    /// `AND flow_status IN ('1','13')`(Approval/Pending —— 对齐 Java
    /// `runningStatus` 口径);软删存活由漏斗照施。`instance_id` 有值时
    /// 叠加主键条件(worker 按单实例补偿的入口)。
    pub async fn select_running_for_resync(
        ctx: &DataContext<'_>,
        instance_id: Option<i64>,
    ) -> Result<Vec<Instance>, DataError> {
        let mut select = Entity::find().filter(
            Condition::all()
                .add(Column::NodeType.gte(0))
                .add(Column::NodeType.lte(1))
                .add(Column::FlowStatus.is_in(["1", "13"])),
        );
        if let Some(id) = instance_id {
            select = select.filter(Column::Id.eq(id));
        }
        let models = ctx
            .select(select.order_by(Column::Id, Order::Asc))
            .without_row_scope()
            .all()
            .await?;
        Ok(models.iter().map(Instance::from).collect())
    }

    /// 插入(事务路径:start 的写闭包内)。
    pub async fn insert_in_txn(
        tx: &TxnContext<'_>,
        instance: &Instance,
    ) -> Result<(), DataError> {
        tx.insert_returning_nothing(instance_to_active_model(instance))
            .await?;
        Ok(())
    }

    /// 按主键更新(连接路径:set_active / update_variable)。
    /// 0 行(不存在或已删)→ `DataError::NotFound`(裁定见文件头)。
    pub async fn update_by_id(
        ctx: &DataContext<'_>,
        instance: &Instance,
    ) -> Result<(), DataError> {
        ctx.update(instance_to_active_model(instance)).await?;
        Ok(())
    }

    /// [`Self::update_by_id`] 的事务内变体(revoke / termination /
    /// invalid 的写闭包内;task_service skip 链同形)。
    pub async fn update_by_id_in_txn(
        tx: &TxnContext<'_>,
        instance: &Instance,
    ) -> Result<(), DataError> {
        tx.update(instance_to_active_model(instance)).await?;
        Ok(())
    }

    pub async fn select_page(
        ctx: &DataContext<'_>,
        flow_code: Option<&str>,
        business_id: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> Result<app_data::Page<Instance>, DataError> {
        let mut select = Entity::find();

        if let Some(code) = flow_code
            && !code.is_empty()
        {
            select = select.filter(Column::BusinessId.contains(code));
        }
        if let Some(bid) = business_id
            && !bid.is_empty()
        {
            select = select.filter(Column::BusinessId.contains(bid));
        }

        select = select.order_by(Column::CreateTime, Order::Desc);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let rows: Vec<Instance> =
            page.rows.iter().map(Instance::from).collect();
        Ok(page.with_rows(rows))
    }

    pub async fn select_running_page(
        ctx: &DataContext<'_>,
        filter: &InstancePageFilter,
        page_no: u64,
        page_size: u64,
    ) -> Result<app_data::Page<Instance>, DataError> {
        let mut select = Entity::find().filter(
            Column::FlowStatus.is_not_in(["4", "5", "6", "8", "9", "10"]),
        );

        select = apply_instance_filters(ctx, select, filter).await?;

        select = select.order_by(Column::CreateTime, Order::Desc);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let rows: Vec<Instance> =
            page.rows.iter().map(Instance::from).collect();
        Ok(page.with_rows(rows))
    }

    pub async fn select_finish_page(
        ctx: &DataContext<'_>,
        filter: &InstancePageFilter,
        page_no: u64,
        page_size: u64,
    ) -> Result<app_data::Page<Instance>, DataError> {
        let mut select = Entity::find().filter(
            Condition::any()
                .add(Column::NodeType.eq(NodeType::End as i8))
                .add(Column::FlowStatus.eq(FlowStatus::Finished.as_str())),
        );

        select = apply_instance_filters(ctx, select, filter).await?;

        select = select.order_by(Column::CreateTime, Order::Desc);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let rows: Vec<Instance> =
            page.rows.iter().map(Instance::from).collect();
        Ok(page.with_rows(rows))
    }

    /// 当前用户发起的流程实例分页。
    ///
    /// 对齐 Java `FlwInstanceServiceImpl.selectCurrentInstanceList`:
    /// 仅按 `flow_instance.create_by = user_id` 过滤,不关联 flow_task/flow_user。
    pub async fn select_current_page(
        ctx: &DataContext<'_>,
        user_id: &str,
        filter: &InstancePageFilter,
        page_no: u64,
        page_size: u64,
    ) -> Result<app_data::Page<Instance>, DataError> {
        let mut select = Entity::find().filter(Column::CreateBy.eq(user_id));

        select = apply_instance_filters(ctx, select, filter).await?;

        select = select.order_by(Column::CreateTime, Order::Desc);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let rows: Vec<Instance> =
            page.rows.iter().map(Instance::from).collect();
        Ok(page.with_rows(rows))
    }

    /// 按实例 ID 集逻辑删(`@TableLogic` 平价,见文件头口径说明)。
    /// `soft_delete_where` 自带存活过滤:重删已删实例返回 Ok(0)
    /// (instance_repo_test 判例钉住);不盖审计(同现状)。
    pub async fn soft_delete_by_ids(
        ctx: &DataContext<'_>,
        ids: &[i64],
    ) -> Result<u64, DataError> {
        ctx.soft_delete_where::<Entity>(
            Condition::all().add(Column::Id.is_in(ids.iter().copied())),
            "1",
        )
        .await
    }

    /// 按业务 ID 集逻辑删,语义同 [`Self::soft_delete_by_ids`]。
    pub async fn soft_delete_by_business_ids(
        ctx: &DataContext<'_>,
        business_ids: &[String],
    ) -> Result<u64, DataError> {
        ctx.soft_delete_where::<Entity>(
            Condition::all()
                .add(Column::BusinessId.is_in(business_ids.iter().cloned())),
            "1",
        )
        .await
    }
}
