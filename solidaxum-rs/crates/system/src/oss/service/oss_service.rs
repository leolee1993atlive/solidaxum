//! oss 服务 —— 编排层。
//!
//! ctx 由 handler 请求期构造(`data_source().context(Principal::..)`)并
//! 透传;oss / oss_config 均为全豁免模块(SysOssMapper /
//! SysOssConfigMapper 无 @DataPermission),读在 repo 内显式
//! without_row_scope,写的审计由漏斗盖。
//!
//! **S3 客户端缓存(OssClientFactory)与漏斗正交**:缓存本体
//! (RwLock<HashMap>)不动;装配读经 repo 的 `system_context` 桥接
//! (见 s3_client.rs),写路径后的 `clear_cache` 失效时序原样保留。

use std::sync::Arc;

use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
    middleware::extractors::CurrentUser,
    serde::encrypt_field::FieldEncryptor,
    vo::page::{TableDataInfo, build_order_by, normalize_paging},
};
use app_data::DataContext;
use axum::extract::Multipart;

use crate::oss::{
    model::oss::{
        OssConfigChangeStatusBo, OssConfigCreateBo, OssConfigQueryBo,
        OssConfigUpdateBo, OssConfigVo, OssQueryBo, OssUploadVo, SysOss,
        SysOssConfig,
    },
    repository::{
        oss_config_repo::OssConfigRepository, oss_repo::OssRepository,
    },
    service::{
        s3_client::OssClientFactory,
        store::{guess_content_type, is_local_endpoint},
    },
};

#[derive(Clone)]
pub struct OssService {
    oss_repo: OssRepository,
    config_repo: OssConfigRepository,
    factory: std::sync::Arc<OssClientFactory>,
}

impl OssService {
    /// 装配期只取 DataSource 快照:全部方法只**转发** ctx(豁免读不解析
    /// resolver,降级快照无影响)。
    pub fn new(infra: &Infrastructure) -> Self {
        Self::with_encryptor(infra, None)
    }

    pub fn with_encryptor(
        infra: &Infrastructure,
        encryptor: Option<Arc<dyn FieldEncryptor>>,
    ) -> Self {
        let config_repo =
            OssConfigRepository::with_encryptor(infra.data_source(), encryptor);
        let factory = Arc::new(OssClientFactory::new(config_repo.clone()));
        Self {
            oss_repo: OssRepository::new(infra.data_source()),
            config_repo,
            factory,
        }
    }

    #[tracing::instrument(skip(self, ctx, multipart, current_user), fields(operator = %current_user.0.username))]
    pub async fn upload(
        &self,
        ctx: &DataContext<'_>,
        mut multipart: Multipart,
        service: Option<String>,
        current_user: &CurrentUser,
    ) -> AppResult<OssUploadVo> {
        while let Some(field) = multipart.next_field().await.map_err(|e| {
            AppError::BadRequest(format!("读取上传文件失败: {}", e))
        })? {
            let original_name = match field.file_name() {
                Some(name) => name.to_string(),
                None => continue,
            };
            let file_suffix = original_name
                .rsplit_once('.')
                .map(|(_, s)| s.to_string())
                .unwrap_or_default();
            let file_name = format!("{}.{}", uuid::Uuid::now_v7(), file_suffix);
            let content_type = field
                .content_type()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());
            let data = field
                .bytes()
                .await
                .map_err(|e| {
                    AppError::Internal(format!("读取文件内容失败: {}", e))
                })?
                .to_vec();

            let client = match service.as_deref() {
                Some(key) if !key.is_empty() => {
                    self.factory.get_or_create(Some(key)).await?
                }
                _ => self.factory.get_or_create(None).await?,
            };
            let key = client.file_key(&file_name);
            client.upload(&key, data, &content_type).await?;
            let url = client.build_url(&key).await?;
            let service_key = client.config_key().to_string();

            // oss_id 取模型层雪花;审计五列由漏斗按 ctx principal 盖章
            let oss = SysOss::new_upload(
                file_name.clone(),
                original_name.clone(),
                file_suffix.clone(),
                url.clone(),
                service_key.clone(),
                current_user,
            );
            self.oss_repo.insert(ctx, &oss).await?;

            return Ok(OssUploadVo {
                oss_id: oss.oss_id.unwrap_or_default(),
                url,
                file_name,
                original_name,
                file_suffix,
                service: service_key,
            });
        }

        Err(AppError::BadRequest("未找到上传文件".to_string()))
    }

    #[tracing::instrument(skip(self, ctx), fields(oss_id = id))]
    pub async fn download(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<(Vec<u8>, String, String)> {
        let oss =
            self.oss_repo.select_by_id(ctx, &id).await?.ok_or_else(|| {
                AppError::NotFound(format!("文件不存在 (id={})", id))
            })?;
        let client = self.factory.get_or_create(Some(&oss.service)).await?;
        let key = client.file_key(&oss.file_name);
        let data = client.download(&key).await?;
        let content_type = guess_content_type(&oss.file_suffix);
        Ok((data, oss.original_name, content_type.to_string()))
    }

    /// 公开静态路由用: 仅当 config_key 命中 local 配置时返回物理路径。
    /// 所有失败(配置不存在/非 local/非法 key)统一归一为 None → 404,
    /// 不向外泄露配置存在性。(无请求身份 —— 装配读在 factory 内经
    /// system_context 桥接,无需 ctx。)
    pub async fn local_file_path(
        &self,
        config_key: &str,
        key: &str,
    ) -> Option<std::path::PathBuf> {
        let store = self.factory.get_or_create(Some(config_key)).await.ok()?;
        store.local_file_path(key)
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn delete_by_ids(
        &self,
        ctx: &DataContext<'_>,
        ids: &[i64],
    ) -> AppResult<()> {
        for id in ids {
            let oss = self.oss_repo.select_by_id(ctx, id).await?;
            if let Some(oss) = oss {
                let client =
                    self.factory.get_or_create(Some(&oss.service)).await?;
                let key = client.file_key(&oss.file_name);
                client.delete(&key).await?;
                self.oss_repo.delete_by_id(ctx, id).await?;
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
    pub async fn list(
        &self,
        ctx: &DataContext<'_>,
        query: OssQueryBo,
    ) -> AppResult<TableDataInfo<SysOss>> {
        let (current, size, _offset) = normalize_paging(&query.page);
        let allowed = [
            ("ossId", "oss_id"),
            ("fileName", "file_name"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "oss_id desc".to_string());

        let mut result = self
            .oss_repo
            .select_page(ctx, &query, current as u64, size as u64, &order_by)
            .await?;
        if result.total <= 0 {
            return Ok(TableDataInfo::empty());
        }

        // 私有 bucket 的 URL 会过期，每次列表查询重新生成
        for row in &mut result.rows {
            if let Ok(client) =
                self.factory.get_or_create(Some(&row.service)).await
                && client.config().access_policy == "0"
            {
                let key = client.file_key(&row.file_name);
                if let Ok(url) = client.build_url(&key).await {
                    row.url = url;
                }
            }
        }

        Ok(result)
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn list_by_ids(
        &self,
        ctx: &DataContext<'_>,
        ids: &[i64],
    ) -> AppResult<Vec<SysOss>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut rows = self.oss_repo.select_by_ids(ctx, ids).await?;

        // 私有 bucket 的 URL 会过期，按 ID 查询时重新生成
        for row in &mut rows {
            if let Ok(client) =
                self.factory.get_or_create(Some(&row.service)).await
                && client.config().access_policy == "0"
            {
                let key = client.file_key(&row.file_name);
                if let Ok(url) = client.build_url(&key).await {
                    row.url = url;
                }
            }
        }

        Ok(rows)
    }

    #[tracing::instrument(skip(self, ctx), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
    pub async fn config_list(
        &self,
        ctx: &DataContext<'_>,
        query: OssConfigQueryBo,
    ) -> AppResult<TableDataInfo<OssConfigVo>> {
        let (current, size, _offset) = normalize_paging(&query.page);
        let allowed = [
            ("ossConfigId", "oss_config_id"),
            ("configKey", "config_key"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "oss_config_id desc".to_string());

        let result = self
            .config_repo
            .select_page(ctx, &query, current as u64, size as u64, &order_by)
            .await?;
        if result.total <= 0 {
            return Ok(TableDataInfo::empty());
        }
        let rows = result.rows.iter().map(|c| c.to_vo()).collect();
        Ok(result.with_rows(rows))
    }

    #[tracing::instrument(skip(self, ctx), fields(oss_config_id = id))]
    pub async fn config_get_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<OssConfigVo> {
        let cfg = self.config_repo.select_by_id(ctx, &id).await?.ok_or_else(
            || AppError::NotFound(format!("OSS配置不存在 (id={})", id)),
        )?;
        Ok(cfg.to_vo())
    }

    #[tracing::instrument(
        skip(self, ctx, bo, current_user),
        fields(operator = %current_user.0.username)
    )]
    pub async fn config_add(
        &self,
        ctx: &DataContext<'_>,
        bo: OssConfigCreateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        if self
            .config_repo
            .select_by_config_key(ctx, &bo.config_key)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(format!(
                "OSS配置key已存在: {}",
                bo.config_key
            )));
        }
        // local 存储恒公开(静态直链无鉴权), 私有配置会让运维误以为文件受保护
        if is_local_endpoint(&bo.endpoint)
            && bo.access_policy.as_deref() == Some("0")
        {
            return Err(AppError::BadRequest(
                "local 本地存储仅支持公开访问(accessPolicy 不能设为私有)"
                    .to_string(),
            ));
        }
        let cfg = SysOssConfig::from_create_bo(bo, current_user);
        self.config_repo.insert(ctx, &cfg).await?;
        self.factory.clear_cache();
        Ok(())
    }

    #[tracing::instrument(
        skip(self, ctx, bo, current_user),
        fields(operator = %current_user.0.username, oss_config_id = ?bo.oss_config_id)
    )]
    pub async fn config_update(
        &self,
        ctx: &DataContext<'_>,
        bo: OssConfigUpdateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        let id = bo.oss_config_id.ok_or_else(|| {
            AppError::BadRequest("OSS配置ID不能为空".to_string())
        })?;
        let mut cfg =
            self.config_repo.select_by_id(ctx, &id).await?.ok_or_else(
                || AppError::NotFound(format!("OSS配置不存在 (id={})", id)),
            )?;

        // local 存储恒公开(静态直链无鉴权), 私有配置会让运维误以为文件受保护;
        // access_policy 为 None 表示保持不变, 仅显式传 "0" 时拦截。
        if is_local_endpoint(&bo.endpoint)
            && bo.access_policy.as_deref() == Some("0")
        {
            return Err(AppError::BadRequest(
                "local 本地存储仅支持公开访问(accessPolicy 不能设为私有)"
                    .to_string(),
            ));
        }

        if bo.config_key != cfg.config_key
            && self
                .config_repo
                .select_by_config_key(ctx, &bo.config_key)
                .await?
                .is_some()
        {
            return Err(AppError::Conflict(format!(
                "OSS配置key已存在: {}",
                bo.config_key
            )));
        }

        cfg.from_update_bo(bo, current_user);
        self.config_repo.update_by_id(ctx, &cfg, &id).await?;
        self.factory.clear_cache();
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx), fields(oss_config_id = id))]
    pub async fn config_delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<()> {
        self.config_repo.delete_by_id(ctx, &id).await?;
        self.factory.clear_cache();
        Ok(())
    }

    /// 切换配置启用状态:启用(Y)时其余全部置 N + 目标置 Y;停用(N)
    /// 只改目标。两个半边在同一 `ctx.transaction`(Java
    /// `updateOssConfigStatus` 带 @Transactional,SysOssConfigServiceImpl
    /// :214 —— 中途失败整体回滚,不再留下"全部已停用、目标未启用"
    /// 的中间态;dict 改名级联同款裁定,向 Java 平价非收紧)。
    #[tracing::instrument(
        skip(self, ctx, bo),
        fields(oss_config_id = ?bo.oss_config_id)
    )]
    pub async fn config_change_status(
        &self,
        ctx: &DataContext<'_>,
        bo: OssConfigChangeStatusBo,
    ) -> AppResult<()> {
        let id = bo.oss_config_id.ok_or_else(|| {
            AppError::BadRequest("OSS配置ID不能为空".to_string())
        })?;
        let status = bo.status.unwrap_or_else(|| "N".to_string());
        let enable = status.eq_ignore_ascii_case("Y");
        ctx.transaction(|tx| {
            Box::pin(async move {
                if enable {
                    OssConfigRepository::update_status_all_in_txn(tx, "N")
                        .await?;
                }
                OssConfigRepository::update_status_by_id_in_txn(
                    tx, &id, &status,
                )
                .await
            })
        })
        .await?;
        self.factory.clear_cache();
        Ok(())
    }
}
