//! 交互式安装子命令模块。
//!
//! 提供两个隐藏子命令,供安装脚本(`install.sh` / Inno Setup `.iss`)驱动:
//!
//! - `--write-config <path> --answers <path>`:从内嵌模板渲染 `config.toml`,
//!   CSPRNG 生成 jwt.secret / security.aes_key;文件已存在时走外科手术式更新
//!   (永不覆盖密钥,保留注释与手改段)。
//! - `--check-config <path>`:用生产同款驱动真实连接 DB + Redis,探活。

pub mod answers;
pub mod checker;
pub mod cli;
pub mod dsn;
pub mod writer;
