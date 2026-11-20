//! 渠道配置 CRUD(钉钉/企微/飞书)。
//!
//! 权限码 `system:messageChannel:list|edit|test`。
//! 密钥回读一律脱敏,保存时空串 = 保留现值。

use std::sync::Arc;

use app_core::error::{AppError, AppResult};
use app_core::middleware::extractors::CurrentUser;
use app_core::vo::response::R;
use axum::{Json, extract::State};
use serde_json::Value;

use crate::app::AppState;
use crate::push::channel_config_service::{self, ChannelSaveBo, ChannelVo};

macro_rules! require_perm {
    ($u:expr, $p:literal) => {
        app_core::permission::require_permission(&$u, $p)?
    };
}

/// `GET /system/messageChannel/list` — 渠道清单(脱敏)。
#[tracing::instrument(skip(state, login_user))]
pub async fn list_channels(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
) -> AppResult<R<Vec<ChannelVo>>> {
    require_perm!(login_user, "system:messageChannel:list");
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let enc = state.infra.field_encryptor();
    let rows =
        channel_config_service::list_channels(&ctx, enc.as_ref()).await?;
    Ok(R::ok_with_data(rows))
}

/// `PUT /system/messageChannel/{channel}` — 保存配置。
#[tracing::instrument(skip(state, login_user, bo))]
pub async fn save_channel(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    axum::extract::Path(channel): axum::extract::Path<String>,
    Json(bo): Json<ChannelSaveBo>,
) -> AppResult<R<()>> {
    require_perm!(login_user, "system:messageChannel:edit");
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let enc = state.infra.field_encryptor();
    channel_config_service::save_channel(&ctx, enc.as_ref(), &channel, bo)
        .await?;
    Ok(R::ok())
}

/// `POST /system/messageChannel/{channel}/test` — 连通性自检(只取 token)。
#[tracing::instrument(skip(state, login_user))]
pub async fn test_channel(
    State(state): State<Arc<AppState>>,
    CurrentUser(login_user): CurrentUser,
    axum::extract::Path(channel): axum::extract::Path<String>,
) -> AppResult<R<Value>> {
    require_perm!(login_user, "system:messageChannel:test");
    let ds = state.infra.data_source();
    let ctx = ds.context(app_data::Principal::from(&login_user));
    let enc = state.infra.field_encryptor();
    let v = channel_config_service::test_channel(&ctx, enc.as_ref(), &channel)
        .await
        .map_err(|e| match e {
            // 渠道侧失败按 400 透出平台原文(便于现场对照后台错误码)
            AppError::Internal(m) => AppError::BadRequest(m),
            other => other,
        })?;
    Ok(R::ok_with_data(v))
}
