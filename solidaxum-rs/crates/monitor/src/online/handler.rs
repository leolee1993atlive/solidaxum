use std::sync::Arc;

use app_core::{
    error::AppError,
    middleware::extractors::CurrentUser,
    vo::{page::PageResult, response::R},
};
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use system::permission::guard;

use super::model::UserOnline;
use crate::OperLogState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineUserQuery {
    pub user_name: Option<String>,
    pub ipaddr: Option<String>,
}

#[tracing::instrument(skip(state))]
pub async fn list_online_users(
    State(state): State<Arc<OperLogState>>,
    CurrentUser(login_user): CurrentUser,
    Query(query): Query<OnlineUserQuery>,
) -> Result<R<PageResult<UserOnline>>, AppError> {
    guard::require_permission(&login_user, "monitor:online:list")?;
    let users = state
        .services
        .online_user_service
        .list_online_users(query.user_name, query.ipaddr)
        .await?;
    Ok(R::ok_with_data(users))
}

#[tracing::instrument(skip(state), fields(token_id = %token_id))]
pub async fn force_logout(
    State(state): State<Arc<OperLogState>>,
    CurrentUser(login_user): CurrentUser,
    Path(token_id): Path<String>,
) -> Result<R<()>, AppError> {
    guard::require_permission(&login_user, "monitor:online:forceLogout")?;
    state
        .services
        .online_user_service
        .force_logout(&token_id)
        .await?;
    Ok(R::ok())
}

#[tracing::instrument(skip(state))]
pub async fn get_current_user_online_info(
    State(state): State<Arc<OperLogState>>,
    CurrentUser(login_user): CurrentUser,
) -> Result<R<PageResult<UserOnline>>, AppError> {
    let users = state
        .services
        .online_user_service
        .get_current_user_online(login_user.username)
        .await?;
    let total = users.len() as i64;
    Ok(R::ok_with_data(PageResult::new(users, total)))
}

#[tracing::instrument(skip(state), fields(token_id = %token_id))]
pub async fn delete_my_session(
    State(state): State<Arc<OperLogState>>,
    CurrentUser(login_user): CurrentUser,
    Path(token_id): Path<String>,
) -> Result<R<()>, AppError> {
    let users = state
        .services
        .online_user_service
        .get_current_user_online(login_user.username)
        .await?;
    if !users.iter().any(|u| u.token_id == token_id) {
        return Err(AppError::Forbidden("只能踢出自己的会话".to_string()));
    }
    state
        .services
        .online_user_service
        .force_logout(&token_id)
        .await?;
    Ok(R::ok())
}

/// 旧批量强退端点已删除，固定返回 404。
pub async fn batch_logout_removed() -> axum::http::StatusCode {
    axum::http::StatusCode::NOT_FOUND
}
