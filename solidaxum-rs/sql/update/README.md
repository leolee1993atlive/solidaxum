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

目前为空。
