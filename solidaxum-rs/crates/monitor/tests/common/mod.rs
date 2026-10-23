//! 集成测试脚手架:建表走实体衍生 DDL(`DataSource::create_table`),
//! 不用手写 DDL 与裸 `DatabaseConnection`,本文件不出现 sea-orm
//! (模板同 system/tests/common)。

use app_data::{DataSource, testing::sqlite_memory_empty};

/// 创建 SQLite 内存库:实体衍生 DDL 建 sys_oper_log + sys_login_info 两张表。
pub async fn setup_db() -> DataSource {
    let ds = sqlite_memory_empty().await;
    ds.create_table(monitor::operlog::repo::Entity)
        .await
        .expect("建 sys_oper_log 表");
    ds.create_table(monitor::logininfo::repo::Entity)
        .await
        .expect("建 sys_login_info 表");
    ds
}
