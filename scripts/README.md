# 构建与部署脚本

本目录包含 `@REPO_NAME@` 后端 / 前端的构建与部署脚本。

支持的平台：

- Linux 原生（使用 `cargo-zigbuild` 构建，目标 glibc 2.28+）
- Windows 原生
- Docker（Linux）

## 构建

| 命令 | 平台 | 产出 |
|---|---|---|
| `bash scripts/build/build-linux.sh --all` | Linux | `dist/<repo>-<ver>-linux-x64.zip` + `dist/<repo>-ui-<ver>-linux-x64.zip` |
| `pwsh scripts/build/build-windows.ps1 -All` | Windows | `dist\<repo>-<ver>-windows-x64.zip` + `dist\<repo>-ui-<ver>-windows-x64.zip` |
| `bash scripts/build/build-docker.sh` | Linux | `dist/<repo>-docker-<ver>.zip` |

其中 `<repo>` 优先从 GitHub 远程地址 `git remote get-url origin` 解析仓库名，无法解析时使用当前目录名。

构建脚本的公共参数：

- `--version` / `-Version`：指定版本号
- `--suffix` / `-Suffix`：在版本号后追加后缀
- `--force` / `-Force`：强制重新构建，覆盖已有产物

Docker 包内置 Redis，可直接开箱使用。

## 安装

每个 zip 解压后自带安装脚本：`install.sh`（Linux）或 `install.ps1`（Windows）。

安装特性：

- 就地安装到当前目录
- 首次安装专用，目录已存在会报错退出
- 非交互式
- 离线可用

Linux 安装使用 systemd 管理服务，Windows 安装使用 WinSW 管理服务。

## 安装后操作

安装完成后，使用者必须：

1. 复制 `config.toml.example` 为 `config.toml`
2. 填入数据库 / Redis / JWT secret 等配置
3. 启动服务：
   - Linux：`systemctl start <repo>`
   - Windows：`Start-Service <repo>`

## 卸载

`uninstall.sh` / `uninstall.ps1` 会停止并删除服务、移除二进制文件，但保留 `config.toml`、`data/`、`logs/` 等数据文件。
