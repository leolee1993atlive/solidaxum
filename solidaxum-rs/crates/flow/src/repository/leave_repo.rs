//! test_leave 仓储(**豁免模块**)。
//!
//! test_leave 无软删列(delete 走**硬删** `delete_where`,漏斗对豁免且
//! 无软删列实体放行)、审计列类型不匹配(policy.rs A 组裁定)—— 读方法
//! 显式 `without_row_scope()`(锚:warm-flow / ruoyi-workflow Mapper 零
//! @DataPermission,工作流数据非行级),审计由调用方手工盖。
//!
//! | 本文件方法 | 消费点 | 漏斗路径 |
//! |---|---|---|
//! | `select_by_id` | LeaveStatusCallback::get_business_info / leave_service.get_by_id / contract 测试读回 | 逃生舱 `one` + `without_row_scope()` |
//! | `select_by_id_in_txn` | LeaveStatusCallback::write_status(事务路径) | 同上(`tx.select`) |
//! | `select_page` | leave_service.list_page | 逃生舱 `page` + `without_row_scope()` |
//! | `insert_in_txn` | leave_service.add 写闭包 | `tx.insert` |
//! | `update_by_id_in_txn` | LeaveStatusCallback::write_status / leave_service.edit 写闭包 | `tx.update`(0 行 → NotFound) |
//! | `update_by_id` | leave_service.submit_and_start 置 waiting(连接路径) | `ctx.update`(0 行 → NotFound) |
//! | `delete_by_ids` | leave_service.delete | `ctx.delete_where`(硬删,0 行 → Ok(0),返回值被忽略) |

use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{
        ActiveValue::{NotSet, Set},
        ColumnTrait, Condition, EntityTrait, Order, QueryFilter, QueryOrder,
        Select,
    },
};

use crate::repository::entity::test_leave::{self, Column, Entity};

fn apply_filters(
    mut select: Select<Entity>,
    leave_type: Option<&str>,
    status: Option<&str>,
    start_leave_days: Option<i32>,
    end_leave_days: Option<i32>,
) -> Select<Entity> {
    if let Some(t) = leave_type
        && !t.is_empty()
    {
        select = select.filter(Column::LeaveType.contains(t));
    }
    if let Some(s) = status
        && !s.is_empty()
    {
        select = select.filter(Column::Status.eq(s));
    }
    if let Some(start) = start_leave_days {
        select = select.filter(Column::LeaveDays.gte(start));
    }
    if let Some(end) = end_leave_days {
        select = select.filter(Column::LeaveDays.lte(end));
    }
    select.order_by(Column::CreateTime, Order::Desc)
}

fn model_to_active_model(model: &test_leave::Model) -> test_leave::ActiveModel {
    test_leave::ActiveModel {
        id: match model.id {
            0 => NotSet,
            _ => Set(model.id),
        },
        leave_type: Set(model.leave_type.clone()),
        remark: Set(model.remark.clone()),
        status: Set(model.status.clone()),
        apply_code: Set(model.apply_code.clone()),
        start_date: Set(model.start_date),
        end_date: Set(model.end_date),
        leave_days: Set(model.leave_days),
        create_dept: Set(model.create_dept),
        create_time: Set(model.create_time),
        create_by: Set(model.create_by),
        update_time: Set(model.update_time),
        update_by: Set(model.update_by),
    }
}

/// 连接路径按主键查(回调 get_business_info / leave_service 用;豁免实体,
/// 显式豁免行级范围)。
pub async fn select_by_id(
    ctx: &DataContext<'_>,
    id: i64,
) -> Result<Option<test_leave::Model>, DataError> {
    ctx.select(Entity::find_by_id(id))
        .without_row_scope()
        .one()
        .await
}

/// 事务路径按主键查(回调 write_status 用;同事务读写,语义同上)。
pub async fn select_by_id_in_txn(
    tx: &TxnContext<'_>,
    id: i64,
) -> Result<Option<test_leave::Model>, DataError> {
    tx.select(Entity::find_by_id(id))
        .without_row_scope()
        .one()
        .await
}

pub async fn select_page(
    ctx: &DataContext<'_>,
    leave_type: Option<&str>,
    status: Option<&str>,
    start_leave_days: Option<i32>,
    end_leave_days: Option<i32>,
    page_no: u64,
    page_size: u64,
) -> Result<app_data::Page<test_leave::Model>, DataError> {
    let page = ctx
        .select(apply_filters(
            Entity::find(),
            leave_type,
            status,
            start_leave_days,
            end_leave_days,
        ))
        .without_row_scope()
        .page(page_no as i64, page_size as i64)
        .await?;
    Ok(page)
}

/// 同一组筛选下的全量列表(导出用,不分页)。
///
/// 导出**不能**走 `select_page`:`Paging` 把 page_size 钳在 1..=200
/// (page.rs 的不变式 1),所以此前那句 `select_page(.., 1, EXPORT_ROW_LIMIT)`
/// 拿到的从来就只有 **200 行** —— 闸门放行、文件照出,只是内容残缺,
/// 不报错也不告警。
///
/// 行数上限由调用方先经 [`count_by_filters`] + `guard_export_rows` 把住
/// (与 logininfo / operlog / user 三处导出同一形状:先 COUNT 再取数),
/// 所以这里无条件全取是安全的。
pub async fn select_list(
    ctx: &DataContext<'_>,
    leave_type: Option<&str>,
    status: Option<&str>,
    start_leave_days: Option<i32>,
    end_leave_days: Option<i32>,
) -> Result<Vec<test_leave::Model>, DataError> {
    ctx.select(apply_filters(
        Entity::find(),
        leave_type,
        status,
        start_leave_days,
        end_leave_days,
    ))
    .without_row_scope()
    .all()
    .await
}

/// 同一组筛选下的**精确**行数,给导出闸门用。
///
/// 导出闸门不能拿 `select_page` 的 total:那是**上限计数**(默认 1 万),
/// 而闸门线是 5 万 —— 一张 20 万行的表会返回 total=10000、闸门放行,
/// 于是静默导出残缺数据。这正是 `export_leave` 注释里点名要防的那种
/// 「分不清正好这么多还是被截断了」。
///
/// 与 logininfo / operlog / user 三处导出同一形状:先 COUNT 再取数。
pub async fn count_by_filters(
    ctx: &DataContext<'_>,
    leave_type: Option<&str>,
    status: Option<&str>,
    start_leave_days: Option<i32>,
    end_leave_days: Option<i32>,
) -> Result<u64, DataError> {
    ctx.select(apply_filters(
        Entity::find(),
        leave_type,
        status,
        start_leave_days,
        end_leave_days,
    ))
    .without_row_scope()
    .count()
    .await
}

/// 事务路径插入(leave_service.add 写闭包内)。
pub async fn insert_in_txn(
    tx: &TxnContext<'_>,
    model: &test_leave::Model,
) -> Result<(), DataError> {
    tx.insert_returning_nothing(model_to_active_model(model))
        .await?;
    Ok(())
}

/// 连接路径更新(leave_service.submit_and_start 置 waiting)。
/// 0 行 → `DataError::NotFound`(口径同 category_repo::update_by_id)。
pub async fn update_by_id(
    ctx: &DataContext<'_>,
    model: &test_leave::Model,
) -> Result<(), DataError> {
    ctx.update(model_to_active_model(model)).await?;
    Ok(())
}

/// 事务路径按主键更新(回调 write_status / leave_service.edit 用;0 行 →
/// NotFound,调用方先 select 后 update,同事务内不可达)。
pub async fn update_by_id_in_txn(
    tx: &TxnContext<'_>,
    model: &test_leave::Model,
) -> Result<(), DataError> {
    tx.update(model_to_active_model(model)).await?;
    Ok(())
}

/// 硬删(test_leave 无软删列,Java 语义同)。0 行 → Ok(0),批量语义
/// 无 NotFound 约定,与原 delete_many 返回值被忽略一致。
pub async fn delete_by_ids(
    ctx: &DataContext<'_>,
    ids: &[i64],
) -> Result<u64, DataError> {
    ctx.delete_where::<Entity>(
        Condition::all().add(Column::Id.is_in(ids.iter().copied())),
    )
    .await
}
