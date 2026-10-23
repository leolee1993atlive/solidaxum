//! operlog 仓储 —— app-data 漏斗内执行(**全豁免模块**)。
//!
//! 漏斗公共契约(执行面/错误)见 `app_data` 模块文档。**方法级裁定(Java 基线逐方法核对)**:现网
//! `SysOperLogMapper.java` 是空接口(extends BaseMapperPlus,
//! **零 `@DataPermission`**)—— 操作日志是全局审计数据;实体无审计列、
//! 无软删列(日志表只增不改,Java 锚即物理 delete/clean)。
//! **sys_oper_log 的 user_id/dept_id 是数据列(记录"谁操作的"),不是
//! 归属列** —— 勿"见列加回"(sys_social 的 user_id 同款裁定,加回会把
//! Java 里全量可见的日志误砍到"仅本人")。统一锚:**SysOperLogMapper
//! 无 @DataPermission —— 审计日志非行级数据**。由此每个读方法显式
//! `without_row_scope()`;DataPolicy 在 `crate::policy` 落
//! CONSTRAINED=false + 全 None。
//!
//! | 本文件方法 | Java 锚(SysOperlogController/ServiceImpl → SysOperLogMapper) | @DataPermission | 漏斗路径 |
//! |---|---|---|---|
//! | `insert` | oper_log 中间件异步记录 → `recordOper`(SysOperLogServiceImpl:46)/`insertOperlog`(:106) | — | `ctx.insert`(无审计列可盖) |
//! | `select_page` | list(Controller:44)→ `selectPageOperLogList`(ServiceImpl:61)→ selectVoPage | ❌ | `without_row_scope()`(逃生舱分页,total 全量口径) |
//! | `select_list` | export(Controller:57)→ `selectOperLogList`(ServiceImpl:119)→ selectVoList | ❌ | `without_row_scope()` |
//! | `delete_by_ids` | remove(Controller:71)→ `deleteOperLogByIds`(ServiceImpl:131)→ deleteByIds(无 @TableLogic,物理删) | — | `ctx.delete_where`(豁免实体批量物理删) |
//! | `clean_all` | clean(Controller:84)→ `cleanOperLog`(ServiceImpl:150,物理清空) | — | `ctx.delete_where`(Condition::all() = 全表 DELETE) |

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

use super::model::{OperLog, OperLogQuery};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sys_oper_log")]
pub struct Model {
    // 主键由应用层雪花算法显式生成,非 DB 自增。MySQL 下若标为自增,
    // 显式插值会令 LAST_INSERT_ID() 返回 0,SeaORM 误判为 RecordNotInserted。
    #[sea_orm(primary_key, auto_increment = false)]
    pub oper_id: i64,
    pub title: String,
    pub business_type: i32,
    pub method: Option<String>,
    pub request_method: String,
    pub operator_type: i32,
    pub oper_name: String,
    pub dept_name: Option<String>,
    pub oper_url: String,
    pub oper_ip: String,
    pub oper_location: String,
    pub oper_param: String,
    pub json_result: String,
    pub status: i32,
    pub error_msg: String,
    pub oper_time: Option<DateTimeUtc>,
    pub cost_time: i64,
    pub user_id: Option<i64>,
    pub dept_id: Option<i64>,
    pub client_key: Option<String>,
    pub device_type: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<&Model> for OperLog {
    fn from(m: &Model) -> Self {
        OperLog {
            oper_id: Some(m.oper_id),
            title: m.title.clone(),
            business_type: m.business_type,
            method: m.method.clone(),
            request_method: m.request_method.clone(),
            operator_type: m.operator_type,
            oper_name: m.oper_name.clone(),
            dept_name: m.dept_name.clone(),
            oper_url: m.oper_url.clone(),
            oper_ip: m.oper_ip.clone(),
            oper_location: m.oper_location.clone(),
            oper_param: m.oper_param.clone(),
            json_result: m.json_result.clone(),
            status: m.status,
            error_msg: m.error_msg.clone(),
            oper_time: m.oper_time,
            cost_time: m.cost_time,
            user_id: m.user_id,
            dept_id: m.dept_id,
            client_key: m.client_key.clone(),
            device_type: m.device_type.clone(),
            browser: m.browser.clone(),
            os: m.os.clone(),
        }
    }
}

impl From<OperLog> for ActiveModel {
    fn from(log: OperLog) -> Self {
        ActiveModel {
            oper_id: match log.oper_id {
                Some(id) => Set(id),
                None => NotSet,
            },
            title: Set(log.title),
            business_type: Set(log.business_type),
            method: Set(log.method),
            request_method: Set(log.request_method),
            operator_type: Set(log.operator_type),
            oper_name: Set(log.oper_name),
            dept_name: Set(log.dept_name),
            oper_url: Set(log.oper_url),
            oper_ip: Set(log.oper_ip),
            oper_location: Set(log.oper_location),
            oper_param: Set(log.oper_param),
            json_result: Set(log.json_result),
            status: Set(log.status),
            error_msg: Set(log.error_msg),
            oper_time: Set(log.oper_time),
            cost_time: Set(log.cost_time),
            user_id: Set(log.user_id),
            dept_id: Set(log.dept_id),
            client_key: Set(log.client_key),
            device_type: Set(log.device_type),
            browser: Set(log.browser),
            os: Set(log.os),
        }
    }
}

#[derive(Clone)]
pub struct OperLogRepository {
    /// 装配期快照(豁免读不解析 resolver,降级快照无影响)。
    ds: std::sync::Arc<DataSource>,
}

impl OperLogRepository {
    pub fn new(ds: std::sync::Arc<DataSource>) -> Self {
        Self { ds }
    }

    /// 无会话桥:oper_log 中间件在响应成形后后台异步落库(tokio::spawn
    /// 的执行面无请求身份)。sys_oper_log 无审计列/软删列/范围列,
    /// 与用户 ctx 逐点一致。
    pub fn system_context(&self) -> DataContext<'_> {
        self.ds.system_context()
    }

    /// 新增操作日志(Java 锚 recordOper SysOperLogServiceImpl:46 /
    /// insertOperlog:106)。实体无审计列,漏斗 insert 无章可盖。
    pub async fn insert(
        &self,
        ctx: &DataContext<'_>,
        log: &OperLog,
    ) -> Result<(), DataError> {
        let active: ActiveModel = log.clone().into();
        ctx.insert_returning_nothing(active).await?;
        Ok(())
    }

    /// 分页查询操作日志。
    ///
    /// 豁免裁定:SysOperLogMapper 无 @DataPermission —— 审计日志非行级
    /// 数据(Java 锚 list Controller:44 → selectPageOperLogList
    /// ServiceImpl:61 → selectVoPage;排序在 Select 构件上,total 全量口径)。
    pub async fn select_page(
        &self,
        ctx: &DataContext<'_>,
        query: &OperLogQuery,
        page_no: u64,
        page_size: u64,
        order_by: &str,
    ) -> Result<TableDataInfo<OperLog>, DataError> {
        let mut select = build_oper_log_query(query);

        let (order_col, order_dir) = parse_order(order_by);
        select = select.order_by(order_col, order_dir);

        let page = ctx
            .select(select)
            .without_row_scope()
            .page(page_no as i64, page_size as i64)
            .await?;
        let rows: Vec<OperLog> = page.rows.iter().map(OperLog::from).collect();
        Ok(TableDataInfo::from_page_rows(&page, rows))
    }

    /// 按过滤条件查询全部操作日志(导出用)。
    ///
    /// 豁免裁定:SysOperLogMapper 无 @DataPermission —— 审计日志非行级
    /// 数据(Java 锚 export Controller:57 → selectOperLogList
    /// ServiceImpl:119 → selectVoList)。
    pub async fn select_list(
        &self,
        ctx: &DataContext<'_>,
        query: &OperLogQuery,
    ) -> Result<Vec<OperLog>, DataError> {
        let select =
            build_oper_log_query(query).order_by(Column::OperId, Order::Desc);
        let models = ctx.select(select).without_row_scope().all().await?;
        Ok(models.iter().map(OperLog::from).collect())
    }

    /// 与 [`Self::select_list`] 同过滤的计数,供导出前的行数闸门使用 ——
    /// 操作日志无上限增长,先 COUNT 再决定要不要取,免得白拉整张表。
    /// 复用 `build_oper_log_query` 保证两者 WHERE 恒一致。
    pub async fn count_by_query(
        &self,
        ctx: &DataContext<'_>,
        query: &OperLogQuery,
    ) -> Result<u64, DataError> {
        ctx.select(build_oper_log_query(query))
            .without_row_scope()
            .count()
            .await
    }

    /// 按主键批量物理删(Java 锚 deleteOperLogByIds
    /// SysOperLogServiceImpl:131 → deleteByIds;无 @TableLogic 即物理删,
    /// 豁免 + 无软删 → delete_where 放行)。
    pub async fn delete_by_ids(
        &self,
        ctx: &DataContext<'_>,
        ids: &[i64],
    ) -> Result<u64, DataError> {
        ctx.delete_where::<Entity>(
            Condition::all().add(Column::OperId.is_in(ids.iter().copied())),
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

fn build_oper_log_query(query: &OperLogQuery) -> Select<Entity> {
    let mut select = Entity::find();

    if let Some(ref title) = query.title
        && !title.is_empty()
    {
        select = select.filter(Column::Title.contains(title));
    }
    if let Some(bt) = query.business_type {
        select = select.filter(Column::BusinessType.eq(bt));
    }
    if let Some(ref bts) = query.business_types
        && !bts.is_empty()
    {
        select = select.filter(Column::BusinessType.is_in(bts.clone()));
    }
    if let Some(s) = query.status {
        select = select.filter(Column::Status.eq(s));
    }
    if let Some(ref name) = query.oper_name
        && !name.is_empty()
    {
        select = select.filter(Column::OperName.contains(name));
    }
    if let Some(ref ip) = query.oper_ip
        && !ip.is_empty()
    {
        select = select.filter(Column::OperIp.contains(ip));
    }
    if let Some(ref key) = query.client_key
        && !key.is_empty()
    {
        select = select.filter(Column::ClientKey.contains(key));
    }
    if let Some(ref browser) = query.browser
        && !browser.is_empty()
    {
        select = select.filter(Column::Browser.contains(browser));
    }
    if let Some(ref os) = query.os
        && !os.is_empty()
    {
        select = select.filter(Column::Os.contains(os));
    }
    if let Some(ref device) = query.device_type
        && !device.is_empty()
    {
        select = select.filter(Column::DeviceType.contains(device));
    }
    if let Some(ref begin) = query.begin_time
        && !begin.is_empty()
        && let Ok(dt) =
            chrono::NaiveDateTime::parse_from_str(begin, "%Y-%m-%d %H:%M:%S")
    {
        select = select.filter(Column::OperTime.gte(dt.and_utc()));
    }
    if let Some(ref end) = query.end_time
        && !end.is_empty()
        && let Ok(dt) =
            chrono::NaiveDateTime::parse_from_str(end, "%Y-%m-%d %H:%M:%S")
    {
        select = select.filter(Column::OperTime.lte(dt.and_utc()));
    }

    select
}

fn parse_order(order_by: &str) -> (Column, Order) {
    let mut parts = order_by.split_whitespace();
    let col = match parts.next().unwrap_or("oper_time") {
        "oper_id" => Column::OperId,
        "title" => Column::Title,
        "business_type" => Column::BusinessType,
        "request_method" => Column::RequestMethod,
        "oper_name" => Column::OperName,
        "oper_url" => Column::OperUrl,
        "oper_ip" => Column::OperIp,
        "status" => Column::Status,
        "oper_time" => Column::OperTime,
        "cost_time" => Column::CostTime,
        _ => Column::OperTime,
    };
    let desc = !parts.next().is_some_and(|d| d.eq_ignore_ascii_case("asc"));
    if desc {
        (col, Order::Desc)
    } else {
        (col, Order::Asc)
    }
}
