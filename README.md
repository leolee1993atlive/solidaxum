# SolidAxum

[RuoYi-Vue-Plus](https://github.com/dromara/RuoYi-Vue-Plus) 的 **Rust + SvelteKit 重写**：
后端从 Spring Boot 换成 Axum + SeaORM，前端从 Vue 3 换成 SvelteKit 5。接口契约与原版
Vue 前端保持一致 —— 同一套调用路径、HTTP 方法和响应信封（`{code,msg,data}`，分页为
`{rows,total}`）。

## 特性

- **认证与权限**：Redis 会话 + 不透明 Token，RBAC，RuoYi 六级数据权限，声明式
  `require_permission` / `require_role`。
- **系统管理**：用户、角色、菜单、部门、岗位、字典、参数、通知公告、客户端、社交登录。
- **代码生成**：从数据库表反向生成 Rust 分层（model/repository/service/handler/route）
  与前端页面。
- **工作流**：warm-flow 引擎的 Rust 移植，含表达式求值、监听器与可视化流程设计器。
- **定时任务**：cron 调度、执行日志、分布式锁去重。
- **监控**：操作日志、登录日志、在线用户、缓存监控、Prometheus 指标。
- **对象存储**：S3 兼容后端与本地磁盘后端，多配置管理，字段级 AES 加密。
- **多数据库**：MySQL / PostgreSQL / SQLite，另含达梦（DM）方言适配。
- **可离线交付**：Linux / Windows 交互式安装包与 Docker 镜像。

## 技术栈

| 层 | 选型 |
|---|---|
| 后端 | Rust (edition 2024)、Axum 0.8、SeaORM 2、Tokio |
| 前端 | SvelteKit 5 / Svelte 5 (runes)、TailwindCSS 4、shadcn-svelte |
| 存储 | MySQL / PostgreSQL / SQLite / 达梦；Redis |
| 构建 | Cargo workspace、pnpm、Vite |

## 目录结构

| 目录 | 内容 |
|---|---|
| [`solidaxum-rs/`](solidaxum-rs/) | Rust/Axum 后端（8 个 crate 的 workspace） |
| [`solidaxum-ui/`](solidaxum-ui/) | SvelteKit 5 SPA 前端，含工作流设计器 |
| [`scripts/`](scripts/) | 仓库守卫脚本与打包构建（Linux / Windows 安装包、Docker） |
| [`docker/`](docker/) | Docker 镜像与 compose 模板 |
| [`bench/`](bench/) | k6 压测脚本与后端对比工具 |

各子项目的细节见 [后端 README](solidaxum-rs/README.md) 与
[前端 README](solidaxum-ui/README.md)。

## 快速开始

### 环境要求

- Rust 1.85+（edition 2024）
- Node.js 20+ 与 pnpm 9+
- MySQL 8.0+ / PostgreSQL / SQLite 任一，以及 Redis

### 后端

```bash
cd solidaxum-rs
cp config.example.toml config.toml     # 按需修改数据库与 Redis 连接
cargo run                              # http://localhost:3001
```

首次运行前先建库并导入种子数据：

```bash
mysql -u root -p solidaxum < solidaxum-rs/sql/solidaxum_init.sql
```

其他方言的初始化脚本在 `solidaxum-rs/sql/{sqlite,postgresql,dm}/`。默认管理员账号
`admin` / `admin123`，**部署后请立即修改**。

健康检查 `GET /health`，Prometheus 指标 `GET /metrics`。

### 前端

```bash
cd solidaxum-ui
pnpm install
pnpm dev        # http://localhost:5173,/dev-api 反代到 3001
```

## 测试

```bash
cd solidaxum-rs
cargo test                                  # 单元 / 仓储测试（SQLite 内存库）
cargo test -p admin --features test-util    # HTTP 契约集成测试
cargo clippy --workspace --all-targets

cd ../solidaxum-ui
pnpm check
pnpm test
```

契约测试以原版 Vue 前端的 API 调用为基准；`--features test-util` 启用
`admin::testkit`，用 SQLite 内存库 + 请求头注入登录态，不需要 Redis。

## 构建交付物

`scripts/build/` 产出自包含、可离线安装的包（后端内嵌交互式安装向导，收集
端口/数据库/Redis 并生成配置，注册为系统服务）：

```bash
bash scripts/build/build-linux.sh --all      # Linux 包（需 cargo-zigbuild + zig）
pwsh scripts/build/build-windows.ps1 -All    # Windows 安装包（需 Inno Setup 6）
bash scripts/build/build-docker.sh           # Docker 镜像（捆绑 Redis）
```

产物输出到 `dist/`（gitignored）。

## 贡献

见 [CONTRIBUTING.md](CONTRIBUTING.md)。安全问题的报告方式见 [SECURITY.md](SECURITY.md)。

## 归属与许可证

本项目以 Apache-2.0 发布，见 [LICENSE](LICENSE)。

SolidAxum 是以下项目的 Rust/Svelte 移植与衍生作品，完整归属声明见
[NOTICE](NOTICE)：

- [RuoYi-Vue-Plus](https://github.com/dromara/RuoYi-Vue-Plus) 与 RuoYi-Vue-Plus-Ui — MIT
- warm-flow — Apache-2.0
- [sea-orm](https://github.com/SeaQL/sea-orm)（`solidaxum-rs/vendor/sea-orm` 为带达梦方言的 fork）— MIT OR Apache-2.0
