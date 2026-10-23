//! 流程分类服务 —— category 链经 `ctx.transaction` 单闭包执行。
//!
//! # 事务形状:守卫提升事务外,写半边单闭包
//!
//! 同 definition/instance/task:**守卫(纯读/前置校验)提升事务外,写半边
//! 单闭包** —— 名称唯一性(BadRequest)/父分类存在性(NotFound)/ancestors
//! 计算(读父行)全部在闭包外以原文案抛出,闭包内只剩单条 insert/update
//! 的 DataError。
//!
//! # 软删口径(delete)
//!
//! Java 锚 `FlwCategoryServiceImpl.java:283`(deleteWithValidByIds)对
//! 0 行**静默返回**:不存在/已删的 id 不报错,delete/spel 两族口径统一为
//! Java 静默。形态:tombstone 构造(`del_flag='1'` + update_by +
//! update_time,审计列随 ActiveModel 携带 —— 漏斗 `update_where` 不盖审计),
//! 落库 `tx.update_where(CategoryId.is_in(ids), tombstone)` 单语句:
//! WHERE = id 条件 AND 软删存活 → 已删行不重复计数、不被二次盖写;
//! 不存在/已删 id 静默 Ok(0)。`has_children` 守卫保留(移植期既有行为)。

use app_core::{
    error::{AppError, AppResult},
    middleware::extractors::CurrentUser,
    util::snowflake,
};
use app_data::sea_orm::{ActiveValue::Set, ColumnTrait, Condition};

use crate::{
    model::dto::{CategoryAddBo, CategoryEditBo},
    repository::{category_repo, entity::flow_category},
};

pub const ROOT_PARENT_ID: i64 = 0;
/// 根节点的祖级列表（对齐 Java：ancestors="0"）
pub const ROOT_ANCESTORS: &str = "0";

/// 执行面全部经请求期 `DataContext`,不再持有数据库连接。
#[derive(Clone, Copy, Default)]
pub struct CategoryService;

impl CategoryService {
    pub fn new() -> Self {
        Self
    }

    fn now() -> chrono::NaiveDateTime {
        chrono::Local::now().naive_local()
    }

    /// 根据父分类计算 ancestors（对齐 Java buildAncestors）：
    ///   根节点 → "0"；非根 → 父ancestors + "," + parent_id
    async fn build_ancestors(
        ctx: &app_data::DataContext<'_>,
        parent_id: i64,
    ) -> AppResult<String> {
        if parent_id == ROOT_PARENT_ID {
            return Ok(ROOT_ANCESTORS.to_string());
        }
        let parent = category_repo::select_by_id(ctx, parent_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("父分类不存在 (id={})", parent_id))
            })?;
        Ok(format!(
            "{},{}",
            parent.ancestors.unwrap_or_default(),
            parent_id
        ))
    }

    pub async fn list_page(
        &self,
        ctx: &app_data::DataContext<'_>,
        category_name: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<flow_category::Model>> {
        Ok(
            category_repo::select_page(ctx, category_name, page_no, page_size)
                .await?,
        )
    }

    pub async fn list_all(
        &self,
        ctx: &app_data::DataContext<'_>,
    ) -> AppResult<Vec<flow_category::Model>> {
        Ok(category_repo::select_all(ctx).await?)
    }

    pub async fn list_all_filtered(
        &self,
        ctx: &app_data::DataContext<'_>,
        category_name: Option<&str>,
    ) -> AppResult<Vec<flow_category::Model>> {
        Ok(category_repo::select_all_filtered(ctx, category_name).await?)
    }

    /// categoryId → category_name 映射（对齐 Java 语义：flow_definition.category 存的是 categoryId）
    pub async fn id_to_name_map(
        &self,
        ctx: &app_data::DataContext<'_>,
    ) -> AppResult<std::collections::HashMap<i64, String>> {
        let all = self.list_all(ctx).await?;
        let map = all
            .into_iter()
            .map(|c| (c.category_id, c.category_name))
            .collect();
        Ok(map)
    }

    pub async fn get_by_id(
        &self,
        ctx: &app_data::DataContext<'_>,
        category_id: i64,
    ) -> AppResult<Option<flow_category::Model>> {
        Ok(category_repo::select_by_id(ctx, category_id).await?)
    }

    pub async fn add(
        &self,
        ctx: &app_data::DataContext<'_>,
        bo: CategoryAddBo,
        user: &CurrentUser,
    ) -> AppResult<flow_category::Model> {
        // 守卫提升(纯读,见文件头)
        // 名称唯一性校验（对齐 Java checkCategoryNameUnique）
        if category_repo::select_by_name(ctx, &bo.category_name)
            .await?
            .is_some()
        {
            return Err(AppError::BadRequest(format!(
                "分类名称已存在: {}",
                bo.category_name
            )));
        }

        if bo.parent_id != ROOT_PARENT_ID
            && category_repo::select_by_id(ctx, bo.parent_id)
                .await?
                .is_none()
        {
            return Err(AppError::NotFound(format!(
                "父分类不存在 (id={})",
                bo.parent_id
            )));
        }

        let ancestors = Self::build_ancestors(ctx, bo.parent_id).await?;
        let now = Self::now();
        let user_id = user.get_user_id();
        let model = flow_category::Model {
            category_id: snowflake::next_id(),
            parent_id: bo.parent_id,
            ancestors: Some(ancestors),
            category_name: bo.category_name,
            order_num: bo.order_num,
            del_flag: Some("0".to_string()),
            create_dept: None,
            create_by: Some(user_id),
            create_time: Some(now),
            update_by: Some(user_id),
            update_time: Some(now),
        };

        // 写半边单闭包
        ctx.transaction(|tx| {
            let model = model.clone();
            Box::pin(async move {
                category_repo::insert_in_txn(tx, &model).await?;
                Ok(())
            })
        })
        .await?;
        Ok(model)
    }

    pub async fn edit(
        &self,
        ctx: &app_data::DataContext<'_>,
        bo: CategoryEditBo,
        user: &CurrentUser,
    ) -> AppResult<flow_category::Model> {
        // 守卫提升(纯读,见文件头)
        let mut model = category_repo::select_by_id(ctx, bo.category_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "分类不存在 (id={})",
                    bo.category_id
                ))
            })?;

        // 名称唯一性校验（排除自身）
        if let Some(existing) =
            category_repo::select_by_name(ctx, &bo.category_name).await?
            && existing.category_id != bo.category_id
        {
            return Err(AppError::BadRequest(format!(
                "分类名称已存在: {}",
                bo.category_name
            )));
        }

        if bo.parent_id != ROOT_PARENT_ID
            && bo.parent_id != model.parent_id
            && category_repo::select_by_id(ctx, bo.parent_id)
                .await?
                .is_none()
        {
            return Err(AppError::NotFound(format!(
                "父分类不存在 (id={})",
                bo.parent_id
            )));
        }

        // 父分类变更时重算 ancestors
        if bo.parent_id != model.parent_id {
            model.ancestors =
                Some(Self::build_ancestors(ctx, bo.parent_id).await?);
        }
        model.parent_id = bo.parent_id;
        model.category_name = bo.category_name;
        model.order_num = bo.order_num;
        model.update_time = Some(Self::now());
        model.update_by = Some(user.get_user_id());

        // 写半边单闭包
        ctx.transaction(|tx| {
            let model = model.clone();
            Box::pin(async move {
                category_repo::update_by_id_in_txn(tx, &model).await?;
                Ok(())
            })
        })
        .await?;
        Ok(model)
    }

    pub async fn delete(
        &self,
        ctx: &app_data::DataContext<'_>,
        category_id: i64,
        user: &CurrentUser,
    ) -> AppResult<()> {
        // 守卫保留:存在子分类拒绝删除(移植期既有行为)
        if category_repo::has_children(ctx, category_id).await? {
            return Err(AppError::BadRequest(
                "存在子分类，不能删除".to_string(),
            ));
        }

        // tombstone 构造保留:审计列随 ActiveModel 携带(`update_where`
        // 不盖审计,Set 什么落什么);落库改单语句批量软删 —— WHERE =
        // id 条件 AND 软删存活 → 不存在/已删 id 静默 Ok(0)(Java
        // `FlwCategoryServiceImpl.java:283` 平价,两族口径统一见文件头)。
        // 返回计数忽略(Java 侧同为 void 语义)。
        let now = Self::now();
        let update_by = user.get_user_id();
        ctx.transaction(|tx| {
            Box::pin(async move {
                tx.update_where::<flow_category::Entity>(
                    Condition::all().add(
                        flow_category::Column::CategoryId.is_in([category_id]),
                    ),
                    flow_category::ActiveModel {
                        del_flag: Set(Some("1".to_string())),
                        update_by: Set(Some(update_by)),
                        update_time: Set(Some(now)),
                        ..Default::default()
                    },
                )
                .await?;
                Ok(())
            })
        })
        .await?;
        Ok(())
    }
}
