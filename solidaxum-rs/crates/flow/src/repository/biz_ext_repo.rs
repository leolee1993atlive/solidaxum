//! flow_instance_biz_ext 仓储(**豁免模块**,flow_instance_biz_ext 是 B 组
//! 实体:policy.rs 裁定 audit_columns=None,防"半盖章"分裂,审计由调用方
//! 手工盖)。
//!
//! canonical 变体收 `ctx: &DataContext<'_>`,错误统一 `DataError`。统一锚:
//! **warm-flow/ruoyi-workflow Mapper 零 @DataPermission —— 工作流数据非
//! 行级**(全仓普查),读方法显式 `without_row_scope()`。
//!
//! | 本文件方法 | Java 锚 | 漏斗路径 |
//! |---|---|---|
//! | `select_by_instance_id` | ruoyi-workflow `FlwInstanceServiceImpl` 业务扩展读取 | 逃生舱 `one` + `without_row_scope()` |
//! | `save_or_update_by_instance_id` | `buildFlowInstanceBizExt`(saveOrUpdateByInstanceId) | 读:同上;写:`ctx.update`(0 行 → NotFound,竞态窗口,见 instance_repo 文件头同款裁定)/ `ctx.insert` |

use app_data::{
    DataContext, DataError,
    sea_orm::{
        ActiveValue::{NotSet, Set},
        ColumnTrait, EntityTrait, QueryFilter,
    },
};
use chrono::Local;

use crate::repository::entity::flow_instance_biz_ext::{
    ActiveModel, Column, Entity, Model,
};

/// 按流程实例 ID 查询业务扩展信息。
pub async fn select_by_instance_id(
    ctx: &DataContext<'_>,
    instance_id: i64,
) -> Result<Option<Model>, DataError> {
    ctx.select(Entity::find().filter(Column::InstanceId.eq(instance_id)))
        .without_row_scope()
        .one()
        .await
}

/// 按流程实例 ID 保存或更新业务扩展信息(对齐 Java saveOrUpdateByInstanceId)。
/// `instance_id` 缺失报 `DataError::Unsupported`(500)。
pub async fn save_or_update_by_instance_id(
    ctx: &DataContext<'_>,
    model: &Model,
) -> Result<(), DataError> {
    let instance_id = model
        .instance_id
        .ok_or_else(|| DataError::Unsupported("instance_id 不能为空".into()))?;
    let now = Local::now().naive_local();

    if let Some(existing) = select_by_instance_id(ctx, instance_id).await? {
        let mut active: ActiveModel = existing.into();
        active.business_id = Set(model.business_id.clone());
        active.business_code = Set(model.business_code.clone());
        active.business_title = Set(model.business_title.clone());
        active.update_time = Set(Some(now));
        active.update_by = Set(model.update_by);
        ctx.update(active).await?;
        return Ok(());
    }

    let active = ActiveModel {
        id: if model.id == 0 { NotSet } else { Set(model.id) },
        create_dept: Set(model.create_dept),
        create_by: Set(model.create_by),
        create_time: Set(Some(now)),
        update_by: Set(model.update_by),
        update_time: Set(Some(now)),
        business_code: Set(model.business_code.clone()),
        business_title: Set(model.business_title.clone()),
        del_flag: Set(Some("0".into())),
        instance_id: Set(Some(instance_id)),
        business_id: Set(model.business_id.clone()),
    };
    ctx.insert_returning_nothing(active).await?;
    Ok(())
}
