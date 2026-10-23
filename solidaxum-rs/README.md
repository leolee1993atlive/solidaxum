# solidaxum-rs

RuoYi-Vue-Plus 后台管理系统的 **Rust 实现**，solidaxum monorepo 的后端。
基于 Axum + SeaORM，接口契约对齐原版 Vue 前端 —— 同一套前端可以直接指向本后端运行。

包含 RBAC、数据权限、代码生成、定时任务、系统监控与工作流引擎。

## 功能范围

| 领域 | 内容 |
|---|---|
| 认证 | 登录 / 注册 / 验证码 / 登出，Redis 会话 + 不透明 Token |
| 权限 | RBAC + RuoYi 六级数据权限，声明式 `require_permission` / `require_role` |
| 系统管理 | 用户、角色、菜单、部门、岗位、字典、参数、通知公告、客户端、社交登录、消息 |
| 文件 | OSS 对象存储 + 多配置管理（S3 兼容），字段级 AES 加密 |
| 代码生成 | 从数据库表反向生成 model/repo/service/handler/route + 前端页面 |
| 工作流 | warm-flow 引擎的 Rust 实现，含设计器接口、监听器、表达式求值 |
| 定时任务 | cron 调度 + 执行日志，分布式锁去重 |
| 监控 | 操作日志、登录日志、在线用户、缓存监控 |
| 基础设施 | 统一响应信封、软删除、雪花 ID、限流、防重提交、XSS 过滤、Prometheus 指标 |

## 架构

8 个 crate 的 workspace，Rust edition 2024：

```
crates/
├── core/      基础设施：配置、错误、响应信封、中间件、雪花 ID、加解密
├── system/    系统管理业务（user/role/menu/dept/...）
├── flow/      工作流引擎（warm-flow 兼容）
├── monitor/   操作日志、登录日志、在线用户、缓存监控
├── job/       定时任务调度与执行日志
├── app-data/  数据访问层：唯一依赖 sea-orm 的业务侧 crate，
│              实体与查询构造经受限 re-export 供其余 crate 使用
├── dm/        达梦数据库适配（sea-orm fork 之下的方言层）
└── admin/     可执行入口：路由组装、启动装配、集成测试脚手架

tools/
└── dm-probe/  达梦方言探测/转换工具（排除在 workspace 之外：链接 unixODBC）
```

### 模块分层

每个业务模块遵循同一套分层：

```
src/{domain}/
  model/       实体 + CreateBo / UpdateBo / ListVo
  repository/  SeaORM 实体 + 数据访问（软删除过滤、分页）
  service/     业务逻辑，经 Service::new(&Infrastructure) 构造
  handler/     Axum handler，返回 R<T>
  route/       Router<Arc<AppState>> 工厂
```

新模块要在 `crates/system/src/lib.rs` 声明，并在 `crates/admin/src/router.rs` 合并路由。

### 几条硬约定

- **ID 出站恒为字符串**。`crates/core/src/vo/response.rs` 在响应边界统一处理雪花 ID，
  否则前端 JS 会丢精度。不走 `R<T>` 信封的裸响应必须改用 `IdJson`。
- **认证是 Redis 会话不是 JWT**。`jwt_util.rs` 已废弃；Bearer 携带的是不透明 UUID，
  服务端查 `global:token:{token}`。TTL 来自 `sys_client.timeout`，每次请求滑动续期。
- **软删除**靠 `deleted_by` / `deleted_date`，repository 层统一过滤。
- 代码注释使用中文。
## 快速开始

### 环境要求

- Rust 1.85+（edition 2024）
- MySQL 8.0+ / PostgreSQL / SQLite 任一
- Redis
- Linux 上需要 unixODBC 开发库（`unixodbc-dev` / `unixODBC-devel`）：
  `crates/dm` 经 `odbc-api` 链接它，含达梦方言的测试二进制才会链接成功

### 配置

编辑 `config.toml`：

```toml
[server]
port = 3001
host = "localhost"
# static_dir = "./static"   # 可选：SPA 静态托管

[db]
db_type = "mysql"           # mysql | postgres | sqlite
url = "localhost"
port = 3306
db_name = "ruoyi"
username = "root"
password = "your_password"

[redis]
url = "localhost"
port = 6379
password = ""
db = 10

[security]
rate_limit_capacity = 100
rate_limit_window_secs = 60
repeat_submit_ttl_seconds = 5
xss_enabled = true
# AES-256 字段加密密钥（32 字节 base64）。生产必须设置。
# 生成：openssl rand 32 | base64
aes_key = ""
```

数据库和 Redis 任一连不上，服务启动即 panic。

### 运行

```bash
cargo run
```

- 服务：`http://localhost:3001`
- 健康检查：`GET /health`
- Prometheus 指标：`GET /metrics`

## 开发命令

```bash
cargo build
```

```bash
cargo test
```

```bash
cargo test -p admin --features test-util
```

```bash
cargo clippy --workspace --all-targets
```

```bash
cargo +nightly fmt
```

`cargo fmt` 的 `imports_granularity` 等选项需要 nightly，稳定版会跳过并告警，
其余格式化照常生效（`max_width = 80`）。仓库代码按 nightly 格式化，CI 用
`cargo +nightly fmt --all --check` 校验。

### 测试分层

| 命令 | 覆盖 | 数量（撰写时） |
|---|---|---|
| `cargo test --workspace` | 各 crate 单元测试 + repo 测试（SQLite 内存库） | 823 |
| `cargo test -p admin --features test-util` | HTTP 契约集成测试 | 315 |
| `cd ../solidaxum-ui && pnpm test` | 前端 | 947 |

契约测试以**原版 Vue 前端的 API 调用**为基准。`--features test-util` 启用
`admin::testkit`：SQLite 内存库 + 种子数据 + 完整路由，`x-test-login-user` 头注入
`LoginUser` 绕过 Redis，用 `tower::oneshot` 驱动，无需真实网络。

尚未修复的缺口以 `#[ignore]` 规格用例编码，修复后去掉 `#[ignore]` 转绿。
依赖 Redis 缓存的接口（config/dict/oss）不在默认脚手架覆盖内。

## 前端

配套 SvelteKit 前端在同 monorepo 的 `../solidaxum-ui`（SvelteKit 5 + Svelte 5 +
TailwindCSS 4 + shadcn-svelte）：

```bash
cd ../solidaxum-ui && pnpm install && pnpm dev
```

侧边栏由后端 `getRouters` 驱动，**Svelte 路由必须与 `sys_menu.path` 对齐**，
否则点进去 404。

## 许可证

Apache-2.0（见仓库根目录 `../LICENSE`）。
