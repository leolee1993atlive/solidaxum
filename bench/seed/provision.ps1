<#
.SYNOPSIS
  建库 + 灌 schema + 灌数据集 + 建模板 + ANALYZE。幂等,重跑即重建。
.EXAMPLE
  pwsh bench/seed/provision.ps1 -Engine pg
  pwsh bench/seed/provision.ps1 -Engine mysql
#>
param(
    [Parameter(Mandatory)][ValidateSet('mysql', 'pg')][string]$Engine,
    [int]$UserCount = 200000,
    [int]$DeptCount = 100
)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
. (Join-Path $here 'gen-seed-sql.ps1')
# 连接助手统一走 lib/db.ps1 —— 凭据与可执行文件路径只有一份,不会漂移。
. (Join-Path $here '../lib/db.ps1')

$RepoRoot = Split-Path -Parent (Split-Path -Parent $here)
$SeedSql = Join-Path $here 'out/seed-data.sql'

# 压测发现的性能缺陷,就地修掉(用户裁定:测试的产出不只是对比数字,
# 发现缺陷本身就是价值)。
#
# 缺陷:sys_user.del_flag 上没有索引,而**两个后端的分页计数都过滤它**:
#   RuoYi : SELECT COUNT(*) AS total FROM sys_user WHERE del_flag='0' AND (del_flag='0')
#   axum  : SELECT COUNT(*) AS num_items FROM (SELECT <20 列> FROM sys_user WHERE del_flag='0') x
# 实测执行计划是 type=ALL 全表扫描 197132 行;一轮 1802 个请求让 MySQL 读了
# 2.4 亿行(Handler_read_rnd_next 差值),平均每请求约 13.3 万行。
#
# 补上后执行计划变成 type=ref + Using index(覆盖索引),单次计数
# 32.6ms → 26.2ms(MySQL,单会话内测,已排除客户端启动开销),约 20%。
#
# 注意两点,报告里要写清:
#   1) **两个后端等价受益** —— 是 schema 层修复,不偏向任何一方。
#   2) 它治标不治本:根本开销是「每次翻页都要数 20 万行」,没有任何索引能让
#      COUNT(*) 变便宜。真正的解法是换分页策略(游标分页 / 缓存总数 /
#      近似计数),那属于应用设计,不在压测范围内。
#
# 另外验证过一个**并非缺陷**的怀疑:sea-orm 把整个投影包进派生表再计数,
# 看着很浪费,但 MySQL 8 与 PG 都会把派生表优化掉 —— 实测两种写法耗时相同
# (MySQL 31.0 vs 32.2ms,PG 62-83 vs 63-68ms)。没验证就写进报告的话就是
# 一条假缺陷。
# 幂等:上游(修 init.sql 的那个会话)已采纳这条建议并把索引写进了脚本,
# 所以这里必须先查再建 —— 否则 MySQL 报 ERROR 1061 Duplicate key name。
# 两种情况都如实打印,便于报告里说清索引到底从哪来。
$script:DelFlagIndexName = 'idx_sys_user_del_flag'

function Add-DelFlagIndex {
    param(
        [Parameter(Mandatory)][ValidateSet('mysql', 'pg')][string]$Engine,
        [string]$DbName = 'ruoyi'
    )
    $exists = if ($Engine -eq 'mysql') {
        [int]((Invoke-MysqlQ "SELECT COUNT(*) FROM information_schema.statistics WHERE table_schema='$DbName' AND table_name='sys_user' AND index_name='$script:DelFlagIndexName';" $DbName) | Select-Object -Last 1)
    }
    else {
        [int]((Invoke-PgQ "SELECT COUNT(*) FROM pg_indexes WHERE tablename='sys_user' AND indexname='$script:DelFlagIndexName';" $DbName) | Select-Object -Last 1)
    }
    if ($exists -gt 0) {
        Write-Host "  [索引] $script:DelFlagIndexName 上游已有,跳过" -ForegroundColor DarkGray
        return
    }
    $sql = "CREATE INDEX $script:DelFlagIndexName ON sys_user(del_flag);"
    if ($Engine -eq 'mysql') { Invoke-MysqlQ $sql $DbName | Out-Null } else { Invoke-PgQ $sql $DbName | Out-Null }
    Write-Host "  [性能缺陷修复] 补 $script:DelFlagIndexName" -ForegroundColor Yellow
}

# solidaxum_init.sql(MySQL 版)有两处损坏,都是 INSERT 语句头丢失:
#
#   行 613       —— flow_instance 的孤立值元组,MySQL 报 ERROR 1064 直接中止
#   行 1089-2316 —— 一段被粘贴进来的 HTTP 响应体(Content-Type + 10 个流程定义
#                    JSON)盖掉了 sys_oper_log 的 INSERT 语句头,后面 1200 多行
#                    值元组因此全部悬空;MySQL 报 Unknown command '\'
#
# PG 版(由 tools/dm-probe 的 mysql2x 从它转换而来)顶层非语句行为 0 —— 转换器
# 把这两块静默丢掉了,实测 flow_instance / sys_oper_log 均 0 行。所以剔掉它们
# 恰好让两个引擎落到同一状态,这正是横向对比需要的。
#
# 这里不改 solidaxum-rs 的源文件(全局约束:不动后端仓库),而是导入前生成一份
# 清洗副本,并把丢弃的块**打印出来** —— 静默丢弃就成了两库数据不一致的暗雷。
#
# 判定规则:处于「顶层」(上一条语句已由 ; 收尾)时,不以合法语句关键字开头的行
# 是垃圾。多行 VALUES 的续行不会被误伤 —— 那时 INSERT 尚未收尾,不在顶层。
function Get-SanitizedInitSql([string]$SrcPath, [string]$DstPath) {
    $stmtStart = '^(--|/\*|DROP|CREATE|INSERT|SET|ALTER|UPDATE|DELETE|COMMENT|SELECT|DO|BEGIN|COMMIT|START|USE|LOCK|UNLOCK)'
    $lines = Get-Content $SrcPath
    $out = New-Object System.Collections.Generic.List[string]
    $blocks = New-Object System.Collections.Generic.List[string]
    $inStatement = $false
    $blockStart = 0
    $blockCount = 0
    $blockHead = ''

    for ($i = 0; $i -lt $lines.Count; $i++) {
        $line = $lines[$i]
        $t = $line.Trim()
        $lineNo = $i + 1

        if (-not $inStatement -and $t -and ($t -notmatch $stmtStart)) {
            if ($blockStart -eq 0) {
                $blockStart = $lineNo
                $blockHead = $t.Substring(0, [Math]::Min(60, $t.Length))
            }
            $blockCount++
            continue
        }

        if ($blockStart -gt 0) {
            $blocks.Add("    行 $blockStart-$($lineNo - 1)($blockCount 行): $blockHead ...")
            $blockStart = 0; $blockCount = 0
        }

        $out.Add($line)
        if ($t -and $t -notmatch '^--') {
            $inStatement = $true
            if ($t.EndsWith(';')) { $inStatement = $false }
        }
    }
    if ($blockStart -gt 0) { $blocks.Add("    行 $blockStart-$($lines.Count)($blockCount 行): $blockHead ...") }

    if ($blocks.Count -gt 0) {
        Write-Host "  [清洗] 丢弃 $($blocks.Count) 个顶层垃圾块(悬空值元组 / 粘贴进来的非 SQL):" -ForegroundColor Yellow
        $blocks | ForEach-Object { Write-Host $_ -ForegroundColor DarkYellow }
    }
    else {
        Write-Host '  [清洗] 无顶层垃圾块' -ForegroundColor DarkGray
    }

    [System.IO.File]::WriteAllLines($DstPath, $out, [System.Text.UTF8Encoding]::new($false))
    $DstPath
}

# PG 专用:把建表语句里的 TIMESTAMP 改成 TIMESTAMPTZ。
#
# 为什么必须改:crates/core/src/model/entity.rs:11,15 的 BaseEntity 把
# create_time / update_time 声明为 Option<DateTime<Utc>>,sqlx 的 PG 驱动
# 要求它对应 TIMESTAMPTZ;而 tools/dm-probe/src/bin/mysql2x.rs:706 把
# datetime 映射成 TIMESTAMP(无时区)。sqlx 的 MySQL 驱动对 DATETIME 宽容,
# 所以这个不匹配只在 PG 上暴露:
#     error occurred while decoding column "create_time": mismatched types;
#     Rust type Option<DateTime<Utc>> (as SQL type TIMESTAMPTZ)
#     is not compatible with SQL type TIMESTAMP
# BaseEntity 被所有实体继承 → axum 在 PG 上读不了任何业务表。
#
# **这是临时措施**。根因在 mysql2x 的类型映射,已转给修 init.sql 的那个会话。
# 上游改成 ("datetime", "TIMESTAMPTZ") 并重新生成脚本后,这里会找不到任何
# 待替换项(函数会如实报告 0 处),届时可以删掉本函数。
#
# 性能上无影响:PG 的 TIMESTAMP 与 TIMESTAMPTZ 都是 8 字节整数。
#
# 2026-09-06 更新:上游已把 mysql2x 的映射改成 TIMESTAMPTZ,所以本函数现在
# 主要用于**反向**归一 —— 给 RuoYi 的那个库要把 TIMESTAMPTZ 改回 TIMESTAMP
# (它的实体是 java.time.LocalDateTime)。函数因此保留,且做成双向归一。

# ---------------------------------------------------------------------------
# PG 专用:把列定义里的 char(n) 改成 varchar(n)。
#
# 这是压测挖出的**最严重的一处性能缺陷**,根因同样在 mysql2x 的类型映射
# (MySQL char(n) → PG char(n) = bpchar)。因果链:
#
#   1. sqlx 与 JDBC 都把字符串参数按 text 绑定
#   2. `bpchar = text` 迫使 PG 给**列**加转换 → (del_flag)::text
#   3. 列上有转换 → 索引条件退化成过滤器 → 扫完全部 20 万条索引项
#   4. 成本估算变高 → PG 选择并行查询
#   5. Windows 上并行 worker 是进程创建,Gather 开销约 150ms,
#      而实际聚合只要 10ms
#
# 实测(sys_user 的分页计数,text 参数):
#   char(1)    : Filter: ((del_flag)::text = '0'::text) + 并行  → 141-172 ms
#   varchar(1) : Index Cond: (del_flag = '0'::text),不并行     → 9.9-11.4 ms
# 端到端 axum-pg @200 迭代/s:p95 140ms → 29ms,p99 163ms → 31ms。
#
# **两个后端等价受益**:RuoYi 走 JDBC,setString 同样发 varchar/text,
# 撞的是同一堵墙。所以这是 schema 层修复,不偏向任何一方。
#
# PG 官方文档本身也不推荐 char(n):「没有任何性能优势,通常还是最慢的一种」。
function Convert-PgCharToVarchar([string]$Path) {
    $text = [System.IO.File]::ReadAllText($Path)
    # 只匹配列定义行的类型位置(行首空白 + 列名 + 空格 + char(n)),
    # 不会误伤已有的 varchar(n),也不碰字符串字面量。
    $pattern = '(?m)(^\s+\w+\s+)char\((\d+)\)'
    $count = ([regex]::Matches($text, $pattern)).Count
    if ($count -gt 0) {
        $text = [regex]::Replace($text, $pattern, '${1}varchar($2)')
        [System.IO.File]::WriteAllText($Path, $text, [System.Text.UTF8Encoding]::new($false))
    }
    Write-Host "  [性能缺陷修复] char(n) → varchar(n):$count 列(避免 bpchar=text 转换废掉索引)" -ForegroundColor Yellow
    $Path
}

function Set-PgTimestampType {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][ValidateSet('TIMESTAMP', 'TIMESTAMPTZ')][string]$Type
    )
    $text = [System.IO.File]::ReadAllText($Path)
    # 只动列定义里的时间类型,不碰字符串字面量('2026-01-01 00:00:00' 这种不含关键字)。
    # 先统一成裸 TIMESTAMP,再按需要加 TZ —— 这样无论上游当前发的是哪种都能归一。
    $norm = [regex]::Replace($text, '(?m)(^\s+\w+\s+)TIMESTAMPTZ\b', '${1}TIMESTAMP')
    $count = ([regex]::Matches($norm, '(?m)(^\s+\w+\s+)TIMESTAMP\b')).Count
    if ($Type -eq 'TIMESTAMPTZ') {
        $norm = [regex]::Replace($norm, '(?m)(^\s+\w+\s+)TIMESTAMP\b', '${1}TIMESTAMPTZ')
    }
    [System.IO.File]::WriteAllText($Path, $norm, [System.Text.UTF8Encoding]::new($false))
    $changed = if ($text -eq $norm) { '(源文件已是此类型)' } else { '(已改写)' }
    Write-Host "  [时间列归一] $count 列 → $Type $changed" -ForegroundColor Yellow
    $Path
}

# ---------- 数据集 ----------
if (-not (Test-Path $SeedSql)) {
    Write-Host "生成数据集($UserCount 用户)..." -ForegroundColor Cyan
    $r = New-SeedSql -OutPath $SeedSql -UserCount $UserCount -DeptCount $DeptCount
    Write-Host "  sha256=$($r.Sha256)" -ForegroundColor DarkGray
}
else {
    Write-Host "复用已有数据集: $SeedSql" -ForegroundColor DarkGray
}

$total = [System.Diagnostics.Stopwatch]::StartNew()

# ---------- MySQL ----------
if ($Engine -eq 'mysql') {
    Write-Host '[MySQL] drop/create ruoyi + ruoyi_tpl' -ForegroundColor Cyan
    Invoke-MysqlQ 'DROP DATABASE IF EXISTS ruoyi; DROP DATABASE IF EXISTS ruoyi_tpl;' '' | Out-Null
    Invoke-MysqlQ 'CREATE DATABASE ruoyi DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci;' '' | Out-Null
    Invoke-MysqlQ 'CREATE DATABASE ruoyi_tpl DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci;' '' | Out-Null

    Write-Host '[MySQL] 灌 schema + 种子' -ForegroundColor Cyan
    $clean = Get-SanitizedInitSql (Join-Path $RepoRoot 'solidaxum-rs/sql/solidaxum_init.sql') (Join-Path $here 'out/init-mysql.clean.sql')
    Invoke-MysqlFile $clean

    Write-Host '[MySQL] 灌数据集(20 万行)' -ForegroundColor Cyan
    Invoke-MysqlFile $SeedSql

    # 验证码不在数据库里关 —— RuoYi 6.X 读的是 captchaProperties.getEnable()
    # (PasswordAuthStrategy.java:64),即 application.yml 的 captcha.enable,
    # 靠 Spring 命令行 --captcha.enable=false 覆盖;axum 读 config.toml 的
    # [captcha] enable。init 脚本里也确实没有 sys.account.captchaEnabled 这个 key,
    # bench/README.md 里改 sys_config 的老办法对 6.X 已失效。

    Add-DelFlagIndex -Engine 'mysql'

    $tables = Get-BizTables 'mysql'
    Write-Host "[MySQL] 建模板库($($tables.Count) 张表)" -ForegroundColor Cyan
    foreach ($t in $tables) {
        Invoke-MysqlQ "CREATE TABLE ruoyi_tpl.$t LIKE ruoyi.$t; INSERT INTO ruoyi_tpl.$t SELECT * FROM ruoyi.$t;" '' | Out-Null
    }

    Write-Host '[MySQL] ANALYZE' -ForegroundColor Cyan
    Invoke-MysqlQ ('ANALYZE TABLE ' + ($tables -join ', ') + ';') | Out-Null
}

# ---------- PostgreSQL ----------
#
# PG 上建**两个库**,数据完全相同,只有时间列类型不同(用户裁定,方案 A):
#   ruoyi     —— TIMESTAMP,给 RuoYi(实体是 java.time.LocalDateTime)
#   ruoyi_tz  —— TIMESTAMPTZ,给 axum(实体是 chrono::DateTime<Utc>)
#
# 为什么必须分开:两个 ORM 对同一列要求互斥的类型,实测 TIMESTAMPTZ 库上
# RuoYi 直接起不来(Cannot convert the column of type TIMESTAMPTZ to requested
# type java.time.LocalDateTime),TIMESTAMP 库上 axum 读不了任何业务表。
#
# 为什么仍然公平:这些时间列不在任何 WHERE 条件里、也没有索引,只被序列化输出;
# PG 里两种类型都是 8 字节整数,存储/索引/比较开销相同,只在读出时多一次
# 时区旋转(每值几纳秒)。报告 §2 会如实声明这处差异。
function Initialize-PgDatabase {
    param(
        [Parameter(Mandatory)][string]$DbName,
        [switch]$TimestampTz
    )
    Write-Host "[PG:$DbName] drop/create" -ForegroundColor Cyan
    # 先踢掉残留连接(含 pgbouncer 的服务端连接),否则 DROP DATABASE 报
    # "is being accessed by other users"。pgbouncer 之后会按需重连。
    Invoke-PgQ "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname='$DbName' AND pid <> pg_backend_pid();" 'postgres' 'postgres' | Out-Null
    Invoke-PgQ "DROP DATABASE IF EXISTS $DbName;" 'postgres' 'postgres' | Out-Null
    Invoke-PgQ "CREATE DATABASE $DbName OWNER ruoyi ENCODING 'UTF8';" 'postgres' 'postgres' | Out-Null

    Write-Host "[PG:$DbName] 灌 schema + 种子" -ForegroundColor Cyan
    # 同样过一遍清洗器 —— PG 版预期丢弃 0 块,这条「无顶层垃圾块」的输出本身
    # 就是「两边导入的是同一批数据」的证据。
    $suffix = if ($TimestampTz) { 'tz' } else { 'plain' }
    $clean = Get-SanitizedInitSql (Join-Path $RepoRoot 'solidaxum-rs/sql/postgresql/solidaxum_init.sql') (Join-Path $here "out/init-pg.$suffix.sql")
    # 归一化而非单向替换:上游 2026-09-06 已把 mysql2x 的映射改成 TIMESTAMPTZ,
    # 所以「给 RuoYi 的那个库」现在反而需要往回改成 TIMESTAMP。压测声明自己
    # 要什么类型,不依赖上游当前发的是哪种。
    $clean = Set-PgTimestampType -Path $clean -Type $(if ($TimestampTz) { 'TIMESTAMPTZ' } else { 'TIMESTAMP' })
    # 两个库都要改 —— RuoYi 走 JDBC 也撞同一堵墙,这是 schema 层修复
    $clean = Convert-PgCharToVarchar $clean
    Invoke-PgFile $clean $DbName

    Write-Host "[PG:$DbName] 灌数据集(20 万行)" -ForegroundColor Cyan
    Invoke-PgFile $SeedSql $DbName

    # 验证码见 MySQL 分支的注释:不在数据库里关。

    # 布尔列修补(同为 mysql2x 映射问题,与 TIMESTAMPTZ 同源)。
    #
    # mysql2x 把 MySQL 的 tinyint 一律映射成 SMALLINT(mysql2x.rs:18),但
    # sys_role 的 menu_check_strictly / dept_check_strictly 在两个后端里都是
    # 布尔:axum 是 bool(crates/system/src/role/model/role.rs:18-19),
    # RuoYi 是 Boolean(SysRole.java:52,57)。SMALLINT 会让 axum 报
    #   Rust type Option<bool> (as SQL type BOOL) is not compatible with SQL type INT2
    #
    # **这一处两边诉求一致,改成 BOOLEAN 对 RuoYi 也是修复,不是妥协。**
    #
    # 用 ALTER 而不是改建表语句:种子数据插的是整数字面量 1/0,PG 的 BOOLEAN
    # 列不接受整数,先建 SMALLINT 灌完再转换最省事。
    Write-Host '  [临时修补] sys_role 布尔列 SMALLINT → BOOLEAN' -ForegroundColor Yellow
    # 必须先摘掉默认值:PG 不会自动把 DEFAULT '1'(整数)转成 boolean,
    # 直接 ALTER TYPE 会报「字段的默认值不能转换成类型 boolean」。
    Invoke-PgQ @'
ALTER TABLE sys_role ALTER COLUMN menu_check_strictly DROP DEFAULT;
ALTER TABLE sys_role ALTER COLUMN dept_check_strictly DROP DEFAULT;
ALTER TABLE sys_role
  ALTER COLUMN menu_check_strictly TYPE BOOLEAN USING (menu_check_strictly <> 0),
  ALTER COLUMN dept_check_strictly TYPE BOOLEAN USING (dept_check_strictly <> 0);
ALTER TABLE sys_role ALTER COLUMN menu_check_strictly SET DEFAULT true;
ALTER TABLE sys_role ALTER COLUMN dept_check_strictly SET DEFAULT true;
'@ $DbName | Out-Null

    Add-DelFlagIndex -Engine 'pg' -DbName $DbName

    $tables = Get-BizTables 'pg' $DbName
    Write-Host "[PG:$DbName] 建模板 schema($($tables.Count) 张表)" -ForegroundColor Cyan
    Invoke-PgQ 'DROP SCHEMA IF EXISTS tpl CASCADE; CREATE SCHEMA tpl;' $DbName | Out-Null
    foreach ($t in $tables) {
        # CREATE TABLE ... AS 不带索引/约束,正合适 —— 模板只用来 SELECT 回灌。
        Invoke-PgQ "CREATE TABLE tpl.$t AS SELECT * FROM public.$t;" $DbName | Out-Null
    }

    Write-Host "[PG:$DbName] ANALYZE" -ForegroundColor Cyan
    Invoke-PgQ 'ANALYZE;' $DbName | Out-Null
}

if ($Engine -eq 'pg') {
    Write-Host '[PG] 确保角色 ruoyi 存在且口令正确' -ForegroundColor Cyan
    # pgbouncer 的 userlist.txt 必须与本脚本写入的口令一致,否则 pgb 单元认证失败。
    Invoke-PgQ "DO `$`$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='ruoyi') THEN CREATE ROLE ruoyi LOGIN; END IF; END `$`$;" 'postgres' 'postgres' | Out-Null
    Invoke-PgQ "ALTER ROLE ruoyi WITH LOGIN PASSWORD '$script:PgPass' CREATEDB;" 'postgres' 'postgres' | Out-Null

    Initialize-PgDatabase -DbName 'ruoyi'                 # RuoYi 用,TIMESTAMP
    Initialize-PgDatabase -DbName 'ruoyi_tz' -TimestampTz # axum 用,TIMESTAMPTZ
}

$total.Stop()

# ---------- 校验 ----------
# PG 建了两个库,两个都要验。MySQL 只有一个。
$dbsToCheck = if ($Engine -eq 'pg') { @('ruoyi', 'ruoyi_tz') } else { @('ruoyi') }

Write-Host "`n[$Engine] 校验(建库耗时 $([math]::Round($total.Elapsed.TotalSeconds,1)) s)" -ForegroundColor Green
foreach ($db in $dbsToCheck) {
    Write-Host "  --- $db ---" -ForegroundColor Cyan
    $counts = Get-TableCounts $Engine $db
    foreach ($t in @('sys_user', 'sys_dept', 'sys_user_role', 'sys_user_post', 'sys_config')) {
        Write-Host ("    {0,-16} {1}" -f $t, $counts[$t])
    }

    # 模板与主库必须一致 —— 不一致说明建模板时漏了表或灌错了顺序
    $mismatch = 0
    $allTables = Get-BizTables $Engine $db
    foreach ($t in $allTables) {
        if ($Engine -eq 'mysql') {
            $a = (Invoke-MysqlQ "SELECT COUNT(*) FROM ruoyi.$t;" '').Trim()
            $b = (Invoke-MysqlQ "SELECT COUNT(*) FROM ruoyi_tpl.$t;" '').Trim()
        }
        else {
            $a = (Invoke-PgQ "SELECT COUNT(*) FROM public.$t;" $db).Trim()
            $b = (Invoke-PgQ "SELECT COUNT(*) FROM tpl.$t;" $db).Trim()
        }
        if ($a -ne $b) { Write-Host "    [NG] $t 主库=$a 模板=$b" -ForegroundColor Red; $mismatch++ }
    }
    if ($mismatch -eq 0) { Write-Host "    [OK] $($allTables.Count) 张表主库与模板行数一致" -ForegroundColor Green }
    else { throw "[$db] 模板不一致:$mismatch 张表" }
}

# PG 的两个库必须装着**完全相同的数据** —— 只有时间列类型不同才是预期,
# 数据有任何差异都说明建库过程出了岔子,后续对比就没有意义了。
if ($Engine -eq 'pg') {
    $d1 = (Invoke-PgQ "SELECT md5(string_agg(user_id::text||user_name||phone_number||nick_name||email||status, ',' ORDER BY user_id)) FROM sys_user WHERE user_id BETWEEN 1000001 AND 1200000;" 'ruoyi' | Select-Object -Last 1).Trim()
    $d2 = (Invoke-PgQ "SELECT md5(string_agg(user_id::text||user_name||phone_number||nick_name||email||status, ',' ORDER BY user_id)) FROM sys_user WHERE user_id BETWEEN 1000001 AND 1200000;" 'ruoyi_tz' | Select-Object -Last 1).Trim()
    if ($d1 -eq $d2) { Write-Host "`n  [OK] ruoyi 与 ruoyi_tz 数据摘要一致: $d1" -ForegroundColor Green }
    else { throw "两个 PG 库数据不一致!ruoyi=$d1 ruoyi_tz=$d2" }
}
