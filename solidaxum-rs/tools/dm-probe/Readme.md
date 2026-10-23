# dm-probe

达梦(以及其他 ODBC 数据源)的 SQL 行为探针。一次跑完 13 项测试,
每项独立、失败不中断,末尾输出汇总清单。

用途:

- 适配新的信创数据库时,先跑一遍,拿到"哪些 SQL 形态原生可用、哪些要改写"的清单
- 验收客户现场环境,确认建库参数没配错

**不是 workspace 成员**(见根 `Cargo.toml` 的 `exclude`)。它链接 unixODBC,
不该让所有开发机和 CI 为此装 ODBC 开发库。要用时进本目录单独 `cargo run`。

## 用法

```bash
cargo run -- "DSN=DM8"
DM_ODBC="DSN=DM8" cargo run          # 密码不进 shell 历史
cargo run --features narrow -- "DSN=DM8"   # 切窄字符,排查中文乱码
```

建议先在 `/etc/odbc.ini` 配好 DSN 并用 `isql DM8` 手工连通,
再跑本程序 —— 各家驱动的连接串参数名不一致,配 DSN 比拼连接串省事。

## 覆盖范围

| 组 | 项 |
| --- | --- |
| 标识符 | 裸标识符、反引号标识符 |
| 占位符 | `?`、`:1` |
| 字符集 | 中文往返(不一致时打印原始字节) |
| 类型 | 雪花 ID 全精度、`datetime` 字面量、`char(1)` 尾空格 |
| SQL 形态 | 批量 `VALUES`、`LIMIT ? OFFSET ?`、`ON DUPLICATE KEY UPDATE` |
| 事务 | 手动提交 + rollback |

## 注意

- 会 `DROP TABLE dm_probe` 再重建,别在有同名表的库上跑
- `Cargo.toml` 里的 `default-features = false` 是刻意的:`odbc-api` 默认 feature 含
  `prompt`,会拖入 winit 整套 GUI 栈,在无桌面的服务器上可能因缺 X11 开发库而编译失败

## 达梦实测结论(2026-08-10,麒麟 V10 + DM8)

12/13 通过,唯一失败项为 `ON DUPLICATE KEY UPDATE`。

## 附:SQL 方言转换器(mysql2dm / mysql2x)

本目录还带两个独立 bin,把 `sql/` 下的 MySQL init 脚本转成其它方言
(纯文本转换,不连库):

```bash
# MySQL → 达梦(生成 sql/dm/*.sql)
cargo run --bin mysql2dm -- ../../sql/solidaxum_init.sql ../../sql/dm/solidaxum_init.sql

# MySQL → SQLite / PostgreSQL(生成 sql/sqlite/、sql/postgresql/)
cargo run --bin mysql2x -- sqlite   ../../sql/solidaxum_init.sql ../../sql/sqlite/solidaxum_init.sql
cargo run --bin mysql2x -- postgres ../../sql/solidaxum_init.sql ../../sql/postgresql/solidaxum_init.sql
```

- 生成文件顶部带"勿手改"头部注释;改库结构请改 MySQL 源文件后重新生成。
- `sql/dm/solidaxum_init.sql` 是早期实机验证产物,未带生成头,
  不要直接用现行 mysql2dm 覆盖它。
- sqlite 产物已由 `node:sqlite` 内存库实跑验证;dm / postgresql 产物
  仅人工抽查,无实机验证。
