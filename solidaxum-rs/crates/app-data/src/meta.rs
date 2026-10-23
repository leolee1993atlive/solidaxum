//! 代码生成的数据库元数据读取(设计 6.3 meta 行;10 节步 3 system 子项)。
//!
//! 从 `system/src/codegen/service/db_meta.rs` 收编而来:三个读取方法
//! (list_tables / get_table_comment / list_columns)的方言分叉 SQL 逐字活在
//! [`crate::dialect`],本模块只做**后端分派 + 行 → 中立类型映射**。
//!
//! # 为什么不经 DataPolicy 门控 API
//!
//! 元数据读取走的是**系统视图**(information_schema / ALL_TABLES /
//! sqlite_master),它们不是业务实体、没有 DataPolicy;调用语义是
//! system_context(代码生成是管理功能,无行范围)。因此 [`MetadataReader`]
//! 不走 `find_by_id`/`page`/`select` 那组门控 API,而是直接经
//! [`crate::context::ContextCore`] 的内部执行器跑方言语句 —— crate 内部
//! 访问,公开面上不新增任何裸 SQL 出口(见 `DataContext::core` 的说明)。

use sea_orm::{ConnectionTrait, DbBackend, QueryResult};

use crate::{
    context::{ContextCore, DataContext},
    dialect,
    error::DataError,
};

/// 中立表元数据(设计 6.3 meta 行)。字段与 db_meta 原有三方言 SELECT 列对齐。
#[derive(Clone, Debug)]
pub struct TableMeta {
    pub table_name: String,
    pub table_comment: String,
    /// 建表/更新时间以**库返回的原始字符串**透传(information_schema 的
    /// DATETIME、达梦的 CRTDATE)。刻意不解析成 DateTimeUtc:下游 VO 对前端
    /// 的契约就是字符串,这里多一次格式化等于改契约。
    pub create_time: Option<String>,
    pub update_time: Option<String>,
}

/// 中立列元数据。字段与 db_meta 原有 SELECT 列对齐(MySQL 版最全)。
#[derive(Clone, Debug)]
pub struct ColumnMeta {
    pub column_name: String,
    pub column_comment: Option<String>,
    /// MySQL/SQLite 有现成 column_type(varchar(50) 这类);**达梦没有** ——
    /// ALL_TAB_COLUMNS 拆成 data_type + 长度/精度/刻度,此字段留空、
    /// 原始三元组随行携带,由消费侧拼装(见 system 侧 `compose_column_type`,
    /// 它是 codegen 逻辑,留在业务 crate)。
    pub column_type: String,
    pub data_type: String,
    /// 达梦方言专有:DATA_LENGTH / DATA_PRECISION / DATA_SCALE。
    /// MySQL/SQLite 不选出 → None。
    pub data_length: Option<i64>,
    pub data_precision: Option<i64>,
    pub data_scale: Option<i64>,
    pub is_pk: bool,
    pub is_auto_increment: bool,
    pub is_nullable: bool,
    pub ordinal_position: i32,
}

/// 元数据读取器。持的是 context 的内部执行核,不持连接 —— 与漏斗的
/// 生命周期模型一致(连接归装配层,context 归请求)。
pub struct MetadataReader<'a> {
    core: &'a ContextCore<'a>,
}

impl<'a> MetadataReader<'a> {
    /// `ctx` 语义上应是 `system_context()`(无行范围);系统视图本就没有
    /// DataPolicy,即便误传用户 context 也不会施加任何范围。
    pub fn new(ctx: &'a DataContext<'a>) -> Self {
        Self { core: ctx.core() }
    }

    fn backend(&self) -> DbBackend {
        self.core.get_database_backend()
    }

    /// 列出库中可导入的业务表(分页 + 过滤),返回 (表清单, 总数)。
    /// 过滤语义(schema、忽略前缀 sj_/flow_/gen_、已导入排除、表名/注释
    /// 模糊匹配)由 [`crate::dialect`] 的 build_table_filter* 实现。
    /// MySQL 的 schema 以连接当前库为准(DATABASE()),`db_name` 仅达梦
    /// 用作 OWNER 过滤(SQLite 单库同样忽略)。
    ///
    /// SQLite 方言没有表注释,`table_comment` 过滤条件在该方言下无从生效
    /// (恒空注释匹配不上),`db_name` 亦被忽略(单库)。
    pub async fn list_tables(
        &self,
        db_name: &str,
        table_name: Option<&str>,
        table_comment: Option<&str>,
        imported_tables: &[String],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<TableMeta>, u64), DataError> {
        let (count_stmt, page_stmt) = match self.backend() {
            DbBackend::Dm => (
                dialect::dm_list_tables_count(
                    db_name,
                    table_name,
                    table_comment,
                    imported_tables,
                ),
                dialect::dm_list_tables_page(
                    db_name,
                    table_name,
                    table_comment,
                    imported_tables,
                    offset,
                    limit,
                ),
            ),
            DbBackend::Sqlite => (
                dialect::sqlite_list_tables_count(table_name, imported_tables),
                dialect::sqlite_list_tables_page(
                    table_name,
                    imported_tables,
                    offset,
                    limit,
                ),
            ),
            _ => (
                dialect::mysql_list_tables_count(
                    db_name,
                    table_name,
                    table_comment,
                    imported_tables,
                ),
                dialect::mysql_list_tables_page(
                    db_name,
                    table_name,
                    table_comment,
                    imported_tables,
                    offset,
                    limit,
                ),
            ),
        };

        let count_result = self.core.query_one_raw(count_stmt).await?;
        let total = count_result
            .map_or(0, |row| row.try_get::<i64>("", "cnt").unwrap_or(0))
            as u64;

        let rows = self.core.query_all_raw(page_stmt).await?;
        let tables: Vec<TableMeta> = rows.iter().map(table_meta_of).collect();

        Ok((tables, total))
    }

    /// 查单表注释。库/视图未给出注释 → None(由调用方决定缺省呈现)。
    /// SQLite 无表注释概念,恒 None。
    pub async fn get_table_comment(
        &self,
        db_name: &str,
        table_name: &str,
    ) -> Result<Option<String>, DataError> {
        let stmt = match self.backend() {
            DbBackend::Dm => dialect::dm_table_comment(db_name, table_name),
            DbBackend::Sqlite => return Ok(None),
            _ => dialect::mysql_table_comment(db_name, table_name),
        };
        let row = self.core.query_one_raw(stmt).await?;
        Ok(row.and_then(|r| r.try_get::<String>("", "table_comment").ok()))
    }

    /// 列出表的全列元数据,按 ordinal_position 升序。
    pub async fn list_columns(
        &self,
        db_name: &str,
        table_name: &str,
    ) -> Result<Vec<ColumnMeta>, DataError> {
        let stmt = match self.backend() {
            DbBackend::Dm => dialect::dm_list_columns(db_name, table_name),
            DbBackend::Sqlite => dialect::sqlite_list_columns(table_name),
            _ => dialect::mysql_list_columns(db_name, table_name),
        };
        let rows = self.core.query_all_raw(stmt).await?;
        let columns: Vec<ColumnMeta> = match self.backend() {
            DbBackend::Dm => rows.iter().map(column_meta_dm_of).collect(),
            DbBackend::Sqlite => {
                rows.iter().map(column_meta_sqlite_of).collect()
            }
            _ => rows.iter().map(column_meta_mysql_of).collect(),
        };
        Ok(columns)
    }
}

/// 三方言的表行都别名成同名四列,共用一个映射。
fn table_meta_of(row: &QueryResult) -> TableMeta {
    TableMeta {
        table_name: row.try_get("", "table_name").unwrap_or_default(),
        table_comment: row.try_get("", "table_comment").unwrap_or_default(),
        create_time: row.try_get("", "create_time").ok(),
        update_time: row.try_get("", "update_time").ok(),
    }
}

/// MySQL 列映射:column_key='PRI' 判主键,extra 含 auto_increment 判自增,
/// is_nullable 以 'YES' 比较 —— 全部与原 db_meta 相同。
fn column_meta_mysql_of(row: &QueryResult) -> ColumnMeta {
    let column_key: String = row.try_get("", "column_key").unwrap_or_default();
    let extra: String = row.try_get("", "extra").unwrap_or_default();
    let is_nullable: String =
        row.try_get("", "is_nullable").unwrap_or_default();

    ColumnMeta {
        column_name: row.try_get("", "column_name").unwrap_or_default(),
        column_comment: row.try_get("", "column_comment").ok(),
        column_type: row.try_get("", "column_type").unwrap_or_default(),
        data_type: row.try_get("", "data_type").unwrap_or_default(),
        data_length: None,
        data_precision: None,
        data_scale: None,
        is_pk: column_key == "PRI",
        is_auto_increment: extra.contains("auto_increment"),
        is_nullable: is_nullable.eq_ignore_ascii_case("YES"),
        ordinal_position: row.try_get("", "ordinal_position").unwrap_or(0),
    }
}

/// 达梦列映射:is_pk/is_identity 是 SQL 里的 CASE 表达式(0/1),
/// nullable 以 'Y' 比较。column_type 见 [`ColumnMeta::column_type`] 的说明
/// —— 留空,长度/精度三元组随行携带,拼装是消费侧(system)的 codegen 逻辑。
fn column_meta_dm_of(row: &QueryResult) -> ColumnMeta {
    let data_type: String = row.try_get("", "data_type").unwrap_or_default();
    let data_length: Option<i64> =
        row.try_get("", "data_length").ok().flatten();
    let data_precision: Option<i64> =
        row.try_get("", "data_precision").ok().flatten();
    let data_scale: Option<i64> = row.try_get("", "data_scale").ok().flatten();
    let nullable: String = row.try_get("", "nullable").unwrap_or_default();
    let is_pk: i64 = row.try_get("", "is_pk").unwrap_or(0);
    let is_identity: i64 = row.try_get("", "is_identity").unwrap_or(0);

    ColumnMeta {
        column_name: row.try_get("", "column_name").unwrap_or_default(),
        column_comment: row.try_get("", "column_comment").ok(),
        column_type: String::new(),
        data_type,
        data_length,
        data_precision,
        data_scale,
        is_pk: is_pk == 1,
        is_auto_increment: is_identity == 1,
        is_nullable: nullable.eq_ignore_ascii_case("Y"),
        ordinal_position: row
            .try_get::<i64>("", "ordinal_position")
            .unwrap_or(0) as i32,
    }
}

/// SQLite 列映射:CASE 表达式产出的 0/1 标志;注释恒 NULL(方言无列注释)。
fn column_meta_sqlite_of(row: &QueryResult) -> ColumnMeta {
    let is_nullable: i64 = row.try_get("", "is_nullable").unwrap_or(0);
    let is_pk: i64 = row.try_get("", "is_pk").unwrap_or(0);
    let is_auto_increment: i64 =
        row.try_get("", "is_auto_increment").unwrap_or(0);

    ColumnMeta {
        column_name: row.try_get("", "column_name").unwrap_or_default(),
        column_comment: row.try_get("", "column_comment").ok(),
        column_type: row.try_get("", "column_type").unwrap_or_default(),
        data_type: row.try_get("", "data_type").unwrap_or_default(),
        data_length: None,
        data_precision: None,
        data_scale: None,
        is_pk: is_pk == 1,
        is_auto_increment: is_auto_increment == 1,
        is_nullable: is_nullable == 1,
        ordinal_position: row
            .try_get::<i64>("", "ordinal_position")
            .unwrap_or(0) as i32,
    }
}
