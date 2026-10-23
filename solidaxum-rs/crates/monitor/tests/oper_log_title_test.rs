use axum::http::Method;
use monitor::middleware::oper_log::{TitleRules, derive_title};

const HDATA_RULES: TitleRules =
    &[("/datasource", "数据源"), ("/transtask", "传输任务")];

#[test]
fn 内置规则优先命中() {
    assert_eq!(
        derive_title("/system/user/list", &Method::GET, &[]),
        "用户管理"
    );
}

#[test]
fn 注册规则补充未覆盖路径() {
    assert_eq!(
        derive_title("/datasource/page", &Method::GET, HDATA_RULES),
        "数据源"
    );
}

#[test]
fn 全不命中回落其他操作() {
    assert_eq!(derive_title("/nowhere", &Method::GET, &[]), "其他操作");
}

#[test]
fn 空注册表等价于原行为() {
    assert_eq!(
        derive_title("/datasource/page", &Method::GET, &[]),
        "其他操作"
    );
}
