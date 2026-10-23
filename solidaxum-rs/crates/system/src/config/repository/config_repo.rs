//! config 仓储(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:现网
//! `SysConfigMapper.java` **零 `@DataPermission`**(接口体为空,全走
//! BaseMapperPlus)—— 参数配置全局生效,数据权限过滤对它无业务含义。
//! 统一锚:**SysConfigMapper 无 @DataPermission —— 参数配置全局生效**。
//! 由此每个读方法显式 `without_row_scope()`;sys_config 的 DataPolicy
//! 落 CONSTRAINED=false,无软删列。
//!
//! | 本文件方法 | Java 锚(SysConfigController/ServiceImpl → SysConfigMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_id` | getInfo(Controller:70)→ `selectConfigById`(ServiceImpl:67)→ selectVoById | ❌ | `without_row_scope()` |
//! | `select_by_key` | `/configKey/{key}`(Controller:81)→ `selectConfigByKey`(ServiceImpl:79,`@Cacheable(SYS_CONFIG)`:77 缓存路径;updateByKey(Controller:131)与唯一性校验也走它) | ❌ | `without_row_scope()`(唯一性须全量) |
//! | `select_page` | list(Controller:44)→ `selectPageConfigList`(ServiceImpl:54)→ selectVoPage | ❌ | `without_row_scope()`(逃生舱分页,total 全量口径) |
//! | `select_list` | export(Controller:57)→ `selectConfigList`(ServiceImpl:101)→ selectVoList | ❌ | `without_row_scope()` |
//! | `insert` | add(Controller:95)→ `insertConfig`(ServiceImpl:131,写路径) | — | `ctx.insert`(审计五列漏斗盖) |
//! | `update_by_id` | edit(Controller:113)→ `updateConfig`(ServiceImpl:148,写路径) | — | `ctx.update`(豁免实体;审计盖 update_*,0 行 → NotFound) |
//! | `delete_by_id`(物理) | remove(Controller:145)→ `deleteConfigByIds`(ServiceImpl:175)→ deleteByIds | — | `ctx.delete`(WHERE=主键,0 行 → NotFound) |
//!
//! **缓存互锚**:Java `selectConfigByKey` 挂 `@Cacheable(SYS_CONFIG)`
//! (ServiceImpl:79)—— Rust 侧 config_service 的 `sys_config:{k}` Redis
//! 旁路与它对位,缓存未命中才落库,落库读即本文件 `select_by_key`
//! (豁免口径);写路径的缓存失效(evict/put)在 service 层,与漏斗无关。

use app_core::{
    model::entity::BaseEntity, repo::str_trim_or_empty, vo::page::TableDataInfo,
};
use app_data::{
    DataContext, DataError, DataSource,
    sea_orm::{
        self, ActiveModelBehavior,
        ActiveValue::{NotSet, Set},
        ColumnTrait, DateTimeUtc, DeriveEntityModel, DerivePrimaryKey,
        DeriveRelation, EntityTrait, EnumIter, Order, PrimaryKeyTrait,
        QueryFilter, QueryOrder, Select,
    },
};

use crate::config::model::config::{Config, ConfigListVo, ConfigPageQuery};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_config")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub config_id: i64,
    pub config_name: String,
    pub config_key: String,
    pub config_value: String,
    pub config_type: String,
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

impl From<&Model> for Config {
    fn from(m: &Model) -> Self {
        Config {
            config_id: Some(m.config_id),
            config_name: m.config_name.clone(),
            config_key: m.config_key.clone(),
            config_value: m.config_value.clone(),
            config_type: m.config_type.clone(),
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
}

/// Config → ActiveModel。审计五列只搬运领域层已持有的值;**写入时由
/// 漏斗统一盖章**(insert 全盖、update 盖 update_*),领域层不再手工盖章。
fn config_to_active_model(config: &Config) -> ActiveModel {
    ActiveModel {
        config_id: match config.config_id {
            Some(id) => Set(id),
            None => NotSet,
        },
        config_name: Set(config.config_name.clone()),
        config_key: Set(config.config_key.clone()),
        config_value: Set(config.config_value.clone()),
        config_type: Set(config.config_type.clone()),
        create_dept: Set(config.base_entity.create_dept),
        create_by: Set(config.base_entity.create_by),
        create_time: Set(config.base_entity.create_time),
        update_by: Set(config.base_entity.update_by),
        update_time: Set(config.base_entity.update_time),
        remark: Set(config.remark.clone()),
    }
}

#[derive(Clone)]
pub struct ConfigRepository {
    /// 装配期快照:方法只转发调用方 ctx(豁免读不解析 resolver,
    /// 降级快照无影响)。
    ds: std::sync::Arc<DataSource>,
}

impl ConfigRepository {
    pub fn new(ds: std::sync::Arc<DataSource>) -> Self {
        Self { ds }
    }

    /// 测试/工具桥接面:system_context。生产零消费,dict_type_repo 同形。
    pub fn system_context(&self) -> DataContext<'_> {
        self.ds.system_context()
    }

    /// 按主键查详情。
    ///
    /// 豁免裁定:SysConfigMapper 无 @DataPermission —— 参数配置全局生效
    /// (Java 锚 selectConfigById ServiceImpl:67 → selectVoById)。
    pub async fn select_by_id(
        &self,
        ctx: &DataContext<'_>,
        config_id: &i64,
    ) -> Result<Option<Config>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(*config_id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Config::from))
    }

    /// 按参数键名查(缓存未命中的落库读 + 唯一性校验)。
    ///
    /// 豁免裁定:SysConfigMapper 无 @DataPermission —— 参数配置全局生效
    /// (Java 锚 selectConfigByKey ServiceImpl:79,`@Cacheable(SYS_CONFIG)`:77
    /// 的落库半边;唯一性校验共用,唯一性须全量)。
    pub async fn select_by_key(
        &self,
        ctx: &DataContext<'_>,
        config_key: &str,
    ) -> Result<Option<Config>, DataError> {
        let model = ctx
            .select(Entity::find().filter(Column::ConfigKey.eq(config_key)))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(Config::from))
    }

    /// 分页查询参数配置列表。
    ///
    /// 豁免裁定:SysConfigMapper 无 @DataPermission —— 参数配置全局生效
    /// (Java 锚 list → selectPageConfigList ServiceImpl:54 → selectVoPage;
    /// 排序在 Select 构件上,total 为全量口径)。
    pub async fn select_page(
        &self,
        ctx: &DataContext<'_>,
        query: &ConfigPageQuery,
        page_no: u64,
        page_size: u64,
        order_by: &str,
    ) -> Result<TableDataInfo<ConfigListVo>, DataError> {
        let mut select = Self::apply_filters(Entity::find(), query);

        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;

        let rows: Vec<ConfigListVo> = page
            .rows
            .iter()
            .map(Config::from)
            .map(|c: Config| c.to_list_vo())
            .collect();

        Ok(TableDataInfo::from_page_rows(&page, rows))
    }

    /// 参数配置列表的过滤条件。**`select_page`(列表)与 `select_list`(导出)共用**，
    /// 保证导出结果与页面列表始终一致。
    fn apply_filters(
        mut select: Select<Entity>,
        query: &ConfigPageQuery,
    ) -> Select<Entity> {
        let config_name = str_trim_or_empty(&query.config_name);
        let config_key = str_trim_or_empty(&query.config_key);
        let config_type = str_trim_or_empty(&query.config_type);

        if !config_name.is_empty() {
            select = select.filter(Column::ConfigName.contains(&config_name));
        }
        if !config_key.is_empty() {
            select = select.filter(Column::ConfigKey.contains(&config_key));
        }
        if !config_type.is_empty() {
            select = select.filter(Column::ConfigType.eq(&config_type));
        }

        select
    }

    /// 按过滤条件查询全部参数配置(不分页,与 select_page 共用同一套 filter)。
    ///
    /// 豁免裁定:SysConfigMapper 无 @DataPermission —— 参数配置全局生效
    /// (Java 锚 export → selectConfigList ServiceImpl:101 → selectVoList)。
    pub async fn select_list(
        &self,
        ctx: &DataContext<'_>,
        query: &ConfigPageQuery,
        order_by: &str,
    ) -> Result<Vec<Config>, DataError> {
        let mut select = Self::apply_filters(Entity::find(), query);

        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let models = ctx.select(select).without_row_scope().all().await?;
        Ok(models.iter().map(Config::from).collect())
    }

    /// 新增参数配置。审计五列由漏斗 insert 统一盖章(principal 取自 ctx)
    /// (Java 锚 insertConfig ServiceImpl:131,`@CachePut(SYS_CONFIG)` 写
    /// 缓存在 service 层)。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        config: &Config,
    ) -> Result<(), DataError> {
        let active = config_to_active_model(config);
        ctx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// 按主键更新。漏斗 update:审计盖 update_*(create_* 搬运领域层
    /// 原值),WHERE = 主键(豁免实体),0 行 → NotFound(原"先查后改"
    /// 预检随漏斗 WHERE 删除,service 侧先读拼领域对象,语义相同;
    /// Java 锚 updateConfig ServiceImpl:148)。
    pub async fn update_by_id(
        &self,
        ctx: &DataContext<'_>,
        config: &Config,
        config_id: &i64,
    ) -> Result<(), DataError> {
        let mut active = config_to_active_model(config);
        active.config_id = Set(*config_id);
        ctx.update(active).await?;
        Ok(())
    }

    /// 物理删除参数配置(sys_config 无 @TableLogic,Java 侧同为物理删)。
    /// 漏斗 delete:WHERE = 主键,0 行 → NotFound(Java 锚
    /// deleteConfigByIds ServiceImpl:175)。
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        config_id: &i64,
    ) -> Result<(), DataError> {
        ctx.delete::<Entity>(*config_id).await
    }
}

fn parse_order(order_by: &str) -> (Column, Order) {
    let mut parts = order_by.split_whitespace();
    let col = match parts.next().unwrap_or("config_id") {
        "config_id" => Column::ConfigId,
        "config_name" => Column::ConfigName,
        "config_key" => Column::ConfigKey,
        "create_time" => Column::CreateTime,
        _ => Column::ConfigId,
    };
    let desc = !parts.next().is_some_and(|d| d.eq_ignore_ascii_case("asc"));
    if desc {
        (col, Order::Desc)
    } else {
        (col, Order::Asc)
    }
}
