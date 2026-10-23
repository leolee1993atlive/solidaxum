use crate::{
    error::{AppError, AppResult},
    model::login_user::LoginUser,
};

pub fn require_permission(user: &LoginUser, perm: &str) -> AppResult<()> {
    if user.has_permission(perm) {
        Ok(())
    } else {
        tracing::warn!(user_id = %user.user_id, perm = %perm, "权限校验失败");
        Err(AppError::Forbidden(format!("缺少权限: {}", perm)))
    }
}

pub fn require_role(user: &LoginUser, role: &str) -> AppResult<()> {
    if user.has_role(role) {
        Ok(())
    } else {
        tracing::warn!(user_id = %user.user_id, role = %role, "角色校验失败");
        Err(AppError::Forbidden(format!("缺少角色: {}", role)))
    }
}

pub fn require_any_permission(
    user: &LoginUser,
    perms: &[&str],
) -> AppResult<()> {
    if perms.iter().any(|p| user.has_permission(p)) {
        Ok(())
    } else {
        tracing::warn!(user_id = %user.user_id, perms = ?perms, "任一权限校验失败");
        Err(AppError::Forbidden(format!("缺少任一权限: {:?}", perms)))
    }
}
