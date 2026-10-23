use axum::{extract::FromRequestParts, http::request::Parts};

use crate::{error::AppError, model::login_user::LoginUser};

/// 从 request extensions 中提取 LoginUser (由 auth 中间件注入)
#[derive(Debug, Clone)]
pub struct CurrentUser(pub LoginUser);

impl CurrentUser {
    pub fn get_user_id(&self) -> i64 {
        self.0.user_id
    }

    pub fn has_permission(&self, perm: &str) -> bool {
        self.0.has_permission(perm)
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.0.has_role(role)
    }
}

impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<LoginUser>()
            .cloned()
            .map(CurrentUser)
            .ok_or_else(|| {
                tracing::warn!("CurrentUser 提取失败: 中间件未设置 LoginUser");
                AppError::Auth("请求缺少认证信息".to_string())
            })
    }
}
