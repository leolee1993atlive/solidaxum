//! 冒烟测试: 用 config crate 加载真实 config.toml,验证 [oss] 段人类可读格式解析。
#![cfg(feature = "test-util")]

use std::path::PathBuf;

use config::{Config, File};

#[test]
fn config_toml_oss_section_parses_human_readable_size() {
    // config.toml 位于 workspace 根(admin crate 上两级: crates/admin/../../config.toml)。
    // 本地配置不入库,干净的克隆里只有示例文件,回落到它。
    let root: PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect();
    let cfg_path = if root.join("config.toml").exists() {
        root.join("config.toml")
    } else {
        root.join("config.example.toml")
    };
    let cfg = Config::builder()
        .add_source(File::with_name(
            cfg_path.to_str().expect("路径转 str 失败"),
        ))
        .build()
        .expect("读取 config.toml / config.example.toml 失败");
    let app: app_core::config::AppConfig =
        cfg.try_deserialize().expect("解析配置失败");
    assert_eq!(
        app.oss.upload_body_limit,
        10 * 1024 * 1024,
        "\"10MB\" 应解析为 10485760 字节"
    );
}
