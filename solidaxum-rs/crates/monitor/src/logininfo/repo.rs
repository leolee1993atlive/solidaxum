//! logininfo 仓储 —— app-data 漏斗内执行(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:现网
//! `SysLoginInfoMapper.java` 是空接口(extends BaseMapperPlus,
//! **零 `@DataPermission`**)—— 登录日志是全局审计数据,数据权限过滤
//! 对它无业务含义;实体无审计列、无软删列(日志表只增不改,Java 锚即
//! 物理 delete/clean)。统一锚:**SysLoginInfoMapper 无 @DataPermission
//! —— 审计日志非行级数据**。由此每个读方法显式 `without_row_scope()`;
//! DataPolicy 在 `crate::policy` 落 CONSTRAINED=false + 全 None。
//!
//! | 本文件方法 | Java 锚(SysLoginInfoController/ServiceImpl → SysLoginInfoMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `insert` | auth 异步记录(登录成败/登出/注册)→ `recordLoginInfo`(SysLoginInfoServiceImpl:57)/`insertLoginInfo`(:138) | — | `ctx.insert`(无审计列可盖) |
//! | `select_page` | list(Controller:47)→ `selectPageLoginInfoList`(ServiceImpl:123)→ selectVoPage | ❌ | `without_row_scope()`(逃生舱分页,total 全量口径) |
//! | `select_list` | export(Controller:60)→ `selectLoginInfoList`(ServiceImpl:151)→ selectVoList | ❌ | `without_row_scope()` |
//! | `delete_by_ids` | remove(Controller:74)→ `deleteLoginInfoByIds`(ServiceImpl:178)→ deleteByIds(无 @TableLogic,物理删) | — | `ctx.delete_where`(豁免实体批量物理删) |
//! | `clean_all` | clean(Controller:87)→ `cleanLoginInfo`(ServiceImpl:186,物理清空) | — | `ctx.delete_where`(Condition::all() = 全表 DELETE) |

use app_core::vo::page::TableDataInfo;
use app_data::{
    DataContext, DataError, DataSource,
    sea_orm::{
        self, ActiveModelBehavior,
        ActiveValue::{NotSet, Set},
        ColumnTrait, Condition, DateTimeUtc, DeriveEntityModel,
        DerivePrimaryKey, DeriveRelation, EntityTrait, EnumIter, Order,
        PrimaryKeyTrait, QueryFilter, QueryOrder, Select,
    },
};

use super::model::{LoginInfo, LoginInfoQuery};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_login_info")]
pub struct Model {
    // 主键由应用层雪花算法显式生成,非 DB 自增(同 sys_oper_log 说明)。
    #[sea_orm(primary_key, auto_increment = false)]
    pub info_id: i64,
    pub user_name: String,
    pub client_key: Option<String>,
    pub device_type: Option<String>,
    pub ipaddr: String,
    pub login_location: String,
    pub browser: String,
    pub os: String,
    pub status: String,
    pub msg: String,
    pub login_time: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<&Model> for LoginInfo {
    fn from(m: &Model) -> Self {
        LoginInfo {
            info_id: Some(m.info_id),
            user_name: m.user_name.clone(),
            client_key: m.client_key.clone(),
            device_type: m.device_type.clone(),
            ipaddr: m.ipaddr.clone(),
            login_location: m.login_location.clone(),
            browser: m.browser.clone(),
            os: m.os.clone(),
            status: m.status.clone(),
            msg: m.msg.clone(),
            login_time: m.login_time,
        }
    }
}

impl From<LoginInfo> for ActiveModel {
    fn from(info: LoginInfo) -> Self {
        ActiveModel {
            info_id: match info.info_id {
                Some(id) => Set(id),
                None => NotSet,
            },
            user_name: Set(info.user_name),
            client_key: Set(info.client_key),
            device_type: Set(info.device_type),
            ipaddr: Set(info.ipaddr),
            login_location: Set(info.login_location),
            browser: Set(info.browser),
            os: Set(info.os),
            status: Set(info.status),
            msg: Set(info.msg),
            login_time: Set(info.login_time),
        }
    }
}

#[derive(Clone)]
pub struct LoginInfoRepository {
    /// 装配期快照(豁免读不解析 resolver,降级快照无影响)。
    ds: std::sync::Arc<DataSource>,
}

impl LoginInfoRepository {
    pub fn new(ds: std::sync::Arc<DataSource>) -> Self {
        Self { ds }
    }

    /// 无会话桥:登录成功/失败/登出都在会话建立前后(auth 四处生产
    /// 消费)。sys_login_info 无审计列/软删列/范围列,与用户 ctx 逐点一致。
    pub fn system_context(&self) -> DataContext<'_> {
        self.ds.system_context()
    }

    /// 新增登录日志(Java 锚 recordLoginInfo SysLoginInfoServiceImpl:57 /
    /// insertLoginInfo:138)。实体无审计列,漏斗 insert 无章可盖。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        info: &LoginInfo,
    ) -> Result<(), DataError> {
        let active: ActiveModel = info.clone().into();
        ctx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// 分页查询登录日志。
    ///
    /// 豁免裁定:SysLoginInfoMapper 无 @DataPermission —— 审计日志非行级
    /// 数据(Java 锚 list Controller:47 → selectPageLoginInfoList
    /// ServiceImpl:123 → selectVoPage;排序在 Select 构件上,total 全量口径)。
    pub async fn select_page(
        &self,
        ctx: &DataContext<'_>,
        query: &LoginInfoQuery,
        page_no: u64,
        page_size: u64,
        order_by: &str,
    ) -> Result<TableDataInfo<LoginInfo>, DataError> {
        let mut select = build_login_info_query(query);

        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let rows: Vec<LoginInfo> =
            page.rows.iter().map(LoginInfo::from).collect();
        Ok(TableDataInfo::from_page_rows(&page, rows))
    }

    /// 按过滤条件查询全部登录日志(导出用)。
    ///
    /// 豁免裁定:SysLoginInfoMapper 无 @DataPermission —— 审计日志非行级
    /// 数据(Java 锚 export Controller:60 → selectLoginInfoList
    /// ServiceImpl:151 → selectVoList)。
    pub async fn select_list(
        &self,
        ctx: &DataContext<'_>,
        query: &LoginInfoQuery,
    ) -> Result<Vec<LoginInfo>, DataError> {
        let select = build_login_info_query(query)
            .order_by(Column::LoginTime, Order::Desc);
        let models = ctx.select(select).without_row_scope().all().await?;
        Ok(models.iter().map(LoginInfo::from).collect())
    }

    /// 与 [`Self::select_list`] 同过滤的计数,供导出前的行数闸门使用 ——
    /// 登录日志无上限增长,先 COUNT 再决定要不要取,免得白拉整张表。
    /// 复用 `build_login_info_query` 保证两者 WHERE 恒一致。
    pub async fn count_by_query(
        &self,
        ctx: &DataContext<'_>,
        query: &LoginInfoQuery,
    ) -> Result<u64, DataError> {
        ctx.select(build_login_info_query(query))
            .without_row_scope()
            .count()
            .await
    }

    /// 按主键批量物理删(Java 锚 deleteLoginInfoByIds
    /// SysLoginInfoServiceImpl:178 → deleteByIds;无 @TableLogic 即物理删,
    /// 豁免 + 无软删 → delete_where 放行)。
    pub async fn delete_by_ids(
        &self,
        ctx: &DataContext<'_>,
        ids: &[i64],
    ) -> Result<u64, DataError> {
        ctx.delete_where::<Entity>(
            Condition::all().add(Column::InfoId.is_in(ids.iter().copied())),
        )
        .await
    }

    /// 物理清空(Java clean 接口即 TRUNCATE 语义);豁免 + 无软删 →
    /// delete_where 放行;Condition::all() = 无 WHERE 的全表 DELETE。
    pub async fn clean_all(
        &self,
        ctx: &DataContext<'_>,
    ) -> Result<u64, DataError> {
        ctx.delete_where::<Entity>(Condition::all()).await
    }
}

fn build_login_info_query(query: &LoginInfoQuery) -> Select<Entity> {
    let mut select = Entity::find();

    if let Some(ref name) = query.user_name
        && !name.is_empty()
    {
        select = select.filter(Column::UserName.contains(name));
    }
    if let Some(ref s) = query.status
        && !s.is_empty()
    {
        select = select.filter(Column::Status.eq(s));
    }
    if let Some(ref ip) = query.ipaddr
        && !ip.is_empty()
    {
        select = select.filter(Column::Ipaddr.contains(ip));
    }
    if let Some(ref begin) = query.begin_time
        && !begin.is_empty()
        && let Ok(dt) =
            chrono::NaiveDateTime::parse_from_str(begin, "%Y-%m-%d %H:%M:%S")
    {
        select = select.filter(Column::LoginTime.gte(dt.and_utc()));
    }
    if let Some(ref end) = query.end_time
        && !end.is_empty()
        && let Ok(dt) =
            chrono::NaiveDateTime::parse_from_str(end, "%Y-%m-%d %H:%M:%S")
    {
        select = select.filter(Column::LoginTime.lte(dt.and_utc()));
    }

    select
}

fn parse_order(order_by: &str) -> (Column, Order) {
    let mut parts = order_by.split_whitespace();
    let col = match parts.next().unwrap_or("login_time") {
        "info_id" => Column::InfoId,
        "user_name" => Column::UserName,
        "ipaddr" => Column::Ipaddr,
        "status" => Column::Status,
        "login_time" => Column::LoginTime,
        _ => Column::LoginTime,
    };
    let desc = !parts.next().is_some_and(|d| d.eq_ignore_ascii_case("asc"));
    if desc {
        (col, Order::Desc)
    } else {
        (col, Order::Asc)
    }
}
