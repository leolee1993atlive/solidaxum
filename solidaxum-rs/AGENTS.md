# AGENTS.md

## Build & Run

```bash
cargo build                  # compile
cargo run                    # run dev server (reads config.toml from CWD)
cargo test                   # run tests (repo/service unit tests; SQLite in-memory)
cargo test -p admin --features test-util   # HTTP 契约集成测试 (见下)
cargo +nightly fmt           # format (max_width = 80, imports_granularity = Crate; needs nightly)
cargo clippy                 # lint
```

### HTTP 集成测试脚手架

`admin` crate 同时是 lib + bin。`admin::build_router(app_state, monitor_state, job_state, AuthMode)`
组装与生产一致的业务路由(不含依赖 Redis 的全局限流/防重/XSS 层与静态托管,
那些在 `main.rs` 叠加),供测试复用。

`--features test-util` 启用 `admin::testkit`:
- `TestApp::new()` — SQLite 内存库 + 种子数据 + 完整路由(`AuthMode::Test`)。
- `test_auth` 中间件从 `x-test-login-user` 头注入 `LoginUser`,**绕过 Redis**;
  缺失则 401(可测未登录)。
- `app.get/post/put/delete(uri, Some(&user))` 用 `tower::oneshot` 驱动,返回
  `(StatusCode, serde_json::Value)`;`assert_ok` / `code()` 断言响应信封。
- 用户构造:`user_with_perms(&["system:dept:list"])` / `super_admin()`。

测试以**原版 Vue 前端**的 API 调用为契约基准(见
`crates/admin/tests/contract_system_test.rs`)。

注意:依赖 Redis 缓存的接口(config/dict/oss 等)暂不在脚手架默认覆盖内,
需另接测试用 Redis。

Running requires a database (MySQL/PostgreSQL/SQLite) and Redis configured in `config.toml`. The app will panic on startup if either is unreachable.

## Architecture

8-crate workspace (`core` / `system` / `admin` / `flow` / `monitor` / `job` / `dm` / `app-data`), Rust edition 2024.

app-data 是唯一依赖 sea-orm 的业务侧 crate(`crates/dm` 在 fork 之下,不计)。core/system/flow/monitor/job/admin 的 sea-orm 直连已全部移除,`scripts/check-sea-orm-uniqueness.sh` 是这条不变量的守卫。业务代码经 `app_data::sea_orm` 受限 re-export 使用实体与查询构造,经 `DataContext`/`TxnContext` 执行。

### Module pattern

Each domain module (`auth`, `sys/user`) follows a strict layer convention:

```
src/{domain}/
  model/      — entity struct + CreateBo / UpdateBo / ListVo DTOs + BO-to-entity conversions
  repository/ — SeaORM entity + data access (soft-delete filters, pagination)
  service/    — business logic, holds a repo instance, constructed via `Service::new(&Infrastructure)`
  handler/    — Axum handler functions, returns `R<T>` response wrapper
  route/      — `Router<Arc<AppState>>` factory, merges into app in main.rs
```

### Shared state

`AppState` (in `src/app.rs`) holds `Infrastructure` (SeaORM `DatabaseConnection` + deadpool-redis + JWT config) and a `ServiceContainer`. All handler state is `Arc<AppState>`.

### OSS 存储(provider 抽象)

`system::oss::service::store::OssStore` 是存储后端抽象, 由 `OssClientFactory`
按 `sys_oss_config` 行构建并缓存, 两种实现:
- `S3ClientWrapper` — S3 协议(MinIO/RustFS/云厂商), endpoint 为 `host:port`。
- `LocalStore` — 本地磁盘, endpoint 以 `file://` 前缀标识(如 `file://./data/oss`),
  `bucket_name` 为其下子目录; 文件经公开静态路由 `GET /oss/file/{config_key}/{*key}`
  直出(不过 auth; 危险类型强制 attachment + nosniff 防同源 XSS)。
上传/下载/删除对调用方透明; `sys_oss.file_name` 是 UUID 存储名,
`original_name` 是显示/下载文件名, 同名文件不冲突。新部署初始化脚本
`sql/solidaxum_init.sql` 默认 local。

### Auth flow

- Unauthenticated routes: `GET /auth/code`, `POST /auth/login`, `POST /auth/register`, `POST /auth/logout`, `GET /oss/file/{config_key}/{*key}`（local 静态直链）.
- All `/system/*` routes go through `middleware::auth`, which validates an opaque token (Bearer) against a Redis session stored under `global:token:{token_value}`.
- Login stores a `LoginUser` (permissions, roles, dept, client info) in Redis and returns the token UUID. Token TTL comes from `sys_client.timeout`; session is refreshed (sliding expiry) on every authenticated request using `config.jwt.refresh_ttl`.
- The `clientid` request header is checked against the session's `client_id` when present.
- `CurrentUser` extractor (in `middleware/extractors.rs`) pulls the `LoginUser` injected by the auth middleware from request extensions; it does not query the DB on every request.
- Declarative permission/role checks: `permission::guard::require_permission` / `require_role` in handlers. Data scope (6-level RuoYi) computed by `PermissionService::get_data_scope` and applied as SeaORM filters in repositories.

### Response wrapper

All handlers return `R<T>` from `common::vo::response`. It serializes as `ApiResponse { code, message, data }` — use `R::ok()`, `R::ok_with_data(...)`, `R::error_with_code_and_message(...)` etc.
- **ID 字段出站恒为字符串**：`crates/core/src/vo/response.rs` 的 `stringify_ids` 在响应边界统一处理 —— 键名匹配 `*Id`/`*Ids`/`*_id`/`*_ids` 或 `id`/`leader`/`avatar`/`dictCode`/`createBy`/`createDept`/`checkedKeys` 等语义 ID 的整数一律转字符串,数组元素继承父键。**新增 VO 无需做任何事**;但**新增不走 `R<T>` 信封的响应**(裸 `axum::Json`)必须改用 `IdJson`,否则 ID 会以数字裸奔并在前端丢精度。

### Entities

All entities include `#[serde(flatten)] base_entity: BaseEntity` with audit fields (version, created_by/date, last_modified_by/date, deleted_by/date). Soft-delete is done by setting `deleted_by`/`deleted_date`; repositories filter on `deleted_by IS NULL AND deleted_date IS NULL`.

### SeaORM repositories

Repositories are backed by SeaORM. Each repo file defines a `Model` with `#[sea_orm(table_name = "...")]`, plus `From<&Model>` to the domain struct. Look at `sys/user/repository/user_repo.rs` for the reference pattern (select_all, select_by_id, select_page, insert, update_by_id, delete_by_id, soft-delete filter).

### Errors

`AppError` in `src/error/app_error.rs` is the unified error type. `AppResult<T>` is its alias. Database errors are translated to Chinese by `error/localizer.rs`.

## Key conventions

- BO structs for create/update input use `validator::Validate` derive; handlers call `.validate()` manually before proceeding.
- Services are constructed in `ServiceContainer::new` — add new services there.
- New modules must be declared in `crates/system/src/lib.rs` with `pub mod {name};` and their route merged into the router in `crates/admin/src/router.rs`.
- Comments in the codebase are in Chinese.
- Prometheus metrics endpoint: `GET /metrics`.
- Health endpoint: `GET /health`.
- Static SPA hosting: set `server.static_dir` in `config.toml`.

## Java 基线核实(RuoYi-Vue-Plus 平价)

本仓是 RuoYi-Vue-Plus 的 Rust 移植,行为差异一律以 Java 源码为基线裁定
(源码在 `../RuoYi-Vue-Plus`)。核实时**不要只查 Controller** —— RuoYi 的
越权探针与业务守卫散在 Controller / ServiceImpl / listener 三处,只看一层
会得出相反的结论:

| 端点 | `checkXxxDataScope` 实际位置 |
|---|---|
| user edit / resetPwd / changeStatus / authRole / getInfo | Controller(`SysUserController:201/:256/:273/:329/:147`) |
| **user remove(软删)** | **ServiceImpl**(`SysUserServiceImpl.deleteUserByIds:531-534`),Controller 只有「不能删自己」 |
| role getInfo / edit / editPermission / changeStatus / authUser selectAll | Controller(`SysRoleController:83/:120/:146/:166/:266`) |
| **role remove** | **ServiceImpl**(`SysRoleServiceImpl.deleteRoleByIds:437`) |
| dept getInfo / edit / remove | Controller(`SysDeptController:73/:106/:143`);换父探针在 `SysDeptServiceImpl:346` |
| post 全部 | 无探针(Java 侧 post 写路径本就不受数据权限约束) |
| user 导入(更新已存在分支) | **listener**(`SysUserImportListener:88`);Rust 的 `import_data` 只新增、已存在即跳过,无此分支故无对应探针 —— 若将来补 `updateSupport`,探针要一起补 |

**这条是踩出来的**:曾有一轮写路径回退只核了四个 Controller,
据 `SysUserController.remove` 干净就判定「Java 软删无探针」并明确不补,
结果 user 软删在回退后完全裸奔(受限管理员可软删任意部门用户),复审时
才发现探针在 ServiceImpl。role 侧同样误判,只是碰巧因为 `select_by_id`
是受辖读而没出事。

核实命令(全目录一次 grep,别按单个文件推断):

```bash
grep -rn "checkUserDataScope\|checkRoleDataScope\|checkDeptDataScope" \
  ../RuoYi-Vue-Plus/ruoyi-modules/ruoyi-system/src/main/java/org/dromara/system/
```

同理,`@DataPermission` 注解只在 Mapper 上、且只对**显式注解的方法**生效
(`lambda()` 链与 BaseMapperPlus 基方法不受辖)—— 判断某个查询是否受辖,
要看它最终落到哪个 Mapper 方法,不能按表名一概而论。

## Frontend

配套 SvelteKit 前端在同一 monorepo 的 `../solidaxum-ui`（SvelteKit 5 + Svelte 5 + TailwindCSS 4 + shadcn-svelte）。

```bash
cd ../solidaxum-ui
pnpm install
pnpm dev          # http://localhost:5173
pnpm check        # svelte-check
pnpm build        # static build
```
