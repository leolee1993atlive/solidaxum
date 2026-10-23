//! client 服务 —— 编排层。
//!
//! ctx 由 handler 请求期构造并透传;客户端**自有**的 client-id 缓存
//! (`sys_client#30d#`,30 天 TTL)留在本层 —— 它缓存的是认证客户端
//! 全局配置,与数据范围缓存(scope cache)无关,失效路径
//! (`invalidate_cached_client`)随写路径触发。

use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
    middleware::extractors::CurrentUser,
    vo::page::{TableDataInfo, build_order_by, normalize_paging},
};
use app_data::DataContext;
use deadpool_redis::Pool;
use redis::AsyncCommands;

use crate::client::{
    model::client::{
        ClientChangeStatusBo, ClientCreateBo, ClientListVo, ClientPageQuery,
        ClientUpdateBo, SysClient,
    },
    repository::client_repo::ClientRepository,
};

const CACHE_PREFIX: &str = "sys_client#30d#";
const CACHE_TTL: u64 = 30 * 24 * 60 * 60;

#[derive(Clone)]
pub struct ClientService {
    client_repo: ClientRepository,
    redis_pool: Pool,
}

impl ClientService {
    /// 装配期只取 DataSource 快照:全部方法只**转发** ctx(豁免读不解析
    /// resolver,降级快照无影响)。
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            client_repo: ClientRepository::new(infra.data_source()),
            redis_pool: infra.pool.redis_pool.clone(),
        }
    }

    /// 按 client_id 查客户端(登录/令牌验证路径)。缓存旁路:读缓存 →
    /// 未命中 → 读数据库 → 写缓存。落库读是仓储豁免读;登录路径无
    /// 请求身份,调用方(auth)传 system_context,豁免读不受影响。
    #[tracing::instrument(skip(self, ctx))]
    pub async fn query_by_client_id(
        &self,
        ctx: &DataContext<'_>,
        client_id: &str,
    ) -> AppResult<SysClient> {
        if let Some(cached) = self.get_cached_client(client_id).await? {
            return Ok(cached);
        }

        let client = self
            .client_repo
            .select_by_client_id(ctx, client_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "客户端不存在 (clientId={})",
                    client_id
                ))
            })?;

        self.set_cached_client(client_id, &client).await?;
        Ok(client)
    }

    #[tracing::instrument(skip(self, ctx), fields(page = query.page.page_num, size = query.page.page_size))]
    pub async fn page(
        &self,
        ctx: &DataContext<'_>,
        query: ClientPageQuery,
    ) -> AppResult<TableDataInfo<ClientListVo>> {
        let (current, size, _offset) = normalize_paging(&query.page);
        let allowed = [
            ("id", "id"),
            ("clientId", "client_id"),
            ("clientKey", "client_key"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "id desc".to_string());

        let page_result = self
            .client_repo
            .select_page(ctx, &query, current as u64, size as u64, &order_by)
            .await?;
        if page_result.total <= 0 {
            return Ok(TableDataInfo::empty());
        }
        tracing::info!(
            current,
            size,
            total = page_result.total,
            "客户端分页查询成功"
        );
        Ok(page_result)
    }

    /// 按查询条件取全部客户端(导出用),与 page 共用同一套 filter/order。
    #[tracing::instrument(skip(self, ctx), fields(page = query.page.page_num, size = query.page.page_size))]
    pub async fn list_all(
        &self,
        ctx: &DataContext<'_>,
        query: ClientPageQuery,
    ) -> AppResult<Vec<SysClient>> {
        let allowed = [
            ("id", "id"),
            ("clientId", "client_id"),
            ("clientKey", "client_key"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "id desc".to_string());

        let list = self.client_repo.select_list(ctx, &query, &order_by).await?;
        Ok(list)
    }

    #[tracing::instrument(skip(self, ctx), fields(client_id = id))]
    pub async fn get_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<SysClient> {
        let client =
            self.client_repo.select_by_id(ctx, &id).await?.ok_or_else(
                || AppError::NotFound(format!("客户端不存在 (id={})", id)),
            )?;
        Ok(client)
    }

    #[tracing::instrument(
        skip(self, ctx, bo, current_user),
        fields(operator = %current_user.0.username)
    )]
    pub async fn insert_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: ClientCreateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        if self
            .client_repo
            .select_by_client_key(ctx, &bo.client_key)
            .await?
            .is_some()
        {
            return Err(AppError::BadRequest(format!(
                "客户端key已存在: {}",
                bo.client_key
            )));
        }

        let client =
            SysClient::from_create_bo(bo, current_user).map_err(|e| {
                AppError::Internal(format!("创建客户端失败: {}", e))
            })?;
        // 审计五列由漏斗按 ctx principal 盖章
        self.client_repo.insert(ctx, &client).await?;
        tracing::info!(
            client_key = ?client.client_key,
            operator = %current_user.0.username,
            "创建客户端成功"
        );
        Ok(())
    }

    #[tracing::instrument(
        skip(self, ctx, bo, current_user),
        fields(operator = %current_user.0.username, client_id = ?bo.id)
    )]
    pub async fn update_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: ClientUpdateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        let id = bo.id.ok_or_else(|| {
            AppError::BadRequest("更新客户端失败: 客户端ID为空".to_string())
        })?;
        let mut client =
            self.client_repo.select_by_id(ctx, &id).await?.ok_or_else(
                || {
                    AppError::NotFound(format!(
                        "更新客户端失败: 客户端不存在 (id={})",
                        id
                    ))
                },
            )?;

        if let Some(existing) = self
            .client_repo
            .select_by_client_key(ctx, &bo.client_key)
            .await?
            && existing.id != Some(id)
        {
            return Err(AppError::BadRequest(format!(
                "客户端key已存在: {}",
                bo.client_key
            )));
        }

        client.from_update_bo(bo, current_user);
        // update_* 由漏斗按 principal 盖章;0 行 → NotFound
        self.client_repo.update_by_id(ctx, &client, &id).await?;
        self.invalidate_cached_client(&client.client_id).await?;
        tracing::info!(
            client_id = id,
            operator = %current_user.0.username,
            "更新客户端成功"
        );
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx), fields(client_id = id))]
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<()> {
        let client =
            self.client_repo.select_by_id(ctx, &id).await?.ok_or_else(
                || AppError::NotFound(format!("客户端不存在 (id={})", id)),
            )?;
        self.client_repo.delete_by_id(ctx, &id).await?;
        self.invalidate_cached_client(&client.client_id).await?;
        tracing::info!(client_id = id, "删除客户端成功");
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx, bo), fields(client_id = %bo.client_id))]
    pub async fn change_status(
        &self,
        ctx: &DataContext<'_>,
        bo: ClientChangeStatusBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        let client = self
            .client_repo
            .select_by_client_id(ctx, &bo.client_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "客户端不存在 (clientId={})",
                    bo.client_id
                ))
            })?;
        let id = client.id.unwrap_or_default();
        // update_by/update_time 由漏斗按 ctx principal 盖章(原手工传
        // current_user.0.user_id 的参数已删)
        self.client_repo.change_status(ctx, id, &bo.status).await?;
        self.invalidate_cached_client(&client.client_id).await?;
        tracing::info!(
            client_id = %bo.client_id,
            status = %bo.status,
            "修改客户端状态成功"
        );
        Ok(())
    }

    /// 客户端自有缓存的失效(与 scope 缓存无关,保持原样)
    async fn invalidate_cached_client(&self, client_id: &str) -> AppResult<()> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis连接获取失败: {}", e))
        })?;
        let key = format!("{}{}", CACHE_PREFIX, client_id);
        let _: () = conn
            .del(&key)
            .await
            .map_err(|e| AppError::Internal(format!("Redis删除失败: {}", e)))?;
        Ok(())
    }

    async fn get_cached_client(
        &self,
        client_id: &str,
    ) -> AppResult<Option<SysClient>> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis连接获取失败: {}", e))
        })?;
        let key = format!("{}{}", CACHE_PREFIX, client_id);
        let result: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| AppError::Internal(format!("Redis读取失败: {}", e)))?;
        match result {
            Some(json) => {
                let client: SysClient =
                    serde_json::from_str(&json).map_err(|e| {
                        AppError::Internal(format!(
                            "Redis缓存反序列化失败: {}",
                            e
                        ))
                    })?;
                Ok(Some(client))
            }
            None => Ok(None),
        }
    }

    async fn set_cached_client(
        &self,
        client_id: &str,
        client: &SysClient,
    ) -> AppResult<()> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis连接获取失败: {}", e))
        })?;
        let key = format!("{}{}", CACHE_PREFIX, client_id);
        let json = serde_json::to_string(client).map_err(|e| {
            AppError::Internal(format!("序列化客户端失败: {}", e))
        })?;
        let _: () = conn
            .set_ex(&key, &json, CACHE_TTL)
            .await
            .map_err(|e| AppError::Internal(format!("Redis写入失败: {}", e)))?;
        Ok(())
    }
}
