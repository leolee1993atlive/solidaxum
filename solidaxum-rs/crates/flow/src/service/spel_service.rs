//! 表达式服务 —— spel 链经 app-data 漏斗执行(零事务)。
//!
//! 方法收 `ctx: &DataContext<'_>`(handler 经 FlowState 的 ds 构造用户 ctx;
//! 连接来源即 ds 快照),不持有数据库连接。
//!
//! # 软删口径(delete)
//!
//! 漏斗 `soft_delete_where` 携带不了审计列 → 逐行软删 update 惯用法
//! (sys_job/category 同款):`ActiveModel { id, del_flag='1', update_by,
//! update_time }` + `ctx.update`。删不存在的记录静默 Ok:先 select 探
//! 存在性,缺失即 Ok 返回 —— 不引入新的 NotFound 语义(与 category 的
//! 先例裁定相反,那里按先例收紧,见其文件头)。

use app_core::{
    error::{AppError, AppResult},
    middleware::extractors::CurrentUser,
    util::snowflake,
};
use app_data::sea_orm::ActiveValue::Set;

use crate::{
    model::dto::{SpelAddBo, SpelEditBo},
    repository::{entity::flow_spel, spel_repo},
};

/// 执行面全部经请求期 `DataContext`,不再持有数据库连接。
#[derive(Clone, Copy, Default)]
pub struct SpelService;

impl SpelService {
    pub fn new() -> Self {
        Self
    }

    fn now() -> chrono::NaiveDateTime {
        chrono::Local::now().naive_local()
    }

    pub async fn list_page(
        &self,
        ctx: &app_data::DataContext<'_>,
        component_name: Option<&str>,
        method_name: Option<&str>,
        status: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<flow_spel::Model>> {
        Ok(spel_repo::select_page(
            ctx,
            component_name,
            method_name,
            status,
            page_no,
            page_size,
        )
        .await?)
    }

    pub async fn get_by_id(
        &self,
        ctx: &app_data::DataContext<'_>,
        id: i64,
    ) -> AppResult<Option<flow_spel::Model>> {
        Ok(spel_repo::select_by_id(ctx, id).await?)
    }

    pub async fn add(
        &self,
        ctx: &app_data::DataContext<'_>,
        bo: SpelAddBo,
        user: &CurrentUser,
    ) -> AppResult<flow_spel::Model> {
        let now = Self::now();
        let user_id = user.get_user_id();
        let model = flow_spel::Model {
            id: snowflake::next_id(),
            component_name: bo.component_name,
            method_name: bo.method_name,
            method_params: bo.method_params,
            view_spel: Some(bo.view_spel),
            remark: bo.remark,
            status: bo.status.or_else(|| Some("0".to_string())),
            del_flag: Some("0".to_string()),
            create_dept: None,
            create_by: Some(user_id),
            create_time: Some(now),
            update_by: Some(user_id),
            update_time: Some(now),
        };

        spel_repo::insert(ctx, &model).await?;
        Ok(model)
    }

    pub async fn edit(
        &self,
        ctx: &app_data::DataContext<'_>,
        bo: SpelEditBo,
        user: &CurrentUser,
    ) -> AppResult<flow_spel::Model> {
        let mut model =
            spel_repo::select_by_id(ctx, bo.id).await?.ok_or_else(|| {
                AppError::NotFound(format!("表达式不存在 (id={})", bo.id))
            })?;

        model.component_name = bo.component_name;
        model.method_name = bo.method_name;
        model.method_params = bo.method_params;
        model.view_spel = Some(bo.view_spel);
        model.remark = bo.remark;
        model.status = bo.status.or(model.status);
        model.update_time = Some(Self::now());
        model.update_by = Some(user.get_user_id());

        spel_repo::update_by_id(ctx, &model).await?;
        Ok(model)
    }

    pub async fn delete(
        &self,
        ctx: &app_data::DataContext<'_>,
        id: i64,
        user: &CurrentUser,
    ) -> AppResult<()> {
        // 删不存在的记录静默 Ok(见文件头「软删口径」)
        if spel_repo::select_by_id(ctx, id).await?.is_none() {
            return Ok(());
        }
        let tombstone = flow_spel::ActiveModel {
            id: Set(id),
            del_flag: Set(Some("1".to_string())),
            update_by: Set(Some(user.get_user_id())),
            update_time: Set(Some(Self::now())),
            ..Default::default()
        };
        spel_repo::soft_delete_by_id(ctx, tombstone).await?;
        Ok(())
    }
}
