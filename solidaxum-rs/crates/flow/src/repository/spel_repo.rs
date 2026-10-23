//! flow_spel 仓储(豁免实体,零事务)。
//!
//! flow_spel 是豁免实体(policy.rs,warm-flow/ruoyi-workflow Mapper 零
//! @DataPermission):读显式 `without_row_scope()`,错误统一 `DataError`。
//! 软删由 service 走逐行软删 update 惯用法(见 spel_service 文件头
//! 「软删口径」),故本文件无仓储级软删函数。

use app_data::{
    DataContext, DataError,
    sea_orm::{
        ActiveValue::{NotSet, Set},
        ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder, Select,
    },
};

use crate::repository::entity::flow_spel::{self, Column, Entity};

fn apply_filters(
    mut select: Select<Entity>,
    component_name: Option<&str>,
    method_name: Option<&str>,
    status: Option<&str>,
) -> Select<Entity> {
    if let Some(name) = component_name
        && !name.is_empty()
    {
        select = select.filter(Column::ComponentName.contains(name));
    }
    if let Some(name) = method_name
        && !name.is_empty()
    {
        select = select.filter(Column::MethodName.contains(name));
    }
    if let Some(s) = status
        && !s.is_empty()
    {
        select = select.filter(Column::Status.eq(s));
    }
    select.order_by(Column::CreateTime, Order::Desc)
}

fn model_to_active_model(model: &flow_spel::Model) -> flow_spel::ActiveModel {
    flow_spel::ActiveModel {
        id: match model.id {
            0 => NotSet,
            _ => Set(model.id),
        },
        component_name: Set(model.component_name.clone()),
        method_name: Set(model.method_name.clone()),
        method_params: Set(model.method_params.clone()),
        view_spel: Set(model.view_spel.clone()),
        remark: Set(model.remark.clone()),
        status: Set(model.status.clone()),
        del_flag: Set(model.del_flag.clone()),
        create_dept: Set(model.create_dept),
        create_by: Set(model.create_by),
        create_time: Set(model.create_time),
        update_by: Set(model.update_by),
        update_time: Set(model.update_time),
    }
}

/// 按主键查询(存活过滤由漏斗软删接管;原版本手滤 del_flag)。
pub async fn select_by_id(
    ctx: &DataContext<'_>,
    id: i64,
) -> Result<Option<flow_spel::Model>, DataError> {
    ctx.select(Entity::find_by_id(id))
        .without_row_scope()
        .one()
        .await
}

pub async fn select_page(
    ctx: &DataContext<'_>,
    component_name: Option<&str>,
    method_name: Option<&str>,
    status: Option<&str>,
    page_no: u64,
    page_size: u64,
) -> Result<app_data::Page<flow_spel::Model>, DataError> {
    let page = ctx
        .select(apply_filters(
            Entity::find(),
            component_name,
            method_name,
            status,
        ))
        .without_row_scope()
        .page(page_no as i64, page_size as i64)
        .await?;
    Ok(page)
}

pub async fn insert(
    ctx: &DataContext<'_>,
    model: &flow_spel::Model,
) -> Result<(), DataError> {
    ctx.insert_returning_nothing(model_to_active_model(model))
        .await?;
    Ok(())
}

/// 按主键更新。0 行(不存在或已删)→ `DataError::NotFound`(口径同
/// category_repo::update_by_id)。
pub async fn update_by_id(
    ctx: &DataContext<'_>,
    model: &flow_spel::Model,
) -> Result<(), DataError> {
    ctx.update(model_to_active_model(model)).await?;
    Ok(())
}

/// 逐行软删 update 惯用法的连接路径形态(service 已组装好墓碑行;
/// `ctx.update` 的 WHERE = 主键 + 软删存活)。
pub async fn soft_delete_by_id(
    ctx: &DataContext<'_>,
    tombstone: flow_spel::ActiveModel,
) -> Result<(), DataError> {
    ctx.update(tombstone).await?;
    Ok(())
}
