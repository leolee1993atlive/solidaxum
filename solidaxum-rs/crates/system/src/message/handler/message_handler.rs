use std::sync::Arc;

use app_core::{
    error::AppError, middleware::extractors::CurrentUser, vo::response::R,
};
use axum::extract::State;

use crate::{app::AppState, message::model::message::MessageBoxVo};

/// 查询当前用户消息盒子数据。
///
/// 对齐 Java `SysMessageController#getBox`：
/// `GET /resource/message/box` → `SysMessageBoxVo { systemList, noticeList, workflowList }`
#[tracing::instrument(skip(state), fields(user_id = %login_user.user_id))]
pub async fn get_message_box(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<MessageBoxVo>, AppError> {
    // message 全豁免(SysMessageMapper 无 @DataPermission),
    // 读在 repo 内显式 without_row_scope。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let box_vo = state
        .services
        .message_service
        .query_message_box(&ctx, login_user.user_id)
        .await?;
    Ok(R::ok_with_data(box_vo))
}
