use axum::{
    extract::Path,
    http::{StatusCode, header},
    response::IntoResponse,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "static/designer/"]
pub struct DesignerAssets;

/// 将请求路径映射到嵌入资源名（空路径或 "/" 返回 "index.html"）
pub(crate) fn resolve_path(path: &str) -> String {
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        "index.html".to_string()
    } else {
        trimmed.to_string()
    }
}

/// GET /designer/*path — 服务嵌入的 warm-flow-ui 静态文件
pub async fn serve_designer(path: Option<Path<String>>) -> impl IntoResponse {
    let raw_path = path.map(|p| p.0).unwrap_or_default();
    let asset_path = resolve_path(&raw_path);

    match DesignerAssets::get(&asset_path) {
        Some(content) => {
            let mime =
                mime_guess::from_path(&asset_path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref().to_string())],
                content.data.to_vec(),
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_html_is_embedded() {
        assert!(
            DesignerAssets::get("index.html").is_some(),
            "index.html must exist in static/designer/"
        );
    }

    #[test]
    fn empty_path_resolves_to_index() {
        let resolved = resolve_path("");
        assert_eq!(resolved, "index.html");
    }

    #[test]
    fn slash_resolves_to_index() {
        let resolved = resolve_path("/");
        assert_eq!(resolved, "index.html");
    }
}
