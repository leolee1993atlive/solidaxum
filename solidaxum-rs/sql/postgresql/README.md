# sql/postgresql — PostgreSQL 版建库脚本

由 `tools/dm-probe` 的 `mysql2x` 转换器自 `sql/solidaxum_init.sql`
(MySQL 版)生成,勿手改;重新生成命令见文件头。

**尚未在真实 PostgreSQL 实例上验证**,部署前请先在测试库全量跑一遍。

## 执行

```bash
createdb app
psql -d app -f sql/postgresql/solidaxum_init.sql
```

脚本不建库,先自行 `CREATE DATABASE`。

## 与 MySQL 版的主要差异

| MySQL | PostgreSQL 版 |
| --- | --- |
| 反引号标识符 | 裸标识符(全小写,PG 折叠后一致) |
| `bigint ... AUTO_INCREMENT` | `BIGSERIAL`(仅 `sys_job_log.job_log_id`) |
| 列内联 `COMMENT '...'` | 表后 `COMMENT ON COLUMN t.c IS '...'` |
| 表尾 `COMMENT='...'` | 表后 `COMMENT ON TABLE t IS '...'` |
| 表尾 `ENGINE=... CHARSET=...` | 删除 |
| `tinyint` / `datetime` | `SMALLINT` / `TIMESTAMPTZ` |
| `char(n)` | `varchar(n)`(本仓 53 列全是 `char(1)`) |
| `longtext` 系 / `longblob` 系 | `TEXT` / `BYTEA` |
| `0x<hex>` 字面量 | `'\x<hex>'`(bytea 十六进制输入) |
| 表内 `KEY` / `UNIQUE KEY` | 表后 `CREATE [UNIQUE] INDEX` |

`NOW()`、批量 `VALUES` 原样保留(PG 原生支持)。

## 为什么 datetime 落到 TIMESTAMPTZ 而不是 TIMESTAMP

实体基类 `BaseEntity`(`crates/core/src/model/entity.rs`)把 `create_time` /
`update_time` 声明为 `Option<DateTime<Utc>>`,sqlx 的 PG 驱动只接受
`TIMESTAMPTZ`;配 `TIMESTAMP` 会在解码时报 `mismatched types`。由于
`BaseEntity` 被所有实体继承,一旦映射错,**所有业务表在 PG 上都读不了**
(表现为登录 401「无效的客户端」,真实原因在日志里)。sqlx 的 MySQL 驱动
对 `DATETIME` 宽容,所以这个错只在 PG 上暴露。

种子里的时间字面量不带时区,PG 会按会话 `TimeZone` 解释它们。脚本因此在
文件头固定 `SET TimeZone = 'UTC';`,保证同一份脚本在任何服务器上导入都得到
相同的时刻。

## 为什么 char(n) 落到 varchar(n)

PG 的 `char(n)` 是 `bpchar`,而 sqlx 把字符串参数按 `text` 绑定。
`bpchar = text` 会迫使 PG 把类型转换加到**列**上:

```
Filter: ((del_flag)::text = '0'::text)     ← 转换在列上,索引退化成过滤器
```

列上一旦带转换,该列的 btree 索引就用不成索引条件,只能全扫;代价估高后
规划器还容易改走并行,而 Windows 上并行 worker 是进程创建,`Gather` 的固定
开销可达百毫秒量级。改成 `varchar(n)` 后恢复 `Index Cond`,并行也不再触发。

本仓 53 个 char 列全部是 `char(1)`,所以不存在 MySQL(取值时去尾空格)与
PG(`bpchar` 补空格)的尾空格语义差异。PG 官方文档亦不推荐 `char(n)`。

> 若已有部署过的 PG 库需要迁移,`ALTER TABLE ... ALTER COLUMN TYPE varchar(n)`
> 会重写整张表并清零可见性映射,索引只读扫描随之不可用、规划器会退回顺序
> 扫描 —— 迁移脚本务必在 `ALTER` 之后补 `VACUUM`,否则会误以为改动无效。
