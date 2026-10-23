# sql/sqlite — SQLite 版建库脚本

由 `tools/dm-probe` 的 `mysql2x` 转换器自 `sql/solidaxum_init.sql`
(MySQL 版)生成,勿手改;重新生成命令见文件头。

## 执行

```bash
sqlite3 app.db < sql/sqlite/solidaxum_init.sql
```

已用 Node `node:sqlite` 在空库上全量实跑验证(39 表,0 错误)。

## 与 MySQL 版的主要差异

| MySQL | SQLite 版 |
| --- | --- |
| `AUTO_INCREMENT` | `INTEGER PRIMARY KEY AUTOINCREMENT`(仅 `sys_job_log.job_log_id`,表级主键相应移除) |
| 列/表 `COMMENT` | 丢弃(SQLite 无注释语法) |
| 表尾 `ENGINE=... CHARSET=...` | 删除 |
| `NOW()` | `CURRENT_TIMESTAMP` |
| `0x<hex>` 字面量 | `x'<hex>'`(BLOB 字面量) |
| 表内 `KEY` / `UNIQUE KEY` | 表后 `CREATE [UNIQUE] INDEX` |

`datetime` / `varchar(n)` / `tinyint` 等类型名原样保留(SQLite 类型亲和性)。
