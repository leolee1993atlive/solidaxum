//! 端到端:经 SeaORM 走达梦。`cargo test -p dm --test e2e_test -- --ignored`
//!
//! 各测试用各自独立的表名:cargo test 默认多线程并行,共享表会互相干扰。

use sea_orm::{
    ConnectionTrait, Database, DbBackend, Statement, TransactionTrait,
};

fn dsn() -> String {
    format!("dm:{}", std::env::var("DM_ODBC").expect("未设置 DM_ODBC"))
}

fn sql(s: &str) -> Statement {
    Statement::from_string(DbBackend::Dm, s)
}

async fn reset_table(db: &sea_orm::DatabaseConnection, table: &str) {
    db.execute_raw(sql(&format!("DROP TABLE {table}")))
        .await
        .ok();
    db.execute_raw(sql(&format!(
        "CREATE TABLE {table} (id BIGINT PRIMARY KEY)"
    )))
    .await
    .unwrap();
}

async fn exists(
    db: &sea_orm::DatabaseConnection,
    table: &str,
    id: i64,
) -> bool {
    let rows = db
        .query_all_raw(sql(&format!("SELECT id FROM {table} WHERE id = {id}")))
        .await
        .unwrap();
    !rows.is_empty()
}

#[tokio::test]
#[ignore = "需要真实达梦"]
async fn backend_is_dm() {
    let db = Database::connect(dsn()).await.unwrap();
    assert_eq!(db.get_database_backend(), DbBackend::Dm);
}

#[tokio::test]
#[ignore = "需要真实达梦"]
async fn rolls_back_transaction() {
    const T: &str = "dm_e2e_rb";
    let db = Database::connect(dsn()).await.unwrap();
    reset_table(&db, T).await;

    let txn = db.begin().await.unwrap();
    txn.execute_raw(sql(&format!("INSERT INTO {T} (id) VALUES (1)")))
        .await
        .unwrap();
    txn.rollback().await.unwrap();

    assert!(!exists(&db, T, 1).await, "回滚后不应有数据");
}

/// 问题 A 的回归测试:事务进行中,另一路非事务写必须独立提交,
/// 不得被并进该事务、随其回滚。proxy 方案正是在这里失败的。
#[tokio::test]
#[ignore = "需要真实达梦"]
async fn concurrent_non_txn_write_survives_txn_rollback() {
    const T: &str = "dm_e2e_cnt";
    let db = Database::connect(dsn()).await.unwrap();
    reset_table(&db, T).await;

    let txn = db.begin().await.unwrap();
    txn.execute_raw(sql(&format!("INSERT INTO {T} (id) VALUES (1)")))
        .await
        .unwrap();

    // 事务尚未结束,另一路非事务写 —— 必须落在另一条连接上
    db.execute_raw(sql(&format!("INSERT INTO {T} (id) VALUES (2)")))
        .await
        .unwrap();

    txn.rollback().await.unwrap();

    assert!(!exists(&db, T, 1).await, "事务内的写未随回滚消失");
    assert!(exists(&db, T, 2).await, "非事务写被并进了事务 —— 隔离失败");
}

/// 事务未提交的数据不得被其他连接读到。
#[tokio::test]
#[ignore = "需要真实达梦"]
async fn concurrent_read_does_not_see_uncommitted() {
    const T: &str = "dm_e2e_cr";
    let db = Database::connect(dsn()).await.unwrap();
    reset_table(&db, T).await;

    let txn = db.begin().await.unwrap();
    txn.execute_raw(sql(&format!("INSERT INTO {T} (id) VALUES (3)")))
        .await
        .unwrap();

    assert!(!exists(&db, T, 3).await, "读到了未提交数据 —— 脏读");

    txn.commit().await.unwrap();
    assert!(exists(&db, T, 3).await, "提交后应可见");
}

/// 事务连接 autocommit 复位回归:事务提交后,紧接的非事务写必须立即持久,
/// 不得因事务连接被错误归还池、带着 `autocommit=off` 状态而悬挂未提交。
/// driver 用"事务连接用完即弃(retire)"杜绝此类泄漏 —— 此测试守住该不变量。
#[tokio::test]
#[ignore = "需要真实达梦"]
async fn write_after_committed_txn_persists() {
    const T: &str = "dm_e2e_wa";
    let db = Database::connect(dsn()).await.unwrap();
    reset_table(&db, T).await;

    let txn = db.begin().await.unwrap();
    txn.execute_raw(sql(&format!("INSERT INTO {T} (id) VALUES (10)")))
        .await
        .unwrap();
    txn.commit().await.unwrap();

    // 非事务写:必须落在 autocommit=on 的健康连接上,立即持久。
    // 若事务连接被错误归还池且 autocommit 未复位,这条写会悬挂未提交,
    // 之后在另一条连接上的 exists() 读不到(达梦默认读已提交)。
    db.execute_raw(sql(&format!("INSERT INTO {T} (id) VALUES (11)")))
        .await
        .unwrap();
    assert!(
        exists(&db, T, 11).await,
        "非事务写未持久 —— 疑似事务连接 autocommit 状态泄漏"
    );
    assert!(exists(&db, T, 10).await, "已提交的事务数据应可见");
}

/// 参数绑定端到端。**上面所有 e2e 都用字面量 SQL,从不绑定参数** ——
/// 值映射对 chrono / JSON / 无符号整数的缺口正是这样漏过去的(实体里
/// create_time、update_time 这类列几乎每张表都有)。此测试守住绑定路径。
///
/// 若失败信息是 `No column for ColIdx`,那指向的是达梦返回的列名大小写,
/// 不是绑定本身 —— 两者要分开判。
#[tokio::test]
#[ignore = "需要真实达梦"]
async fn binds_datetime_and_text_parameters() {
    const T: &str = "dm_e2e_bind";
    let db = Database::connect(dsn()).await.unwrap();
    db.execute_raw(sql(&format!("DROP TABLE {T}"))).await.ok();
    db.execute_raw(sql(&format!(
        "CREATE TABLE {T} (id BIGINT PRIMARY KEY, created datetime, payload VARCHAR(200))"
    )))
    .await
    .unwrap();

    let ts = chrono::NaiveDate::from_ymd_opt(2026, 8, 13)
        .unwrap()
        .and_hms_milli_opt(9, 28, 24, 212)
        .unwrap()
        .and_utc();

    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Dm,
        format!("INSERT INTO {T} (id, created, payload) VALUES (?, ?, ?)"),
        [1i64.into(), ts.into(), "达梦".into()],
    ))
    .await
    .expect("绑定参数写入失败 —— 值映射有缺口");

    let row = db
        .query_one_raw(sql(&format!(
            "SELECT created, payload FROM {T} WHERE id = 1"
        )))
        .await
        .unwrap()
        .expect("应有一行");
    assert_eq!(
        row.try_get::<chrono::DateTime<chrono::Utc>>("", "created")
            .expect("读回时间失败"),
        ts,
        "时间往返必须一致 —— 写入按 UTC 墙钟,读回也按 UTC 解释"
    );
    assert_eq!(
        row.try_get::<String>("", "payload").expect("读回文本失败"),
        "达梦"
    );
}

/// 事务里一条语句报错(主键冲突)之后,同一事务必须还能继续执行并正常提交。
///
/// 语句级错误不该报废连接:若报废,后续语句拿不到连接,而 `commit()` 会误报
/// 成功 —— 数据其实已随断连被隐式回滚,调用方毫不知情。达梦与 Oracle 同源,
/// 语句级错误只回滚该语句,事务本身继续有效。
#[tokio::test]
#[ignore = "需要真实达梦"]
async fn statement_error_does_not_kill_the_transaction() {
    const T: &str = "dm_e2e_se";
    let db = Database::connect(dsn()).await.unwrap();
    reset_table(&db, T).await;

    let txn = db.begin().await.unwrap();
    txn.execute_raw(sql(&format!("INSERT INTO {T} (id) VALUES (20)")))
        .await
        .unwrap();

    let dup = txn
        .execute_raw(sql(&format!("INSERT INTO {T} (id) VALUES (20)")))
        .await;
    assert!(dup.is_err(), "重复主键应报错");

    txn.execute_raw(sql(&format!("INSERT INTO {T} (id) VALUES (21)")))
        .await
        .expect("语句级错误后事务应仍然可用 —— 连接被误弃了");
    txn.commit().await.expect("提交应成功");

    assert!(exists(&db, T, 20).await, "首条写入应已提交");
    assert!(exists(&db, T, 21).await, "错误之后的写入应已提交");
}
