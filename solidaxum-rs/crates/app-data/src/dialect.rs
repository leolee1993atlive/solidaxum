//! 方言 SQL 唯一出口(设计 6.3 dialect 行,crate 私有)。
//!
//! 这里收编 `crates/system/src/codegen/service/db_meta.rs` 的元数据查询方言
//! 分叉:MySQL(information_schema 三条)与达梦(ALL_TABLES/ALL_TAB_COLUMNS
//! 三条)的 SQL 文本**逐字搬移** —— 它们是达梦实机验证过的,不重写、不"改进"。
//! `DatabaseBackend::Dm` 的分支只出现在本模块(设计 9 节 CI 守卫断言的口径)。
//!
//! SQLite 是**测试方言**:生产部署只有 MySQL/达梦,但元数据读取器的往返
//! 语义要在 CI 的 SQLite 内存库上验证(建表 → list_tables/list_columns),
//! 因此补了 sqlite_master/pragma_table_info 一组。它没有达梦那样的实机
//! 验证包袱,风格上向 MySQL 组看齐。
//!
//! `TABLE_IGNORE_PREFIXES` 与 `{{ table_name }}` 模板的语义随 `build_table_filter`
//! 一起迁入(它们是过滤行为的一部分,不是 codegen 逻辑)。

use sea_orm::{DbBackend, Statement, Value};

const TABLE_IGNORE_PREFIXES: &[&str] = &["sj_", "flow_", "gen_"];

fn build_table_filter(
    _db_name: &str,
    table_name: Option<&str>,
    table_comment: Option<&str>,
    imported_tables: &[String],
) -> (String, Vec<Value>) {
    // MySQL 单实例单连接:dataName 是数据源名(如 "master"),
    // 不是 schema 名;schema 以连接当前库为准(DATABASE())。
    // 直接把 dataName 当 table_schema 过滤会查出空表(走查实测)。
    // 达梦单实例多 schema,OWNER 过滤留在 build_table_filter_dm。
    let mut conditions: Vec<String> = vec![
        "table_schema = DATABASE()".to_string(),
        "table_type = 'BASE TABLE'".to_string(),
    ];
    let mut values: Vec<Value> = Vec::new();

    for prefix in TABLE_IGNORE_PREFIXES {
        conditions.push("table_name NOT LIKE ?".to_string());
        values.push(format!("{}%", prefix).into());
    }

    if !imported_tables.is_empty() {
        let placeholders: Vec<&str> =
            imported_tables.iter().map(|_| "?").collect();
        conditions
            .push(format!("table_name NOT IN ({})", placeholders.join(",")));
        for t in imported_tables {
            values.push(t.clone().into());
        }
    }

    if let Some(name) = table_name.filter(|s| !s.is_empty()) {
        conditions.push("LOWER(table_name) LIKE ?".to_string());
        values.push(format!("%{}%", name.to_lowercase()).into());
    }
    if let Some(comment) = table_comment.filter(|s| !s.is_empty()) {
        conditions.push("LOWER(table_comment) LIKE ?".to_string());
        values.push(format!("%{}%", comment.to_lowercase()).into());
    }

    let where_clause = conditions.join(" AND ");
    (where_clause, values)
}

/// 达梦版表过滤:作用在 `ALL_TABLES t LEFT JOIN ALL_TAB_COMMENTS tc` 上。
/// 达梦为单实例单库架构,MySQL 的 table_schema 对应达梦的 schema(OWNER)。
/// 达梦实例可能大小写敏感,表名比较统一走 LOWER()。
fn build_table_filter_dm(
    owner: &str,
    table_name: Option<&str>,
    table_comment: Option<&str>,
    imported_tables: &[String],
) -> (String, Vec<Value>) {
    let mut conditions: Vec<String> = vec!["t.OWNER = ?".to_string()];
    let mut values: Vec<Value> = vec![owner.into()];

    for prefix in TABLE_IGNORE_PREFIXES {
        conditions.push("LOWER(t.TABLE_NAME) NOT LIKE ?".to_string());
        values.push(format!("{}%", prefix).into());
    }

    if !imported_tables.is_empty() {
        let placeholders: Vec<String> = imported_tables
            .iter()
            .map(|_| "LOWER(?)".to_string())
            .collect();
        conditions.push(format!(
            "LOWER(t.TABLE_NAME) NOT IN ({})",
            placeholders.join(",")
        ));
        for t in imported_tables {
            values.push(t.clone().into());
        }
    }

    if let Some(name) = table_name.filter(|s| !s.is_empty()) {
        conditions.push("LOWER(t.TABLE_NAME) LIKE ?".to_string());
        values.push(format!("%{}%", name.to_lowercase()).into());
    }
    if let Some(comment) = table_comment.filter(|s| !s.is_empty()) {
        conditions.push("LOWER(tc.COMMENTS) LIKE ?".to_string());
        values.push(format!("%{}%", comment.to_lowercase()).into());
    }

    let where_clause = conditions.join(" AND ");
    (where_clause, values)
}

/// SQLite 测试方言的表过滤:sqlite_master 无 schema 列(单库),无表注释列
/// (注释条件无从谈起);系统内部表(sqlite_sequence 等)以 `sqlite_` 前缀
/// 排除 —— AUTOINCREMENT 建表会自动造出 sqlite_sequence,不排除会漏进清单。
/// 其余条件(忽略前缀 / 已导入排除 / 表名模糊)与 MySQL 版同构。
fn build_table_filter_sqlite(
    table_name: Option<&str>,
    imported_tables: &[String],
) -> (String, Vec<Value>) {
    let mut conditions: Vec<String> =
        vec!["type = 'table'".to_string(), "name NOT LIKE ?".to_string()];
    let mut values: Vec<Value> = vec!["sqlite_%".into()];

    for prefix in TABLE_IGNORE_PREFIXES {
        conditions.push("name NOT LIKE ?".to_string());
        values.push(format!("{}%", prefix).into());
    }

    if !imported_tables.is_empty() {
        let placeholders: Vec<&str> =
            imported_tables.iter().map(|_| "?").collect();
        conditions.push(format!("name NOT IN ({})", placeholders.join(",")));
        for t in imported_tables {
            values.push(t.clone().into());
        }
    }

    if let Some(name) = table_name.filter(|s| !s.is_empty()) {
        conditions.push("LOWER(name) LIKE ?".to_string());
        values.push(format!("%{}%", name.to_lowercase()).into());
    }

    let where_clause = conditions.join(" AND ");
    (where_clause, values)
}

// ---- MySQL(information_schema)------------------------------------------------

pub(crate) fn mysql_list_tables_count(
    db_name: &str,
    table_name: Option<&str>,
    table_comment: Option<&str>,
    imported_tables: &[String],
) -> Statement {
    let (where_clause, values) =
        build_table_filter(db_name, table_name, table_comment, imported_tables);

    let count_sql = format!(
        "SELECT COUNT(*) as cnt FROM information_schema.tables WHERE {}",
        where_clause
    );
    Statement::from_sql_and_values(DbBackend::MySql, &count_sql, values)
}

pub(crate) fn mysql_list_tables_page(
    db_name: &str,
    table_name: Option<&str>,
    table_comment: Option<&str>,
    imported_tables: &[String],
    offset: u64,
    limit: u64,
) -> Statement {
    let (where_clause, values) =
        build_table_filter(db_name, table_name, table_comment, imported_tables);

    // information_schema 的列名按数据字典返回**大写**(TABLE_NAME 等),
    // sea-orm 按标签取值,必须显式 AS 小写别名(SQLite 组同款手法)。
    let page_sql = format!(
        "SELECT table_name AS table_name, table_comment AS table_comment, \
         create_time AS create_time, update_time AS update_time \
         FROM information_schema.tables \
         WHERE {} ORDER BY create_time DESC LIMIT ? OFFSET ?",
        where_clause
    );
    let mut page_values = values;
    page_values.push((limit as i64).into());
    page_values.push((offset as i64).into());
    Statement::from_sql_and_values(DbBackend::MySql, &page_sql, page_values)
}

pub(crate) fn mysql_table_comment(
    _db_name: &str,
    table_name: &str,
) -> Statement {
    // 列标签同大写问题,见 mysql_list_tables_page 的说明。
    let sql = "SELECT table_comment AS table_comment \
               FROM information_schema.tables \
               WHERE table_schema = DATABASE() AND table_name = ?";
    Statement::from_sql_and_values(DbBackend::MySql, sql, [table_name.into()])
}

pub(crate) fn mysql_list_columns(
    _db_name: &str,
    table_name: &str,
) -> Statement {
    // 列标签同大写问题,见 mysql_list_tables_page 的说明。
    let sql = "SELECT column_name AS column_name, column_comment AS column_comment, \
         column_type AS column_type, data_type AS data_type, \
         is_nullable AS is_nullable, column_key AS column_key, \
         extra AS extra, ordinal_position AS ordinal_position \
         FROM information_schema.columns \
         WHERE table_schema = DATABASE() AND table_name = ? \
         ORDER BY ordinal_position";

    Statement::from_sql_and_values(DbBackend::MySql, sql, [table_name.into()])
}

// ---- 达梦(ALL_TABLES / ALL_TAB_COLUMNS)--------------------------------------

pub(crate) fn dm_list_tables_count(
    owner: &str,
    table_name: Option<&str>,
    table_comment: Option<&str>,
    imported_tables: &[String],
) -> Statement {
    let (where_clause, values) = build_table_filter_dm(
        owner,
        table_name,
        table_comment,
        imported_tables,
    );
    const FROM: &str = "FROM ALL_TABLES t \
            LEFT JOIN ALL_TAB_COMMENTS tc \
            ON tc.OWNER = t.OWNER AND tc.TABLE_NAME = t.TABLE_NAME";

    let count_sql =
        format!("SELECT COUNT(*) AS cnt {FROM} WHERE {where_clause}");
    Statement::from_sql_and_values(DbBackend::Dm, &count_sql, values)
}

/// 达梦版 list_tables 分页语句。查 `ALL_TABLES` + `ALL_TAB_COMMENTS`,
/// 建表时间取自 `SYS.SYSOBJECTS.CRTDATE`(达梦无 update_time,返回 None)。
/// CRTDATE 要 join SYS.SYSOBJECTS;SCHID 经 schema 对象子查询得到。
/// 该子查询的 ? 在 SELECT 列表里,绑定值须排在 WHERE 参数之前。
pub(crate) fn dm_list_tables_page(
    owner: &str,
    table_name: Option<&str>,
    table_comment: Option<&str>,
    imported_tables: &[String],
    offset: u64,
    limit: u64,
) -> Statement {
    let (where_clause, values) = build_table_filter_dm(
        owner,
        table_name,
        table_comment,
        imported_tables,
    );
    const FROM: &str = "FROM ALL_TABLES t \
            LEFT JOIN ALL_TAB_COMMENTS tc \
            ON tc.OWNER = t.OWNER AND tc.TABLE_NAME = t.TABLE_NAME";

    let page_sql = format!(
        "SELECT t.TABLE_NAME AS table_name, tc.COMMENTS AS table_comment, \
         o.CRTDATE AS create_time, NULL AS update_time \
         {FROM} \
         LEFT JOIN SYS.SYSOBJECTS o \
         ON o.NAME = t.TABLE_NAME AND o.TYPE$ = 'SCHOBJ' AND o.SUBTYPE$ = 'UTAB' \
         AND o.SCHID IN (SELECT ID FROM SYS.SYSOBJECTS WHERE NAME = ? AND TYPE$ = 'SCH') \
         WHERE {where_clause} ORDER BY o.CRTDATE DESC LIMIT ? OFFSET ?"
    );
    let mut page_values = vec![owner.into()];
    page_values.extend(values);
    page_values.push((limit as i64).into());
    page_values.push((offset as i64).into());
    Statement::from_sql_and_values(DbBackend::Dm, &page_sql, page_values)
}

pub(crate) fn dm_table_comment(owner: &str, table_name: &str) -> Statement {
    let sql = "SELECT COMMENTS AS table_comment FROM ALL_TAB_COMMENTS \
               WHERE OWNER = ? AND LOWER(TABLE_NAME) = LOWER(?)";
    Statement::from_sql_and_values(
        DbBackend::Dm,
        sql,
        [owner.into(), table_name.into()],
    )
}

/// 达梦版 list_columns:查 `ALL_TAB_COLUMNS` + `ALL_COL_COMMENTS`;
/// 主键由 `ALL_CONSTRAINTS`/`ALL_CONS_COLUMNS` 约束视图 join 得到,
/// 自增由 `SYS.SYSCOLUMNS.INFO2 = 1` 判定(实机验证:identity 列 INFO2=1,
/// 普通列 INFO2=0;达梦无 ALL_TAB_IDENTITY_COLS 视图)。
pub(crate) fn dm_list_columns(owner: &str, table_name: &str) -> Statement {
    let sql = "SELECT c.COLUMN_NAME AS column_name, cc.COMMENTS AS column_comment, \
         c.DATA_TYPE AS data_type, c.DATA_LENGTH AS data_length, \
         c.DATA_PRECISION AS data_precision, c.DATA_SCALE AS data_scale, \
         c.NULLABLE AS nullable, c.COLUMN_ID AS ordinal_position, \
         CASE WHEN EXISTS ( \
             SELECT 1 FROM ALL_CONSTRAINTS ac \
             JOIN ALL_CONS_COLUMNS acc \
             ON ac.CONSTRAINT_NAME = acc.CONSTRAINT_NAME AND ac.OWNER = acc.OWNER \
             WHERE ac.CONSTRAINT_TYPE = 'P' AND ac.OWNER = c.OWNER \
             AND ac.TABLE_NAME = c.TABLE_NAME AND acc.COLUMN_NAME = c.COLUMN_NAME \
         ) THEN 1 ELSE 0 END AS is_pk, \
         CASE WHEN EXISTS ( \
             SELECT 1 FROM SYS.SYSCOLUMNS sc JOIN SYS.SYSOBJECTS o ON sc.ID = o.ID \
             WHERE o.NAME = c.TABLE_NAME AND sc.NAME = c.COLUMN_NAME AND sc.INFO2 = 1 \
             AND o.SCHID IN (SELECT ID FROM SYS.SYSOBJECTS WHERE NAME = c.OWNER AND TYPE$ = 'SCH') \
         ) THEN 1 ELSE 0 END AS is_identity \
         FROM ALL_TAB_COLUMNS c \
         LEFT JOIN ALL_COL_COMMENTS cc \
         ON cc.OWNER = c.OWNER AND cc.TABLE_NAME = c.TABLE_NAME \
         AND cc.COLUMN_NAME = c.COLUMN_NAME \
         WHERE c.OWNER = ? AND LOWER(c.TABLE_NAME) = LOWER(?) \
         ORDER BY c.COLUMN_ID";

    Statement::from_sql_and_values(
        DbBackend::Dm,
        sql,
        [owner.into(), table_name.into()],
    )
}

// ---- SQLite(测试方言)--------------------------------------------------------

/// SQLite 无建表时间(sqlite_master 不记),排序退化为表名序;
/// 注释恒为空串。db_name 参数在此方言无意义,由 meta 层丢弃。
pub(crate) fn sqlite_list_tables_count(
    table_name: Option<&str>,
    imported_tables: &[String],
) -> Statement {
    let (where_clause, values) =
        build_table_filter_sqlite(table_name, imported_tables);
    let count_sql = format!(
        "SELECT COUNT(*) AS cnt FROM sqlite_master WHERE {}",
        where_clause
    );
    Statement::from_sql_and_values(DbBackend::Sqlite, &count_sql, values)
}

pub(crate) fn sqlite_list_tables_page(
    table_name: Option<&str>,
    imported_tables: &[String],
    offset: u64,
    limit: u64,
) -> Statement {
    let (where_clause, values) =
        build_table_filter_sqlite(table_name, imported_tables);
    let page_sql = format!(
        "SELECT name AS table_name, '' AS table_comment, \
         NULL AS create_time, NULL AS update_time \
         FROM sqlite_master \
         WHERE {} ORDER BY name LIMIT ? OFFSET ?",
        where_clause
    );
    let mut page_values = values;
    page_values.push((limit as i64).into());
    page_values.push((offset as i64).into());
    Statement::from_sql_and_values(DbBackend::Sqlite, &page_sql, page_values)
}

/// SQLite 列清单:pragma_table_info 表值函数给 cid(0 起,补 1 对齐两方言的
/// 1 起 ordinal)、声明类型(兼作 column_type 与 data_type)、notnull、pk 位。
/// 可空判定额外要求非主键:SQLite 的 INTEGER PRIMARY KEY(rowid 别名)在
/// pragma 里 notnull=0,但主键实际上不可能为 NULL,与两方言的语义对齐。
/// 自增判定:列为 INTEGER 主键且建表 DDL 带 AUTOINCREMENT(sqlite_master.sql)。
/// 与达梦分页语句同一形状的怪癖:SELECT 列表里的 ?(自增子查询的表名)
/// 必须排在 FROM 里 pragma_table_info(?) 的参数之前。
pub(crate) fn sqlite_list_columns(table_name: &str) -> Statement {
    let sql = "SELECT p.name AS column_name, NULL AS column_comment, \
         p.type AS column_type, p.type AS data_type, \
         CASE WHEN p.\"notnull\" = 0 AND p.pk = 0 THEN 1 ELSE 0 END AS is_nullable, \
         CASE WHEN p.pk > 0 THEN 1 ELSE 0 END AS is_pk, \
         CASE WHEN p.pk > 0 AND UPPER(TRIM(p.type)) = 'INTEGER' AND EXISTS ( \
             SELECT 1 FROM sqlite_master m \
             WHERE m.type = 'table' AND m.name = ? \
             AND m.sql LIKE '%AUTOINCREMENT%' \
         ) THEN 1 ELSE 0 END AS is_auto_increment, \
         p.cid + 1 AS ordinal_position \
         FROM pragma_table_info(?) p \
         ORDER BY p.cid";

    Statement::from_sql_and_values(
        DbBackend::Sqlite,
        sql,
        [table_name.into(), table_name.into()],
    )
}

/// 表级估算行数(分页总数策略「目录估算」的方言分叉)。
///
/// 读的是**优化器统计目录**,不扫表 —— 代价与表大小无关。代价是它
/// **不认 WHERE、也不是精确值**,只配用来显示数量级:
///
/// - PG `pg_class.reltuples` 由 ANALYZE / autovacuum 维护。20 万行 sys_user
///   实测 200002(真值 200002,ANALYZE 刚跑过)。从未 ANALYZE 过的表
///   PG 14+ 返回 **-1**,调用方判负丢弃。
/// - MySQL `information_schema.tables.table_rows` 是 InnoDB 的采样估算。
///   同一张表实测 197132 vs 真值 200002 —— **偏差 1.4%**。所以它只能进
///   展示位,**不得**参与页数计算。
///
/// SQLite 没有这类目录(sqlite_stat1 要先 ANALYZE,且只存索引基数);
/// 达梦的 `ALL_TABLES.NUM_ROWS` 需要 DBMS_STATS 且**没有实机验证过** ——
/// 本模块其余达梦 SQL 都是实机验证过才写下的,这里不破例。两者返回 None,
/// 调用方退化成「只显示上限」。
pub(crate) fn approx_row_count(
    backend: DbBackend,
    table_name: &str,
) -> Option<Statement> {
    match backend {
        DbBackend::Postgres => Some(Statement::from_sql_and_values(
            DbBackend::Postgres,
            // 占位符是 $1 不是 ? —— sea-orm 把 SQL 原样交给 sqlx,不改写占位符。
            // to_regclass 按 search_path 解析,与业务查询同解;表不存在时返回
            // NULL,`oid = NULL` 自然出空集(不报错)。
            "SELECT reltuples::bigint AS est FROM pg_class \
             WHERE oid = to_regclass($1)",
            [table_name.into()],
        )),
        DbBackend::MySql => Some(Statement::from_sql_and_values(
            DbBackend::MySql,
            // CAST(.. AS SIGNED):table_rows 是 BIGINT UNSIGNED,
            // 不转的话 try_get::<i64> 会因类型不符报错。
            "SELECT CAST(table_rows AS SIGNED) AS est \
             FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_name = ?",
            [table_name.into()],
        )),
        // DatabaseBackend 是 non_exhaustive:没验证过的后端一律"没有估算值",
        // 由调用方退化成只显示上限 —— 猜一条 SQL 出去只会给出错的数。
        DbBackend::Sqlite | DbBackend::Dm | _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 平移自 crates/system/src/codegen/service/db_meta.rs 的同名测试:
    // 用户输入绝不出现在 SQL 文本里,一律走绑定值。
    #[test]
    fn filter_uses_placeholders_not_interpolation() {
        let (where_clause, values) = build_table_filter(
            "test_db",
            Some("evil'; DROP TABLE x;--"),
            None,
            &[],
        );
        // 用户输入绝不出现在 SQL 文本里
        assert!(
            !where_clause.contains("evil"),
            "用户输入未在 WHERE 子句中出现"
        );
        assert!(
            where_clause.contains("LIKE ?"),
            "使用 ? 占位符而非字符串插值"
        );
        // 输入作为绑定值传递
        let has_evil = values.iter().any(|v| format!("{v:?}").contains("evil"));
        assert!(has_evil, "用户输入通过绑定值传递");

        // imported_tables 中的值也应为绑定
        let (where_clause2, values2) = build_table_filter(
            "test_db",
            None,
            None,
            &["evil_table';--".to_string()],
        );
        assert!(
            !where_clause2.contains("evil_table"),
            "imported_tables 也不应直接拼接"
        );
        assert!(
            values2
                .iter()
                .any(|v| format!("{v:?}").contains("evil_table")),
            "imported_tables 通过绑定值传递"
        );
    }

    /// 达梦版过滤同一手法的锚:LOWER(?) 包裹的 NOT IN 与 LOWER() 比较。
    #[test]
    fn dm_filter_uses_placeholders_not_interpolation() {
        let (where_clause, values) = build_table_filter_dm(
            "SYSDBA",
            Some("evil'; DROP TABLE x;--"),
            None,
            &[],
        );
        assert!(
            !where_clause.contains("evil"),
            "用户输入未在 WHERE 子句中出现"
        );
        assert!(where_clause.contains("LOWER(t.TABLE_NAME) LIKE ?"));
        assert!(
            values.iter().any(|v| format!("{v:?}").contains("evil")),
            "用户输入通过绑定值传递"
        );

        let (where_clause2, values2) = build_table_filter_dm(
            "SYSDBA",
            None,
            None,
            &["evil_table';--".to_string()],
        );
        assert!(!where_clause2.contains("evil_table"));
        // 达梦版 NOT IN 里是 LOWER(?),不是裸 ?
        assert!(where_clause2.contains("NOT IN (LOWER(?))"));
        assert!(
            values2
                .iter()
                .any(|v| format!("{v:?}").contains("evil_table")),
            "imported_tables 通过绑定值传递"
        );
    }

    /// 固定入参下的 MySQL 语句全文锚(逐字)。WHERE 段由
    /// build_table_filter 决定:DATABASE() + 3 个忽略前缀,无其它条件。
    #[test]
    fn mysql_sql_texts_are_verbatim() {
        const W: &str = "table_schema = DATABASE() AND table_type = 'BASE TABLE' \
             AND table_name NOT LIKE ? AND table_name NOT LIKE ? \
             AND table_name NOT LIKE ?";

        let count = mysql_list_tables_count("test_db", None, None, &[]);
        assert_eq!(
            count.sql,
            format!(
                "SELECT COUNT(*) as cnt FROM information_schema.tables WHERE {W}"
            )
        );
        assert_eq!(count.db_backend, DbBackend::MySql);

        let page = mysql_list_tables_page("test_db", None, None, &[], 5, 7);
        assert_eq!(
            page.sql,
            format!(
                "SELECT table_name AS table_name, \
                 table_comment AS table_comment, \
                 create_time AS create_time, update_time AS update_time \
                 FROM information_schema.tables \
                 WHERE {W} ORDER BY create_time DESC LIMIT ? OFFSET ?"
            )
        );
        // LIMIT/OFFSET 的绑定顺序:先 limit 后 offset(与占位符顺序一致)。
        // 取 offset=5、limit=7 使两个数字互不为子串,断言无歧义。
        // 5 个绑定值 = 3 个忽略前缀 + limit + offset(schema 走 DATABASE(),
        // 不再占绑定位)。
        let values = page.values.as_ref().unwrap().0.clone();
        assert_eq!(values.len(), 5);
        assert!(format!("{:?}", values[3]).contains('7'), "limit 在前");
        assert!(format!("{:?}", values[4]).contains('5'), "offset 在后");

        let comment = mysql_table_comment("test_db", "sys_user");
        assert_eq!(
            comment.sql,
            "SELECT table_comment AS table_comment \
             FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_name = ?"
        );

        let columns = mysql_list_columns("test_db", "sys_user");
        assert_eq!(
            columns.sql,
            "SELECT column_name AS column_name, \
             column_comment AS column_comment, \
             column_type AS column_type, data_type AS data_type, \
             is_nullable AS is_nullable, column_key AS column_key, \
             extra AS extra, ordinal_position AS ordinal_position \
             FROM information_schema.columns \
             WHERE table_schema = DATABASE() AND table_name = ? \
             ORDER BY ordinal_position"
        );
    }

    /// 达梦语句全文锚 + SELECT 列表占位符顺序的怪癖(原 db_meta.rs :217):
    /// SCHID 子查询的 ? 在 SELECT 列表,绑定值必须排在 WHERE 参数之前。
    #[test]
    fn dm_sql_texts_and_placeholder_order() {
        const W: &str = "t.OWNER = ? \
             AND LOWER(t.TABLE_NAME) NOT LIKE ? \
             AND LOWER(t.TABLE_NAME) NOT LIKE ? \
             AND LOWER(t.TABLE_NAME) NOT LIKE ?";
        const FROM: &str = "FROM ALL_TABLES t \
             LEFT JOIN ALL_TAB_COMMENTS tc \
             ON tc.OWNER = t.OWNER AND tc.TABLE_NAME = t.TABLE_NAME";

        let count = dm_list_tables_count("SYSDBA", None, None, &[]);
        assert_eq!(
            count.sql,
            format!("SELECT COUNT(*) AS cnt {FROM} WHERE {W}")
        );
        assert_eq!(count.db_backend, DbBackend::Dm);

        let page = dm_list_tables_page("SYSDBA", None, None, &[], 5, 7);
        assert_eq!(
            page.sql,
            format!(
                "SELECT t.TABLE_NAME AS table_name, tc.COMMENTS AS table_comment, \
                 o.CRTDATE AS create_time, NULL AS update_time \
                 {FROM} \
                 LEFT JOIN SYS.SYSOBJECTS o \
                 ON o.NAME = t.TABLE_NAME AND o.TYPE$ = 'SCHOBJ' AND o.SUBTYPE$ = 'UTAB' \
                 AND o.SCHID IN (SELECT ID FROM SYS.SYSOBJECTS WHERE NAME = ? AND TYPE$ = 'SCH') \
                 WHERE {W} ORDER BY o.CRTDATE DESC LIMIT ? OFFSET ?"
            )
        );

        // 占位符出现顺序:SCHID 子查询(位于 SELECT 列表)的 ? 必须在
        // 主 WHERE 的第一个占位符(OWNER)之前 —— 注意不能拿子查询自己的
        // WHERE 定位主句,主 WHERE 由 where_clause 的首条件锚定。
        let schid_frag = page
            .sql
            .find("o.SCHID IN (SELECT ID FROM SYS.SYSOBJECTS WHERE NAME = ?")
            .unwrap();
        let main_where = page.sql.find(" WHERE t.OWNER").unwrap();
        let schid_ph = page.sql[schid_frag..].find('?').unwrap() + schid_frag;
        let owner_ph = page.sql[main_where..].find('?').unwrap() + main_where;
        assert!(
            schid_ph < owner_ph,
            "SELECT 列表占位符必须排在主 WHERE 之前(绑定顺序即出现顺序)"
        );

        // 绑定值顺序:owner(SCHID 子查询)→ WHERE 值 → limit → offset
        // 7 个绑定值 = 1 SCHID + 1 OWNER + 3 忽略前缀 + limit + offset
        let values = page.values.as_ref().unwrap().0.clone();
        assert_eq!(values.len(), 7);
        assert!(
            format!("{:?}", values[0]).contains("SYSDBA"),
            "首个绑定值是 SCHID 子查询的 schema 名"
        );
        assert!(
            format!("{:?}", values[1]).contains("SYSDBA"),
            "第二个绑定值才是 WHERE 的 OWNER"
        );

        let comment = dm_table_comment("SYSDBA", "SYS_USER");
        assert_eq!(
            comment.sql,
            "SELECT COMMENTS AS table_comment FROM ALL_TAB_COMMENTS \
             WHERE OWNER = ? AND LOWER(TABLE_NAME) = LOWER(?)"
        );
        assert_eq!(comment.db_backend, DbBackend::Dm);

        let columns = dm_list_columns("SYSDBA", "SYS_USER");
        assert_eq!(
            columns.sql,
            "SELECT c.COLUMN_NAME AS column_name, cc.COMMENTS AS column_comment, \
             c.DATA_TYPE AS data_type, c.DATA_LENGTH AS data_length, \
             c.DATA_PRECISION AS data_precision, c.DATA_SCALE AS data_scale, \
             c.NULLABLE AS nullable, c.COLUMN_ID AS ordinal_position, \
             CASE WHEN EXISTS ( \
                 SELECT 1 FROM ALL_CONSTRAINTS ac \
                 JOIN ALL_CONS_COLUMNS acc \
                 ON ac.CONSTRAINT_NAME = acc.CONSTRAINT_NAME AND ac.OWNER = acc.OWNER \
                 WHERE ac.CONSTRAINT_TYPE = 'P' AND ac.OWNER = c.OWNER \
                 AND ac.TABLE_NAME = c.TABLE_NAME AND acc.COLUMN_NAME = c.COLUMN_NAME \
             ) THEN 1 ELSE 0 END AS is_pk, \
             CASE WHEN EXISTS ( \
                 SELECT 1 FROM SYS.SYSCOLUMNS sc JOIN SYS.SYSOBJECTS o ON sc.ID = o.ID \
                 WHERE o.NAME = c.TABLE_NAME AND sc.NAME = c.COLUMN_NAME AND sc.INFO2 = 1 \
                 AND o.SCHID IN (SELECT ID FROM SYS.SYSOBJECTS WHERE NAME = c.OWNER AND TYPE$ = 'SCH') \
             ) THEN 1 ELSE 0 END AS is_identity \
             FROM ALL_TAB_COLUMNS c \
             LEFT JOIN ALL_COL_COMMENTS cc \
             ON cc.OWNER = c.OWNER AND cc.TABLE_NAME = c.TABLE_NAME \
             AND cc.COLUMN_NAME = c.COLUMN_NAME \
             WHERE c.OWNER = ? AND LOWER(c.TABLE_NAME) = LOWER(?) \
             ORDER BY c.COLUMN_ID"
        );
    }

    /// SQLite 测试方言的文本锚:sqlite_ 内部表排除 + 同样的占位符纪律。
    #[test]
    fn sqlite_sql_texts_use_placeholders() {
        let page = sqlite_list_tables_page(Some("probe"), &[], 0, 10);
        assert!(page.sql.starts_with(
            "SELECT name AS table_name, '' AS table_comment, \
                 NULL AS create_time, NULL AS update_time \
                 FROM sqlite_master \
                 WHERE type = 'table' AND name NOT LIKE ?"
        ));
        assert!(page.sql.ends_with("ORDER BY name LIMIT ? OFFSET ?"));
        assert!(page.sql.contains("LOWER(name) LIKE ?"), "表名过滤走占位符");

        let columns = sqlite_list_columns("meta_probe");
        // 两个 ?:自增子查询的表名在前,pragma_table_info 的参数在后
        assert_eq!(columns.sql.matches('?').count(), 2);
        let exists_pos = columns.sql.find("m.name = ?").unwrap();
        let pragma_pos = columns.sql.find("pragma_table_info(?)").unwrap();
        assert!(exists_pos < pragma_pos);
        let values = columns.values.as_ref().unwrap().0.clone();
        assert!(format!("{:?}", values[0]).contains("meta_probe"));
        assert!(format!("{:?}", values[1]).contains("meta_probe"));
    }
}
