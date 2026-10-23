//! 免盖章种子原语不得泄进默认 build(设计 9 节第 4 条门控):
//! `insert_unstamped` 只在 `test-util` feature 下存在,默认面调用必须 E0599。
//! 否则"为了方便"去掉 cfg 时无任何测试变红,生产代码就能绕过审计盖章。
//!
//! 实参用泛型占位:默认 build 里 `app_data::testing` 同样不存在,连造一个
//! 具体 ActiveModel 都不该需要 —— 方法本身缺席,实参类型无关紧要。
use app_data::DataSource;

#[allow(dead_code)]
async fn seed<A>(ds: &DataSource, am: A) {
    let ctx = ds.system_context();
    let _ = ctx.insert_unstamped(am).await;
}

fn main() {}
