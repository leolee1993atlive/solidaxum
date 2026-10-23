//! config 服务 —— 编排层。
//!
//! ctx 由 handler 请求期构造并透传;Redis 缓存旁路(`sys_config:{k}`,
//! 对位 Java `@Cacheable(SYS_CONFIG)` selectConfigByKey ServiceImpl:77)
//! 留在本层 —— 缓存命中不落库,未命中才走仓储的豁免读。

use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
    middleware::extractors::CurrentUser,
    vo::page::{TableDataInfo, build_order_by, normalize_paging},
};
use app_data::DataContext;
use deadpool_redis::Pool;
use redis::AsyncCommands;

use crate::config::{
    model::config::{
        Config, ConfigCreateBo, ConfigListVo, ConfigPageQuery, ConfigUpdateBo,
    },
    repository::config_repo::ConfigRepository,
};

const CACHE_KEY_PREFIX: &str = "sys_config:";

#[derive(Clone)]
pub struct ConfigService {
    repo: ConfigRepository,
    redis_pool: Pool,
}

impl ConfigService {
    /// 装配期只取 DataSource 快照:全部方法只**转发** ctx(豁免读不解析
    /// resolver,降级快照无影响)。
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            repo: ConfigRepository::new(infra.data_source()),
            redis_pool: infra.pool.redis_pool.clone(),
        }
    }

    fn cache_key(config_key: &str) -> String {
        format!("{}{}", CACHE_KEY_PREFIX, config_key)
    }

    /// 直接查 DB 的配置值(不走 Redis 缓存),未找到则返回 default。
    /// 用于内部开关校验(如注册开关),避免依赖 Redis 即可测试。
    /// (落库读是仓储豁免读;调用方传 system_context 或用户 ctx 均可,
    /// ctx 的范围不影响口径)
    pub async fn get_value_or_default(
        &self,
        ctx: &DataContext<'_>,
        config_key: &str,
        default: &str,
    ) -> String {
        match self.repo.select_by_key(ctx, config_key).await {
            Ok(Some(cfg)) => cfg.config_value,
            _ => default.to_string(),
        }
    }

    /// 缓存旁路: 读缓存 → 未命中 → 读数据库 → 写缓存
    /// 返回原始 config_value 字符串 (非 JSON)
    /// (对位 Java `@Cacheable(SYS_CONFIG)` selectConfigByKey
    /// ServiceImpl:77;落库读是仓储豁免读)
    #[tracing::instrument(skip(self, ctx), fields(config_key = config_key))]
    pub async fn get_by_key(
        &self,
        ctx: &DataContext<'_>,
        config_key: &str,
    ) -> AppResult<String> {
        let key = Self::cache_key(config_key);
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis连接获取失败: {}", e))
        })?;

        // 1. 读缓存
        let cached: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| AppError::Internal(format!("Redis读取失败: {}", e)))?;
        if let Some(val) = cached {
            return Ok(val);
        }

        // 2. 读数据库
        let config =
            self.repo.select_by_key(ctx, config_key).await?.ok_or_else(
                || {
                    AppError::NotFound(format!(
                        "配置不存在 (configKey={})",
                        config_key
                    ))
                },
            )?;

        // 3. 写缓存 (无 TTL — 持久缓存)
        let _: () = conn
            .set(&key, &config.config_value)
            .await
            .map_err(|e| AppError::Internal(format!("Redis写入失败: {}", e)))?;

        Ok(config.config_value)
    }

    /// 写入缓存
    async fn set_cache(
        &self,
        config_key: &str,
        config_value: &str,
    ) -> AppResult<()> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis连接获取失败: {}", e))
        })?;
        let key = Self::cache_key(config_key);
        let _: () = conn
            .set(&key, config_value)
            .await
            .map_err(|e| AppError::Internal(format!("Redis写入失败: {}", e)))?;
        Ok(())
    }

    /// 失效缓存
    async fn invalidate_cache(&self, config_key: &str) -> AppResult<()> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis连接获取失败: {}", e))
        })?;
        let key = Self::cache_key(config_key);
        let _: () = conn
            .del(&key)
            .await
            .map_err(|e| AppError::Internal(format!("Redis删除失败: {}", e)))?;
        Ok(())
    }

    /// 根据参数键名更新参数值并失效缓存
    #[tracing::instrument(
        skip(self, ctx, current_user),
        fields(operator = %current_user.0.username, config_key = config_key)
    )]
    pub async fn update_by_key(
        &self,
        ctx: &DataContext<'_>,
        config_key: &str,
        config_value: &str,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        // 1. 获取已有记录
        let mut existing =
            self.repo.select_by_key(ctx, config_key).await?.ok_or_else(
                || {
                    AppError::NotFound(format!(
                        "更新参数配置失败: 配置不存在 (configKey={})",
                        config_key
                    ))
                },
            )?;

        // 2. 更新字段(update_* 由漏斗按 principal 盖章,原手工
        // base_entity.update 不再需要)
        existing.config_value = config_value.to_string();

        let config_id = existing.config_id.ok_or_else(|| {
            AppError::Internal(format!(
                "更新参数配置失败: 配置ID为空 (configKey={})",
                config_key
            ))
        })?;

        // 3. 持久化
        self.repo.update_by_id(ctx, &existing, &config_id).await?;

        // 4. 失效缓存 (best-effort: 已提交 DB, 缓存失败只记录日志)
        if let Err(e) = self.invalidate_cache(config_key).await {
            tracing::warn!(
                config_key = config_key,
                error = %e,
                "按键名更新参数配置后失效缓存失败"
            );
        }

        tracing::info!(
            config_key = config_key,
            operator = %current_user.0.username,
            "按键名更新参数配置成功"
        );
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
    pub async fn page(
        &self,
        ctx: &DataContext<'_>,
        query: ConfigPageQuery,
    ) -> AppResult<TableDataInfo<ConfigListVo>> {
        let (current, size, _offset) = normalize_paging(&query.page);
        let allowed = [
            ("configId", "config_id"),
            ("configName", "config_name"),
            ("configKey", "config_key"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "config_id asc".to_string());

        let page_result = self
            .repo
            .select_page(ctx, &query, current as u64, size as u64, &order_by)
            .await?;
        if page_result.total <= 0 {
            return Ok(TableDataInfo::empty());
        }
        tracing::info!(
            current,
            size,
            total = page_result.total,
            "参数配置分页查询成功"
        );
        Ok(page_result)
    }

    /// 按查询条件取全部参数配置(导出用),与 page 共用同一套 filter/order。
    #[tracing::instrument(skip(self, ctx), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
    pub async fn list_all(
        &self,
        ctx: &DataContext<'_>,
        query: ConfigPageQuery,
    ) -> AppResult<Vec<Config>> {
        let allowed = [
            ("configId", "config_id"),
            ("configName", "config_name"),
            ("configKey", "config_key"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "config_id asc".to_string());

        let list = self.repo.select_list(ctx, &query, &order_by).await?;
        Ok(list)
    }

    #[tracing::instrument(skip(self, ctx), fields(config_id = id))]
    pub async fn get_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<Config> {
        let config =
            self.repo.select_by_id(ctx, &id).await?.ok_or_else(|| {
                AppError::NotFound(format!("配置不存在 (id={})", id))
            })?;
        Ok(config)
    }

    #[tracing::instrument(
        skip(self, ctx, bo, current_user),
        fields(operator = %current_user.0.username)
    )]
    pub async fn insert_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: ConfigCreateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        // 1. 检查参数键名唯一性
        if self
            .repo
            .select_by_key(ctx, &bo.config_key)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(format!(
                "参数键名已存在 (configKey={})",
                bo.config_key
            )));
        }

        // 2. 创建实体并插入(审计五列由漏斗按 ctx principal 盖章)
        let config = Config::from_create_bo(bo, current_user);
        self.repo.insert(ctx, &config).await?;

        // 3. 写入缓存
        self.set_cache(&config.config_key, &config.config_value)
            .await?;

        tracing::info!(
            config_key = %config.config_key,
            operator = %current_user.0.username,
            "创建参数配置成功"
        );
        Ok(())
    }

    #[tracing::instrument(
        skip(self, ctx, bo, current_user),
        fields(operator = %current_user.0.username, config_id = ?bo.config_id)
    )]
    pub async fn update_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: ConfigUpdateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        let config_id = bo.config_id.ok_or_else(|| {
            AppError::BadRequest("更新参数配置失败: 参数ID为空".to_string())
        })?;

        // 1. 获取已有记录
        let mut existing =
            self.repo.select_by_id(ctx, &config_id).await?.ok_or_else(
                || {
                    AppError::NotFound(format!(
                        "更新参数配置失败: 配置不存在 (id={})",
                        config_id
                    ))
                },
            )?;

        let old_config_key = existing.config_key.clone();

        // 2. 检查新参数键名唯一性 (如果键名变更)
        if bo.config_key != old_config_key {
            if self
                .repo
                .select_by_key(ctx, &bo.config_key)
                .await?
                .is_some()
            {
                return Err(AppError::Conflict(format!(
                    "参数键名已存在 (configKey={})",
                    bo.config_key
                )));
            }

            // 3. 失效旧缓存
            self.invalidate_cache(&old_config_key).await?;
        }

        // 4. 更新实体(update_* 由漏斗按 principal 盖章)
        existing.from_update_bo(bo, current_user);
        self.repo.update_by_id(ctx, &existing, &config_id).await?;

        // 5. 写入新缓存
        self.set_cache(&existing.config_key, &existing.config_value)
            .await?;

        tracing::info!(
            config_id = config_id,
            operator = %current_user.0.username,
            "更新参数配置成功"
        );
        Ok(())
    }

    #[tracing::instrument(
        skip(self, ctx, current_user),
        fields(operator = %current_user.0.username, config_id = id)
    )]
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        // 1. 获取已有记录
        let existing =
            self.repo.select_by_id(ctx, &id).await?.ok_or_else(|| {
                AppError::NotFound(format!(
                    "删除参数配置失败: 配置不存在 (id={})",
                    id
                ))
            })?;

        // 2. 删除记录(0 行 → NotFound 由漏斗承担)
        self.repo.delete_by_id(ctx, &id).await?;

        // 3. 失效缓存
        self.invalidate_cache(&existing.config_key).await?;

        tracing::info!(
            config_id = id,
            operator = %current_user.0.username,
            "删除参数配置成功"
        );
        Ok(())
    }

    /// 刷新全部缓存
    #[tracing::instrument(skip(self))]
    pub async fn refresh_cache(&self) -> AppResult<()> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis连接获取失败: {}", e))
        })?;

        // 使用 SCAN 避免阻塞 Redis
        let pattern = format!("{}*", CACHE_KEY_PREFIX);
        let mut keys = Vec::new();
        let mut cursor: u64 = 0;
        loop {
            let (next_cursor, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut *conn)
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Redis SCAN 失败: {}", e))
                })?;
            keys.extend(batch);
            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        if !keys.is_empty() {
            let _: () = conn.del(&keys).await.map_err(|e| {
                AppError::Internal(format!("Redis删除失败: {}", e))
            })?;
        }

        tracing::info!("刷新参数配置缓存成功, 删除 {} 个键", keys.len());
        Ok(())
    }
}
