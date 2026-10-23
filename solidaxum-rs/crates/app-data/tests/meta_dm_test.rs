//! 元数据读取的后端特定方言测试(自 system 测试套件挪入,设计 9.4:
//! "真正需要后端特定行为的测试属于 app-data 自身的 dialect 测试套件"):
//!
//! - SQLite 路径常跑:同一探针形状下断言 list_tables / list_columns /
//!   get_table_comment 的派生字段语义("SQLite-backed where possible"
//!   —— 无 DM 实例的 CI 里钉住往返行为)。
//! - 达梦分支保留实机验证:`DM_ODBC="DSN=DM8;UID=..;PWD=.." cargo test
//!   -p app_data --test meta_dm_test -- --ignored`;其 SQL 文本本身由
//!   本 crate 的 dialect 单元测试逐字锚定。
//!
//! 自建探针表(含 identity、PK、注释、可空列),逐项验证从
//! ALL_TABLES/ALL_TAB_COLUMNS/ALL_CONSTRAINTS/SYSCOLUMNS 派生的字段语义。
//!
//! 与 system 侧原文件的两处适配(薄委托 DbMetaReader → MetadataReader
//! 本体,断言等价性见各注释):
//! - `get_table_comment` 返回 `Option<String>`(委托层才对 None 缺省空串);
//! - 达梦无现成 column_type(中立类型留空,长度三元组随行,拼装是
//!   system 侧 codegen 逻辑)—— 类型断言落在 data_type/data_length 上。

use app_data::meta::MetadataReader;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DbBackend, Statement,
};

const TABLE: &str = "dm_meta_probe";

async fn exec(db: &sea_orm::DatabaseConnection, sql: &str) {
    db.execute_raw(Statement::from_string(DbBackend::Dm, sql))
        .await
        .unwrap();
}

async fn setup() -> sea_orm::DatabaseConnection {
    let dsn =
        format!("dm:{}", std::env::var("DM_ODBC").expect("未设置 DM_ODBC"));
    let db = Database::connect(dsn).await.unwrap();
    // 首跑时表不存在,DROP 失败属预期,忽略
    let _ = db
        .execute_raw(Statement::from_string(
            DbBackend::Dm,
            format!("DROP TABLE {TABLE}"),
        ))
        .await;
    exec(
        &db,
        &format!(
            "CREATE TABLE {TABLE} (\
             user_id BIGINT IDENTITY(1,1) PRIMARY KEY, \
             user_name VARCHAR(50) NOT NULL, \
             remark VARCHAR(200) NULL)"
        ),
    )
    .await;
    exec(&db, &format!("COMMENT ON TABLE {TABLE} IS '探针表'")).await;
    exec(
        &db,
        &format!("COMMENT ON COLUMN {TABLE}.user_name IS '用户名'"),
    )
    .await;
    db
}

#[tokio::test]
#[ignore = "需要真实达梦"]
async fn lists_tables_and_columns_on_dm() {
    let db = setup().await;
    // 数据访问经 app-data 漏斗:system_context(代码生成是管理功能,无行范围)
    let ds = app_data::DataSource::from_connection(db);
    let ctx = ds.system_context();
    let reader = MetadataReader::new(&ctx);

    // list_tables:表名过滤 + 注释 + 总数
    let (tables, total) = reader
        .list_tables("SYSDBA", Some("meta_probe"), None, &[], 0, 10)
        .await
        .unwrap();
    assert!(total >= 1, "list_tables 未找到探针表");
    let t = tables
        .iter()
        .find(|t| t.table_name.eq_ignore_ascii_case(TABLE))
        .expect("分页结果中应有探针表");
    assert_eq!(t.table_comment, "探针表");

    // 按注释过滤
    let (by_comment, _) = reader
        .list_tables("SYSDBA", None, Some("探针"), &[], 0, 10)
        .await
        .unwrap();
    assert!(
        by_comment
            .iter()
            .any(|t| t.table_name.eq_ignore_ascii_case(TABLE)),
        "按注释过滤未命中"
    );

    // get_table_comment(MetadataReader 本体返回 Option,库给出注释 → Some)
    let comment = reader.get_table_comment("SYSDBA", TABLE).await.unwrap();
    assert_eq!(comment.as_deref(), Some("探针表"));

    // list_columns:逐字段派生语义
    let cols = reader.list_columns("SYSDBA", TABLE).await.unwrap();
    assert_eq!(cols.len(), 3);

    let user_id = cols
        .iter()
        .find(|c| c.column_name.eq_ignore_ascii_case("user_id"))
        .expect("缺 user_id 列");
    assert!(user_id.is_pk, "user_id 应判为主键");
    assert!(user_id.is_auto_increment, "user_id 应判为自增(INFO2=1)");
    assert!(!user_id.is_nullable, "user_id 不可空");
    assert_eq!(user_id.ordinal_position, 1);

    let user_name = cols
        .iter()
        .find(|c| c.column_name.eq_ignore_ascii_case("user_name"))
        .expect("缺 user_name 列");
    assert!(!user_name.is_pk);
    assert!(!user_name.is_auto_increment);
    assert!(!user_name.is_nullable);
    assert_eq!(user_name.column_comment.as_deref(), Some("用户名"));
    // 达梦无现成 column_type(中立类型留空,拼装归 system 侧 codegen)——
    // 等价断言落在原始三元组上:DATA_TYPE 含类型名、DATA_LENGTH=50
    assert!(
        user_name.data_type.to_lowercase().contains("varchar"),
        "data_type 应含类型名,实际: {}",
        user_name.data_type
    );
    assert_eq!(
        user_name.data_length,
        Some(50),
        "DATA_LENGTH 应为 50,实际: {:?}",
        user_name.data_length
    );

    let remark = cols
        .iter()
        .find(|c| c.column_name.eq_ignore_ascii_case("remark"))
        .expect("缺 remark 列");
    assert!(remark.is_nullable, "remark 可空");
}

/// 达梦实机断言的 SQLite 移植:同一探针形状钉住往返行为。注释类断言
/// (表/列注释)是达梦/MySQL 才有的概念,在 SQLite 上降级为 None。
/// (与 meta_sqlite_test.rs 的 meta_probe 探针互为对照:那边盖过滤/
/// 排除/内部表,本用例盖逐字段派生语义。)
#[tokio::test]
async fn lists_tables_and_columns_on_sqlite() {
    let mut opt = ConnectOptions::new("sqlite::memory:");
    // 内存库必须单连接,否则不同连接看到不同的库
    opt.max_connections(1).min_connections(1);
    let db = Database::connect(opt).await.unwrap();
    for ddl in [
        format!(
            "CREATE TABLE {TABLE} (\
             user_id INTEGER PRIMARY KEY AUTOINCREMENT, \
             user_name VARCHAR(50) NOT NULL, \
             remark VARCHAR(200) NULL)"
        ),
        "CREATE TABLE gen_hidden (id INTEGER PRIMARY KEY)".to_string(),
    ] {
        db.execute_raw(Statement::from_string(DbBackend::Sqlite, ddl))
            .await
            .unwrap();
    }

    let ds = app_data::DataSource::from_connection(db);
    let ctx = ds.system_context();
    let reader = MetadataReader::new(&ctx);

    // list_tables:表名过滤 + 总数
    let (tables, total) = reader
        .list_tables("main", Some("meta_probe"), None, &[], 0, 10)
        .await
        .unwrap();
    assert!(total >= 1, "list_tables 未找到探针表");
    assert!(
        tables
            .iter()
            .any(|t| t.table_name.eq_ignore_ascii_case(TABLE)),
        "分页结果中应有探针表"
    );
    // gen_ 前缀被 TABLE_IGNORE_PREFIXES 过滤(语义随 build_table_filter 迁移)
    assert!(
        !tables.iter().any(|t| t.table_name.starts_with("gen_")),
        "gen_ 前缀表不得出现在可选清单"
    );

    // 已导入排除
    let (excluded, _) = reader
        .list_tables("main", None, None, &[TABLE.to_string()], 0, 10)
        .await
        .unwrap();
    assert!(
        !excluded
            .iter()
            .any(|t| t.table_name.eq_ignore_ascii_case(TABLE)),
        "imported_tables 应排除探针表"
    );

    // get_table_comment:SQLite 无表注释 → None(委托层对它缺省空串,
    // 呈现层行为不变)
    assert!(
        reader
            .get_table_comment("main", TABLE)
            .await
            .unwrap()
            .is_none(),
        "无注释应为 None(委托层呈现空串)"
    );

    // list_columns:逐字段派生语义(与达梦断言同构)
    let cols = reader.list_columns("main", TABLE).await.unwrap();
    assert_eq!(cols.len(), 3);

    let user_id = cols
        .iter()
        .find(|c| c.column_name.eq_ignore_ascii_case("user_id"))
        .expect("缺 user_id 列");
    assert!(user_id.is_pk, "user_id 应判为主键");
    assert!(
        user_id.is_auto_increment,
        "user_id 应判为自增(AUTOINCREMENT)"
    );
    assert!(!user_id.is_nullable, "user_id 不可空");
    assert_eq!(user_id.ordinal_position, 1);

    let user_name = cols
        .iter()
        .find(|c| c.column_name.eq_ignore_ascii_case("user_name"))
        .expect("缺 user_name 列");
    assert!(!user_name.is_pk);
    assert!(!user_name.is_auto_increment);
    assert!(!user_name.is_nullable);
    // SQLite 有现成 column_type(PRAGMA table_info 原文)
    assert!(
        user_name.column_type.to_lowercase().contains("varchar"),
        "column_type 应含类型名,实际: {}",
        user_name.column_type
    );
    assert!(
        user_name.column_type.contains("50"),
        "column_type 应含长度,实际: {}",
        user_name.column_type
    );

    let remark = cols
        .iter()
        .find(|c| c.column_name.eq_ignore_ascii_case("remark"))
        .expect("缺 remark 列");
    assert!(remark.is_nullable, "remark 可空");
}
