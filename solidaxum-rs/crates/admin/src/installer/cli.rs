//! 手写 argv 解析(不引入 clap)。
//!
//! 只需识别三个 flag:`--write-config <path>` / `--check-config <path>` /
//! `--answers <path>`。互斥校验:前两者不能同时出现;`--write-config` 必须配
//! `--answers`。无 flag → [`Cli::Serve`] 落正常启动路径。

use std::path::PathBuf;

/// 解析后的命令行意图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cli {
    /// 正常启动服务(无安装子命令 flag)。
    Serve,
    /// `--write-config <config> --answers <answers>`:渲染配置文件。
    WriteConfig { config: PathBuf, answers: PathBuf },
    /// `--check-config <config>`:检测 DB/Redis 连通性。
    CheckConfig { config: PathBuf },
}

/// 解析 argv(`std::env::args()`)。第一个元素(程序名)会被跳过。
///
/// # 错误
/// - 未知 flag
/// - `--write-config` 未配 `--answers`(或反之)
/// - `--write-config` 与 `--check-config` 同时出现
/// - flag 后缺少路径参数
pub fn parse<I, S>(args: I) -> anyhow::Result<Cli>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter();
    // 跳过程序名(argv[0])
    args.next();

    let mut write_config: Option<PathBuf> = None;
    let mut check_config: Option<PathBuf> = None;
    let mut answers: Option<PathBuf> = None;

    let mut iter = args.peekable();
    while let Some(raw) = iter.next() {
        let arg = raw.into();
        match arg.as_str() {
            "--write-config" => {
                if write_config.is_some() {
                    anyhow::bail!("--write-config 重复指定");
                }
                let v = take_path(&mut iter, "--write-config")?;
                write_config = Some(v);
            }
            "--check-config" => {
                if check_config.is_some() {
                    anyhow::bail!("--check-config 重复指定");
                }
                let v = take_path(&mut iter, "--check-config")?;
                check_config = Some(v);
            }
            "--answers" => {
                if answers.is_some() {
                    anyhow::bail!("--answers 重复指定");
                }
                let v = take_path(&mut iter, "--answers")?;
                answers = Some(v);
            }
            other => {
                anyhow::bail!(
                    "未知参数: {other}(支持: --write-config / --check-config / --answers)"
                );
            }
        }
    }

    match (write_config, check_config, answers) {
        (None, None, None) => Ok(Cli::Serve),
        (Some(config), None, Some(answers)) => {
            Ok(Cli::WriteConfig { config, answers })
        }
        (Some(_), None, None) => {
            anyhow::bail!("--write-config 必须配合 --answers <path> 使用")
        }
        (None, Some(config), None) => Ok(Cli::CheckConfig { config }),
        (None, Some(_), Some(_)) => {
            anyhow::bail!(
                "--check-config 不接受 --answers(仅 --write-config 需要)"
            )
        }
        (Some(_), Some(_), _) => {
            anyhow::bail!("--write-config 与 --check-config 互斥,不可同时使用")
        }
        // --answers 单独出现(无 --write-config)
        (None, None, Some(_)) => {
            anyhow::bail!("--answers 仅在配合 --write-config 时使用")
        }
    }
}

/// 从迭代器取下一个值作为路径,缺失则报错。
fn take_path<I, S>(
    iter: &mut std::iter::Peekable<I>,
    flag: &str,
) -> anyhow::Result<PathBuf>
where
    I: Iterator<Item = S>,
    S: Into<String>,
{
    match iter.next() {
        Some(v) => Ok(PathBuf::from(v.into())),
        None => anyhow::bail!("{flag} 需要一个路径参数"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        std::iter::once("solidaxum".to_string())
            .chain(parts.iter().map(|s| s.to_string()))
            .collect()
    }

    #[test]
    fn no_flags_serves() {
        assert_eq!(parse(args(&[])).unwrap(), Cli::Serve);
    }

    #[test]
    fn write_config_with_answers() {
        let cli =
            parse(args(&["--write-config", "c.toml", "--answers", "a.toml"]))
                .unwrap();
        assert_eq!(
            cli,
            Cli::WriteConfig {
                config: PathBuf::from("c.toml"),
                answers: PathBuf::from("a.toml"),
            }
        );
    }

    #[test]
    fn write_config_without_answers_errors() {
        assert!(parse(args(&["--write-config", "c.toml"])).is_err());
    }

    #[test]
    fn answers_without_write_config_errors() {
        assert!(parse(args(&["--answers", "a.toml"])).is_err());
    }

    #[test]
    fn check_config_alone() {
        let cli = parse(args(&["--check-config", "c.toml"])).unwrap();
        assert_eq!(
            cli,
            Cli::CheckConfig {
                config: PathBuf::from("c.toml")
            }
        );
    }

    #[test]
    fn check_config_with_answers_errors() {
        assert!(
            parse(args(&["--check-config", "c.toml", "--answers", "a.toml"]))
                .is_err()
        );
    }

    #[test]
    fn write_and_check_mutually_exclusive() {
        assert!(
            parse(args(&[
                "--write-config",
                "c.toml",
                "--check-config",
                "c2.toml",
                "--answers",
                "a.toml",
            ]))
            .is_err()
        );
    }

    #[test]
    fn unknown_flag_errors() {
        assert!(parse(args(&["--foo"])).is_err());
    }

    #[test]
    fn missing_path_errors() {
        assert!(parse(args(&["--write-config"])).is_err());
        assert!(parse(args(&["--check-config"])).is_err());
        assert!(parse(args(&["--answers"])).is_err());
    }

    #[test]
    fn flag_order_independent() {
        // answers 在前,write-config 在后
        let cli =
            parse(args(&["--answers", "a.toml", "--write-config", "c.toml"]))
                .unwrap();
        assert!(matches!(cli, Cli::WriteConfig { .. }));
    }
}
