# 数据库连接助手 —— provision.ps1 与 dbreset.ps1 共用。
#
# 抽出来的理由:凭据、可执行文件路径、库名这三样一旦在两个脚本里各写一份,
# 改一处忘一处不会报错,只会连到错的库上安静地污染一批结果。

# 客户端路径与凭据均可由环境变量覆盖,仓库里不落本机路径或口令。
$script:MysqlExe = if ($env:BENCH_MYSQL_EXE) { $env:BENCH_MYSQL_EXE } else { 'mysql.exe' }
$script:PsqlExe  = if ($env:BENCH_PSQL_EXE) { $env:BENCH_PSQL_EXE } else { 'psql.exe' }
$script:RedisCli = if ($env:BENCH_REDIS_CLI) { $env:BENCH_REDIS_CLI } else { 'redis-cli.exe' }

$script:DbName    = 'ruoyi'
$script:MysqlTpl  = 'ruoyi_tpl'     # MySQL 模板:同实例独立库
$script:PgTplSchema = 'tpl'          # PG 模板:同库内 schema(PG 不支持跨库查询)
$script:MysqlUser = if ($env:BENCH_MYSQL_USER) { $env:BENCH_MYSQL_USER } else { 'root' }
$script:MysqlPass = if ($env:BENCH_MYSQL_PASS) { $env:BENCH_MYSQL_PASS } else { 'bench_password' }
$script:PgUser    = if ($env:BENCH_PG_USER) { $env:BENCH_PG_USER } else { 'ruoyi' }
$script:PgPass    = if ($env:BENCH_PG_PASS) { $env:BENCH_PG_PASS } else { 'bench_password' }

# -N -B = 无表头 + tab 分隔,输出可直接切分,不用剥 ASCII 表格边框。
function Invoke-MysqlQ([string]$Sql, [string]$DbName = $script:DbName) {
    # 变量名不能叫 $args —— 那是 PowerShell 函数里的自动变量,赋值它的行为
    # 依版本而异,是个经典静默陷阱。
    $a = @('-u', $script:MysqlUser, "-p$script:MysqlPass", '-h', '127.0.0.1',
        '-P', '3306', '--protocol=TCP', '-N', '-B')
    if ($DbName) { $a += @('-D', $DbName) }
    $a += @('-e', $Sql)
    $out = & $script:MysqlExe @a 2>&1 | Where-Object { $_ -notmatch 'Using a password' }
    if ($LASTEXITCODE -ne 0) { throw "mysql 失败:`n$($out -join "`n")" }
    $out
}

# -t -A = 无表头 + 无对齐(不填充空格),-F 指定分隔符。
function Invoke-PgQ([string]$Sql, [string]$DbName = $script:DbName, [string]$User = $script:PgUser, [string]$Sep = "`t") {
    $env:PGPASSWORD = $script:PgPass
    $out = & $script:PsqlExe -U $User -h 127.0.0.1 -p 5432 -d $DbName -v ON_ERROR_STOP=1 -t -A -F $Sep -c $Sql 2>&1
    if ($LASTEXITCODE -ne 0) { throw "psql 失败:`n$($out -join "`n")" }
    $out
}

function Invoke-MysqlFile([string]$File, [string]$DbName = $script:DbName) {
    # mysql.exe 没有 -f <file>,只能走 stdin;用 cmd 重定向而非 PowerShell 管道 ——
    # 59MB 文件过 PowerShell 管道会逐行装箱成对象,慢几十倍。
    $out = & cmd /c "`"$script:MysqlExe`" -u $script:MysqlUser -p$script:MysqlPass -h 127.0.0.1 -P 3306 --protocol=TCP $DbName < `"$File`" 2>&1" |
        Where-Object { $_ -notmatch 'Using a password' }
    # 把 mysql 的原始报错带进异常。吞掉它的话只剩一句「导入失败」,定位一次要重跑三遍。
    if ($LASTEXITCODE -ne 0) { throw "mysql 导入失败 $File`n$($out -join "`n")" }
}

function Invoke-PgFile([string]$File, [string]$DbName = $script:DbName, [string]$User = $script:PgUser) {
    $env:PGPASSWORD = $script:PgPass
    $out = & $script:PsqlExe -U $User -h 127.0.0.1 -p 5432 -d $DbName -v ON_ERROR_STOP=1 -q -f $File 2>&1
    if ($LASTEXITCODE -ne 0) { throw "psql 导入失败 $File`:`n$($out -join "`n")" }
}

# 业务表清单从 information_schema 实时查,不硬编码。
#
# 硬编码过一次就栽了:清单里写的 sys_logininfor 在 schema 里其实叫
# sys_login_info(RuoYi 6.X 上游也已改名),还漏了 sys_job/flow_* 等
# 十几张表。实时查则永远跟着 init 脚本走,加表删表都不用改这里。
function Get-BizTables {
    param(
        [Parameter(Mandatory)][ValidateSet('mysql', 'pg')][string]$Engine,
        # PG 上有两个库(ruoyi 给 RuoYi 用 TIMESTAMP,ruoyi_tz 给 axum 用 TIMESTAMPTZ),
        # 所以库名必须能按调用方指定,不能写死。见 cells.ps1 文件头。
        [string]$DbName = $script:DbName
    )
    if ($Engine -eq 'mysql') {
        @(Invoke-MysqlQ "SELECT table_name FROM information_schema.tables WHERE table_schema='$DbName' AND table_type='BASE TABLE' ORDER BY table_name;" $DbName |
            ForEach-Object { $_.Trim() } | Where-Object { $_ })
    }
    else {
        @(Invoke-PgQ "SELECT table_name FROM information_schema.tables WHERE table_schema='public' AND table_type='BASE TABLE' ORDER BY table_name;" $DbName |
            ForEach-Object { $_.Trim() } | Where-Object { $_ })
    }
}

# 一次往返拿到全部表的行数。逐表 COUNT 要 39 次往返,复位校验每轮都做,
# 攒起来是可观的开销。
function Get-TableCounts {
    param(
        [Parameter(Mandatory)][ValidateSet('mysql', 'pg')][string]$Engine,
        [string]$DbName = $script:DbName
    )
    $tables = Get-BizTables $Engine $DbName
    if ($tables.Count -eq 0) { throw "[$Engine/$DbName] 查不到任何业务表 —— 库还没建?" }
    $prefix = if ($Engine -eq 'pg') { 'public.' } else { '' }
    $union = ($tables | ForEach-Object { "SELECT '$_' AS t, COUNT(*) AS n FROM $prefix$_" }) -join ' UNION ALL '
    $raw = if ($Engine -eq 'mysql') { Invoke-MysqlQ "$union;" $DbName } else { Invoke-PgQ "$union;" $DbName }

    $map = [ordered]@{}
    foreach ($line in $raw) {
        if (-not $line) { continue }
        $p = $line -split "`t"
        if ($p.Count -ge 2) { $map[$p[0].Trim()] = [int]$p[1].Trim() }
    }
    $map
}
