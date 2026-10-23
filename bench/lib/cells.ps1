# 六个配置单元的唯一真源。纯函数、无副作用、可单测(bench/tests/cells.tests.ps1)。
#
# 为什么单独成文件:BaseUrl / DSN / JDBC / Redis db 一旦在多处各写一份,
# 迟早出现「改了 run.ps1 忘了改 matrix.ps1」的不一致,而这种不一致不会报错,
# 只会安静地污染一整批结果。

$script:DbHost    = '127.0.0.1'
$script:DbName    = 'ruoyi'

# PostgreSQL 上两个后端需要**互斥**的时间列类型,只能各用一个库(用户裁定,方案 A):
#   ruoyi     —— create_time 等为 TIMESTAMP,给 RuoYi(实体是 java.time.LocalDateTime,
#                读 TIMESTAMPTZ 会抛 Cannot convert the column of type TIMESTAMPTZ
#                to requested type java.time.LocalDateTime)
#   ruoyi_tz  —— **同一份数据**,时间列为 TIMESTAMPTZ,给 axum(实体是
#                chrono::DateTime<Utc>,读 TIMESTAMP 会抛 mismatched types)
#
# 两库除时间列类型外完全一致,且这些列不在任何 WHERE 条件里、也没有索引,只被
# 序列化输出。PG 里 TIMESTAMP 与 TIMESTAMPTZ 都是 8 字节整数,存储/索引/比较开销
# 相同,只在读出时多一次时区旋转(每值几纳秒)—— 对比仍然公平,但报告必须如实声明。
#
# MySQL 无此问题(两个驱动对 DATETIME 都宽容),两个后端共用同一个 ruoyi 库。
$script:PgTzDbName = 'ruoyi_tz'

# 后端监听端口。**必须选在动态端口范围之外。**
#
# 本机的 TCP 动态(临时)端口范围被改成了 1024 起、共 13977 个,即 1024-15000
# (Windows 默认是 49152-65535)。落在这个区间的端口随时可能被某条出站连接
# 当临时端口占走,监听就会失败 —— 报的是 WSAEACCES(os error 10013)
# 「以一种访问权限不允许的方式做了一个访问套接字的尝试」,而不是常见的
# 「端口已被占用」,极具误导性。
#
# 实测:批次 2-4 跑完后 3000 与 3001 被 nekobox_core 的一条出站连接占住
# (Get-NetTCPConnection 显示 LocalPort=3000 Established 到远端),axum 因此
# 起不来;批次 0 那次莫名的「8080 端口已被占用」多半也是同一原因 ——
# 8080 同样落在 1024-15000 内。
#
# 选 18000/18080:> 15000,不会被临时端口分配碰到。
$script:AxumPort  = 18000
$script:RuoyiPort = 18080
# 凭据一律从环境变量读取,仓库里不落任何口令。
$script:MysqlUser = if ($env:BENCH_MYSQL_USER) { $env:BENCH_MYSQL_USER } else { 'root' }
$script:MysqlPass = if ($env:BENCH_MYSQL_PASS) { $env:BENCH_MYSQL_PASS } else { 'bench_password' }
# PG 侧刻意用 ruoyi 角色而不是 postgres:pgbouncer 的 userlist.txt 里需要
# 有这个条目,pgb 单元的认证依赖它。
$script:PgUser    = if ($env:BENCH_PG_USER) { $env:BENCH_PG_USER } else { 'ruoyi' }
$script:PgPass    = if ($env:BENCH_PG_PASS) { $env:BENCH_PG_PASS } else { 'bench_password' }

# 共用本机 Redis(6379,无 requirepass),两个后端各用一个库。
#
# 这里绕过一大圈,机制值得记下来:Redisson 判的是 `password != null` 才发 AUTH,
# 而 **Spring 的 YAML 加载器把空值绑成空字符串,不是 null**。于是:
#   - 命令行 `--spring.data.redis.password=`  → 空串 → 照发 AUTH → 被拒
#   - 覆盖 yml 里写 `password:`               → 空串 → 照发 AUTH → 被拒
#   - 连改源文件写成 `password:` 也一样        → 空串 → 照发 AUTH → 被拒(实测)
# YAML 根本表达不出 null。唯一有效的做法是让这个键**完全不存在**。
#
# 所以直接把 RuoYi-Vue-Plus/ruoyi-admin/src/main/resources/application-dev.yml
# 里的 `password: ruoyi123` **整行删掉**(该文件留有注释说明)。删一行,
# 胜过外挂一个带口令的 Redis 实例。
$script:RedisPort = 6379
$script:RedisPass = ''

$script:Cells = @(
    @{ Id = 'axum-mysql';  Backend = 'axum';  Db = 'mysql'; Via = 'direct';    Port = 3306; CountMode = 'capped' }
    @{ Id = 'axum-pg';     Backend = 'axum';  Db = 'pg';    Via = 'direct';    Port = 5432; CountMode = 'capped' }
    @{ Id = 'axum-pgb';    Backend = 'axum';  Db = 'pg';    Via = 'pgbouncer'; Port = 6432; CountMode = 'capped' }
    @{ Id = 'ruoyi-mysql'; Backend = 'ruoyi'; Db = 'mysql'; Via = 'direct';    Port = 3306; CountMode = 'capped' }
    @{ Id = 'ruoyi-pg';    Backend = 'ruoyi'; Db = 'pg';    Via = 'direct';    Port = 5432; CountMode = 'capped' }
    @{ Id = 'ruoyi-pgb';   Backend = 'ruoyi'; Db = 'pg';    Via = 'pgbouncer'; Port = 6432; CountMode = 'capped' }
    # 「同口径」基线:除计数口径外与上面三个 axum 单元逐项相同。
    # RuoYi 侧没有第二种口径,故不设对应变体 —— 它的一份数据同时作为
    # 两组基线的对照。
    @{ Id = 'axum-mysql-exact'; Backend = 'axum'; Db = 'mysql'; Via = 'direct';    Port = 3306; CountMode = 'exact' }
    @{ Id = 'axum-pg-exact';    Backend = 'axum'; Db = 'pg';    Via = 'direct';    Port = 5432; CountMode = 'exact' }
    @{ Id = 'axum-pgb-exact';   Backend = 'axum'; Db = 'pg';    Via = 'pgbouncer'; Port = 6432; CountMode = 'exact' }
)

# 批次 0-6 的单元集合:不含 exact 变体,与第一轮完全相同。
function Get-CellIds {
    @($script:Cells | Where-Object { $_.CountMode -ne 'exact' } | ForEach-Object { $_.Id })
}

# 读场景与极限场景的配置集合(设计 §1 的双基线):
# axum 三个单元 × 两种口径 + ruoyi 三个单元 = 9。
function Get-ReadBaselineCellIds {
    @($script:Cells | ForEach-Object { $_.Id })
}

function Get-Cell([Parameter(Mandatory)][string]$Id) {
    $c = $script:Cells | Where-Object { $_.Id -eq $Id }
    if (-not $c) {
        throw "未知单元: $Id(可选: $(($script:Cells | ForEach-Object { $_.Id }) -join ', '))"
    }
    $c
}

# axum 全部端点(含 /health、/metrics)挂在 /api 前缀下 ——
# crates/admin/src/router.rs:245-248。RuoYi 的 context-path 是 /。
# 两边 BASE_URL 因此不对称;上一版 run.ps1 的默认值漏了 /api,探活必然 404。
function Get-CellBaseUrl([Parameter(Mandatory)][hashtable]$Cell) {
    if ($Cell.Backend -eq 'axum') { "http://127.0.0.1:$script:AxumPort/api" } else { "http://127.0.0.1:$script:RuoyiPort" }
}

# 探活要求真实端点返回 200,而非「端口开着就算」—— Spring 启动过程中端口
# 可能已监听但应用未就绪。
function Get-CellProbeUrl([Parameter(Mandatory)][hashtable]$Cell) {
    if ($Cell.Backend -eq 'axum') { "http://127.0.0.1:$script:AxumPort/api/health" }
    else { "http://127.0.0.1:$script:RuoyiPort/auth/code" }
}

# 压测专用库号。刻意避开 db0(ruoyi 默认)与 db10(axum 默认)——
# 扫描时两者分别有 2 / 9 个 key,属于用户其他工作,绝不能 flush。
# 复位时的 FLUSHDB 只认这两个号(见 dbreset.ps1 的 Test-BenchRedisDb)。
function Get-CellRedisDb([Parameter(Mandatory)][hashtable]$Cell) {
    if ($Cell.Backend -eq 'axum') { 14 } else { 15 }
}

function Get-CellRedisPort([Parameter(Mandatory)][hashtable]$Cell) { $script:RedisPort }
function Get-CellRedisPass([Parameter(Mandatory)][hashtable]$Cell) { $script:RedisPass }

# 该单元连哪个库。只有 PG + axum 走 ruoyi_tz,其余都是 ruoyi。见文件头说明。
function Get-CellDbName([Parameter(Mandatory)][hashtable]$Cell) {
    if ($Cell.Db -eq 'pg' -and $Cell.Backend -eq 'axum') { $script:PgTzDbName } else { $script:DbName }
}

function Get-CellDbUser([Parameter(Mandatory)][hashtable]$Cell) {
    if ($Cell.Db -eq 'mysql') { $script:MysqlUser } else { $script:PgUser }
}

function Get-CellDbPass([Parameter(Mandatory)][hashtable]$Cell) {
    if ($Cell.Db -eq 'mysql') { $script:MysqlPass } else { $script:PgPass }
}

function Get-CellJdbcDriver([Parameter(Mandatory)][hashtable]$Cell) {
    if ($Cell.Db -eq 'mysql') { 'com.mysql.cj.jdbc.Driver' } else { 'org.postgresql.Driver' }
}

function Get-CellJdbcUrl([Parameter(Mandatory)][hashtable]$Cell) {
    if ($Cell.Db -eq 'mysql') {
        # 刻意不写 useSSL / sslMode:让 Connector/J 用默认 PREFERRED,与 sqlx 的
        # 默认 ssl-mode=PREFERRED 对齐(公平性 F13)。axum 的 DSN 由
        # crates/admin/src/installer/dsn.rs:57 硬编码生成,无法注入 ssl 参数,
        # 所以只能让 Java 侧也走驱动默认值。
        'jdbc:mysql://{0}:{1}/{2}?useUnicode=true&characterEncoding=utf8&zeroDateTimeBehavior=convertToNull&serverTimezone=GMT%2B8&allowPublicKeyRetrieval=true&nullCatalogMeansCurrent=true&rewriteBatchedStatements=true' -f $script:DbHost, $Cell.Port, (Get-CellDbName $Cell)
    }
    else {
        # 直连与经 pgbouncer 用**完全相同**的 URL(仅端口不同)。
        # 事务池化下的预处理语句由 pgbouncer 的 max_prepared_statements=200 兜住
        # (1.25.1 已实测生效),不用 prepareThreshold=0 —— 后者会让 Java 侧
        # 每次都重新 parse,而 Rust 侧(sqlx)仍在用预处理语句,是不公平项。
        'jdbc:postgresql://{0}:{1}/{2}?reWriteBatchedInserts=true' -f $script:DbHost, $Cell.Port, (Get-CellDbName $Cell)
    }
}

# 生成该单元的 RuoYi 压测覆盖层(application-bench.yml)。
#
# 用法:--spring.profiles.active=dev,bench --spring.config.additional-location=file:<workdir>\
#
# 为什么用一个**全新的 profile 名** bench:jar 里没有 application-bench.yml,
# 这份外部文件就是该 profile 的唯一来源,不存在同名竞争。早先试过在
# additional-location 里放 application-dev.yml 去压 jar 内同名文件,失败了。
#
# 为什么**每个值都写全、不靠继承**:只覆盖 redis 端口、让口令继承
# application-dev.yml 时实测拿到 WRONGPASS(父文件里口令明明是对的)。
# 显式声明既绕开了这个坑,也让报告能只凭这一个文件复现配置。
#
# 唯一不在这里管的是 **redis 口令**:它必须为 null,而 YAML 表达不出 null
# (空值会被绑成空字符串,Redisson 照样发 AUTH)。只能让这个键完全不存在 ——
# 已在 application-dev.yml 里整行删除,见该文件注释。
function Get-CellRuoyiYaml([Parameter(Mandatory)][hashtable]$Cell) {
    @"
# 压测覆盖层,由 bench/lib/cells.ps1 Get-CellRuoyiYaml 生成,勿手改。
# 单元: $($Cell.Id)
#
# redis 口令刻意不在这里出现:必须是 null,而 YAML 加载器会丢弃 null 键,
# 在这里写 `password:` 等于没写。它由 application-dev.yml 置空。
server:
  port: $script:RuoyiPort
spring:
  data:
    redis:
      host: 127.0.0.1
      port: $script:RedisPort
      database: $(Get-CellRedisDb $Cell)
  datasource:
    dynamic:
      primary: master
      strict: true
      datasource:
        master:
          driverClassName: $(Get-CellJdbcDriver $Cell)
          url: $(Get-CellJdbcUrl $Cell)
          username: $(Get-CellDbUser $Cell)
          password: $(Get-CellDbPass $Cell)
      hikari:
        maxPoolSize: 20      # 与 axum 的 db_max_connections 对齐(F1)
        minIdle: 10
      p6spy: false           # dev profile 默认开,每条 SQL 同步写日志(F3)
mybatis-plus:
  sql-log:
    enabled: false           # 与 p6spy 同类的坑,dev profile 默认 true(F3)
captcha:
  enable: false              # RuoYi 6.X 读的是这里,不是 sys_config(F7)
api-decrypt:
  enabled: false             # /auth/login 带 @ApiEncrypt,不关则明文 POST 被挡 403(F14)
logging:
  level:
    org.dromara: warn        # 与 RUST_LOG=warn 对齐(F2)
"@
}

function Get-CellWorkDir([Parameter(Mandatory)][hashtable]$Cell) {
    $benchRoot = Split-Path -Parent $PSScriptRoot
    Join-Path $benchRoot ".work/$($Cell.Id)"
}

# 生成该单元的 config.toml 全文。
# axum 读 CWD 下的 config.toml(crates/admin/src/main.rs:84,File::with_name),
# 既无 --config 参数也无环境变量覆盖 —— 所以按单元切换只能靠工作目录。
function Get-CellAxumConfig([Parameter(Mandatory)][hashtable]$Cell) {
    $dbType = if ($Cell.Db -eq 'mysql') { 'mysql' } else { 'postgres' }
    @"
# 压测生成文件,勿手改。来源:bench/lib/cells.ps1 Get-CellAxumConfig
# 单元: $($Cell.Id)

[server]
port = $script:AxumPort
host = "127.0.0.1"   # 不用 "localhost":Windows 下会只绑 IPv6 ::1

[db]
db_type = "$dbType"
url = "$script:DbHost"
port = $($Cell.Port)
db_name = "$(Get-CellDbName $Cell)"
username = "$(Get-CellDbUser $Cell)"
password = "$(Get-CellDbPass $Cell)"

[redis]
url = "127.0.0.1"
port = $script:RedisPort
password = "$script:RedisPass"
db = $(Get-CellRedisDb $Cell)

[jwt]
secret = "bench-only-secret-not-for-production"
access_ttl = 3600
refresh_ttl = 604800

[log]
dir = "logs"
prefix = "axum-bench"

[pool]
db_max_connections = 20
db_min_connections = 10
# Redis 池必须对齐 RuoYi 的 Redisson connectionPoolSize: 32
# (application-dev.yml:189)。第一版写 16,正好是对方的一半 —— 与当初毁掉
# 上一轮测试的「DB 池 10 vs 20」是同一类错误。
#
# 后果不是「慢一点」而是硬失败:axum 的 Redis 池等待超时是 5 秒
# (crates/admin/src/app.rs:35,deadpool_redis::Timeouts.wait),池一耗尽,
# 请求就整齐地卡在 p50=5005ms 的墙上并计为错误 —— 实测 200rps 下
# 78.75% 错误率、p50 5005ms,看起来像「框架不行」,其实是池子配小了一半。
redis_max_connections = 32

[captcha]
enable = false
captcha_type = "math"

[security]
# 限流必须**真正关闭**,而不是把阈值抬高。
#
# 真正的开关是 capacity == 0(middleware/rate_limiter.rs:27 直接 return)。
# 第一版写 1000000,阈值是不会触发了,但中间件**照常执行**:每请求多一次
# 取连接 + INCR。实测这占掉 axum 每请求 9.02 次 Redis 命令中的 3 次
# (UNWATCH + PING + INCR)—— 而 RuoYi 的 @RateLimiter 是注解级,
# AuthController / SysUserController 上一个都没有,即**根本没有这项开销**。
#
# 让 axum 为一个对方完全没有的功能付 1/3 的 Redis 往返,是实打实的不公平。
rate_limit_capacity = 0
rate_limit_window_secs = 60

# 防重复提交保持默认 5 秒,**不要设成 0**。
# 实测:中间件没有禁用路径(crates/core/src/middleware/repeat_submit.rs:58),
# 对所有 POST/PUT 无条件执行 SET ... NX EX <ttl>,ttl=0 会被 Redis 拒绝
# (invalid expire time in 'set' command),结果是每个 POST/PUT 都 500。
# 配 0 不是"关闭",是把功能弄坏。
#
# 5 秒正好等于 RuoYi @RepeatSubmit 的默认 interval(5000ms),两边窗口对齐。
# 压测脚本靠"让每次请求体天然不同"来避免误杀,而不是靠关掉这个特性:
#   - login 场景每次用不同的压测账号(20 万个,口令都是 admin123)
#   - write_mix 的请求体带 VU+迭代号
repeat_submit_ttl_seconds = 5
xss_enabled = true
xss_exclude_paths = []
aes_key = ""

[codegen]
package_name = "crates.system"
frontend_type = "vue"
author = "SolidAxum"

[oss]
upload_body_limit = "10MB"

[app]
# 双基线(设计 §1):capped = 现状,exact = 与 RuoYi 相同的全量计数。
count_mode = "$($Cell.CountMode)"
count_cap = 10000
"@
}
