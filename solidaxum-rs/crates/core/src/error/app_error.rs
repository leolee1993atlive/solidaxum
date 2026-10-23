use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

use crate::{error::localizer, vo::response::R};

#[derive(Error, Debug)]
pub enum AppError {
    #[error("JWT错误：{0}")]
    JwtTokenError(String),

    #[error("数据库错误: {0}")]
    DbError(#[from] app_data::sea_orm::DbErr),

    #[error("认证错误: {0}")]
    Auth(String),

    #[error("验证错误: {0}")]
    Validation(String),

    #[error("未找到: {0}")]
    NotFound(String),

    #[error("内部错误: {0}")]
    Internal(String),

    #[error("禁止访问: {0}")]
    Forbidden(String),

    #[error("错误请求: {0}")]
    BadRequest(String),

    #[error("资源冲突: {0}")]
    Conflict(String),
}

impl From<validator::ValidationErrors> for AppError {
    fn from(err: validator::ValidationErrors) -> Self {
        AppError::Validation(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<regex::Error> for AppError {
    fn from(err: regex::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

/// 数据层错误的 HTTP 语义映射(设计 8 节):行级范围拒绝呈现为 NotFound
/// 不泄露存在性 —— 与 DataError 刻意没有 Forbidden 同源。
impl From<app_data::DataError> for AppError {
    fn from(err: app_data::DataError) -> Self {
        match err {
            app_data::DataError::NotFound => {
                AppError::NotFound("数据不存在".to_string())
            }
            app_data::DataError::Conflict => {
                AppError::Conflict("数据冲突:唯一键已存在".to_string())
            }
            app_data::DataError::Backend(e) => AppError::DbError(e),
            app_data::DataError::Unsupported(msg) => AppError::Internal(msg),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::DbError(e) => {
                tracing::error!(variant = "DbError", error = %e, "应用错误: 数据库错误");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    localizer::translate_db_error(e),
                )
            }
            AppError::JwtTokenError(e) => {
                tracing::warn!(variant = "JwtTokenError", error = %e, "应用错误: JWT错误");
                (StatusCode::UNAUTHORIZED, e.to_string())
            }
            AppError::Auth(e) => {
                tracing::warn!(variant = "Auth", error = %e, "应用错误: 认证错误");
                (StatusCode::UNAUTHORIZED, e.clone())
            }
            AppError::Validation(e) => {
                tracing::warn!(variant = "Validation", error = %e, "应用错误: 验证错误");
                (StatusCode::UNPROCESSABLE_ENTITY, e.clone())
            }
            AppError::NotFound(e) => {
                tracing::warn!(variant = "NotFound", error = %e, "应用错误: 未找到");
                (StatusCode::NOT_FOUND, e.clone())
            }
            AppError::Internal(e) => {
                tracing::error!(variant = "Internal", error = %e, "应用错误: 内部错误");
                (StatusCode::INTERNAL_SERVER_ERROR, e.clone())
            }
            AppError::Forbidden(e) => {
                tracing::warn!(variant = "Forbidden", error = %e, "应用错误: 禁止访问");
                (StatusCode::FORBIDDEN, e.clone())
            }
            AppError::BadRequest(e) => {
                tracing::warn!(variant = "BadRequest", error = %e, "应用错误: 错误请求");
                (StatusCode::BAD_REQUEST, e.clone())
            }
            AppError::Conflict(e) => {
                tracing::warn!(variant = "Conflict", error = %e, "应用错误: 资源冲突");
                (StatusCode::CONFLICT, e.clone())
            }
        };
        let body = R::<()>::error_with_code_and_message(
            status.as_u16() as i32,
            message,
        );
        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, response::IntoResponse};

    use super::*;

    #[tokio::test]
    async fn test_db_error_response_body_is_translated() {
        let db_err = app_data::sea_orm::DbErr::Custom(
            "Binder Error: Cannot compare values of type TIMESTAMP and type INTEGER".to_string()
        );
        let app_err = AppError::DbError(db_err);
        let response = app_err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body_bytes =
            to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert!(
            body_str.contains("绑定错误"),
            "response body should contain translated title, got: {}",
            body_str,
        );
    }

    /// 设计 8 节:DataError 四个变体的 HTTP 语义映射逐分支核对。
    #[test]
    fn data_error_maps_to_app_error_branch_by_branch() {
        use app_data::DataError;
        assert!(matches!(
            AppError::from(DataError::NotFound),
            AppError::NotFound(_)
        ));
        assert!(matches!(
            AppError::from(DataError::Conflict),
            AppError::Conflict(_)
        ));
        let db =
            DataError::Backend(app_data::sea_orm::DbErr::Custom("x".into()));
        assert!(matches!(AppError::from(db), AppError::DbError(_)));
        let unsup = DataError::Unsupported("构造不支持".into());
        assert!(matches!(AppError::from(unsup), AppError::Internal(_)));
    }
}
