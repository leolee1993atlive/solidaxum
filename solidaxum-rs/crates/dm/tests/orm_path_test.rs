//! 实体级端到端:经 SeaORM 的 ORM 路径(ActiveModel / Entity::find)走达梦。
//!
//! 其余 e2e 都是手写字面量 SQL,**不经过实体、不绑定参数**,因此漏掉了三类问题:
//! 入参映射的类型覆盖、中间表示的列序、达梦对 SeaORM 所生成标识符的大小写处理。
//! 这三类恰好各出过一次事故(见 crates/dm/src/value.rs、row.rs 的模块注释),
//! 故单设本文件守住 ORM 路径。
//!
//! `DM_ODBC="DSN=DM8;UID=…;PWD=…" cargo test -p dm --test orm_path_test -- --ignored`

use sea_orm::{
    ActiveValue::Set, ConnectionTrait, Database, DbBackend, EntityTrait,
    QuerySelect, Statement,
};

/// 探针实体。字段名刻意让**列序与列名字典序相反**(`probe_id` < `zzz_name`
/// 成立,但下面按 (probe_id, zzz_name) 取值时,若中间表示按列名排序则会
/// 换成 (zzz_name, probe_id),把中文当 i64 解析)—— 见 `dm::row::DmRow` 的说明。
mod probe {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "dm_orm_e2e")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub probe_id: i64,
        pub zzz_name: String,
        pub created_at: Option<DateTimeUtc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

fn dsn() -> String {
    format!("dm:{}", std::env::var("DM_ODBC").expect("未设置 DM_ODBC"))
}

fn sql(s: &str) -> Statement {
    Statement::from_string(DbBackend::Dm, s)
}

#[tokio::test]
#[ignore = "需要真实达梦"]
async fn entity_roundtrip_through_orm_path() {
    let db = Database::connect(dsn()).await.unwrap();

    db.execute_raw(sql("DROP TABLE dm_orm_e2e")).await.ok();
    db.execute_raw(sql(
        "CREATE TABLE dm_orm_e2e (probe_id BIGINT PRIMARY KEY, \
         zzz_name VARCHAR(50), created_at datetime)",
    ))
    .await
    .unwrap();

    // 毫秒非零:达梦 datetime 默认精度 6,时间必须往返到亚秒仍相等
    let ts = chrono::NaiveDate::from_ymd_opt(2026, 8, 14)
        .unwrap()
        .and_hms_milli_opt(11, 22, 33, 444)
        .unwrap()
        .and_utc();
    // 19 位雪花 ID:文本中间表示不得损失精度
    let id: i64 = 1957203961908830209;

    // —— 写:ActiveModel 走 to_params,含 chrono 绑定 ——
    probe::Entity::insert(probe::ActiveModel {
        probe_id: Set(id),
        zzz_name: Set("达梦实体".to_owned()),
        created_at: Set(Some(ts)),
    })
    .exec(&db)
    .await
    .expect("ActiveModel 插入失败 —— 多半是入参映射缺类型");

    // —— 读:Entity::find 走 SeaORM 生成的反引号标识符 SELECT ——
    let rows = probe::Entity::find()
        .all(&db)
        .await
        .expect("Entity::find 失败 —— 多半是列名大小写对不上");
    assert_eq!(rows.len(), 1, "应恰好 1 行");
    let m = &rows[0];
    assert_eq!(m.probe_id, id, "19 位雪花 ID 精度丢失");
    assert_eq!(m.zzz_name, "达梦实体", "中文往返出错");
    assert_eq!(m.created_at, Some(ts), "时间往返不一致(含毫秒)");

    // —— 多列 into_tuple:按 SELECT 列序取值,不是按列名字典序 ——
    let tuples: Vec<(i64, String)> = probe::Entity::find()
        .select_only()
        .column(probe::Column::ProbeId)
        .column(probe::Column::ZzzName)
        .into_tuple()
        .all(&db)
        .await
        .expect("多列 into_tuple 失败 —— 多半是中间表示丢了列序");
    assert_eq!(tuples, vec![(id, "达梦实体".to_owned())], "列序错位");

    // —— 达梦原样保留反引号标识符的大小写,不折叠为大写 ——
    // 这是按列名 try_get 能命中的前提;若哪天换实例后此断言失败,
    // 先查建库参数 CASE_SENSITIVE。
    let raw = db
        .query_all_raw(sql("SELECT `probe_id`, `zzz_name` FROM `dm_orm_e2e`"))
        .await
        .unwrap();
    let names = raw.first().expect("应有一行").column_names();
    assert_eq!(
        names,
        vec!["probe_id".to_owned(), "zzz_name".to_owned()],
        "反引号标识符的大小写或顺序被改变了"
    );

    db.execute_raw(sql("DROP TABLE dm_orm_e2e")).await.ok();
}
