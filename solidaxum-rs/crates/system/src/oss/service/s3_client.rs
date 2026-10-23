use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use app_core::error::{AppError, AppResult};
use async_trait::async_trait;
use aws_config::{BehaviorVersion, SdkConfig};
use aws_sdk_s3::{
    Client,
    config::{Builder, Credentials, Region},
    primitives::ByteStream,
};

use super::{
    local_store::LocalStore,
    store::{OssStore, is_local_endpoint},
};
use crate::oss::{
    model::oss::SysOssConfig, repository::oss_config_repo::OssConfigRepository,
};

pub struct S3ClientWrapper {
    client: Client,
    /// 专用于预签名的客户端:签向对外 domain(未配置 domain 时与 client 同源)。
    /// 见 presign_host —— 上传走内网,签出来的链接得让浏览器打得开。
    presign_client: Client,
    config: SysOssConfig,
}

impl S3ClientWrapper {
    pub async fn upload(
        &self,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> AppResult<()> {
        let body = ByteStream::from(data);
        self.client
            .put_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .body(body)
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("S3上传失败: {}", e)))?;
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> AppResult<()> {
        self.client
            .delete_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("S3删除失败: {}", e)))?;
        Ok(())
    }

    pub async fn download(&self, key: &str) -> AppResult<Vec<u8>> {
        let resp = self
            .client
            .get_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("S3下载失败: {}", e)))?;
        let data = resp
            .body
            .collect()
            .await
            .map_err(|e| AppError::Internal(format!("S3读取失败: {}", e)))?
            .into_bytes()
            .to_vec();
        Ok(data)
    }

    /// 生成文件访问 URL。
    /// - access_policy="1" 或任何非 "0" 值(公开): 直接返回公有直链。
    /// - access_policy="0"(私有): 返回 1 小时有效的预签名 URL。
    pub async fn build_url(&self, key: &str) -> AppResult<String> {
        // 公开策略：直接拼直链
        if !is_private_policy(&self.config.access_policy) {
            return Ok(build_public_url(
                &self.config.is_https,
                &self.config.endpoint,
                &self.config.domain_url,
                &self.config.bucket_name,
                key,
            ));
        }

        // 私有策略：生成预签名 URL（1 小时有效）
        use std::time::Duration;

        use aws_sdk_s3::presigning::PresigningConfig;

        let expiry = PresigningConfig::expires_in(Duration::from_secs(3600))
            .map_err(|e| {
                AppError::Internal(format!("预签名配置失败: {}", e))
            })?;

        let presigned = self
            .presign_client
            .get_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .presigned(expiry)
            .await
            .map_err(|e| {
                AppError::Internal(format!("预签名 URL 生成失败: {}", e))
            })?;

        Ok(presigned.uri().to_string())
    }

    pub fn config(&self) -> &SysOssConfig {
        &self.config
    }

    pub fn prefix(&self) -> &str {
        &self.config.prefix
    }

    pub fn config_key(&self) -> &str {
        &self.config.config_key
    }

    pub fn file_key(&self, file_name: &str) -> String {
        file_key_with_prefix(&self.config.prefix, file_name)
    }
}

/// 判断 OSS 配置是否为私有访问策略(纯函数,便于单测)。
/// "0" = 私有(需预签名),其它值(包括 "1" / 空) = 公开。
pub fn is_private_policy(access_policy: &str) -> bool {
    access_policy == "0"
}

/// RuoYi 内置云厂商标识(OssConstant.CLOUD_SERVICE)。
const CLOUD_SERVICE: [&str; 4] = ["aliyun", "qcloud", "qiniu", "obs"];

/// 是否使用路径风格访问(对齐 OssClientConfig.resolvePathStyleAccess)。
/// endpoint 命中内置云厂商 → 站点风格(virtual-hosted);否则(自建 MinIO /
/// RustFS 等)→ 路径风格。客户端寻址与 URL 拼接必须用同一判定,否则会出现
/// "能传上去但链接打不开"。
pub fn use_path_style(endpoint: &str) -> bool {
    !CLOUD_SERVICE.iter().any(|s| endpoint.contains(s))
}

/// 预签名 URL 应当签向的主机(对齐 DefaultOssClientImpl:82-89)。
/// 后端连的是内网 endpoint,浏览器要打开的是对外 domain —— 预签名必须签向
/// 后者,否则签名域名不匹配,链接在浏览器里打不开。
pub fn presign_host<'a>(endpoint: &'a str, domain_url: &'a str) -> &'a str {
    if domain_url.is_empty() {
        endpoint
    } else {
        domain_url
    }
}

/// 去掉已带的协议头(对齐 BucketUrlUtil.removeHttpProtocolHeader)。
fn strip_scheme(base: &str) -> &str {
    for prefix in ["http://", "https://"] {
        if base.len() >= prefix.len()
            && base[..prefix.len()].eq_ignore_ascii_case(prefix)
        {
            return &base[prefix.len()..];
        }
    }
    base
}

/// 构建公开访问 URL(纯函数,便于单测)。
/// 对齐 RuoYi:base 优先 domain_url、否则 endpoint;风格按 **endpoint** 判定,
/// 路径风格拼 `{base}/{bucket}`,站点风格拼 `{bucket}.{base}`;最后接 key。
pub fn build_public_url(
    is_https: &str,
    endpoint: &str,
    domain_url: &str,
    bucket: &str,
    key: &str,
) -> String {
    let scheme = if is_https.eq_ignore_ascii_case("Y") {
        "https"
    } else {
        "http"
    };
    let base = if domain_url.is_empty() {
        endpoint
    } else {
        domain_url
    };
    let base = strip_scheme(base).trim_end_matches('/');
    let bucket_url = if use_path_style(endpoint) {
        format!("{}://{}/{}", scheme, base, bucket)
    } else {
        format!("{}://{}.{}", scheme, bucket, base)
    };
    format!("{}/{}", bucket_url, key)
}

/// 根据 prefix 拼接文件 key(纯函数,便于单测)。
/// prefix 为空 → 直接 file_name;否则 prefix/file_name(去除多余 /)。
pub fn file_key_with_prefix(prefix: &str, file_name: &str) -> String {
    let p = prefix.trim_matches('/');
    if p.is_empty() {
        file_name.to_string()
    } else {
        format!("{}/{}", p, file_name)
    }
}

pub struct OssClientFactory {
    cache: RwLock<HashMap<String, Arc<dyn OssStore>>>,
    config_repo: OssConfigRepository,
}

impl OssClientFactory {
    pub fn new(config_repo: OssConfigRepository) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            config_repo,
        }
    }

    pub async fn get_or_create(
        &self,
        config_key: Option<&str>,
    ) -> AppResult<Arc<dyn OssStore>> {
        // 装配读的 ctx 桥接 —— 客户端装配是基础设施语义(public
        // 静态路由 local_file_path 也走这里,无请求身份),system_context
        // 对位;oss_config 全豁免(without_row_scope),不解析 resolver,
        // system_context 语义下无影响。缓存本体(RwLock<HashMap>)不动。
        let ctx = self.config_repo.system_context();
        let key = if let Some(k) = config_key {
            k.to_string()
        } else {
            self.config_repo
                .select_by_status(&ctx, "Y")
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest("未找到默认OSS配置".to_string())
                })?
                .config_key
        };

        {
            let cache = self.cache.read().map_err(|_| {
                AppError::Internal("OSS客户端缓存锁异常".to_string())
            })?;
            if let Some(client) = cache.get(&key) {
                return Ok(client.clone());
            }
        }

        let cfg = self
            .config_repo
            .select_by_config_key(&ctx, &key)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("OSS配置不存在: {}", key))
            })?;

        // endpoint 以 file:// 开头 → 本地磁盘 provider; 否则 S3 协议。
        let store: Arc<dyn OssStore> = if is_local_endpoint(&cfg.endpoint) {
            Arc::new(LocalStore::from_config(cfg).await?)
        } else {
            let (client, presign_client) = build_s3_client(&cfg).await?;
            Arc::new(S3ClientWrapper {
                client,
                presign_client,
                config: cfg,
            })
        };

        {
            let mut cache = self.cache.write().map_err(|_| {
                AppError::Internal("OSS客户端缓存锁异常".to_string())
            })?;
            cache.insert(key, store.clone());
        }

        Ok(store)
    }

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }
}

/// 返回 (操作客户端, 预签名客户端)。两者只在配置了 domain_url 时不同源。
async fn build_s3_client(config: &SysOssConfig) -> AppResult<(Client, Client)> {
    if config.endpoint.is_empty() {
        return Err(AppError::BadRequest("OSS endpoint不能为空".to_string()));
    }
    if config.bucket_name.is_empty() {
        return Err(AppError::BadRequest("OSS bucketName不能为空".to_string()));
    }

    let region = if config.region.is_empty() {
        "us-east-1".to_string()
    } else {
        config.region.clone()
    };

    let sdk_config: SdkConfig = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(region))
        .credentials_provider(Credentials::new(
            &config.access_key,
            &config.secret_key,
            None,
            None,
            "oss-config",
        ))
        .load()
        .await;

    let scheme = if config.is_https.eq_ignore_ascii_case("Y") {
        "https"
    } else {
        "http"
    };
    // 寻址风格必须与 build_public_url 一致,否则传得上去、链接打不开。
    let path_style = use_path_style(&config.endpoint);

    let build_for = |host: &str| {
        let s3_config = Builder::from(&sdk_config)
            .endpoint_url(format!("{}://{}", scheme, strip_scheme(host)))
            .force_path_style(path_style)
            .build();
        Client::from_conf(s3_config)
    };

    let client = build_for(&config.endpoint);
    let presign_host = presign_host(&config.endpoint, &config.domain_url);
    let presign_client = if presign_host == config.endpoint {
        client.clone()
    } else {
        build_for(presign_host)
    };

    Ok((client, presign_client))
}

#[async_trait]
impl OssStore for S3ClientWrapper {
    async fn upload(
        &self,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> AppResult<()> {
        self.upload(key, data, content_type).await
    }

    async fn download(&self, key: &str) -> AppResult<Vec<u8>> {
        self.download(key).await
    }

    async fn delete(&self, key: &str) -> AppResult<()> {
        self.delete(key).await
    }

    async fn build_url(&self, key: &str) -> AppResult<String> {
        self.build_url(key).await
    }

    fn file_key(&self, file_name: &str) -> String {
        self.file_key(file_name)
    }

    fn config_key(&self) -> &str {
        self.config_key()
    }

    fn config(&self) -> &SysOssConfig {
        self.config()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_policy_only_for_zero() {
        assert!(is_private_policy("0"));
        assert!(!is_private_policy("1"));
        assert!(!is_private_policy(""));
        assert!(!is_private_policy("public"));
    }

    #[test]
    fn path_style_for_self_hosted_endpoint() {
        assert!(use_path_style("127.0.0.1:9000"));
        assert!(use_path_style("rustfs.internal:9000"));
        assert!(use_path_style("s3.example.com"));
    }

    #[test]
    fn site_style_for_builtin_cloud_vendors() {
        assert!(!use_path_style("oss-cn-beijing.aliyuncs.com"));
        assert!(!use_path_style("cos.ap-beijing.myqcloud.com"));
        assert!(!use_path_style("s3-cn-north-1.qiniucs.com"));
        assert!(!use_path_style("obs.cn-north-4.myhuaweicloud.com"));
    }

    #[test]
    fn public_url_path_style_puts_bucket_in_path() {
        let url = build_public_url("N", "127.0.0.1:9000", "", "ruoyi", "a.txt");
        assert_eq!(url, "http://127.0.0.1:9000/ruoyi/a.txt");
    }

    #[test]
    fn public_url_site_style_puts_bucket_in_host() {
        let url = build_public_url(
            "N",
            "oss-cn-beijing.aliyuncs.com",
            "",
            "ruoyi",
            "a.txt",
        );
        assert_eq!(url, "http://ruoyi.oss-cn-beijing.aliyuncs.com/a.txt");
    }

    #[test]
    fn public_url_uses_domain_as_base_but_keeps_bucket() {
        let url = build_public_url(
            "Y",
            "s3.example.com",
            "cdn.example.com",
            "ruoyi",
            "a/b.txt",
        );
        assert_eq!(url, "https://cdn.example.com/ruoyi/a/b.txt");
    }

    #[test]
    fn public_url_style_is_decided_by_endpoint_not_domain() {
        // domain 是自建域名，但 endpoint 指向阿里云 —— 风格仍按 endpoint 判定
        let url = build_public_url(
            "N",
            "oss-cn-beijing.aliyuncs.com",
            "cdn.example.com",
            "ruoyi",
            "f.txt",
        );
        assert_eq!(url, "http://ruoyi.cdn.example.com/f.txt");
    }

    #[test]
    fn public_url_strips_scheme_already_in_base() {
        let url = build_public_url(
            "Y",
            "s3.example.com",
            "http://cdn.example.com",
            "ruoyi",
            "f.txt",
        );
        assert_eq!(url, "https://cdn.example.com/ruoyi/f.txt");
    }

    #[test]
    fn public_url_strips_trailing_slash_in_base() {
        let url =
            build_public_url("Y", "s3.example.com/", "", "ruoyi", "f.txt");
        assert_eq!(url, "https://s3.example.com/ruoyi/f.txt");
    }

    #[test]
    fn presign_host_prefers_domain_when_configured() {
        assert_eq!(
            presign_host("rustfs:9000", "oss.example.com"),
            "oss.example.com"
        );
    }

    #[test]
    fn presign_host_falls_back_to_endpoint() {
        assert_eq!(presign_host("rustfs:9000", ""), "rustfs:9000");
    }

    #[test]
    fn file_key_without_prefix_returns_filename_as_is() {
        assert_eq!(file_key_with_prefix("", "a.txt"), "a.txt");
        assert_eq!(file_key_with_prefix("/", "a.txt"), "a.txt");
        assert_eq!(file_key_with_prefix("///", "a.txt"), "a.txt");
    }

    #[test]
    fn file_key_with_prefix_joins_with_single_slash() {
        assert_eq!(file_key_with_prefix("uploads", "a.txt"), "uploads/a.txt");
        assert_eq!(file_key_with_prefix("/uploads/", "a.txt"), "uploads/a.txt");
    }
}
