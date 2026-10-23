//! 本地磁盘存储 provider。endpoint 形如 `file://./data/oss`,
//! 落盘路径 = <endpoint去前缀>/<bucket_name>/<prefix>/<uuid>.<suffix>。

use std::path::{Component, Path, PathBuf};

use app_core::error::{AppError, AppResult};
use async_trait::async_trait;

use super::{s3_client::file_key_with_prefix, store::OssStore};
use crate::oss::model::oss::SysOssConfig;

pub struct LocalStore {
    config: SysOssConfig,
    /// <endpoint去前缀>/<bucket_name>, 构建时已 create_dir_all。
    root: PathBuf,
}

impl LocalStore {
    pub async fn from_config(config: SysOssConfig) -> AppResult<Self> {
        let ep = config.endpoint.trim();
        let base = if super::store::is_local_endpoint(ep) {
            &ep[7..]
        } else {
            return Err(AppError::BadRequest(format!(
                "local 存储 endpoint 必须以 file:// 开头: {}",
                config.endpoint
            )));
        };
        if base.is_empty() {
            return Err(AppError::BadRequest(
                "local 存储 endpoint 目录不能为空".to_string(),
            ));
        }
        // bucket_name 作为根下子目录; 为空则直接用根目录。
        let root = if config.bucket_name.is_empty() {
            PathBuf::from(base)
        } else {
            PathBuf::from(base).join(&config.bucket_name)
        };
        tokio::fs::create_dir_all(&root).await.map_err(|e| {
            AppError::Internal(format!(
                "创建本地存储目录失败 {}: {}",
                root.display(),
                e
            ))
        })?;
        Ok(Self { config, root })
    }
}

/// 把对象 key 安全地拼到 root 下: 逐段校验, 拒绝 `..`/根路径/盘符前缀,
/// 返回 None 表示非法 key(调用方一律映射为 404)。
fn safe_join(root: &Path, key: &str) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for comp in Path::new(key).components() {
        match comp {
            Component::Normal(seg) => path.push(seg),
            Component::CurDir => {}
            // ParentDir / RootDir / Prefix(Windows 盘符) 一律拒绝
            _ => return None,
        }
    }
    Some(path)
}

#[async_trait]
impl OssStore for LocalStore {
    async fn upload(
        &self,
        key: &str,
        data: Vec<u8>,
        _content_type: &str,
    ) -> AppResult<()> {
        let path = safe_join(&self.root, key)
            .ok_or_else(|| AppError::BadRequest("非法文件key".to_string()))?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                AppError::Internal(format!("创建目录失败: {}", e))
            })?;
        }
        // 先写临时文件再 rename, 避免半截文件被读到。
        let file_name =
            path.file_name().and_then(|n| n.to_str()).unwrap_or("blob");
        let tmp = path.with_file_name(format!(
            ".{file_name}.tmp-{}",
            uuid::Uuid::now_v7().simple()
        ));
        tokio::fs::write(&tmp, &data).await.map_err(|e| {
            AppError::Internal(format!("本地文件写入失败: {}", e))
        })?;
        if let Err(e) = tokio::fs::rename(&tmp, &path).await {
            // rename 失败时临时文件已成孤儿, 尽力清理
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(AppError::Internal(format!("本地文件落盘失败: {}", e)));
        }
        Ok(())
    }

    async fn download(&self, key: &str) -> AppResult<Vec<u8>> {
        let path = safe_join(&self.root, key)
            .ok_or_else(|| AppError::BadRequest("非法文件key".to_string()))?;
        tokio::fs::read(&path).await.map_err(|e| {
            AppError::NotFound(format!(
                "本地文件读取失败 {}: {}",
                path.display(),
                e
            ))
        })
    }

    async fn delete(&self, key: &str) -> AppResult<()> {
        let path = match safe_join(&self.root, key) {
            Some(p) => p,
            None => return Ok(()), // 非法 key 本来就不可能存在, 幂等成功
        };
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => {
                Err(AppError::Internal(format!("本地文件删除失败: {}", e)))
            }
        }
    }

    /// local 恒公开语义: domain_url 非空拼绝对地址, 否则给相对路径
    /// (前后端同源部署直接用); access_policy="0" 按设计忽略。
    /// 路径带 `/api` 前缀 —— 业务 API(含公开直链)统一挂在前缀下。
    async fn build_url(&self, key: &str) -> AppResult<String> {
        let path = format!("/api/oss/file/{}/{}", self.config.config_key, key);
        let base = self.config.domain_url.trim_end_matches('/');
        Ok(if base.is_empty() {
            path
        } else {
            format!("{base}{path}")
        })
    }

    fn file_key(&self, file_name: &str) -> String {
        file_key_with_prefix(&self.config.prefix, file_name)
    }

    fn config_key(&self) -> &str {
        &self.config.config_key
    }

    fn config(&self) -> &SysOssConfig {
        &self.config
    }

    fn local_file_path(&self, key: &str) -> Option<PathBuf> {
        safe_join(&self.root, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(root: &Path) -> SysOssConfig {
        SysOssConfig {
            oss_config_id: Some(1),
            config_key: "local".into(),
            access_key: String::new(),
            secret_key: String::new(),
            bucket_name: "default".into(),
            prefix: String::new(),
            endpoint: format!(
                "file://{}",
                root.display().to_string().replace('\\', "/")
            ),
            domain_url: String::new(),
            is_https: "N".into(),
            region: String::new(),
            status: "Y".into(),
            access_policy: "1".into(),
            ext1: None,
            remark: None,
            base_entity: app_core::model::entity::BaseEntity::new(0),
        }
    }

    fn temp_root(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("oss-local-{tag}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn safe_join_accepts_normal_relative_keys() {
        let root = Path::new("/data/oss/default");
        assert_eq!(
            safe_join(root, "a/b.txt"),
            Some(PathBuf::from("/data/oss/default/a/b.txt"))
        );
        // CurDir 段吞掉
        assert_eq!(
            safe_join(root, "a/./b.txt"),
            Some(PathBuf::from("/data/oss/default/a/b.txt"))
        );
    }

    #[test]
    fn safe_join_rejects_traversal_and_absolute() {
        let root = Path::new("/data/oss/default");
        assert_eq!(safe_join(root, "../etc/passwd"), None);
        assert_eq!(safe_join(root, "a/../../b"), None);
        assert_eq!(safe_join(root, "/etc/passwd"), None);
    }

    #[tokio::test]
    async fn build_url_shapes() {
        let root = temp_root("url");
        let mut cfg = test_config(&root);
        let store = LocalStore::from_config(cfg.clone()).await.unwrap();
        assert_eq!(
            store.build_url("p/a.txt").await.unwrap(),
            "/api/oss/file/local/p/a.txt"
        );
        cfg.domain_url = "https://cdn.example.com/".into();
        let store = LocalStore::from_config(cfg).await.unwrap();
        assert_eq!(
            store.build_url("p/a.txt").await.unwrap(),
            "https://cdn.example.com/api/oss/file/local/p/a.txt"
        );
    }

    #[tokio::test]
    async fn upload_download_delete_roundtrip() {
        let root = temp_root("roundtrip");
        let store = LocalStore::from_config(test_config(&root)).await.unwrap();
        let key = store.file_key("0192a1b2.png");
        assert_eq!(key, "0192a1b2.png"); // 无 prefix 原样
        store
            .upload(&key, b"png-bytes".to_vec(), "image/png")
            .await
            .unwrap();
        assert_eq!(store.download(&key).await.unwrap(), b"png-bytes");
        assert_eq!(
            store.local_file_path(&key),
            Some(store.root.join("0192a1b2.png"))
        );
        store.delete(&key).await.unwrap();
        assert!(store.download(&key).await.is_err());
        // 幂等删除
        store.delete(&key).await.unwrap();
    }

    #[tokio::test]
    async fn upload_rejects_traversal_key() {
        let root = temp_root("traverse");
        let store = LocalStore::from_config(test_config(&root)).await.unwrap();
        assert!(
            store
                .upload("../x", b"bad".to_vec(), "text/plain")
                .await
                .is_err()
        );
        assert_eq!(store.local_file_path("../x"), None);
    }
}
