//! 代码生成的 DB 元数据读取(瘦委托,设计 6.3 meta 行 / 10 节步 3 system 子项)。
//!
//! 数据访问已收编到 `app_data::meta::MetadataReader`:三方言的元数据 SQL
//! (MySQL information_schema、达梦 ALL_TABLES/ALL_TAB_COLUMNS、SQLite 测试
//! 方言)逐字活在 app-data 的 dialect 模块 —— 那是达梦实机验证过的文本,
//! 本文件不再出现任何 SQL 与
//! `DbBackend` 分叉。剩下的职责:
//!
//! - [`DbMetaReader`]:薄委托,持有 context 上的 `MetadataReader`;
//! - `From<TableMeta>` / `From<ColumnMeta>`:中立类型 → 本模块 VO 的映射;
//! - 纯 codegen 逻辑原地不动:`compose_column_type` / `resolve_java_type` /
//!   `resolve_html_type` 等是代码生成规则,不是数据访问。

use app_core::error::{AppError, AppResult};
use app_data::meta::{ColumnMeta, MetadataReader, TableMeta};

use crate::codegen::model::gen_table::DbTableVo;

pub struct DbMetaReader<'a> {
    reader: MetadataReader<'a>,
}

impl<'a> DbMetaReader<'a> {
    /// `ctx` 语义上应是 `system_context()`(代码生成是管理功能,无行范围)。
    pub fn new(ctx: &'a app_data::DataContext<'a>) -> Self {
        Self {
            reader: MetadataReader::new(ctx),
        }
    }

    pub async fn list_tables(
        &self,
        db_name: &str,
        table_name: Option<&str>,
        table_comment: Option<&str>,
        imported_tables: &[String],
        offset: u64,
        limit: u64,
    ) -> AppResult<(Vec<DbTableVo>, u64)> {
        let (tables, total) = self
            .reader
            .list_tables(
                db_name,
                table_name,
                table_comment,
                imported_tables,
                offset,
                limit,
            )
            .await
            .map_err(to_app_err)?;
        Ok((tables.into_iter().map(DbTableVo::from).collect(), total))
    }

    pub async fn get_table_comment(
        &self,
        db_name: &str,
        table_name: &str,
    ) -> AppResult<String> {
        Ok(self
            .reader
            .get_table_comment(db_name, table_name)
            .await
            .map_err(to_app_err)?
            .unwrap_or_default())
    }

    pub async fn list_columns(
        &self,
        db_name: &str,
        table_name: &str,
    ) -> AppResult<Vec<RawColumnInfo>> {
        Ok(self
            .reader
            .list_columns(db_name, table_name)
            .await
            .map_err(to_app_err)?
            .into_iter()
            .map(RawColumnInfo::from)
            .collect())
    }
}

/// DataError → AppError。orphan 规则不允许跨 crate 的 From impl,
/// 私有映射函数足矣(调用点都在本文件)。
fn to_app_err(e: app_data::DataError) -> AppError {
    match e {
        app_data::DataError::NotFound => {
            AppError::NotFound("元数据不存在".to_string())
        }
        app_data::DataError::Backend(err) => AppError::DbError(err),
        other => AppError::Internal(other.to_string()),
    }
}

impl From<TableMeta> for DbTableVo {
    fn from(m: TableMeta) -> Self {
        DbTableVo {
            table_name: m.table_name,
            table_comment: m.table_comment,
            create_time: m.create_time,
            update_time: m.update_time,
        }
    }
}

impl From<ColumnMeta> for RawColumnInfo {
    fn from(m: ColumnMeta) -> Self {
        // 达梦方言无现成 column_type(中立类型留空 + 携带长度/精度三元组),
        // MySQL 风格的拼装是 codegen 逻辑,留在本 crate(compose_column_type)。
        let column_type = if m.column_type.is_empty() {
            compose_column_type(
                &m.data_type,
                m.data_length,
                m.data_precision,
                m.data_scale,
            )
        } else {
            m.column_type
        };
        RawColumnInfo {
            column_name: m.column_name,
            column_comment: m.column_comment,
            column_type,
            data_type: m.data_type,
            is_pk: m.is_pk,
            is_auto_increment: m.is_auto_increment,
            is_nullable: m.is_nullable,
            ordinal_position: m.ordinal_position,
        }
    }
}

/// 把达梦的类型名与长度/精度拼成 MySQL 风格的 column_type(如 `varchar(50)`、
/// `decimal(19,2)`)—— 下游 `get_column_length`/`get_column_scale` 从该字符串解析。
fn compose_column_type(
    data_type: &str,
    data_length: Option<i64>,
    data_precision: Option<i64>,
    data_scale: Option<i64>,
) -> String {
    let dt = data_type.to_lowercase();
    if let Some(p) = data_precision.filter(|p| *p > 0) {
        return format!("{dt}({p},{})", data_scale.unwrap_or(0));
    }
    const SIZED: &[&str] = &[
        "char",
        "varchar",
        "varchar2",
        "nvarchar",
        "nchar",
        "binary",
        "varbinary",
    ];
    if SIZED.contains(&dt.as_str())
        && let Some(len) = data_length.filter(|l| *l > 0)
    {
        return format!("{dt}({len})");
    }
    dt
}

#[derive(Clone, Debug)]
pub struct RawColumnInfo {
    pub column_name: String,
    pub column_comment: Option<String>,
    pub column_type: String,
    pub data_type: String,
    pub is_pk: bool,
    pub is_auto_increment: bool,
    pub is_nullable: bool,
    pub ordinal_position: i32,
}

pub fn resolve_java_type(
    column_type: &str,
    column_length: i32,
    column_scale: i32,
    column_name: &str,
) -> &'static str {
    let data_type = column_type
        .split('(')
        .next()
        .unwrap_or(column_type)
        .to_lowercase();

    if is_boolean_column(&data_type, column_length, column_scale, column_name) {
        return "Boolean";
    }

    const STR_TYPES: &[&str] = &[
        "char",
        "varchar",
        "enum",
        "set",
        "nchar",
        "nvarchar",
        "varchar2",
        "nvarchar2",
    ];
    const TEXT_TYPES: &[&str] = &[
        "tinytext",
        "text",
        "mediumtext",
        "longtext",
        "binary",
        "varbinary",
        "blob",
        "ntext",
        "image",
        "bytea",
    ];

    if STR_TYPES.contains(&data_type.as_str())
        || TEXT_TYPES.contains(&data_type.as_str())
    {
        return "String";
    }

    const TIME_TYPES: &[&str] = &[
        "datetime",
        "time",
        "date",
        "timestamp",
        "year",
        "interval",
        "smalldatetime",
        "datetime2",
        "datetimeoffset",
        "timestamptz",
    ];
    if TIME_TYPES.contains(&data_type.as_str()) {
        return "LocalDateTime";
    }

    const DECIMAL_TYPES: &[&str] =
        &["decimal", "numeric", "money", "smallmoney"];
    const FLOAT_TYPES: &[&str] = &[
        "float",
        "float4",
        "float8",
        "double",
        "real",
        "double precision",
    ];
    const BIGINT_TYPES: &[&str] = &["bigint", "int8", "bigserial"];
    const INT_TYPES: &[&str] = &[
        "smallint",
        "mediumint",
        "int",
        "int2",
        "int4",
        "integer",
        "smallserial",
        "serial",
    ];

    if DECIMAL_TYPES.contains(&data_type.as_str()) {
        return if column_scale > 0 {
            "BigDecimal"
        } else {
            resolve_integer_type(column_length)
        };
    }
    if FLOAT_TYPES.contains(&data_type.as_str()) {
        return "Double";
    }
    if BIGINT_TYPES.contains(&data_type.as_str()) {
        return "Long";
    }
    if INT_TYPES.contains(&data_type.as_str()) {
        return "Integer";
    }
    if data_type == "number" {
        return if column_scale > 0 {
            "BigDecimal"
        } else {
            resolve_integer_type(column_length)
        };
    }

    "Long"
}

fn is_boolean_column(
    data_type: &str,
    column_length: i32,
    column_scale: i32,
    column_name: &str,
) -> bool {
    if column_scale > 0 {
        return false;
    }
    if matches!(data_type, "bit" | "boolean" | "bool") {
        return true;
    }
    if matches!(
        data_type,
        "tinyint" | "number" | "numeric" | "decimal" | "char" | "nchar"
    ) && column_length == 1
        && is_switch_column(column_name)
    {
        return true;
    }
    false
}

fn resolve_integer_type(column_length: i32) -> &'static str {
    if column_length > 0 && column_length <= 9 {
        "Integer"
    } else {
        "Long"
    }
}

fn is_switch_column(column_name: &str) -> bool {
    let lower = column_name.to_lowercase();
    let ends = [
        "status",
        "flag",
        "enabled",
        "disabled",
        "available",
        "visible",
    ];
    ends.iter().any(|e| lower.ends_with(e))
        || lower.starts_with("is_")
        || lower.starts_with("has_")
        || lower.starts_with("enable_")
        || lower.starts_with("disable_")
}

pub fn resolve_html_type(
    column_type: &str,
    column_length: i32,
    column_scale: i32,
    column_name: &str,
) -> &'static str {
    let data_type = column_type
        .split('(')
        .next()
        .unwrap_or(column_type)
        .to_lowercase();
    let lower_name = column_name.to_lowercase();

    const STR_TYPES: &[&str] = &[
        "char",
        "varchar",
        "enum",
        "set",
        "nchar",
        "nvarchar",
        "varchar2",
        "nvarchar2",
    ];
    const TEXT_TYPES: &[&str] = &[
        "tinytext",
        "text",
        "mediumtext",
        "longtext",
        "binary",
        "varbinary",
        "blob",
        "ntext",
        "image",
        "bytea",
    ];
    const TIME_TYPES: &[&str] = &[
        "datetime",
        "time",
        "date",
        "timestamp",
        "year",
        "interval",
        "smalldatetime",
        "datetime2",
        "datetimeoffset",
        "timestamptz",
    ];
    const NUMBER_TYPES: &[&str] = &[
        "tinyint",
        "smallint",
        "mediumint",
        "int",
        "int2",
        "int4",
        "int8",
        "number",
        "integer",
        "bit",
        "bigint",
        "float",
        "float4",
        "float8",
        "double",
        "decimal",
        "numeric",
        "real",
        "double precision",
        "smallserial",
        "serial",
        "bigserial",
        "money",
        "smallmoney",
    ];

    // 1) 按数据类型计算默认 html_type（匹配 Java GenUtils.java:104-135）
    let mut html: &str = if STR_TYPES.contains(&data_type.as_str()) {
        "input"
    } else if TEXT_TYPES.contains(&data_type.as_str()) {
        "textarea"
    } else if TIME_TYPES.contains(&data_type.as_str()) {
        "datetime"
    } else if NUMBER_TYPES.contains(&data_type.as_str()) {
        "inputNumber"
    } else {
        "input"
    };

    // 2) 名称后缀覆盖（在类型默认值之上叠加）
    if lower_name.ends_with("status") {
        // status 后缀：布尔列优先 switch，否则 radio
        html = if is_boolean_column(
            &data_type,
            column_length,
            column_scale,
            column_name,
        ) {
            "switch"
        } else {
            "radio"
        };
    } else if lower_name.ends_with("type") || lower_name.ends_with("sex") {
        html = "select";
    } else if lower_name.ends_with("sort")
        || lower_name.ends_with("order_num")
        || lower_name.ends_with("order")
        || lower_name.ends_with("rank")
        || lower_name.ends_with("seq")
        || lower_name.ends_with("sequence")
    {
        html = "inputNumber";
    } else if lower_name.ends_with("image")
        || lower_name.ends_with("avatar")
        || lower_name.ends_with("logo")
        || lower_name.ends_with("picture")
    {
        html = "imageUpload";
    } else if lower_name.ends_with("file") || lower_name.ends_with("attachment")
    {
        html = "fileUpload";
    } else if lower_name.ends_with("remark")
        || lower_name.ends_with("description")
        || lower_name.ends_with("desc")
        || lower_name.ends_with("note")
    {
        html = "textarea";
    } else if lower_name.ends_with("content")
        || lower_name.ends_with("html")
        || lower_name.ends_with("body")
    {
        html = "editor";
    } else if is_boolean_column(
        &data_type,
        column_length,
        column_scale,
        column_name,
    ) {
        // 其他布尔列（非 status 命名，如 is_enabled）→ switch
        html = "switch";
    }

    html
}

#[cfg(test)]
mod tests {
    use super::*;

    // 占位符纪律的测试已随 build_table_filter 平移到
    // crates/app-data/src/dialect.rs(filter_uses_placeholders_not_interpolation
    // 及达梦版),SQL 文本由那里的逐字锚定测试接管。

    #[test]
    fn resolve_html_type_prefers_datetime_for_time_columns() {
        assert_eq!(
            resolve_html_type("datetime", 0, 0, "create_time"),
            "datetime"
        );
        assert_eq!(resolve_html_type("varchar", 64, 0, "status"), "radio");
        assert_eq!(resolve_html_type("varchar", 255, 0, "remark"), "textarea");
    }

    /// 达梦列经 From 映射时,空 column_type + 长度三元组由本 crate 的
    /// compose_column_type 拼装(codegen 逻辑留在 system 的交接锚)。
    #[test]
    fn dm_column_meta_composes_column_type_on_mapping() {
        let m = ColumnMeta {
            column_name: "user_name".into(),
            column_comment: Some("用户名".into()),
            column_type: String::new(), // 达梦方言无现成 column_type
            data_type: "VARCHAR".into(),
            data_length: Some(50),
            data_precision: None,
            data_scale: None,
            is_pk: false,
            is_auto_increment: false,
            is_nullable: false,
            ordinal_position: 2,
        };
        let raw = RawColumnInfo::from(m);
        assert_eq!(raw.column_type, "varchar(50)");
        assert_eq!(raw.data_type, "VARCHAR");
        assert!(!raw.is_nullable);

        // decimal 走精度分支:precision>0 时 (p,s)
        let m = ColumnMeta {
            column_name: "amount".into(),
            column_comment: None,
            column_type: String::new(),
            data_type: "DECIMAL".into(),
            data_length: None,
            data_precision: Some(19),
            data_scale: Some(2),
            is_pk: false,
            is_auto_increment: false,
            is_nullable: true,
            ordinal_position: 3,
        };
        assert_eq!(RawColumnInfo::from(m).column_type, "decimal(19,2)");

        // MySQL 形状(column_type 已有)原样透传,不二次加工
        let m = ColumnMeta {
            column_name: "id".into(),
            column_comment: None,
            column_type: "bigint(20)".into(),
            data_type: "bigint".into(),
            data_length: None,
            data_precision: None,
            data_scale: None,
            is_pk: true,
            is_auto_increment: true,
            is_nullable: false,
            ordinal_position: 1,
        };
        let raw = RawColumnInfo::from(m);
        assert_eq!(raw.column_type, "bigint(20)");
        assert!(raw.is_pk && raw.is_auto_increment);
    }

    /// TableMeta → DbTableVo:字符串时间戳原样透传(Vue 契约不变)。
    #[test]
    fn table_meta_maps_straight_to_vo() {
        let m = TableMeta {
            table_name: "sys_user".into(),
            table_comment: "用户表".into(),
            create_time: Some("2026-08-14 10:00:00".into()),
            update_time: None,
        };
        let vo = DbTableVo::from(m);
        assert_eq!(vo.table_name, "sys_user");
        assert_eq!(vo.table_comment, "用户表");
        assert_eq!(vo.create_time.as_deref(), Some("2026-08-14 10:00:00"));
        assert!(vo.update_time.is_none());
    }
}
