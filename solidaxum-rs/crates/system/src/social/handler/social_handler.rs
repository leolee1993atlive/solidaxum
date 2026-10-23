use std::sync::Arc;

use app_core::{
    error::AppError, middleware::extractors::CurrentUser, vo::response::R,
};
use axum::extract::State;

use crate::{app::AppState, social::model::social::SocialVo};

#[tracing::instrument(skip(state), fields(user_id = %login_user.user_id))]
pub async fn list_social_accounts(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<Vec<SocialVo>>, AppError> {
    // social 全豁免(SysSocialMapper 无 @DataPermission),
    // 读在 repo 内显式 without_row_scope,软删过滤由漏斗照施。
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let result = state
        .services
        .social_service
        .list_by_user_id(&ctx, login_user.user_id)
        .await?;
    Ok(R::ok_with_data(result))
}
