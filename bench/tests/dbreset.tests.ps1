# dbreset.ps1 里纯函数(SQL 生成)的单测。
# Reset-BenchState / Get-StateFingerprint 要连真库,属集成级,由冒烟用例的
# 「破坏 → 复位 → 比对指纹」步骤验证,不在这里。
. (Join-Path $PSScriptRoot '../lib/dbreset.ps1')

$tables = @('sys_user', 'sys_dept', 'sys_login_info')

section '复位 SQL —— MySQL'
$my = New-ResetSql -Engine 'mysql' -Tables $tables
assert_match '关外键检查' 'SET FOREIGN_KEY_CHECKS=0' $my
assert_match '开外键检查' 'SET FOREIGN_KEY_CHECKS=1' $my
assert_match 'TRUNCATE sys_user'  'TRUNCATE TABLE sys_user;' $my
assert_match '从模板库回灌'        'INSERT INTO sys_user SELECT * FROM ruoyi_tpl.sys_user;' $my
assert_match '三张表都覆盖'        'TRUNCATE TABLE sys_login_info;' $my
# TRUNCATE 会把表统计清零。不 ANALYZE 的话规划器以为表是空的,
# 同一个查询在不同轮次可能走完全不同的执行计划 —— 测出来的就不是性能差异了。
assert_match 'ANALYZE 重建统计' 'ANALYZE TABLE sys_user, sys_dept, sys_login_info;' $my

section '复位 SQL —— PostgreSQL'
$pg = New-ResetSql -Engine 'pg' -Tables $tables
# 一条 TRUNCATE 带全部表,比逐表 TRUNCATE 少 39 次往返,也免去顺序问题
assert_match '单条 TRUNCATE 带全部表' 'TRUNCATE TABLE public.sys_user, public.sys_dept, public.sys_login_info CASCADE;' $pg
assert_match '从 tpl schema 回灌' 'INSERT INTO public.sys_user SELECT * FROM tpl.sys_user;' $pg
# 必须是 VACUUM (ANALYZE) 而非只 ANALYZE:复位回灌 60 万行后,
# autovacuum(naptime 60s)会在测量窗口里跑起来抢 I/O —— 实测一轮 30 秒的
# 读场景期间跑了 3 次,而检查点 0 次。MySQL 没有 autovacuum,不显式 VACUUM
# 就等于让 PG 白背一个由复位方式造成的开销。这是公平性,不是优化。
assert_match 'VACUUM 兼 ANALYZE' 'VACUUM (ANALYZE) public.sys_user;' $pg
assert_notmatch '不能只 ANALYZE' "`nANALYZE public.sys_user;" $pg
# RESTART IDENTITY 会把序列复位到 start value(1),而模板里已有数据 ——
# 两者不等,下次插入就撞主键。必须 setval 到回灌后的实际 max。
assert_match '序列 setval 修复' 'setval' $pg
assert_notmatch '不用 RESTART IDENTITY' 'RESTART IDENTITY' $pg

section '空表清单 —— 不能生成半截 SQL'
assert_throws 'MySQL 空清单抛异常' { New-ResetSql -Engine 'mysql' -Tables @() }
assert_throws 'PG 空清单抛异常'    { New-ResetSql -Engine 'pg' -Tables @() }

section 'Redis 库白名单 —— 硬拒绝非压测库'
# 压测走专用实例 6380;库号白名单是第二道防线,万一有人把端口改回 6379,
# 它仍能挡住对用户 db0/db10 的 flush。
assert_eq 'db14 允许'  $true  (Test-BenchRedisDb 14)
assert_eq 'db15 允许'  $true  (Test-BenchRedisDb 15)
assert_eq 'db0 拒绝'   $false (Test-BenchRedisDb 0)
assert_eq 'db10 拒绝'  $false (Test-BenchRedisDb 10)
