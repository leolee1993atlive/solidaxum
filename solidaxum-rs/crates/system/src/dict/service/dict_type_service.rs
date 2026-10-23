//! dict_type 服务 —— 编排层。
//!
//! ctx 由 handler 请求期构造(`data_source().context(Principal::..)`)并
//! 透传;Redis 缓存旁路(`sys_dict_type:{t}` / `sys_dict:{t}`,对位 Java
//! `@Cacheable(SYS_DICT_TYPE/SYS_DICT)`)留在本层 —— 缓存命中不落库,
//! 未命中才走仓储的豁免读。写路径:改名分支的级联(cascade)与自身
//! update 在同一 `ctx.transaction`(Java `updateDictType` 的
//! @Transactional ServiceImpl:192 平价,dept 换父分支同款)。

use app_core::{
    error::{AppError, AppResult},
    infra::Infrastructure,
    middleware::extractors::CurrentUser,
    vo::page::{TableDataInfo, build_order_by, normalize_paging},
};
use app_data::DataContext;
use deadpool_redis::Pool;
use redis::AsyncCommands;

use crate::dict::{
    model::dict_type::{
        DictType, DictTypeCreateBo, DictTypeListVo, DictTypePageQuery,
        DictTypeUpdateBo,
    },
    repository::{
        dict_data_repo::DictDataRepository, dict_type_repo::DictTypeRepository,
    },
};

const CACHE_TTL: u64 = 30 * 24 * 60 * 60; // 30天
const CACHE_KEY_PREFIX: &str = "sys_dict_type:";
const DICT_DATA_CACHE_PREFIX: &str = "sys_dict:";

#[derive(Clone)]
pub struct DictTypeService {
    repo: DictTypeRepository,
    /// dict_data 半边:删除守卫(exists_by_type)与改名级联的对侧仓储
    /// (级联本体是关联函数,不经实例;守卫读要实例)。同一 DataSource
    /// 快照构造,与 repo 同源。
    data_repo: DictDataRepository,
    redis_pool: Pool,
}

impl DictTypeService {
    /// 装配期只取 DataSource 快照:全部方法只**转发** ctx,用户请求的
    /// ctx 由 handler 请求期构造(豁免读不解析 resolver,降级快照无影响)。
    pub fn new(infra: &Infrastructure) -> Self {
        let ds = infra.data_source();
        Self {
            repo: DictTypeRepository::new(ds.clone()),
            data_repo: DictDataRepository::new(ds),
            redis_pool: infra.pool.redis_pool.clone(),
        }
    }

    fn cache_key(dict_type: &str) -> String {
        format!("{}{}", CACHE_KEY_PREFIX, dict_type)
    }

    /// 缓存旁路: 读缓存 → 未命中 → 读数据库 → 写缓存
    /// (对位 Java `@Cacheable(SYS_DICT_TYPE)` selectDictTypeByType
    /// ServiceImpl:130;落库读是仓储豁免读,字典数据非行级)
    #[tracing::instrument(skip(self, ctx), fields(dict_type = dict_type))]
    pub async fn get_by_type(
        &self,
        ctx: &DataContext<'_>,
        dict_type: &str,
    ) -> AppResult<DictType> {
        // 1. 读缓存
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis连接获取失败: {}", e))
        })?;
        let key = Self::cache_key(dict_type);
        let cached: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| AppError::Internal(format!("Redis读取失败: {}", e)))?;

        if let Some(json) = cached {
            let dict: DictType = serde_json::from_str(&json).map_err(|e| {
                AppError::Internal(format!("Redis缓存反序列化失败: {}", e))
            })?;
            return Ok(dict);
        }

        // 2. 读数据库
        let dict = self.repo.select_by_type(ctx, dict_type).await?.ok_or_else(
            || {
                AppError::NotFound(format!(
                    "字典类型不存在 (dictType={})",
                    dict_type
                ))
            },
        )?;

        // 3. 写缓存
        let json = serde_json::to_string(&dict).map_err(|e| {
            AppError::Internal(format!("序列化字典类型失败: {}", e))
        })?;
        let _: () = conn
            .set_ex(&key, &json, CACHE_TTL)
            .await
            .map_err(|e| AppError::Internal(format!("Redis写入失败: {}", e)))?;

        Ok(dict)
    }

    /// 写入缓存
    async fn set_cache(&self, dict_type: &DictType) -> AppResult<()> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis连接获取失败: {}", e))
        })?;
        let key = Self::cache_key(&dict_type.dict_type);
        let json = serde_json::to_string(dict_type).map_err(|e| {
            AppError::Internal(format!("序列化字典类型失败: {}", e))
        })?;
        let _: () = conn
            .set_ex(&key, &json, CACHE_TTL)
            .await
            .map_err(|e| AppError::Internal(format!("Redis写入失败: {}", e)))?;
        Ok(())
    }

    /// 失效缓存
    async fn invalidate_cache(&self, dict_type: &str) -> AppResult<()> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis连接获取失败: {}", e))
        })?;
        let key = Self::cache_key(dict_type);
        let _: () = conn
            .del(&key)
            .await
            .map_err(|e| AppError::Internal(format!("Redis删除失败: {}", e)))?;
        Ok(())
    }

    /// 失效字典数据缓存 (sys_dict:{dict_type})
    async fn invalidate_dict_data_cache(
        &self,
        dict_type: &str,
    ) -> AppResult<()> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis连接获取失败: {}", e))
        })?;
        let key = format!("{}{}", DICT_DATA_CACHE_PREFIX, dict_type);
        let _: () = conn
            .del(&key)
            .await
            .map_err(|e| AppError::Internal(format!("Redis删除失败: {}", e)))?;
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
    pub async fn page(
        &self,
        ctx: &DataContext<'_>,
        query: DictTypePageQuery,
    ) -> AppResult<TableDataInfo<DictTypeListVo>> {
        let (current, size, _offset) = normalize_paging(&query.page);
        let allowed = [
            ("dictId", "dict_id"),
            ("dictName", "dict_name"),
            ("dictType", "dict_type"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "create_time desc".to_string());

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
            "字典类型分页查询成功"
        );
        Ok(page_result)
    }

    #[tracing::instrument(skip(self, ctx), fields(dict_id = id))]
    pub async fn get_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<DictType> {
        let dict_type =
            self.repo.select_by_id(ctx, &id).await?.ok_or_else(|| {
                AppError::NotFound(format!("字典类型不存在 (id={})", id))
            })?;
        Ok(dict_type)
    }

    /// 查询全部 (用于下拉选择)
    #[tracing::instrument(skip(self, ctx))]
    pub async fn list(
        &self,
        ctx: &DataContext<'_>,
    ) -> AppResult<Vec<DictTypeListVo>> {
        let list = self.repo.select_all(ctx).await?;
        Ok(list.iter().map(|d| d.to_list_vo()).collect())
    }

    #[tracing::instrument(
        skip(self, ctx, bo, current_user),
        fields(operator = %current_user.0.username)
    )]
    pub async fn insert_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: DictTypeCreateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        // 1. 检查字典类型唯一性
        if self
            .repo
            .select_by_type(ctx, &bo.dict_type)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(format!(
                "字典类型已存在 (dictType={})",
                bo.dict_type
            )));
        }

        // 2. 创建实体并插入(审计五列由漏斗按 ctx principal 盖章)
        let dict_type = DictType::from_create_bo(bo, current_user);
        self.repo.insert(ctx, &dict_type).await?;

        // 3. 写入缓存
        self.set_cache(&dict_type).await?;

        tracing::info!(
            dict_type = %dict_type.dict_type,
            operator = %current_user.0.username,
            "创建字典类型成功"
        );
        Ok(())
    }

    /// 修改字典类型。改名时级联改写 sys_dict_data.dict_type —— 级联与
    /// 自身 update 在同一 `ctx.transaction`(Java `updateDictType` 带
    /// @Transactional ServiceImpl:192,dept 换父分支同款;级联中途
    /// 失败整体回滚,不再留下"type 已改、data 未跟"的悬挂引用)。
    #[tracing::instrument(
        skip(self, ctx, bo, current_user),
        fields(operator = %current_user.0.username, dict_id = ?bo.dict_id)
    )]
    pub async fn update_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: DictTypeUpdateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        let dict_id = bo.dict_id.ok_or_else(|| {
            AppError::BadRequest("更新字典类型失败: 字典类型ID为空".to_string())
        })?;

        // 1. 获取已有记录
        let mut existing = self
            .repo
            .select_by_id(ctx, &dict_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "更新字典类型失败: 字典类型不存在 (id={})",
                    dict_id
                ))
            })?;

        let old_dict_type = existing.dict_type.clone();
        let renamed = bo.dict_type != old_dict_type;

        // 2. 检查新字典类型唯一性 (如果字典类型变更)
        if renamed {
            if self
                .repo
                .select_by_type(ctx, &bo.dict_type)
                .await?
                .is_some()
            {
                return Err(AppError::Conflict(format!(
                    "字典类型已存在 (dictType={})",
                    bo.dict_type
                )));
            }

            // 失效旧缓存(写路径的 evict 在提交前做,失败即中止 —— 与
            // Java CacheUtils.evict 的事务内时序一致)
            self.invalidate_cache(&old_dict_type).await?;
            self.invalidate_dict_data_cache(&old_dict_type).await?;
        }

        // 3. 更新实体(+ 改名级联):同事务(Java @Transactional)
        existing.from_update_bo(bo, current_user);
        let dict_type = existing.clone();
        ctx.transaction(|tx| {
            Box::pin(async move {
                if renamed {
                    DictDataRepository::update_dict_type_in_txn(
                        tx,
                        &old_dict_type,
                        &dict_type.dict_type,
                    )
                    .await?;
                }
                DictTypeRepository::update_by_id_in_txn(tx, &dict_type).await
            })
        })
        .await?;

        // 4. 写入新缓存
        self.set_cache(&existing).await?;

        tracing::info!(
            dict_id = dict_id,
            operator = %current_user.0.username,
            "更新字典类型成功"
        );
        Ok(())
    }

    #[tracing::instrument(
        skip(self, ctx, current_user),
        fields(operator = %current_user.0.username, dict_id = id)
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
                    "删除字典类型失败: 字典类型不存在 (id={})",
                    id
                ))
            })?;

        // 2. 检查是否有字典数据引用(Java deleteDictTypeByIds 的守卫,
        // ServiceImpl:147 exists → "已分配,不能删除")
        if self
            .data_repo
            .exists_by_type(ctx, &existing.dict_type)
            .await?
        {
            return Err(AppError::BadRequest(
                "字典类型下有数据，不允许删除".to_string(),
            ));
        }

        // 3. 删除记录
        self.repo.delete_by_id(ctx, &id).await?;

        // 4. 失效缓存
        self.invalidate_cache(&existing.dict_type).await?;

        tracing::info!(
            dict_id = id,
            operator = %current_user.0.username,
            "删除字典类型成功"
        );
        Ok(())
    }

    /// 刷新全部缓存 (字典类型 + 字典数据)
    #[tracing::instrument(skip(self))]
    pub async fn refresh_cache(&self) -> AppResult<()> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis连接获取失败: {}", e))
        })?;

        // 清除 sys_dict_type:* 和 sys_dict:* 缓存键
        let patterns = [
            format!("{}*", CACHE_KEY_PREFIX),
            format!("{}*", DICT_DATA_CACHE_PREFIX),
        ];
        let mut total_deleted = 0usize;
        for pattern in &patterns {
            let mut keys = Vec::new();
            let mut cursor: u64 = 0;
            loop {
                let (next_cursor, batch): (u64, Vec<String>) =
                    redis::cmd("SCAN")
                        .arg(cursor)
                        .arg("MATCH")
                        .arg(pattern)
                        .arg("COUNT")
                        .arg(100)
                        .query_async(&mut *conn)
                        .await
                        .map_err(|e| {
                            AppError::Internal(format!(
                                "Redis SCAN 失败: {}",
                                e
                            ))
                        })?;
                keys.extend(batch);
                cursor = next_cursor;
                if cursor == 0 {
                    break;
                }
            }

            if !keys.is_empty() {
                total_deleted += keys.len();
                let _: () = conn.del(&keys).await.map_err(|e| {
                    AppError::Internal(format!("Redis删除失败: {}", e))
                })?;
            }
        }

        tracing::info!("刷新字典缓存成功, 删除 {} 个键", total_deleted);
        Ok(())
    }
}
