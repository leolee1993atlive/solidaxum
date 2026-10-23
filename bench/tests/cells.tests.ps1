# cells.ps1 的单测。由 bench/test-bench.ps1 dot-source(断言函数已在作用域内)。
. (Join-Path $PSScriptRoot '../lib/cells.ps1')

section '单元注册表'
assert_eq '共 6 个单元' 6 (Get-CellIds).Count
assert_eq 'id 列表' 'axum-mysql,axum-pg,axum-pgb,ruoyi-mysql,ruoyi-pg,ruoyi-pgb' ((Get-CellIds) -join ',')
assert_throws '未知 id 抛异常' { Get-Cell 'nope' }

section 'BaseUrl —— axum 带 /api 前缀,ruoyi 不带'
# 依据 crates/admin/src/router.rs:245-248。上一版 run.ps1 默认值少了 /api,会 404。
assert_eq 'axum BaseUrl'  'http://127.0.0.1:18000/api'        (Get-CellBaseUrl (Get-Cell 'axum-mysql'))
assert_eq 'ruoyi BaseUrl' 'http://127.0.0.1:18080'           (Get-CellBaseUrl (Get-Cell 'ruoyi-mysql'))
assert_eq 'axum 探活'     'http://127.0.0.1:18000/api/health' (Get-CellProbeUrl (Get-Cell 'axum-pg'))
assert_eq 'ruoyi 探活'    'http://127.0.0.1:18080/auth/code'  (Get-CellProbeUrl (Get-Cell 'ruoyi-pg'))

section 'Redis —— 共用 6379,靠库号白名单保护用户数据'
# db0(ruoyi 默认)与 db10(axum 默认)里有用户的数据,复位时的 FLUSHDB
# 只认 14/15(见 dbreset.ps1 的 Test-BenchRedisDb)。
assert_eq 'axum → db14'  14 (Get-CellRedisDb (Get-Cell 'axum-pgb'))
assert_eq 'ruoyi → db15' 15 (Get-CellRedisDb (Get-Cell 'ruoyi-pgb'))
assert_eq '端口 6379' 6379 (Get-CellRedisPort (Get-Cell 'axum-mysql'))
assert_eq '无口令'    ''   (Get-CellRedisPass (Get-Cell 'ruoyi-mysql'))

section 'JDBC'
$mysqlJdbc = Get-CellJdbcUrl (Get-Cell 'ruoyi-mysql')
assert_match 'MySQL 主机端口库名' 'jdbc:mysql://127.0.0.1:3306/ruoyi' $mysqlJdbc
# 刻意不写 useSSL —— 让 Connector/J 用默认 sslMode=PREFERRED,与 sqlx 的默认
# ssl-mode=PREFERRED 对齐(公平性 F13)。axum 的 DSN 由
# crates/admin/src/installer/dsn.rs:57 硬编码生成,无法注入 ssl 参数,
# 所以只能让 Java 侧也走驱动默认值。
assert_notmatch 'MySQL 不写死 useSSL' 'useSSL' $mysqlJdbc
assert_notmatch 'MySQL 不写死 sslMode' 'sslMode' $mysqlJdbc
assert_eq 'MySQL 驱动' 'com.mysql.cj.jdbc.Driver' (Get-CellJdbcDriver (Get-Cell 'ruoyi-mysql'))

$pgDirect = Get-CellJdbcUrl (Get-Cell 'ruoyi-pg')
$pgBounce = Get-CellJdbcUrl (Get-Cell 'ruoyi-pgb')
assert_match 'PG 直连端口 5432'  'jdbc:postgresql://127.0.0.1:5432/ruoyi' $pgDirect
assert_match 'PG 经 pgb 端口 6432' 'jdbc:postgresql://127.0.0.1:6432/ruoyi' $pgBounce
# 两条 URL 除端口外必须完全一致:pgbouncer 侧靠 max_prepared_statements=200
# 支持预处理语句(已实测生效),而不是让 JDBC 退化成 prepareThreshold=0。
# 后者会让 Java 侧比 Rust 侧多做一遍 parse,是不公平项。
assert_eq 'PG 两条 URL 仅端口不同' ($pgDirect -replace ':5432', ':PORT') ($pgBounce -replace ':6432', ':PORT')
assert_eq 'PG 驱动' 'org.postgresql.Driver' (Get-CellJdbcDriver (Get-Cell 'ruoyi-pg'))

section '数据库账号'
assert_eq 'MySQL 用 root'    'root'  (Get-CellDbUser (Get-Cell 'axum-mysql'))
# PG 用 ruoyi 角色而非 postgres:pgbouncer 的 userlist.txt 里有它,pgb 单元依赖
assert_eq 'PG 用 ruoyi 角色' 'ruoyi' (Get-CellDbUser (Get-Cell 'axum-pgb'))

section 'axum config.toml 生成'
$cfg = Get-CellAxumConfig (Get-Cell 'axum-pgb')
assert_match '端口 18000'       'port = 18000'            $cfg
assert_match '绑 127.0.0.1'     'host = "127.0.0.1"'      $cfg
assert_match 'db_type postgres' 'db_type = "postgres"'    $cfg
assert_match 'pgb 端口 6432'    'port = 6432'             $cfg
assert_match 'DB 池上限 20'     'db_max_connections = 20' $cfg
assert_match 'DB 池下限 10'     'db_min_connections = 10' $cfg
# Redis 池必须等于 RuoYi 的 Redisson connectionPoolSize: 32。
# 配成 16(对方一半)会让请求整齐卡在 5 秒的池等待超时上并计为错误 ——
# 实测 200rps 下 78.75% 错误率、p50 5005ms,看着像框架不行,其实是池子小一半。
assert_match 'Redis 池 32'      'redis_max_connections = 32' $cfg
assert_notmatch 'Redis 池不能是 16' 'redis_max_connections = 16' $cfg
assert_match 'redis 端口 6379'   'port = 6379'             $cfg
assert_match 'redis 无口令'      'password = ""'           $cfg
assert_match 'redis db14'       'db = 14'                 $cfg
assert_match '验证码关闭'        'enable = false'          $cfg
# 限流的真正开关是 capacity == 0(middleware/rate_limiter.rs:27 直接 return)
assert_match '限流真正关闭'      'rate_limit_capacity = 0'         $cfg
# 抬高阈值不等于关闭:中间件照常执行,每请求多付一次取连接 + INCR,
# 占 axum 每请求 9.02 次 Redis 命令中的 3 次,而 RuoYi 无此功能
assert_notmatch '不能只抬阈值'    'rate_limit_capacity = 1000000'   $cfg
# 防重复提交必须保持 5,**绝不能设 0**:中间件没有禁用路径
# (middleware/repeat_submit.rs:58),对所有 POST/PUT 无条件
# SET ... NX EX <ttl>,ttl=0 被 Redis 拒绝(invalid expire time)→
# 每个 POST/PUT 都 500。配 0 不是"关闭",是把功能弄坏。
# 5 秒正好等于 RuoYi @RepeatSubmit 的默认 interval(5000ms),两边窗口对齐。
assert_match '防重提交 5 秒'     'repeat_submit_ttl_seconds = 5'   $cfg
assert_notmatch '绝不能配成 0'   'repeat_submit_ttl_seconds = 0'   $cfg

$cfgMy = Get-CellAxumConfig (Get-Cell 'axum-mysql')
assert_match 'mysql 类型' 'db_type = "mysql"' $cfgMy
assert_match 'mysql 端口' 'port = 3306'       $cfgMy
assert_match 'mysql 账号' 'username = "root"' $cfgMy

section 'RuoYi 压测覆盖层(application-bench.yml)'
$yml = Get-CellRuoyiYaml (Get-Cell 'ruoyi-pgb')
assert_match '端口 18080'       'port: 18080'                       $yml
assert_match 'redis 端口 6379'   'port: 6379'                        $yml
assert_match 'redis db15'       'database: 15'                      $yml
# redis 口令刻意不出现在这里:它必须是 null,而 YAML 表达不出 null ——
# 空值会被绑成空字符串,Redisson 照样发 AUTH。只能让这个键完全不存在,
# 已在 application-dev.yml 里整行删除。
assert_notmatch 'redis 口令不在覆盖层' 'password: ruoyi123'          $yml
assert_notmatch '覆盖层不写空 password' "      password:`n"          $yml
assert_match 'PG 驱动'          'driverClassName: org.postgresql.Driver' $yml
assert_match 'pgb 端口 6432'    ':6432/ruoyi'                       $yml
assert_match '池上限 20'        'maxPoolSize: 20'                   $yml
assert_match 'p6spy 关闭'        'p6spy: false'                      $yml
assert_match 'sql-log 关闭'      'enabled: false'                    $yml
assert_match '日志 warn'         'org.dromara: warn'                 $yml
# 每个依赖值都必须显式写全:实测只覆盖 redis 端口、让口令继承
# application-dev.yml 会拿到 WRONGPASS(父文件口令明明是对的)。
assert_match 'redis host 也显式'  'host: 127.0.0.1'                   $yml

$ymlMy = Get-CellRuoyiYaml (Get-Cell 'ruoyi-mysql')
assert_match 'MySQL 驱动' 'driverClassName: com.mysql.cj.jdbc.Driver' $ymlMy
assert_match 'MySQL 端口' ':3306/ruoyi'                               $ymlMy
assert_match 'axum 单元不该用它但仍可生成' 'port: 18080' (Get-CellRuoyiYaml (Get-Cell 'axum-mysql'))

section '工作目录按单元隔离'
assert_match 'axum-pg 工作目录' 'axum-pg' (Get-CellWorkDir (Get-Cell 'axum-pg'))
assert_match 'axum-pgb 工作目录' 'axum-pgb' (Get-CellWorkDir (Get-Cell 'axum-pgb'))

section '监听端口必须在动态端口范围之外'
# 本机 TCP 动态端口范围是 1024-15000(默认应为 49152-65535,被改过)。
# 落在区间内的端口随时会被某条出站连接当临时端口占走,监听失败且报的是
# WSAEACCES(os error 10013)而不是「端口已被占用」,极具误导性 ——
# 实测 3000/3001 被 nekobox_core 的出站连接占住,axum 因此起不来。
$axumUrl = Get-CellBaseUrl (Get-Cell 'axum-pg')
$ruoyiUrl = Get-CellBaseUrl (Get-Cell 'ruoyi-pg')
assert_eq 'axum 端口 > 15000'  $true ([int](($axumUrl -split ':')[2] -split '/')[0] -gt 15000)
assert_eq 'ruoyi 端口 > 15000' $true ([int](($ruoyiUrl -split ':')[2] -split '/')[0] -gt 15000)

section '双基线单元 —— 只多在计数口径上,其余必须与本体逐项相同'
# 双基线的意义就在于「除了口径什么都没变」。库、端口、Redis 库号、
# BaseUrl 有任何一项不同,两组数字就不再是同一个东西的两种口径。
assert_eq '读基线共 9 个配置' 9 (Get-ReadBaselineCellIds).Count
assert_eq '原批次仍是 6 个单元' 6 (Get-CellIds).Count
assert_eq '读基线 id 列表' `
    'axum-mysql,axum-mysql-exact,axum-pg,axum-pg-exact,axum-pgb,axum-pgb-exact,ruoyi-mysql,ruoyi-pg,ruoyi-pgb' `
    ((Get-ReadBaselineCellIds | Sort-Object) -join ',')

foreach ($pair in @(@('axum-mysql', 'axum-mysql-exact'), @('axum-pg', 'axum-pg-exact'), @('axum-pgb', 'axum-pgb-exact'))) {
    $a = Get-Cell $pair[0]
    $b = Get-Cell $pair[1]
    assert_eq "$($pair[1]) 同库"        (Get-CellDbName $a)  (Get-CellDbName $b)
    assert_eq "$($pair[1]) 同端口"      $a.Port              $b.Port
    assert_eq "$($pair[1]) 同 Redis 库" (Get-CellRedisDb $a) (Get-CellRedisDb $b)
    assert_eq "$($pair[1]) 同 BaseUrl"  (Get-CellBaseUrl $a) (Get-CellBaseUrl $b)
    assert_eq "$($pair[1]) 口径为 exact"  'exact'  $b.CountMode
    assert_eq "$($pair[0]) 口径为 capped" 'capped' $a.CountMode
}

section '生成的 config.toml 带上口径'
assert_match 'exact 单元写 exact' 'count_mode = "exact"'  (Get-CellAxumConfig (Get-Cell 'axum-pg-exact'))
assert_match '默认单元写 capped'  'count_mode = "capped"' (Get-CellAxumConfig (Get-Cell 'axum-pg'))
assert_match '上限值写死 10000'   'count_cap = 10000'     (Get-CellAxumConfig (Get-Cell 'axum-pg'))

section 'exact 单元仍被各处识别为 axum 后端'
# 文件名解析、进程名映射、交替顺序都靠 'axum-' 前缀,加后缀不能破坏它们。
. (Join-Path $PSScriptRoot '../lib/sampler.ps1')
assert_eq '进程名映射' 'solidaxum' (Get-BackendProcName 'axum-pg-exact')
assert_eq 'report 的文件名正则认得' $true ('axum-pg-exact.read_mix.r100.rep1.summary.json' -match '^(?<cell>[a-z-]+)\.(?<sc>[a-z_]+)\.r(?<rate>\d+)\.rep(?<rep>\d+)\.summary\.json$')
