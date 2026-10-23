//! local 存储的公开静态直出(不过 auth, 等价 public bucket)。
//!
//! 安全要点:
//! - 路径拼接与防穿越全部在 `LocalStore::local_file_path` 内完成,
//!   handler 不自己拼路径;
//! - 危险类型(html/svg/js 等)强制 attachment + nosniff, 防同源存储型 XSS;
//! - 文件名是不可变 UUID, 长缓存 immutable。

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

use crate::{
    app::AppState,
    oss::service::store::{guess_content_type, needs_attachment},
};

pub async fn serve_local_file(
    State(state): State<Arc<AppState>>,
    Path((config_key, key)): Path<(String, String)>,
) -> Response {
    let Some(path) = state
        .services
        .oss_service
        .local_file_path(&config_key, &key)
        .await
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let suffix = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_string();

    match tokio::fs::read(&path).await {
        Ok(data) => {
            let mut builder = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, guess_content_type(&suffix))
                .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
                .header(
                    header::CACHE_CONTROL,
                    "public, max-age=31536000, immutable",
                );
            if needs_attachment(&suffix) {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .replace('"', "");
                builder = builder.header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{name}\""),
                );
            }
            builder.body(Body::from(data)).unwrap_or_else(|_| {
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            })
        }
        // 文件不存在与读失败统一 404(与对象存储语义一致: 不区分不存在/不可读)
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
