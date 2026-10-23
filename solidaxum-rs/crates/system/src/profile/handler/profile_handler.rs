use std::sync::Arc;

use app_core::{
    error::AppError, extract::AppJson, middleware::extractors::CurrentUser,
    vo::response::R,
};
use app_data::Principal;
use axum::extract::State;
use validator::Validate;

use crate::{
    app::AppState,
    user::model::user::{ProfileVo, SysUserPasswordBo, SysUserProfileBo},
};

/// 获取个人中心信息 (仅需登录)
#[tracing::instrument(skip(state))]
pub async fn get_profile(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<ProfileVo>, AppError> {
    // ctx 请求期构造 —— `infra.data_source()` 此刻必带 resolver
    // (装配在容器构造后、开服务前完成,见 admin/app.rs 装配顺序注释);
    // 绑定 Arc 局部变量再借出,避免临时值悬垂。
    let ds = state.infra.data_source();
    let ctx = ds.context(Principal::from(&login_user));
    let profile = state
        .services
        .profile_service
        .get_profile(&ctx, login_user.user_id)
        .await?;
    Ok(R::ok_with_data(profile))
}

/// 更新个人中心信息 (仅需登录)
#[tracing::instrument(skip(state, bo), fields(operator = %current_user.0.username))]
pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<SysUserProfileBo>,
) -> Result<R<()>, AppError> {
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(Principal::from(&current_user.0));
    state
        .services
        .profile_service
        .update_profile(&ctx, current_user.0.user_id, bo)
        .await?;
    Ok(R::ok())
}

/// 修改密码 (仅需登录)
#[tracing::instrument(skip(state, bo), fields(operator = %current_user.0.username))]
pub async fn update_password(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    AppJson(bo): AppJson<SysUserPasswordBo>,
) -> Result<R<()>, AppError> {
    bo.validate()?;
    let ds = state.infra.data_source();
    let ctx = ds.context(Principal::from(&current_user.0));
    state
        .services
        .profile_service
        .update_password(
            &ctx,
            current_user.0.user_id,
            &bo.old_password,
            &bo.new_password,
        )
        .await?;
    Ok(R::ok())
}
