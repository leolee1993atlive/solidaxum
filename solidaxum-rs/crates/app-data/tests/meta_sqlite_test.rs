//! MetadataReader 的 SQLite 往返集成测试(设计 6.3 meta 模块,任务 4)。
//!
//! MySQL/达梦两方言的 SQL 文本由 `dialect` 单元测试逐字锚定(CI 无对应
//! 实例);本文件锚定**执行语义**:建表 → list_tables / list_columns /
//! get_table_comment 走通且派生字段(pk / 自增 / 可空 / 列序 / 过滤 / 排除)
//! 正确。探针表刻意镜像同套件 `meta_dm_test.rs` 的达梦探针
//! (identity 主键 + NOT NULL + 可空列),使两套断言可互为对照。

use app_data::meta::MetadataReader;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DbBackend, Statement,
};

/// 单连接内存库 + 探针表。`gen_zzz` 用于验证忽略前缀过滤。
/// (AUTOINCREMENT 会自动造出 sqlite_sequence,顺带验证内部表排除。)
async fn sqlite_with_probe() -> app_data::DataSource {
    let mut opt = ConnectOptions::new("sqlite::memory:");
    // 内存库必须单连接,否则不同连接看到不同的库
    opt.max_connections(1).min_connections(1);
    let db = Database::connect(opt).await.expect("sqlite 内存库连接");
    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE meta_probe (\
         id INTEGER PRIMARY KEY AUTOINCREMENT, \
         user_name VARCHAR(50) NOT NULL, \
         remark VARCHAR(200) NULL)",
    ))
    .await
    .expect("建探针表");
    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE gen_zzz (id INTEGER PRIMARY KEY)",
    ))
    .await
    .expect("建 gen_ 前缀表");
    app_data::DataSource::from_connection(db)
}

#[tokio::test]
async fn list_tables_round_trip_on_sqlite() {
    let ds = sqlite_with_probe().await;
    let ctx = ds.system_context();
    let reader = MetadataReader::new(&ctx);

    let (tables, total) = reader
        .list_tables("main", None, None, &[], 0, 10)
        .await
        .unwrap();
    assert!(total >= 1, "至少应看到探针表");
    let probe = tables
        .iter()
        .find(|t| t.table_name == "meta_probe")
        .expect("清单中应有 meta_probe");
    // SQLite 无表注释/建表时间:中立类型给空串与 None
    assert_eq!(probe.table_comment, "");
    assert!(probe.create_time.is_none());
    assert!(probe.update_time.is_none());
    // gen_ 前缀被忽略,sqlite_sequence(内部表)也不得出现
    assert!(
        !tables.iter().any(|t| t.table_name.starts_with("gen_")),
        "gen_ 前缀表应被 TABLE_IGNORE_PREFIXES 过滤"
    );
    assert!(
        !tables.iter().any(|t| t.table_name.starts_with("sqlite_")),
        "sqlite 内部表不得出现"
    );

    // 表名模糊过滤
    let (by_name, _) = reader
        .list_tables("main", Some("ETA_PRO"), None, &[], 0, 10)
        .await
        .unwrap();
    assert!(
        by_name.iter().any(|t| t.table_name == "meta_probe"),
        "大小写不敏感的表名过滤未命中"
    );
    let (no_hit, _) = reader
        .list_tables("main", Some("no_such_table"), None, &[], 0, 10)
        .await
        .unwrap();
    assert!(no_hit.is_empty(), "未命中过滤应返回空清单");

    // 已导入排除(导入了就不再出现在可选清单)
    let (excluded, _) = reader
        .list_tables("main", None, None, &["meta_probe".to_string()], 0, 10)
        .await
        .unwrap();
    assert!(
        !excluded.iter().any(|t| t.table_name == "meta_probe"),
        "imported_tables 应排除探针表"
    );

    // 分页:limit=1 时第一页只有 1 张表
    let (page1, _) = reader
        .list_tables("main", None, None, &[], 0, 1)
        .await
        .unwrap();
    assert_eq!(page1.len(), 1, "limit 生效");
}

#[tokio::test]
async fn list_columns_round_trip_on_sqlite() {
    let ds = sqlite_with_probe().await;
    let ctx = ds.system_context();
    let reader = MetadataReader::new(&ctx);

    let cols = reader.list_columns("main", "meta_probe").await.unwrap();
    assert_eq!(cols.len(), 3);

    let id = cols
        .iter()
        .find(|c| c.column_name == "id")
        .expect("缺 id 列");
    assert!(id.is_pk, "id 应判为主键");
    assert!(id.is_auto_increment, "id 应判为自增(DDL 带 AUTOINCREMENT)");
    assert!(!id.is_nullable, "id 不可空");
    assert_eq!(id.ordinal_position, 1);

    let user_name = cols
        .iter()
        .find(|c| c.column_name == "user_name")
        .expect("缺 user_name 列");
    assert!(!user_name.is_pk);
    assert!(!user_name.is_auto_increment);
    assert!(!user_name.is_nullable);
    assert!(user_name.column_comment.is_none(), "SQLite 无列注释");
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
        .find(|c| c.column_name == "remark")
        .expect("缺 remark 列");
    assert!(remark.is_nullable, "remark 可空");
    assert_eq!(remark.ordinal_position, 3, "列序 = 声明序");
}

#[tokio::test]
async fn get_table_comment_is_none_on_sqlite() {
    let ds = sqlite_with_probe().await;
    let ctx = ds.system_context();
    let reader = MetadataReader::new(&ctx);
    assert!(
        reader
            .get_table_comment("main", "meta_probe")
            .await
            .unwrap()
            .is_none(),
        "SQLite 无表注释,应返回 None(由调用方决定缺省呈现)"
    );
}
