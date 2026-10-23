//! client 仓储(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:现网
//! `SysClientMapper.java` **零 `@DataPermission`**(接口体为空,全走
//! BaseMapperPlus)—— 认证客户端是全局配置,数据权限过滤对它无业务
//! 含义。统一锚:**SysClientMapper 无 @DataPermission —— 认证客户端
//! 是全局配置**。由此每个读方法显式 `without_row_scope()`;sys_client
//! 的 DataPolicy 是 CONSTRAINED=false + 软删列 del_flag
//! (漏斗自动施加存活过滤)。
//!
//! | 本文件方法 | Java 锚(SysClientController/ServiceImpl → SysClientMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `select_by_client_id` | 登录/令牌路径 `queryByClientId`(SysClientServiceImpl:65)→ `lambda().eq(...).voOne()` | ❌ | `without_row_scope()`(认证读必须全量) |
//! | `select_by_client_key` | `checkClickKeyUnique`(SysClientServiceImpl:191)→ `lambda().exists()` | ❌ | `without_row_scope()`(唯一性须全量) |
//! | `select_by_id` | getInfo(Controller:74)→ `queryById`(SysClientServiceImpl:51) → selectVoById | ❌ | `without_row_scope()` |
//! | `select_page` | list(Controller:48)→ `queryPageList`(SysClientServiceImpl:79) → selectVoPage | ❌ | `without_row_scope()`(逃生舱分页,total 全量口径) |
//! | `select_list` | export(Controller:61)→ `queryList`(SysClientServiceImpl:93) → selectVoList | ❌ | `without_row_scope()` |
//! | `insert` | add(Controller:89)→ `insertByBo`(SysClientServiceImpl:123,写路径) | — | `ctx.insert`(审计五列漏斗盖) |
//! | `update_by_id` | edit(Controller:106)→ `updateByBo`(SysClientServiceImpl:147,写路径) | — | `ctx.update`(豁免实体;审计盖 update_*,0 行 → NotFound) |
//! | `delete_by_id`(软删) | remove(Controller:135)→ `deleteWithValidByIds`(SysClientServiceImpl:180)→ deleteByIds(@TableLogic 软删) | — | 软删 = del_flag 的一次 `ctx.update`(post delete_by_id 同款;审计盖 update_*,0 行 → NotFound) |
//! | `change_status` | Controller:122 changeStatus → `updateClientStatus`(SysClientServiceImpl:164)状态半边 | — | `ctx.update` 单列(审计盖 update_*,0 行 → NotFound) |
//!
//! **客户端自有缓存**:service 层的 client-id 缓存(`sys_client#30d#`,
//! 对位 Java 侧 client 缓存)与 scope 缓存无关,失效路径留在 service,
//! 不在本仓储职责内。

use app_core::{
    model::{entity::BaseEntity, status::DelFlag},
    repo::str_trim_or_empty,
    vo::page::TableDataInfo,
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

use crate::client::model::client::{ClientListVo, ClientPageQuery, SysClient};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_client")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub client_id: String,
    pub client_key: String,
    pub client_secret: String,
    pub grant_type: String,
    pub device_type: Option<String>,
    pub access_path: Option<String>,
    pub ip_whitelist: Option<String>,
    pub active_timeout: i32,
    pub timeout: i32,
    pub status: String,
    pub del_flag: String,
    pub create_dept: Option<i64>,
    pub create_by: Option<i64>,
    pub create_time: Option<DateTimeUtc>,
    pub update_by: Option<i64>,
    pub update_time: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<&Model> for SysClient {
    fn from(m: &Model) -> Self {
        SysClient {
            id: Some(m.id),
            client_id: m.client_id.clone(),
            client_key: m.client_key.clone(),
            client_secret: m.client_secret.clone(),
            grant_type: m.grant_type.clone(),
            device_type: m.device_type.clone(),
            access_path: m.access_path.clone(),
            ip_whitelist: m.ip_whitelist.clone(),
            active_timeout: m.active_timeout,
            timeout: m.timeout,
            status: m.status.clone(),
            del_flag: Some(m.del_flag.clone()),
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

/// SysClient → ActiveModel。审计五列只搬运领域层已持有的值;**写入时由
/// 漏斗统一盖章**(insert 全盖、update 盖 update_*),领域层不再手工盖章。
fn client_to_active_model(client: &SysClient) -> ActiveModel {
    ActiveModel {
        id: match client.id {
            Some(id) => Set(id),
            None => NotSet,
        },
        client_id: Set(client.client_id.clone()),
        client_key: Set(client.client_key.clone()),
        client_secret: Set(client.client_secret.clone()),
        grant_type: Set(client.grant_type.clone()),
        device_type: Set(client.device_type.clone()),
        access_path: Set(client.access_path.clone()),
        ip_whitelist: Set(client.ip_whitelist.clone()),
        active_timeout: Set(client.active_timeout),
        timeout: Set(client.timeout),
        status: Set(client.status.clone()),
        del_flag: Set(client
            .del_flag
            .clone()
            .unwrap_or_else(|| "0".to_string())),
        create_dept: Set(client.base_entity.create_dept),
        create_by: Set(client.base_entity.create_by),
        create_time: Set(client.base_entity.create_time),
        update_by: Set(client.base_entity.update_by),
        update_time: Set(client.base_entity.update_time),
    }
}

#[derive(Clone)]
pub struct ClientRepository {
    /// 装配期快照:方法只转发调用方 ctx(豁免读不解析 resolver,
    /// 降级快照无影响)。
    ds: std::sync::Arc<DataSource>,
}

impl ClientRepository {
    pub fn new(ds: std::sync::Arc<DataSource>) -> Self {
        Self { ds }
    }

    /// 测试/工具桥接面:system_context。生产零消费,dict_type_repo 同形。
    pub fn system_context(&self) -> DataContext<'_> {
        self.ds.system_context()
    }

    /// 按业务 client_id 查(登录/令牌的客户端验证路径)。
    ///
    /// 豁免裁定:SysClientMapper 无 @DataPermission —— 认证客户端是
    /// 全局配置(认证读必须全量;软删过滤由漏斗照施,原手工
    /// soft_delete_filter 已删)。
    pub async fn select_by_client_id(
        &self,
        ctx: &DataContext<'_>,
        client_id: &str,
    ) -> Result<Option<SysClient>, DataError> {
        let model = ctx
            .select(Entity::find().filter(Column::ClientId.eq(client_id)))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(SysClient::from))
    }

    /// 按 client_key 查(唯一性校验用)。
    ///
    /// 豁免裁定:SysClientMapper 无 @DataPermission —— 认证客户端是
    /// 全局配置(唯一性校验须全量)。
    pub async fn select_by_client_key(
        &self,
        ctx: &DataContext<'_>,
        client_key: &str,
    ) -> Result<Option<SysClient>, DataError> {
        let model = ctx
            .select(Entity::find().filter(Column::ClientKey.eq(client_key)))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(SysClient::from))
    }

    /// 按主键查详情。
    ///
    /// 豁免裁定:SysClientMapper 无 @DataPermission —— 认证客户端是
    /// 全局配置(Java 锚 queryById SysClientServiceImpl:51)。
    pub async fn select_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: &i64,
    ) -> Result<Option<SysClient>, DataError> {
        let model = ctx
            .select(Entity::find_by_id(*id))
            .without_row_scope()
            .one()
            .await?;
        Ok(model.as_ref().map(SysClient::from))
    }

    /// 分页查询客户端列表。
    ///
    /// 豁免裁定:SysClientMapper 无 @DataPermission —— 认证客户端是
    /// 全局配置(Java 锚 queryPageList SysClientServiceImpl:79 → selectVoPage;排序在
    /// Select 构件上,total 为全量口径)。
    pub async fn select_page(
        &self,
        ctx: &DataContext<'_>,
        query: &ClientPageQuery,
        page_no: u64,
        page_size: u64,
        order_by: &str,
    ) -> Result<TableDataInfo<ClientListVo>, DataError> {
        let mut select = Self::apply_filters(Entity::find(), query);

        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;

        let rows: Vec<ClientListVo> = page
            .rows
            .iter()
            .map(SysClient::from)
            .map(|c: SysClient| c.to_list_vo())
            .collect();

        Ok(TableDataInfo::from_page_rows(&page, rows))
    }

    /// 客户端列表的过滤条件。**`select_page`(列表)与 `select_list`(导出)共用**，
    /// 保证导出结果与页面列表始终一致。
    fn apply_filters(
        mut select: Select<Entity>,
        query: &ClientPageQuery,
    ) -> Select<Entity> {
        let client_key = str_trim_or_empty(&query.client_key);
        let client_secret = str_trim_or_empty(&query.client_secret);
        let status = str_trim_or_empty(&query.status);

        if !client_key.is_empty() {
            select = select.filter(Column::ClientKey.contains(&client_key));
        }
        // clientSecret 是凭据:精确匹配,不用 contains —— 模糊匹配等于给出
        // 逐字符探测的侧信道。
        if !client_secret.is_empty() {
            select = select.filter(Column::ClientSecret.eq(&client_secret));
        }
        if !status.is_empty() {
            select = select.filter(Column::Status.eq(&status));
        }

        select
    }

    /// 按过滤条件查询全部客户端(不分页,与 select_page 共用同一套 filter)。
    ///
    /// 豁免裁定:SysClientMapper 无 @DataPermission —— 认证客户端是
    /// 全局配置(Java 锚 export → queryList SysClientServiceImpl:93 → selectVoList)。
    pub async fn select_list(
        &self,
        ctx: &DataContext<'_>,
        query: &ClientPageQuery,
        order_by: &str,
    ) -> Result<Vec<SysClient>, DataError> {
        let mut select = Self::apply_filters(Entity::find(), query);

        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let models = ctx.select(select).without_row_scope().all().await?;
        Ok(models.iter().map(SysClient::from).collect())
    }

    /// 新增客户端。审计五列由漏斗 insert 统一盖章(principal 取自 ctx)
    /// (Java 锚 insertByBo SysClientServiceImpl:123)。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        client: &SysClient,
    ) -> Result<(), DataError> {
        let active = client_to_active_model(client);
        ctx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// 按主键更新。漏斗 update:审计盖 update_*(create_* 搬运领域层
    /// 原值),WHERE = 主键 + 软删存活(豁免实体无行级范围),0 行 →
    /// NotFound(原"先查后改"预检随漏斗 WHERE 删除;Java 锚
    /// updateByBo SysClientServiceImpl:147)。
    pub async fn update_by_id(
        &self,
        ctx: &DataContext<'_>,
        client: &SysClient,
        id: &i64,
    ) -> Result<(), DataError> {
        let mut active = client_to_active_model(client);
        active.id = Set(*id);
        ctx.update(active).await?;
        Ok(())
    }

    /// 软删客户端(del_flag 置 '1',对齐 Java @TableLogic deleteByIds)。
    /// 漏斗的 `delete` 是物理删除,不用于此 —— 软删是 del_flag 的一次
    /// update,同样享受审计盖章(update_*)与软删存活过滤(已删行的
    /// 二次软删 0 行 → NotFound;post delete_by_id 同款)。
    pub async fn delete_by_id(
        &self,
        ctx: &DataContext<'_>,
        id: &i64,
    ) -> Result<(), DataError> {
        let active = ActiveModel {
            id: Set(*id),
            del_flag: Set(DelFlag::Deleted.as_str().to_string()),
            ..Default::default()
        };
        ctx.update(active).await?;
        Ok(())
    }

    /// 改客户端状态(单列 update;update_by/update_time 由漏斗按
    /// principal 盖章,原手工 Set 已删;WHERE = 主键 + 软删存活,
    /// 0 行 → NotFound —— 原手工"客户端不存在或已删除"预检的语义
    /// 由漏斗 WHERE 承担)。
    pub async fn change_status(
        &self,
        ctx: &DataContext<'_>,
        id: i64,
        status: &str,
    ) -> Result<(), DataError> {
        let active = ActiveModel {
            id: Set(id),
            status: Set(status.to_string()),
            ..Default::default()
        };
        ctx.update(active).await?;
        Ok(())
    }
}

fn parse_order(order_by: &str) -> (Column, Order) {
    let mut parts = order_by.split_whitespace();
    let col = match parts.next().unwrap_or("id") {
        "id" => Column::Id,
        "client_id" => Column::ClientId,
        "client_key" => Column::ClientKey,
        "create_time" => Column::CreateTime,
        _ => Column::Id,
    };
    let desc = !parts.next().is_some_and(|d| d.eq_ignore_ascii_case("asc"));
    if desc {
        (col, Order::Desc)
    } else {
        (col, Order::Asc)
    }
}
