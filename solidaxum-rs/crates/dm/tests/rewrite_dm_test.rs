//! 需要真实达梦。`cargo test -p dm --test rewrite_dm_test -- --ignored`

use odbc_api::{Cursor, IntoParameter};

#[test]
#[ignore = "需要真实达梦"]
fn merge_actually_upserts() {
    let conn = dm::connect_raw(&std::env::var("DM_ODBC").unwrap()).unwrap();
    let _ = conn.execute("DROP TABLE dm_merge_probe", (), None);
    conn.execute(
        "CREATE TABLE dm_merge_probe (id BIGINT PRIMARY KEY, payload VARCHAR(50))",
        (),
        None,
    )
    .unwrap();

    let sql = dm::rewrite::rewrite(
        "INSERT INTO dm_merge_probe (id, payload) VALUES (?, ?) \
         ON DUPLICATE KEY UPDATE payload = VALUES(payload)",
    )
    .unwrap()
    .into_owned();

    // MERGE 的 USING 子句里每列一个占位符,参数需按列顺序重复传入
    conn.execute(&sql, (&1i64, &"first".into_parameter()), None)
        .unwrap();
    conn.execute(&sql, (&1i64, &"second".into_parameter()), None)
        .unwrap();

    let mut cur = conn
        .execute("SELECT payload FROM dm_merge_probe WHERE id = 1", (), None)
        .unwrap()
        .unwrap();
    let mut row = cur.next_row().unwrap().unwrap();
    let mut buf = Vec::new();
    row.get_text(1, &mut buf).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&buf).trim(),
        "second",
        "第二次应更新而非报错"
    );
}
