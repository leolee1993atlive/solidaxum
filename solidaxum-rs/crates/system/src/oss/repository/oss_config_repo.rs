//! oss_config 仓储(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:现网
//! `SysOssConfigMapper.java` **零 `@DataPermission`**(接口体为空,全走
//! BaseMapperPlus)—— 存储桶配置是全局配置(唯一性校验与 S3 客户端装配
//! 读都须全量)。统一锚:**SysOssConfigMapper 无 @DataPermission ——
//! 桶配置是全局配置**。由此每个读方法显式 `without_row_scope()`;
//! sys_oss_config 无软删列(policy.rs 如实),删除是物理删。
//!
//! | 本文件方法 | Java 锚(SysOssConfigController/ServiceImpl → SysOssConfigMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_id` | getInfo(Controller:61)→ `queryById`(SysOssConfigServiceImpl:72)→ selectVoById | ❌ | `without_row_scope()` |
//! | `select_by_config_key` | `checkConfigKeyUnique`(ServiceImpl:198)`lambda().eq(configKey).one()`;同键读亦是 OssClientFactory 客户端装配路径 | ❌ | `without_row_scope()`(唯一性/装配读须全量) |
//! | `select_by_status` | Java OssFactory 默认配置装配(init:SysOssConfigServiceImpl:53 起;对位 Rust OssClientFactory 懒装配) | ❌ | `without_row_scope()`(默认配置装配读须全量) |
//! | `select_page` | list(Controller:49)→ `queryPageList`(ServiceImpl:84)→ selectVoPage | ❌ | `without_row_scope()`(逃生舱分页,total 全量口径) |
//! | `insert` | add(Controller:76)→ `insertByBo`(ServiceImpl:113,写路径) | — | `ctx.insert`(审计五列漏斗盖) |
//! | `update_by_id` | edit(Controller:90)→ `updateByBo`(ServiceImpl:132,写路径) | — | `ctx.update`(豁免实体;审计盖 update_*,0 行 → NotFound) |
//! | `update_status_by_id_in_txn` | changeStatus(Controller:118)→ `updateOssConfigStatus`(ServiceImpl:215)**updateById 半边**(@Transactional:214) | — | `tx.update` 单列(审计盖 update_*,0 行 → NotFound) |
//! | `update_status_all_in_txn` | 同上 `.set(status,NO).updateCount()` **全表半边**(@Transactional:214) | — | 读-改-写逐行 `tx.update`(逃生舱无按条件批量 SET 原语,dict `update_dict_type_in_txn` 同款) |
//! | `delete_by_id`(物理) | remove(Controller:103)→ `deleteWithValidByIds`(ServiceImpl:171)→ deleteByIds | — | `ctx.delete`(WHERE=主键,0 行 → NotFound) |
//!
//! **加密互锚**:access_key/secret_key 的字段加密(FieldEncryptor)是
//! 仓储内务(读解密/写加密),与漏斗正交,原样保留;装配读经
//! `system_context` 桥接(OssClientFactory,public 静态路由无请求身份)。

use std::sync::Arc;

use app_core::{
    error::AppError, model::entity::BaseEntity, repo::str_trim_or_empty,
    serde::encrypt_field::FieldEncryptor, vo::page::TableDataInfo,
};
use app_data::{
    DataContext, DataError, DataSource, TxnContext,
    sea_orm::{
        self, ActiveModelBehavior,
        ActiveValue::{NotSet, Set},
        ColumnTrait, DateTimeUtc, DeriveEntityModel, DerivePrimaryKey,
        DeriveRelation, EntityTrait, EnumIter, Order, PrimaryKeyTrait,
        QueryFilter, QueryOrder,
    },
};

use crate::oss::model::oss::{OssConfigQueryBo, SysOssConfig};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_oss_config")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub oss_config_id: i64,
    pub config_key: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket_name: String,
    pub prefix: String,
    pub endpoint: String,
    pub domain_url: String,
    pub is_https: String,
    pub region: String,
    pub status: String,
    pub access_policy: String,
    pub ext1: Option<String>,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTimeUtc>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTimeUtc>,
    pub remark: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

fn config_to_active_model(config: &SysOssConfig) -> ActiveModel {
    ActiveModel {
        oss_config_id: match config.oss_config_id {
            Some(id) => Set(id),
            None => NotSet,
        },
        config_key: Set(config.config_key.clone()),
        access_key: Set(config.access_key.clone()),
        secret_key: Set(config.secret_key.clone()),
        bucket_name: Set(config.bucket_name.clone()),
        prefix: Set(config.prefix.clone()),
        endpoint: Set(config.endpoint.clone()),
        domain_url: Set(config.domain_url.clone()),
        is_https: Set(config.is_https.clone()),
        region: Set(config.region.clone()),
        status: Set(config.status.clone()),
        access_policy: Set(config.access_policy.clone()),
        ext1: Set(config.ext1.clone()),
        create_dept: Set(config.base_entity.create_dept),
        create_by: Set(config.base_entity.create_by),
        create_time: Set(config.base_entity.create_time),
        update_by: Set(config.base_entity.update_by),
        update_time: Set(config.base_entity.update_time),
        remark: Set(config.remark.clone()),
    }
}

#[derive(Clone)]
pub struct OssConfigRepository {
    /// 装配期快照:方法只转发调用方 ctx(豁免读不解析 resolver,
    /// 降级快照无影响)。
    ds: Arc<DataSource>,
    encryptor: Option<Arc<dyn FieldEncryptor>>,
}

impl OssConfigRepository {
    pub fn new(ds: Arc<DataSource>) -> Self {
        Self {
            ds,
            encryptor: None,
        }
    }

    pub fn with_encryptor(
        ds: Arc<DataSource>,
        encryptor: Option<Arc<dyn FieldEncryptor>>,
    ) -> Self {
        Self { ds, encryptor }
    }

    /// 测试/工具桥接面:system_context。OssClientFactory 的装配读走这里
    /// (public 静态路由无请求身份;豁免读不解析 resolver,system_context
    /// 语义下无影响),notice/client 同形。
    pub fn system_context(&self) -> DataContext<'_> {
        self.ds.system_context()
    }

    /// 从 Model 转换为 SysOssConfig，若有加密器则解密密钥字段。
    fn model_to_config(&self, m: &Model) -> SysOssConfig {
        let (access_key, secret_key) = match &self.encryptor {
            Some(enc) => {
                let access_key =
                    enc.decrypt(&m.access_key).unwrap_or_else(|e| {
                        tracing::warn!(
                            error = %e,
                            field = "access_key",
                            "OSS access_key 解密失败，使用原值"
                        );
                        m.access_key.clone()
                    });
                let secret_key =
                    enc.decrypt(&m.secret_key).unwrap_or_else(|e| {
                        tracing::warn!(
                            error = %e,
                            field = "secret_key",
                            "OSS secret_key 解密失败，使用原值"
                        );
                        m.secret_key.clone()
                    });
                (access_key, secret_key)
            }
            None => (m.access_key.clone(), m.secret_key.clone()),
        };
        SysOssConfig {
            oss_config_id: Some(m.oss_config_id),
            config_key: m.config_key.clone(),
            access_key,
            secret_key,
            bucket_name: m.bucket_name.clone(),
            prefix: m.prefix.clone(),
            endpoint: m.endpoint.clone(),
            domain_url: m.domain_url.clone(),
            is_https: m.is_https.clone(),
            region: m.region.clone(),
            status: m.status.clone(),
            access_policy: m.access_policy.clone(),
            ext1: m.ext1.clone(),
            remark: m.remark.clone(),
            base_entity: BaseEntity {
                create_dept: m.create_dept,
                create_by: m.create_by,
                create_time: m.create_time,
                update_by: m.update_by,
                update_time: m.update_time,
            },
        }
    }

    /// 加密 SysOssConfig 的密钥字段，返回可用于持久化的克隆。
    fn encrypt_config(
        &self,
        config: &SysOssConfig,
    ) -> Result<SysOssConfig, AppError> {
        match &self.encryptor {
            Some(enc) => {
                let mut cloned = config.clone();
                cloned.access_key =
                    enc.encrypt(&config.access_key).map_err(|e| {
                        AppError::Internal(format!(
                            "access_key 加密失败: {}",
                            e
                        ))
                    })?;
                cloned.secret_key =
                    enc.encrypt(&config.secret_key).map_err(|e| {
                        AppError::Internal(format!(
                            "secret_key 加密失败: {}",
                            e
                        ))
                    })?;
                Ok(cloned)
            }
            None => Ok(config.clone()),
        }
    }

    /// 按主键查配置详情。
    ///
    /// 豁免裁定:SysOssConfigMapper 无 @DataPermission —— 桶配置是全局
    /// 配置(Java 锚 queryById SysOssConfigServiceImpl:72)。
    pub async fn select_by_id(
        &self,
        ctx: &DataContext<'_>,
        oss_config_id: &i64,
    ) -> Result<Option<SysOssConfig>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(*oss_config_id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(|m| self.model_to_config(m)))
    }

    /// 按 config_key 查(唯一性校验 + S3 客户端装配读)。
    ///
    /// 豁免裁定:SysOssConfigMapper 无 @DataPermission —— 桶配置是全局
    /// 配置(唯一性/装配读须全量;Java 锚 checkConfigKeyUnique
    /// SysOssConfigServiceImpl:198)。
    pub async fn select_by_config_key(
        &self,
        ctx: &DataContext<'_>,
        config_key: &str,
    ) -> Result<Option<SysOssConfig>, DataError> {
        let model = ctx
            .select(Entity::find().filter(Column::ConfigKey.eq(config_key)))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(|m| self.model_to_config(m)))
    }

    /// 新增配置。审计五列由漏斗 insert 统一盖章(principal 取自 ctx)
    /// (Java 锚 insertByBo SysOssConfigServiceImpl:113)。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        config: &SysOssConfig,
    ) -> Result<(), DataError> {
        let encrypted = self.encrypt_config(config).map_err(|e| {
            app_data::sea_orm::DbErr::Custom(format!("OSS 配置加密失败: {}", e))
        })?;
        let active = config_to_active_model(&encrypted);
        ctx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// 按主键更新。漏斗 update:审计盖 update_*(create_* 搬运领域层
    /// 原值),WHERE = 主键(豁免实体),0 行 → NotFound(原"先查后改"
    /// 预检随漏斗 WHERE 删除,service 侧先读拼领域对象,语义相同;
    /// Java 锚 updateByBo SysOssConfigServiceImpl:132)。
    pub async fn update_by_id(
        &self,
        ctx: &DataContext<'_>,
        config: &SysOssConfig,
        oss_config_id: &i64,
    ) -> Result<(), DataError> {
        let encrypted = self.encrypt_config(config).map_err(|e| {
            app_data::sea_orm::DbErr::Custom(format!("OSS 配置加密失败: {}", e))
        })?;
        let mut active = config_to_active_model(&encrypted);
        active.oss_config_id = Set(*oss_config_id);
        ctx.update(active).await?;
        Ok(())
    }

    /// 按状态查(默认配置装配读,status='Y')。
    ///
    /// 豁免裁定:SysOssConfigMapper 无 @DataPermission —— 桶配置是全局
    /// 配置(默认配置装配读须全量;对位 Java OssFactory 的启动装配,
    /// SysOssConfigServiceImpl init:53)。
    pub async fn select_by_status(
        &self,
        ctx: &DataContext<'_>,
        status: &str,
    ) -> Result<Option<SysOssConfig>, DataError> {
        let model = ctx
            .select(Entity::find().filter(Column::Status.eq(status)))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(|m| self.model_to_config(m)))
    }

    /// 全表置状态(changeStatus 的"其余全部 N"半边,事务内)。
    ///
    /// 逃生舱无按条件批量 SET 原语,按 dict `update_dict_type_in_txn`
    /// 先例:豁免读受影响行 → 逐行 `tx.update`(审计盖 update_*;
    /// Java 锚 updateOssConfigStatus SysOssConfigServiceImpl:215 的
    /// `.set(status,NO).updateCount()` 半边,@Transactional:214)。
    /// 关联函数:生产调用点在 service 的 ctx.transaction 闭包内,
    /// dict 同口径。
    pub async fn update_status_all_in_txn(
        tx: &TxnContext<'_>,
        status: &str,
    ) -> Result<(), DataError> {
        let rows = tx.select(Entity::find()).without_row_scope().all().await?;
        for row in rows {
            let active = ActiveModel {
                oss_config_id: Set(row.oss_config_id),
                status: Set(status.to_string()),
                ..Default::default()
            };
            tx.update(active).await?;
        }
        Ok(())
    }

    /// 按主键置状态(changeStatus 的目标半边,事务内)。
    ///
    /// 漏斗 tx.update:审计盖 update_*,WHERE = 主键,0 行 → NotFound
    /// (Java 锚 updateOssConfigStatus 的 updateById 半边,
    /// SysOssConfigServiceImpl:215,@Transactional:214)。
    pub async fn update_status_by_id_in_txn(
        tx: &TxnContext<'_>,
        oss_config_id: &i64,
        status: &str,
    ) -> Result<(), DataError> {
        let active = ActiveModel {
            oss_config_id: Set(*oss_config_id),
            status: Set(status.to_string()),
            ..Default::default()
        };
        tx.update(active).await?;
        Ok(())
    }

    /// 物理删除配置(sys_oss_config 无 @TableLogic,Java 侧同为物理删)。
    /// 漏斗 delete:WHERE = 主键,0 行 → NotFound(原实现静默 Ok;
    /// Java 锚 deleteWithValidByIds SysOssConfigServiceImpl:171)。
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        oss_config_id: &i64,
    ) -> Result<(), DataError> {
        ctx.delete::<Entity>(*oss_config_id).await
    }

    /// 分页查询配置列表。
    ///
    /// 豁免裁定:SysOssConfigMapper 无 @DataPermission —— 桶配置是全局
    /// 配置(Java 锚 queryPageList SysOssConfigServiceImpl:84 →
    /// selectVoPage,buildQueryWrapper:97;排序在 Select 构件上,
    /// total 为全量口径)。
    pub async fn select_page(
        &self,
        ctx: &DataContext<'_>,
        query: &OssConfigQueryBo,
        page_no: u64,
        page_size: u64,
        order_by: &str,
    ) -> Result<TableDataInfo<SysOssConfig>, DataError> {
        let config_key = str_trim_or_empty(&query.config_key);
        let bucket_name = str_trim_or_empty(&query.bucket_name);
        let status = str_trim_or_empty(&query.status);

        let mut select = Entity::find();

        if !config_key.is_empty() {
            select = select.filter(Column::ConfigKey.contains(&config_key));
        }
        if !bucket_name.is_empty() {
            select = select.filter(Column::BucketName.contains(&bucket_name));
        }
        if !status.is_empty() {
            select = select.filter(Column::Status.eq(&status));
        }

        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;

        let rows: Vec<SysOssConfig> =
            page.rows.iter().map(|m| self.model_to_config(m)).collect();

        Ok(TableDataInfo::from_page_rows(&page, rows))
    }
}

fn parse_order(order_by: &str) -> (Column, Order) {
    let mut parts = order_by.split_whitespace();
    let col = match parts.next().unwrap_or("oss_config_id") {
        "oss_config_id" => Column::OssConfigId,
        "config_key" => Column::ConfigKey,
        "create_time" => Column::CreateTime,
        _ => Column::OssConfigId,
    };
    let desc = !parts.next().is_some_and(|d| d.eq_ignore_ascii_case("asc"));
    if desc {
        (col, Order::Desc)
    } else {
        (col, Order::Asc)
    }
}
