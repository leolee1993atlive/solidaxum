use std::sync::Arc;

use app_core::{
    error::AppError, middleware::extractors::CurrentUser, vo::response::R,
};
use axum::extract::{Path, State};

use crate::handler::{FlowState, user_ctx};

#[tracing::instrument(skip(state, login_user))]
pub async fn get_chart(
    State(state): State<Arc<FlowState>>,
    CurrentUser(login_user): CurrentUser,
    Path(instance_id): Path<i64>,
) -> Result<R<serde_json::Value>, AppError> {
    app_core::permission::require_permission(
        &login_user,
        "workflow:instance:*",
    )?;
    let dctx = user_ctx(&state, &CurrentUser(login_user));
    let instance = state
        .ins_service
        .get_by_id(&dctx, instance_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("实例不存在 (id={})", instance_id))
        })?;

    // def_json 是设计器持有的动态 JSON(节点/边属性由前端画布定义),
    // 后端仅原样透传 —— Value 在此为合法边界,不强行类型化
    // (对比:带状态高亮的 chart 走 /warm-flow/query-flow-chart,
    //  其 service 出口已是类型化的 DefJson)。
    let chart = match instance.def_json.as_deref() {
        Some(json_str) if !json_str.is_empty() => {
            serde_json::from_str(json_str).map_err(|e| {
                AppError::Internal(format!("解析流程图数据失败: {}", e))
            })?
        }
        _ => serde_json::Value::Object(serde_json::Map::new()),
    };

    Ok(R::ok_with_data(chart))
}
