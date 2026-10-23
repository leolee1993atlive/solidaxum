//! dict_data 服务 —— 编排层。
//!
//! ctx 由 handler 请求期构造并透传;Redis 缓存旁路(`sys_dict:{t}`,
//! 对位 Java `@Cacheable(SYS_DICT)` selectDictDataByType ServiceImpl:106)
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

use crate::dict::{
    model::dict_data::{
        DictData, DictDataCreateBo, DictDataListVo, DictDataPageQuery,
        DictDataUpdateBo,
    },
    repository::dict_data_repo::DictDataRepository,
};

const CACHE_TTL: u64 = 30 * 24 * 60 * 60; // 30天
const CACHE_KEY_PREFIX: &str = "sys_dict:";

#[derive(Clone)]
pub struct DictDataService {
    repo: DictDataRepository,
    redis_pool: Pool,
}

impl DictDataService {
    /// 装配期只取 DataSource 快照:全部方法只**转发** ctx(豁免读不解析
    /// resolver,降级快照无影响)。
    pub fn new(infra: &Infrastructure) -> Self {
        Self {
            repo: DictDataRepository::new(infra.data_source()),
            redis_pool: infra.pool.redis_pool.clone(),
        }
    }

    fn cache_key(dict_type: &str) -> String {
        format!("{}{}", CACHE_KEY_PREFIX, dict_type)
    }

    /// 缓存旁路: 读缓存 → 未命中 → 读数据库 → 写缓存
    /// (对位 Java `@Cacheable(SYS_DICT)` selectDictDataByType
    /// ServiceImpl:106;落库读是仓储豁免读,字典数据非行级)
    #[tracing::instrument(skip(self, ctx), fields(dict_type = dict_type))]
    pub async fn get_by_type(
        &self,
        ctx: &DataContext<'_>,
        dict_type: &str,
    ) -> AppResult<Vec<DictDataListVo>> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            AppError::Internal(format!("Redis连接获取失败: {}", e))
        })?;
        let key = Self::cache_key(dict_type);

        // 1. 读缓存
        let cached: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| AppError::Internal(format!("Redis读取失败: {}", e)))?;

        if let Some(json) = cached {
            let vos: Vec<DictDataListVo> = serde_json::from_str(&json)
                .map_err(|e| {
                    AppError::Internal(format!("Redis缓存反序列化失败: {}", e))
                })?;
            return Ok(vos);
        }

        // 2. 读数据库
        let data = self.repo.select_by_type(ctx, dict_type).await?;
        let vos: Vec<DictDataListVo> =
            data.iter().map(|d| d.to_list_vo()).collect();

        // 3. 写缓存
        let json = serde_json::to_string(&vos).map_err(|e| {
            AppError::Internal(format!("序列化字典数据失败: {}", e))
        })?;
        let _: () = conn
            .set_ex(&key, &json, CACHE_TTL)
            .await
            .map_err(|e| AppError::Internal(format!("Redis写入失败: {}", e)))?;

        Ok(vos)
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

    /// 全量查询(供 export;不过滤 dict_type)
    pub async fn list_all(
        &self,
        ctx: &DataContext<'_>,
    ) -> AppResult<Vec<DictDataListVo>> {
        let list = self.repo.select_all(ctx).await?;
        Ok(list.iter().map(|d| d.to_list_vo()).collect())
    }

    #[tracing::instrument(skip(self, ctx), fields(page = ?query.page.page_num, size = ?query.page.page_size))]
    pub async fn page(
        &self,
        ctx: &DataContext<'_>,
        query: DictDataPageQuery,
    ) -> AppResult<TableDataInfo<DictDataListVo>> {
        let (current, size, _offset) = normalize_paging(&query.page);
        let allowed = [
            ("dictCode", "dict_code"),
            ("dictSort", "dict_sort"),
            ("dictLabel", "dict_label"),
            ("dictValue", "dict_value"),
            ("dictType", "dict_type"),
            ("createTime", "create_time"),
        ];
        let order_by = build_order_by(
            &query.page.order_by_column,
            &query.page.is_asc,
            &allowed,
        )
        .unwrap_or_else(|| "dict_sort asc".to_string());

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
            "字典数据分页查询成功"
        );
        Ok(page_result)
    }

    #[tracing::instrument(skip(self, ctx), fields(dict_code = id))]
    pub async fn get_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
    ) -> AppResult<DictData> {
        let dict_data =
            self.repo.select_by_id(ctx, &id).await?.ok_or_else(|| {
                AppError::NotFound(format!("字典数据不存在 (id={})", id))
            })?;
        Ok(dict_data)
    }

    #[tracing::instrument(
        skip(self, ctx, bo, current_user),
        fields(operator = %current_user.0.username)
    )]
    pub async fn insert_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: DictDataCreateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        // H-3: (dict_type, dict_value) 唯一性校验(对齐 Java checkDictDataUnique)
        if self
            .repo
            .exists_by_type_and_value(ctx, &bo.dict_type, &bo.dict_value, None)
            .await?
        {
            return Err(AppError::Conflict(format!(
                "新增字典数据'{}'失败,字典键值已存在",
                bo.dict_value
            )));
        }
        let dict_data = DictData::from_create_bo(bo, current_user);
        // 审计五列由漏斗按 ctx principal 盖章
        self.repo.insert(ctx, &dict_data).await?;

        // 失效缓存
        self.invalidate_cache(&dict_data.dict_type).await?;

        tracing::info!(
            dict_type = %dict_data.dict_type,
            operator = %current_user.0.username,
            "创建字典数据成功"
        );
        Ok(())
    }

    #[tracing::instrument(
        skip(self, ctx, bo, current_user),
        fields(operator = %current_user.0.username, dict_code = ?bo.dict_code)
    )]
    pub async fn update_by_bo(
        &self,
        ctx: &DataContext<'_>,
        bo: DictDataUpdateBo,
        current_user: &CurrentUser,
    ) -> AppResult<()> {
        let dict_code = bo.dict_code.ok_or_else(|| {
            AppError::BadRequest("更新字典数据失败: 字典编码为空".to_string())
        })?;

        // 1. 获取已有记录
        let mut existing =
            self.repo.select_by_id(ctx, &dict_code).await?.ok_or_else(
                || {
                    AppError::NotFound(format!(
                        "更新字典数据失败: 字典数据不存在 (id={})",
                        dict_code
                    ))
                },
            )?;

        let old_dict_type = existing.dict_type.clone();

        // H-3: (dict_type, dict_value) 唯一性校验,排除自身 dict_code
        // (对齐 Java checkDictDataUnique 的 neIfPresent)
        if self
            .repo
            .exists_by_type_and_value(
                ctx,
                &bo.dict_type,
                &bo.dict_value,
                Some(dict_code),
            )
            .await?
        {
            return Err(AppError::Conflict(format!(
                "修改字典数据'{}'失败,字典键值已存在",
                bo.dict_value
            )));
        }

        // 2. 更新实体(update_* 由漏斗按 principal 盖章)
        existing.from_update_bo(bo, current_user);
        self.repo.update_by_id(ctx, &existing, &dict_code).await?;

        // 3. 失效缓存 (新旧 dict_type 都失效)
        if existing.dict_type != old_dict_type {
            self.invalidate_cache(&old_dict_type).await?;
        }
        self.invalidate_cache(&existing.dict_type).await?;

        tracing::info!(
            dict_code = dict_code,
            operator = %current_user.0.username,
            "更新字典数据成功"
        );
        Ok(())
    }

    #[tracing::instrument(
        skip(self, ctx, current_user),
        fields(operator = %current_user.0.username, dict_code = id)
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
                    "删除字典数据失败: 字典数据不存在 (id={})",
                    id
                ))
            })?;

        // 2. 删除记录(0 行 → NotFound 由漏斗承担)
        self.repo.delete_by_id(ctx, &id).await?;

        // 3. 失效缓存
        self.invalidate_cache(&existing.dict_type).await?;

        tracing::info!(
            dict_code = id,
            operator = %current_user.0.username,
            "删除字典数据成功"
        );
        Ok(())
    }
}
