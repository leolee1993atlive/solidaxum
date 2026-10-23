//! 流程变量注册表服务 —— variable 链经 app-data 漏斗执行(零事务)。
//!
//! 形状照 spel_service:方法收 `ctx: &DataContext<'_>`,不持有数据库连接。
//!
//! # 删除口径
//!
//! flow_variable **无 del_flag**(参考实现 DDL 锚)→ 物理删
//! `variable_repo::delete_by_ids`,删不存在的 id 静默 Ok(0 行,
//! 批量语义无 NotFound 约定,同 spel 软删的静默口径)。
//!
//! # 唯一性
//!
//! uk_flow_variable_name:预校验(等值查询)+ 漏斗结构化判定
//! (`DataError::Conflict`)双保险,冲突统一翻译为
//! `AppError::Conflict("变量名已存在:xxx")`。

use app_core::{
    error::{AppError, AppResult},
    middleware::extractors::CurrentUser,
    util::snowflake,
};

use crate::{
    model::dto::{VariableAddBo, VariableEditBo},
    repository::{entity::flow_variable, variable_repo},
};

/// data_type 枚举锚(DDL 注释 String/Long/Boolean)。
const DATA_TYPES: [&str; 3] = ["String", "Long", "Boolean"];

/// 执行面全部经请求期 `DataContext`,不再持有数据库连接。
#[derive(Clone, Copy, Default)]
pub struct VariableService;

impl VariableService {
    pub fn new() -> Self {
        Self
    }

    fn now() -> chrono::NaiveDateTime {
        chrono::Local::now().naive_local()
    }

    fn check_data_type(data_type: &str) -> AppResult<()> {
        if DATA_TYPES.contains(&data_type) {
            Ok(())
        } else {
            Err(AppError::BadRequest(format!(
                "不支持的数据类型: {data_type}(仅支持 String/Long/Boolean)"
            )))
        }
    }

    pub async fn list_page(
        &self,
        ctx: &app_data::DataContext<'_>,
        variable_name: Option<&str>,
        label: Option<&str>,
        status: Option<&str>,
        page_no: u64,
        page_size: u64,
    ) -> AppResult<app_data::Page<flow_variable::Model>> {
        Ok(variable_repo::select_page(
            ctx,
            variable_name,
            label,
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
    ) -> AppResult<Option<flow_variable::Model>> {
        Ok(variable_repo::select_by_id(ctx, id).await?)
    }

    pub async fn add(
        &self,
        ctx: &app_data::DataContext<'_>,
        bo: VariableAddBo,
        user: &CurrentUser,
    ) -> AppResult<flow_variable::Model> {
        Self::check_data_type(&bo.data_type)?;
        if variable_repo::select_by_variable_name(ctx, &bo.variable_name)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(format!(
                "变量名已存在: {}",
                bo.variable_name
            )));
        }
        let now = Self::now();
        let user_id = user.get_user_id();
        let model = flow_variable::Model {
            id: snowflake::next_id(),
            variable_name: bo.variable_name,
            label: bo.label,
            data_type: bo.data_type,
            source: bo.source,
            dict_type: bo.dict_type,
            remark: bo.remark,
            status: bo.status.unwrap_or_else(|| "0".to_string()),
            create_dept: None,
            create_by: Some(user_id),
            create_time: Some(now),
            update_by: Some(user_id),
            update_time: Some(now),
        };

        variable_repo::insert(ctx, &model)
            .await
            .map_err(|e| match e {
                app_data::DataError::Conflict => AppError::Conflict(format!(
                    "变量名已存在: {}",
                    model.variable_name
                )),
                other => other.into(),
            })?;
        Ok(model)
    }

    pub async fn edit(
        &self,
        ctx: &app_data::DataContext<'_>,
        bo: VariableEditBo,
        user: &CurrentUser,
    ) -> AppResult<flow_variable::Model> {
        Self::check_data_type(&bo.data_type)?;
        let mut model =
            variable_repo::select_by_id(ctx, bo.id).await?.ok_or_else(
                || AppError::NotFound(format!("流程变量不存在 (id={})", bo.id)),
            )?;

        // 改名撞唯一键 → 友好冲突(排除自身)
        if model.variable_name != bo.variable_name
            && let Some(exist) =
                variable_repo::select_by_variable_name(ctx, &bo.variable_name)
                    .await?
            && exist.id != bo.id
        {
            return Err(AppError::Conflict(format!(
                "变量名已存在: {}",
                bo.variable_name
            )));
        }

        model.variable_name = bo.variable_name;
        model.label = bo.label;
        model.data_type = bo.data_type;
        model.source = bo.source;
        model.dict_type = bo.dict_type;
        model.remark = bo.remark;
        model.status = bo.status.unwrap_or_else(|| model.status.clone());
        model.update_time = Some(Self::now());
        model.update_by = Some(user.get_user_id());

        variable_repo::update_by_id(ctx, &model).await.map_err(
            |e| match e {
                app_data::DataError::Conflict => AppError::Conflict(format!(
                    "变量名已存在: {}",
                    model.variable_name
                )),
                other => other.into(),
            },
        )?;
        Ok(model)
    }

    pub async fn delete(
        &self,
        ctx: &app_data::DataContext<'_>,
        ids: &[i64],
    ) -> AppResult<()> {
        // 物理删;不存在的 id 静默 Ok(见文件头「删除口径」)
        variable_repo::delete_by_ids(ctx, ids).await?;
        Ok(())
    }
}
