use std::sync::{LazyLock, Mutex};

use snowflake::SnowflakeIdGenerator;

static ID_GENERATOR: LazyLock<Mutex<SnowflakeIdGenerator>> =
    LazyLock::new(|| Mutex::new(SnowflakeIdGenerator::new(1, 1)));

pub fn next_id() -> i64 {
    ID_GENERATOR
        .lock()
        .expect("snowflake 生成器锁中毒")
        .generate()
}
