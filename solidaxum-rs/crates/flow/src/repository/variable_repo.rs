//! flow_variable 仓储(豁免实体,零事务)。
//!
//! flow_variable 是变量注册表(参考实现 update_flow_registry.sql),豁免
//! 实体(policy.rs):读显式 `without_row_scope()`,错误统一 `DataError`。
//! **无 del_flag,删除一律物理删** `delete_where`(0 行 → Ok(0),
//! 批量语义无 NotFound 约定,口径同 leave_repo::delete_by_ids)。
//! 唯一键 uk_flow_variable_name 由漏斗 insert 的结构化判定落成
//! `DataError::Conflict`(service 层再翻译为友好文案)。

use app_data::{
    DataContext, DataError,
    sea_orm::{
        ActiveValue::{NotSet, Set},
        ColumnTrait, Condition, EntityTrait, Order, QueryFilter, QueryOrder,
        Select,
    },
};

use crate::repository::entity::flow_variable::{self, Column, Entity};

fn apply_filters(
    mut select: Select<Entity>,
    variable_name: Option<&str>,
    label: Option<&str>,
    status: Option<&str>,
) -> Select<Entity> {
    if let Some(name) = variable_name
        && !name.is_empty()
    {
        select = select.filter(Column::VariableName.contains(name));
    }
    if let Some(l) = label
        && !l.is_empty()
    {
        select = select.filter(Column::Label.contains(l));
    }
    if let Some(s) = status
        && !s.is_empty()
    {
        select = select.filter(Column::Status.eq(s));
    }
    select.order_by(Column::CreateTime, Order::Desc)
}

fn model_to_active_model(
    model: &flow_variable::Model,
) -> flow_variable::ActiveModel {
    flow_variable::ActiveModel {
        id: match model.id {
            0 => NotSet,
            _ => Set(model.id),
        },
        variable_name: Set(model.variable_name.clone()),
        label: Set(model.label.clone()),
        data_type: Set(model.data_type.clone()),
        source: Set(model.source.clone()),
        dict_type: Set(model.dict_type.clone()),
        remark: Set(model.remark.clone()),
        status: Set(model.status.clone()),
        create_dept: Set(model.create_dept),
        create_by: Set(model.create_by),
        create_time: Set(model.create_time),
        update_by: Set(model.update_by),
        update_time: Set(model.update_time),
    }
}

/// 按主键查询。
pub async fn select_by_id(
    ctx: &DataContext<'_>,
    id: i64,
) -> Result<Option<flow_variable::Model>, DataError> {
    ctx.select(Entity::find_by_id(id))
        .without_row_scope()
        .one()
        .await
}

/// 按变量名等值查询(唯一键),add/edit 的唯一性预校验用。
pub async fn select_by_variable_name(
    ctx: &DataContext<'_>,
    variable_name: &str,
) -> Result<Option<flow_variable::Model>, DataError> {
    ctx.select(Entity::find().filter(Column::VariableName.eq(variable_name)))
        .without_row_scope()
        .one()
        .await
}

pub async fn select_page(
    ctx: &DataContext<'_>,
    variable_name: Option<&str>,
    label: Option<&str>,
    status: Option<&str>,
    page_no: u64,
    page_size: u64,
) -> Result<app_data::Page<flow_variable::Model>, DataError> {
    let page = ctx
        .select(apply_filters(Entity::find(), variable_name, label, status))
        .without_row_scope()
        .page(page_no as i64, page_size as i64)
        .await?;
    Ok(page)
}

pub async fn insert(
    ctx: &DataContext<'_>,
    model: &flow_variable::Model,
) -> Result<(), DataError> {
    ctx.insert_returning_nothing(model_to_active_model(model))
        .await?;
    Ok(())
}

/// 按主键更新。0 行(不存在)→ `DataError::NotFound`(口径同
/// spel_repo::update_by_id)。
pub async fn update_by_id(
    ctx: &DataContext<'_>,
    model: &flow_variable::Model,
) -> Result<(), DataError> {
    ctx.update(model_to_active_model(model)).await?;
    Ok(())
}

/// 按主键集合物理删除(无 del_flag,行彻底移除)。0 行 → Ok(0),
/// 批量语义无 NotFound 约定。
pub async fn delete_by_ids(
    ctx: &DataContext<'_>,
    ids: &[i64],
) -> Result<u64, DataError> {
    let cond = ids
        .iter()
        .fold(Condition::any(), |c, id| c.add(Column::Id.eq(*id)));
    ctx.delete_where::<Entity>(cond).await
}
