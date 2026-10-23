//! 需要真实达梦。设 DM_ODBC 环境变量后以 `cargo test -p dm -- --ignored` 运行。

#[test]
#[ignore = "需要真实达梦,设 DM_ODBC 后以 --ignored 运行"]
fn environment_is_reusable_across_calls() {
    let a = dm::env::environment() as *const _;
    let b = dm::env::environment() as *const _;
    assert_eq!(a, b, "Environment 必须是单例");
}

#[test]
#[ignore = "需要真实达梦,设 DM_ODBC 后以 --ignored 运行"]
fn connects_to_dm() {
    let conn_str = std::env::var("DM_ODBC").expect("未设置 DM_ODBC");
    let conn = dm::connect_raw(&conn_str).expect("连接失败");
    assert!(!conn.is_dead().unwrap(), "连接应当是活的");
}
