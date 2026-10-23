//! 实例业务扩展字段的读/写编排 —— 收 `ctx: &DataContext<'_>`
//! (instance_service / task_service 消费)。

use std::sync::Arc;

use app_core::error::AppResult;
use app_data::DataContext;

use crate::{
    repository::{biz_ext_repo, entity::flow_instance_biz_ext},
    resolver::BusinessStatusCallback,
};

/// 解析流程实例的业务展示字段：优先 `flow_instance_biz_ext`，回退业务回调。
pub async fn resolve_business_fields(
    ctx: &DataContext<'_>,
    callback: &Arc<dyn BusinessStatusCallback>,
    instance_id: i64,
    business_id: &str,
) -> AppResult<(Option<String>, Option<String>)> {
    if let Some(ext) =
        biz_ext_repo::select_by_instance_id(ctx, instance_id).await?
    {
        return Ok((ext.business_code, ext.business_title));
    }
    if let Ok(Some(info)) = callback.get_business_info(business_id).await {
        return Ok((info.code, info.title));
    }
    Ok((None, None))
}

/// 持久化流程实例业务扩展信息（对齐 Java buildFlowInstanceBizExt）。
pub async fn save_instance_biz_ext(
    ctx: &DataContext<'_>,
    instance_id: i64,
    business_id: &str,
    business_code: Option<String>,
    business_title: Option<String>,
    operator_id: Option<i64>,
) -> AppResult<()> {
    let code = business_code
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis().to_string());
    let model = flow_instance_biz_ext::Model {
        id: app_core::util::snowflake::next_id(),
        instance_id: Some(instance_id),
        business_id: Some(business_id.to_string()),
        business_code: Some(code),
        business_title,
        create_dept: None,
        create_by: operator_id,
        create_time: None,
        update_by: operator_id,
        update_time: None,
        del_flag: Some("0".into()),
    };
    biz_ext_repo::save_or_update_by_instance_id(ctx, &model).await?;
    Ok(())
}
