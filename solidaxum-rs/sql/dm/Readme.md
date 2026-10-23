# sql/dm — RuoYi 建表脚本的达梦(DM8)版本

`solidaxum_init.sql` 由 `sql/solidaxum_init.sql`(MySQL 版)经
`tools/dm-probe` 的 `mysql2dm` 转换器生成,并在达梦实机全新 schema 上
全量验证通过(698 条语句,0 失败,重复执行幂等)。
**注意:该文件是实机验证时的产物,现行 mysql2dm 已有行为变化
(REPLACE INTO 改写、生成头等),不要用它重新生成覆盖本文件。**

## 建库参数(硬约束,建库后不可修改)

| 参数 | 要求 | 原因 |
| --- | --- | --- |
| `CASE_SENSITIVE` | **N(不敏感)** | 敏感模式下裸标识符被折叠为大写,与脚本的小写标识符不匹配,系统无法部署 |

达梦默认 `CASE_SENSITIVE=Y`,与脚本的小写标识符不匹配,系统无法部署;建库时必须显式关闭。

## 执行步骤

达梦为单实例单库架构:MySQL 的 database 对应达梦的 **schema**,而 schema 即用户。

```sql
-- 以 SYSDBA 执行:建用户(=建 schema)并授权
CREATE USER RUOYI IDENTIFIED BY "<强密码>" DEFAULT TABLESPACE MAIN;
GRANT RESOURCE TO RUOYI;
```

然后以 `RUOYI` 用户连接,逐条执行 `solidaxum_init.sql` 全文:

- disql:`start /path/to/solidaxum_init.sql`
- 或本仓库探针:`DM_ODBC="DSN=<dsn>;UID=RUOYI;PWD=<密码>" dm-sql "@sql/dm/solidaxum_init.sql"`

脚本幂等:各表 `DROP TABLE IF EXISTS` + `CREATE`,重复执行会清空重建。

## schema(必读)

达梦是**单实例单库**架构:MySQL 的 database 对应达梦的 **schema**,而 schema 即用户。
连接后的默认 schema 是**登录账号自己的那个**。

因此如果按上面的步骤以 `RUOYI` 建表、却用别的账号(比如 `SYSDBA`)跑应用,
所有裸表名都解析不到:

```
State: 42S02, Native error: -2106, Message: 无效的表或视图名[sys_user]
```

两种解法:

1. **让应用以建表账号登录** —— `username` 填 `RUOYI`,零配置。仅当运维允许应用直接用
   schema 属主账号时可行。
2. **显式指定 schema** —— 应用连上后会先发 `SET SCHEMA "<名字>"`,之后裸表名即在该
   schema 下解析。账号与 schema 属主可以不是同一个(企业环境的常态)。

```toml
[db]
db_type = "dm"
db_name = "DM8"        # ODBC 数据源名,不是 schema 名
username = "SYSDBA"
password = "<密码>"
schema  = "RUOYI"      # ← 裸表名在这个 schema 下解析;不填则用登录账号自己的
```

`schema` 是可选项,其余后端忽略它。名字会被双引号包裹后下发,所以带连字符的
schema 名(实例上确实存在)也支持,大小写按填写的原样匹配。

## ODBC 数据源与密码

应用连库靠 `config.toml`,**DSN 里不需要存密码**:

```toml
[db]
db_type = "dm"
db_name = "DM8"        # ODBC 数据源名,不是 schema 名
username = "RUOYI"
password = "<密码>"
```

`build_dsn`(`crates/admin/src/app.rs`)据此拼出 `dm:DSN=DM8;UID=RUOYI;PWD=…`,
凭据随连接串下发。DSN 只需配好驱动、SERVER、TCP_PORT。

**但集成测试不走这条路** —— 它们把环境变量 `DM_ODBC` 原样当连接串用,所以必须自带凭据:

```bash
DM_ODBC="DSN=DM8;UID=RUOYI;PWD=<密码>" cargo test -p dm -- --ignored
```

其中 `resolves_bare_table_names_in_configured_schema` 还需要 `DM_SCHEMA` 指向装过
建表脚本的 schema(未设则跳过):

```bash
DM_ODBC="DSN=DM8;UID=SYSDBA;PWD=<密码>" DM_SCHEMA=RUOYI cargo test -p dm -- --ignored
```

⚠️ **达梦的 ODBC 配置对话框不保存密码。** 那里的「登录 ID / 密码」两个框归
「测试用户输入的登录 ID 和密码」这个选项管,是给测试连接按钮用的;确定之后
注册表里 `UID` 会留下,`PWD` 仍是空串(实测 Windows `HKLM\SOFTWARE\ODBC\ODBC.INI\DM8`)。

密码缺失时驱动返回:

```
State: HY000, Native error: -71008, Message: 需要更多信息 (SQLDriverConnect)
```

这条报错**不是**连不上服务器,而是驱动想弹窗补信息却弹不出来 —— `odbc-api` 的
`prompt` feature 被刻意禁用(它会拖入 winit 整套 GUI 栈,无桌面的麒麟服务器上
编译不过,见 crate 依赖注释)。补全连接串里的 `UID`/`PWD` 即可。

若确实想让 DSN 自带密码(那样 `DM_ODBC="DSN=DM8"` 就够),绕开对话框直接写:

```powershell
Set-ItemProperty 'HKLM:\SOFTWARE\ODBC\ODBC.INI\DM8' -Name PWD -Value '<密码>'   # 需管理员
```

Linux/麒麟对应 `/etc/odbc.ini` 的 `PWD=` 项。注意两种做法都是明文落盘。

## 与 MySQL 版的差异清单

| MySQL | 达梦版 |
| --- | --- |
| `CREATE DATABASE` / `USE` / `SET FOREIGN_KEY_CHECKS` | 删除(schema 即用户,外键检查无对应开关) |
| 列内联 `COMMENT '...'` | 表后 `COMMENT ON COLUMN t.c IS '...'` |
| 表尾 `COMMENT='...'` | 表后 `COMMENT ON TABLE t IS '...'` |
| 表尾 `ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=...` | 删除 |
| `AUTO_INCREMENT` | `IDENTITY(1,1)` |
| `tinyint(n)`(显示宽度) | `TINYINT` |
| `longtext` / `mediumtext` / `tinytext` | `TEXT` |
| `longblob` / `mediumblob` | `BLOB` |
| `PRIMARY KEY (...) USING BTREE` | 去 `USING BTREE` |
| 表内 `KEY` / `UNIQUE KEY` | 表后 `CREATE [UNIQUE] INDEX` |
| 字符串内 MySQL 反斜杠转义(`\"` `\\` `\n` 等) | 字面字符(引号双写);达梦不解释反斜杠转义 |

未改动:`datetime`、`varchar`、`char`、`decimal`、`int`、`bigint`、`text`、
反引号标识符、批量 `VALUES` —— 达梦原生支持(实机探针验证)。

## 实机验证记录(2026-08-14,DM8 @ 127.0.0.1:5236)

`DM_ODBC=… cargo test -p dm -- --ignored` 与 `-p system --test db_meta_dm_test`
全部通过(17 项)。其中几条结论对后续改动有约束力:

| 结论 | 出处 |
| --- | --- |
| 反引号标识符的大小写**原样保留**,不折叠为大写 —— 按列名 `try_get` 能命中 | `orm_path_test` |
| 19 位雪花 ID 经文本中间表示往返无精度损失 | `orm_path_test` |
| `datetime` 往返到**毫秒**精确相等 | `orm_path_test` |
| 语句级错误(主键冲突)**不中止事务**,同一事务后续语句仍可执行并提交(Oracle 式语义) | `e2e_test::statement_error_does_not_kill_the_transaction` |
| `ON DUPLICATE KEY UPDATE` 改写成的 `MERGE INTO` 可执行且语义正确(首插入、次更新) | `rewrite_dm_test` |
| 事务隔离成立:未提交不可见、非事务写不随事务回滚、提交后非事务写立即持久 | `e2e_test` 三条隔离测试 |

换实例或升级达梦版本后,这些结论需重新验证 —— 尤其第一条依赖建库参数
`CASE_SENSITIVE=N`。

## 源文件的已知问题(未在本仓库修复)

MySQL 版源文件在 2026-07-31 清理运行时数据时,删除了 `sys_oper_log` 的
INSERT 头但留下了约 1200 行 VALUES 孤儿数据(含原始 HTTP multipart 载荷,
原文件第 1090–2316 行)。该段在任何数据库上都不是合法 SQL;转换器第一遍
已将此类"语句外的孤儿行"整段剔除。**MySQL 原文件存在同样的可执行性缺陷,
如需修复请另行处理上游 `sql/solidaxum_init.sql`。**
