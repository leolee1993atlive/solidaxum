//! 需要真实达梦。`cargo test -p dm --test row_test -- --ignored`

#[test]
#[ignore = "需要真实达梦"]
fn reads_column_names_and_values() {
    let conn = dm::connect_raw(&std::env::var("DM_ODBC").unwrap()).unwrap();
    let cursor = conn
        .execute("SELECT 1 AS num, '达梦' AS txt", (), None)
        .unwrap()
        .expect("应有结果集");
    let rows = dm::row::cursor_to_rows(cursor).unwrap();

    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    let names: Vec<&str> = r.keys().map(String::as_str).collect();
    assert!(r.get("num").is_some(), "列名应为 num,实际: {names:?}");
    assert!(r.get("txt").is_some(), "列名应为 txt,实际: {names:?}");
    // 列序必须与 SELECT 一致,不能被列名排序打乱
    assert_eq!(names, vec!["num", "txt"], "列序应与 SELECT 一致");
}
