use std::sync::OnceLock;

use odbc_api::Environment;

/// ODBC Environment 全进程单例。
///
/// `Connection<'a>` 借用 `Environment`,而 `spawn_blocking` 要求 `'static`,
/// 所以 Environment 必须活到进程结束。已验证 `Environment: Send + Sync`。
static ENV: OnceLock<Environment> = OnceLock::new();

pub fn environment() -> &'static Environment {
    ENV.get_or_init(|| {
        Environment::new().expect("初始化 ODBC Environment 失败")
    })
}
