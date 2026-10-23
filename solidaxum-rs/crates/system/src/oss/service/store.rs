//! 对象存储后端抽象。
//!
//! 现有两种实现: `S3ClientWrapper`(S3 协议: MinIO/RustFS/云厂商) 与
//! `LocalStore`(本地磁盘, endpoint 以 `file://` 前缀标识)。

use std::path::PathBuf;

use app_core::error::AppResult;
use async_trait::async_trait;

use crate::oss::model::oss::SysOssConfig;

/// 存储后端统一接口, `OssClientFactory` 缓存并分发 `Arc<dyn OssStore>`。
#[async_trait]
pub trait OssStore: Send + Sync {
    /// 上传对象(key 含 prefix, 由 `file_key` 生成)。
    async fn upload(
        &self,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> AppResult<()>;
    /// 读取对象全部字节。
    async fn download(&self, key: &str) -> AppResult<Vec<u8>>;
    /// 删除对象; 对象不存在视为成功(幂等)。
    async fn delete(&self, key: &str) -> AppResult<()>;
    /// 生成浏览器可访问的 URL(S3: 公有直链或预签名; local: 静态路由路径)。
    async fn build_url(&self, key: &str) -> AppResult<String>;
    /// 按配置的 prefix 拼对象 key。
    fn file_key(&self, file_name: &str) -> String;
    fn config_key(&self) -> &str;
    fn config(&self) -> &SysOssConfig;
    /// local 实现覆写: key → 物理路径(内部完成拼接与防穿越校验,
    /// 非法 key 返回 None); S3 用默认 None。供公开静态路由取文件。
    fn local_file_path(&self, key: &str) -> Option<PathBuf> {
        let _ = key;
        None
    }
}

/// endpoint 以 `file://` 前缀(大小写不敏感)标识 local 本地磁盘配置。
/// 用 `get(..7)` 而非 `[..7]` 切片: 第 7 字节落在多字节字符中间时
/// 切片会 panic。
pub fn is_local_endpoint(endpoint: &str) -> bool {
    endpoint
        .get(..7)
        .is_some_and(|p| p.eq_ignore_ascii_case("file://"))
}

/// 按文件后缀猜 Content-Type(从 oss_service 迁入, 静态路由与下载共用)。
pub fn guess_content_type(suffix: &str) -> &'static str {
    match suffix.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        _ => "application/octet-stream",
    }
}

/// 同源直出时会被浏览器当活跃内容解析的类型, 强制 attachment 防存储型
/// XSS(MinIO 异源部署天然免疫, local 与主站同源必须防)。
pub fn needs_attachment(suffix: &str) -> bool {
    matches!(
        suffix.to_ascii_lowercase().as_str(),
        "html" | "htm" | "svg" | "js" | "mjs" | "css" | "xml" | "txt"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_endpoint_prefix_detection() {
        assert!(is_local_endpoint("file://./data/oss"));
        assert!(is_local_endpoint("file:///var/lib/app/oss"));
        assert!(is_local_endpoint("FILE://D:/data/oss"));
        assert!(!is_local_endpoint("127.0.0.1:9000"));
        assert!(!is_local_endpoint(""));
        assert!(!is_local_endpoint("file:"));
        // 非 ASCII: 第 7 字节落在多字节字符中间也不得 panic
        assert!(!is_local_endpoint("存储路径/oss"));
        assert!(!is_local_endpoint("123456中文"));
    }

    #[test]
    fn dangerous_types_force_attachment() {
        assert!(needs_attachment("html"));
        assert!(needs_attachment("SVG"));
        assert!(needs_attachment("js"));
        assert!(needs_attachment("txt"));
        assert!(!needs_attachment("png"));
        assert!(!needs_attachment("pdf"));
        assert!(!needs_attachment("zip"));
    }
}
