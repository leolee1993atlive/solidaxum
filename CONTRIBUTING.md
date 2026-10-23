# 贡献指南

感谢你愿意参与 SolidAxum。这个项目是 RuoYi-Vue-Plus 的 Rust/Svelte 重写，行为以
Java 参考实现为基线 —— 提交之前请先读完下面几条。

## 开发环境

- Rust 1.85+（edition 2024）
- Node.js 20.19+ 或 22.12+，pnpm 9+
- MySQL / PostgreSQL / SQLite 任一，以及 Redis
- Linux 上 `crates/dm` 链接 unixODBC，需要 `unixodbc-dev`（Ubuntu/Debian）
  或 `unixODBC-devel`（RHEL 系）才能链接带达梦方言的测试二进制

```bash
# 后端
cd solidaxum-rs
cp config.example.toml config.toml
cargo test

# 前端
cd ../solidaxum-ui
pnpm install
pnpm check && pnpm test
```

## 提交前必须通过

```bash
cd solidaxum-rs
cargo +nightly fmt          # rustfmt.toml 用了 imports_granularity 等 nightly 选项
cargo clippy --workspace --all-targets
cargo test
cargo test -p admin --features test-util

cd ../solidaxum-ui
pnpm check
pnpm test
pnpm build
```

另外请跑一遍仓库守卫脚本，它们钉住了几条架构不变量（禁止裸 SQL、达梦方言隔离、
sea-orm 唯一入口等）：

```bash
bash scripts/check-all-guards.sh
```

## 代码约定

- **契约优先**：接口路径、HTTP 方法、响应信封必须与 RuoYi-Vue-Plus 的 Vue 前端调用
  一致。这类差异按破坏性 bug 处理，不是风格问题。
- **分层**：每个业务模块遵循 `model / repository / service / handler / route`。
  新模块在 `crates/system/src/lib.rs` 声明，并在 `crates/admin/src/router.rs` 合并路由。
- **ID 出站恒为字符串**：雪花 ID 直接序列化会让 JS 丢精度，统一走 `R<T>` 信封或
  `IdJson`。
- **注释用中文**，与现有代码保持一致。
- 前端使用 Svelte 5 runes，遵循 `solidaxum-ui/` 里已有的组件与主题钩子。

## 提交信息

采用 Conventional Commits 风格，一句话说清「做了什么」，正文解释「为什么」。一个提交
只做一件事，前后端联动的改动可以放在同一个提交里。

## 新增依赖

- Rust 依赖加在 `solidaxum-rs/Cargo.toml` 的 `[workspace.dependencies]`。
  **注意 `[patch.crates-io]` 必须留在文件末尾**，新键要加在它之前，否则 vendored
  sea-orm fork 会从依赖图里静默消失。
- 前端依赖用 `pnpm add`，并提交 `pnpm-lock.yaml`。

## 报告问题

- 功能缺陷与改进建议请开 issue，附上复现步骤、期望行为与实际行为。
- 安全漏洞**不要**开公开 issue，按 [SECURITY.md](SECURITY.md) 私下报告。
