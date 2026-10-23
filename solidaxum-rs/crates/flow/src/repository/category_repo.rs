//! flow_category 仓储(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:
//! ruoyi-workflow `FlwCategoryMapper.java` **零 `@DataPermission`**(接口体
//! 仅两个 lambda default 方法,BaseMapperPlus 基方法亦无注解;对照组 grep
//! 在 ruoyi-system SysUserMapper 命中 8 处,证明 grep 有效)。统一锚:
//! **ruoyi-workflow FlwCategoryMapper 零 @DataPermission —— 流程分类是
//! 全局功能定义,按树共享而非按部门,数据权限过滤对它无业务含义**。由此
//! 每个读方法显式 `without_row_scope()`(豁免必须写出这行字才能过审);
//! flow_category 的 DataPolicy 见 `crate::policy`(CONSTRAINED=false、
//! soft_delete=(DelFlag,"0")),漏斗对读接管软删过滤。
//!
//! | 本文件方法 | Java 锚(FlwCategoryController → FlwCategoryServiceImpl → FlwCategoryMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_id` | getInfo(Controller:74)→ queryById(ServiceImpl:50)→ `categoryMapper.selectVoById` | ❌ | `without_row_scope()`(软删存活由漏斗滤) |
//! | `select_category_ids_by_parent_id` | deleteWithValidByIds 守卫等(ServiceImpl:283)→ `selectCategoryIdsByParentId`(Mapper:38,内联 `selectListByParentId`:28 的 findInSet) | ❌ | `without_row_scope()`(全量读后内存展开) |
//! | `select_by_name` | checkCategoryNameUnique(ServiceImpl:149)→ lambda `eq(categoryName)` | ❌ | `without_row_scope()`(唯一性须全量) |
//! | `select_all` | selectCategoryTreeList(ServiceImpl:110)/queryCategory(:133)→ lambda 全量 | ❌ | `without_row_scope()` |
//! | `select_all_filtered` | list/export(Controller:47/:61)→ queryList(ServiceImpl:98)→ `selectVoList(lqw)` | ❌ | `without_row_scope()` |
//! | `select_page` | Java list 无分页(Controller:47 全量返回),分页为移植侧形态;同走 queryList 零注解链 | ❌ | 逃生舱 `page` + `without_row_scope()` |
//! | `insert` / `insert_in_txn` | add(Controller:88)→ insertByBo(ServiceImpl:208)→ `categoryMapper.insert` | — | `ctx/tx.insert`(B 组审计列类型可盖章但裁定 None,policy.rs 头注;防半盖章分裂) |
//! | `update_by_id` / `update_by_id_in_txn` | edit(Controller:105)→ updateByBo(ServiceImpl:227)→ `updateById` | — | `ctx/tx.update`(豁免实体无行级范围;0 行 → NotFound → 404) |
//! | `has_children` | hasChildByCategoryId(ServiceImpl:178)→ lambda `exists()` | ❌ | 逃生舱 `count()` 原语 |
//!
//! 软删不在此层:`category_service` 的删除走单语句批量软删
//! (`tx.update_where(CategoryId.is_in(ids), tombstone)`,tombstone 的
//! ActiveModel 携带 del_flag='1' + update_by + update_time —— 恰是
//! `update_where` 不盖审计、审计列随 ActiveModel 携带的惯用法),
//! 0 行静默 Ok(0)(Java `FlwCategoryServiceImpl.java:283` 平价),
//! 故本文件无仓储级软删函数。

use std::collections::HashMap;

use app_data::{
    DataContext, DataError, TxnContext,
    sea_orm::{
        ActiveValue::{NotSet, Set},
        ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder, Select,
    },
};

use crate::repository::entity::flow_category::{self, Column, Entity};

/// 展开分类 id:返回 `parent_id` 及其所有子孙分类 id(纯函数)。
/// 对齐 Java `FlwCategoryMapper.selectCategoryIdsByParentId`(按父子关系展开)。
fn collect_category_ids(
    all: &[flow_category::Model],
    parent_id: i64,
) -> Vec<i64> {
    let mut children_map: HashMap<i64, Vec<i64>> = HashMap::new();
    for category in all {
        children_map
            .entry(category.parent_id)
            .or_default()
            .push(category.category_id);
    }

    fn collect(map: &HashMap<i64, Vec<i64>>, parent: i64, out: &mut Vec<i64>) {
        if let Some(children) = map.get(&parent) {
            for &child in children {
                if !out.contains(&child) {
                    out.push(child);
                    collect(map, child, out);
                }
            }
        }
    }

    let mut ids = vec![parent_id];
    collect(&children_map, parent_id, &mut ids);
    ids
}

fn apply_filters(
    mut select: Select<Entity>,
    category_name: Option<&str>,
) -> Select<Entity> {
    if let Some(name) = category_name
        && !name.is_empty()
    {
        select = select.filter(Column::CategoryName.contains(name));
    }
    select.order_by(Column::OrderNum, Order::Asc)
}

fn model_to_active_model(
    model: &flow_category::Model,
) -> flow_category::ActiveModel {
    flow_category::ActiveModel {
        category_id: match model.category_id {
            0 => NotSet,
            _ => Set(model.category_id),
        },
        parent_id: Set(model.parent_id),
        ancestors: Set(model.ancestors.clone()),
        category_name: Set(model.category_name.clone()),
        order_num: Set(model.order_num),
        del_flag: Set(model.del_flag.clone()),
        create_dept: Set(model.create_dept),
        create_by: Set(model.create_by),
        create_time: Set(model.create_time),
        update_by: Set(model.update_by),
        update_time: Set(model.update_time),
    }
}

// ─── canonical(方法级锚与漏斗路径见文件头审计表)───

/// 按主键查询分类(存活过滤由漏斗软删接管)。
pub async fn select_by_id(
    ctx: &DataContext<'_>,
    category_id: i64,
) -> Result<Option<flow_category::Model>, DataError> {
    ctx.select(Entity::find_by_id(category_id))
        .without_row_scope()
        .one()
        .await
}

/// 展开分类 id:返回 `parent_id` 及其所有子孙分类 id。
/// 对齐 Java `FlwCategoryMapper.selectCategoryIdsByParentId`(按 ancestors 找后代)。
pub async fn select_category_ids_by_parent_id(
    ctx: &DataContext<'_>,
    parent_id: i64,
) -> Result<Vec<i64>, DataError> {
    let all = ctx
        .select(Entity::find().order_by(Column::OrderNum, Order::Asc))
        .without_row_scope()
        .all()
        .await?;
    Ok(collect_category_ids(&all, parent_id))
}

/// 按分类名称查询（对齐 Java checkCategoryNameUnique，唯一性校验用 name）
pub async fn select_by_name(
    ctx: &DataContext<'_>,
    name: &str,
) -> Result<Option<flow_category::Model>, DataError> {
    ctx.select(Entity::find().filter(Column::CategoryName.eq(name)))
        .without_row_scope()
        .one()
        .await
}

pub async fn select_all(
    ctx: &DataContext<'_>,
) -> Result<Vec<flow_category::Model>, DataError> {
    ctx.select(Entity::find().order_by(Column::OrderNum, Order::Asc))
        .without_row_scope()
        .all()
        .await
}

pub async fn select_all_filtered(
    ctx: &DataContext<'_>,
    category_name: Option<&str>,
) -> Result<Vec<flow_category::Model>, DataError> {
    ctx.select(apply_filters(Entity::find(), category_name))
        .without_row_scope()
        .all()
        .await
}

pub async fn select_page(
    ctx: &DataContext<'_>,
    category_name: Option<&str>,
    page_no: u64,
    page_size: u64,
) -> Result<app_data::Page<flow_category::Model>, DataError> {
    let page = ctx
        .select(apply_filters(Entity::find(), category_name))
        .without_row_scope()
        .page(page_no as i64, page_size as i64)
        .await?;
    Ok(page)
}

pub async fn insert(
    ctx: &DataContext<'_>,
    model: &flow_category::Model,
) -> Result<(), DataError> {
    ctx.insert_returning_nothing(model_to_active_model(model))
        .await?;
    Ok(())
}

/// [`insert`] 的事务内变体(category_service 写闭包内)。
pub async fn insert_in_txn(
    tx: &TxnContext<'_>,
    model: &flow_category::Model,
) -> Result<(), DataError> {
    tx.insert_returning_nothing(model_to_active_model(model))
        .await?;
    Ok(())
}

/// 按主键更新。漏斗 update 的 WHERE = 主键 + 软删存活,0 行(不存在或
/// 已删)→ `DataError::NotFound`(404;更新已删分类不会静默复活)。
pub async fn update_by_id(
    ctx: &DataContext<'_>,
    model: &flow_category::Model,
) -> Result<(), DataError> {
    ctx.update(model_to_active_model(model)).await?;
    Ok(())
}

/// [`update_by_id`] 的事务内变体(category_service 写闭包内),
/// 0 行语义同上。
pub async fn update_by_id_in_txn(
    tx: &TxnContext<'_>,
    model: &flow_category::Model,
) -> Result<(), DataError> {
    tx.update(model_to_active_model(model)).await?;
    Ok(())
}

/// 是否存在子分类。
pub async fn has_children(
    ctx: &DataContext<'_>,
    parent_id: i64,
) -> Result<bool, DataError> {
    let n = ctx
        .select(Entity::find().filter(Column::ParentId.eq(parent_id)))
        .without_row_scope()
        .count()
        .await?;
    Ok(n > 0)
}
