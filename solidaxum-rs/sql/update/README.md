# sql/update — 存量库变更脚本

存放**已部署库**的增量变更脚本(ALTER / UPDATE / 种子补插等)。
全新部署不需要本目录:空库直接执行 `sql/solidaxum_init.sql`
(或 `dm/`、`sqlite/`、`postgresql/` 下的方言版)。

## 约定

- 文件命名:`YYYY-MM-DD_<简述>.sql`,按日期序执行。
- 每个文件头部注明:来源需求、影响表、**是否幂等**、重复执行的处理方式。
- 非幂等脚本(ALTER 加列、INSERT 种子)在文件头写明"已执行则跳过本节"。
- 变更稳定后折回 `sql/solidaxum_init.sql` 并同步各方言目录
  (`dm/`、`sqlite/`、`postgresql/`),然后从本目录移除。

目前脚本:
- `2026-10-31_b5_message_channel.sql` — B5 消息渠道配置与发送日志
  (幂等;MySQL 见本目录,达梦/SQLite/PostgreSQL 同名脚本在各方言 update 目录)
- `2026-10-31_b5_user_platform_ids.sql` — B5 sys_user 三平台推送 ID
  (非幂等 ALTER 加列;已执行则跳过,重复执行会报列已存在)
- `2026-10-31_b5_notice_target.sql` — B5 定向公告与已读回执
  (非幂等 ALTER 加列;CREATE TABLE IF NOT EXISTS 段可重复执行)
