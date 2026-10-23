# solidaxum-ui

[solidaxum-rs](../solidaxum-rs) 的配套前端（同一 monorepo）。SvelteKit 5 + Svelte 5 (runes) +
TailwindCSS 4 + shadcn-svelte，接口契约对齐 RuoYi-Vue-Plus 原版 Vue 前端。

**纯 SPA**：`adapter-static` + `fallback: index.html`，根布局 `ssr = false`、
`prerender = false`。构建产物是一堆静态文件，可以直接扔给后端的
`server.static_dir` 托管。

## 快速开始

```bash
pnpm install
```

```bash
pnpm dev
```

开发服务在 `http://localhost:5173`，`/dev-api` 反代到 `http://localhost:3001`
（后端默认端口）。所以**跑前端之前要先把后端起起来**，否则登录直接失败。

代理配置在 `vite.config.ts`，`dev` 和 `preview` 两处都要改。

## 命令

```bash
pnpm dev
```

```bash
pnpm build
```

```bash
pnpm check
```

```bash
pnpm test
```

```bash
pnpm format
```

`pnpm check` 是 `svelte-check`（类型 + 模板检查），`pnpm lint` 只是
`prettier --check`，不是 ESLint。提交前至少跑 `pnpm check` 和 `pnpm test`。

## 结构

```
src/
├── routes/
│   ├── (app)/          主应用，带侧边栏 + 顶栏 + 多页签
│   │   ├── system/     用户/角色/菜单/部门/岗位/字典/参数/公告/客户端/文件
│   │   ├── monitor/    在线用户/缓存/定时任务/任务日志
│   │   ├── workflow/   流程定义/设计器/分类/表达式/流程监控
│   │   ├── task/       待办/已办/抄送/我发起的
│   │   ├── tool/       代码生成
│   │   └── user/       个人中心
│   ├── (auth)/         登录 / 注册 / 忘记密码 —— 路由组不参与 URL，登录页在 /
│   └── (errors)/       401 / 403 / 404 / 500 / 503
└── lib/
    ├── api/            契约层：client.ts + 按域拆分的 API 模块
    ├── components/     ui/(shadcn 27 个原语) + 业务组件 + layout + crud/data-table
    ├── composables/    use-crud-table、crud-page-state
    ├── hooks/          use-dict、use-table-url-state、is-mobile
    ├── context/        auth / theme / search / layout（Svelte context + runes）
    ├── menu/           getRouters → NavNode 树的转换与图标映射
    ├── stores/         多页签、连接状态
    └── themes/         3 套配色（default / de-swiss-design / ibm-carbon）
```

### API 层

所有请求走 `src/lib/api/client.ts`：

- 自动附 `Authorization: Bearer <token>` 和 `clientid` 头（token 存 localStorage
  的 `solidaxum_token`）
- 响应信封 `{ code, msg, data }`，`code !== 200` 统一 toast 并抛
  `ApiClientNotifiedError`（调用方 catch 到它就不用再提示一遍）
- `401` → 清会话并跳 `/?session=expired`
- 列表用 `getList()` 解嵌套的 `R<TableDataInfo<T>>`，返回扁平 `{ rows, total }`
- 单次请求可传 `{ silent: true }` 抑制 toast，用于「前端循环逐条发」的批量场景

**文件下载不走 client**。`downloadBlob()` 用裸 `fetch` 取 blob（后端要
`Authorization` 头，裸 `<a href>` 会 401），它抛的是普通 `Error`，**全局 toast
管不到**——调用方的 `catch` 里必须自己 `toast.error`，否则下载失败完全静默。

## 三条踩过坑的约定

### 1. 路由必须与后端菜单 path 对齐

侧边栏不是前端写死的，是后端 `getRouters` 返回的 `sys_menu` 树
（`src/lib/menu/transform.ts` 转成 `NavNode`）。菜单 `path` 拼出来的地址就是
用户点击后要去的地址。

所以**新增页面时，Svelte 路由目录要跟着 `sys_menu.path` 走**，而不是反过来改
后端菜单数据。路径对不上的表现是：菜单能看见，点进去 404。

### 2. ID 一律当字符串

后端主键是雪花 ID（i64），超出 JS `Number.MAX_SAFE_INTEGER`，转 number 会**静默
丢精度**。后端已在响应边界把所有 ID 字段序列化成字符串，前端也必须一路保持字符串。

这条有测试强制：`src/lib/api/id-contract.test.ts` 会扫描全部源文件，禁止
`Number(xxxId)`、`parseInt(id)`、以及 API 层把 ID 声明成 `number`。包括
`leader` / `avatar` / `createBy` / `dictCode` 这些**不带 Id 后缀的语义 ID**。

### 3. 用真浏览器验证异步渲染

Claude Code 的内置浏览器**不会刷新 Svelte 5 的异步 `$state`**，页面会一直停在
「加载中」。这是工具的假阳性，不是代码 bug —— 遇到这种情况换真浏览器确认，
别去改本来没问题的代码。

## 组件与数据表

- **shadcn-svelte 原语**在 `lib/components/ui/`（27 个，基于 bits-ui +
  tailwind-variants），按 shadcn 惯例是拷贝进仓库的源码，可以直接改
- **CRUD 页面**用 `CrudTableShell` / `TreeTableShell` + `useCrudTable`
  组合，分页、排序、筛选、URL 状态同步都在里面
- **表格**基于 `@tanstack/table-core`（headless，渲染层自己写）
- **富文本**用 tiptap，输出经 `dompurify` 净化后由 `SafeHtml` 渲染
- **图表**用 echarts
- **多页签**在 `stores/tabs.svelte.ts`，带页面级缓存

## 测试

```bash
pnpm test
```

vitest + happy-dom + @testing-library/svelte，110 个测试文件、918 个用例
（撰写时）。测试文件与被测代码同目录（`*.test.ts`）。

除了常规单元测试，还有两类值得注意的：

- **契约测试** —— 以原版 Vue 前端的 API 调用为基准，钉住请求路径、参数形态、
  响应字段（如 `api/system/export-contract.test.ts`、`api/workflow/task-contract.test.ts`）
- **源码扫描测试** —— 上面提到的 ID 契约，靠遍历源文件而不是运行代码来强制约定

## 许可证

Apache-2.0（随主仓库）。
