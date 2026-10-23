//! 把漏斗强制性变成回归测试(设计 9.2):compile_fail 组必须编译失败,
//! pass 组必须编译成功 —— 后者证明受限 re-export 足够定义真实实体。
//!
//! # 门控:只在默认 feature 下运行(test-util 下整体跳过)
//!
//! 文件顶部的 `#![cfg(not(feature = "test-util"))]` 让本测试目标在
//! `--features test-util` 下编译成**空测试二进制**(0 个用例,自动绿),
//! 因此 `.stderr` 基线与运行口径只剩一种:**默认 feature**。
//!
//! 为什么必须挡住 test-util:`src/testing.rs`(`#[cfg(any(test,
//! feature = "test-util"))]`)一旦进入库的编译单元,其中三个实体的
//! `DataPolicy` impl 会改变 rustc 对 `no_data_policy.rs` 的 E0277 帮助文本
//! (把 testing 模块的 impl 列进候选),套件必然 mismatch;而集成测试
//! 编译库时 `cfg(test)` 不成立,默认 feature 下 testing.rs 根本不在库里,
//! 基线也就**免疫于 src/testing.rs 的 impl 增删**。
//!
//! 规范运行命令:`cargo test -p app_data`(默认 feature,含本套件);
//! `--features test-util` 下无需收窄范围 —— 本目标自空转,直接跑
//! `cargo test -p app_data --features test-util` 也不会碰 trybuild。
#![cfg(not(feature = "test-util"))]

#[test]
fn compile_fail_suite() {
    let t = trybuild::TestCases::new();
    t.pass("tests/compile_pass/*.rs");
    t.compile_fail("tests/compile_fail/*.rs");
}
