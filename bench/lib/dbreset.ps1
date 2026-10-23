# 每轮状态复位。设计见 spec §5。
#
# 为什么必须每轮做:read_mix 写 sys_oper_log,login 每轮往 sys_login_info 写数千行,
# write_mix 直接改 sys_user。不复位则这些表跨轮单调累积,越靠后的轮次越吃亏 ——
# 而交替执行顺序(F10)只能抵消热漂移,抵消不了单调累积。
#
# 为什么不 drop/restore:DROP DATABASE(PG)与 dump 重载(MySQL)都会踢掉后端
# 连接池里的全部连接,强制后端每轮重启。按 60 轮、RuoYi 开机 60s + 预热 60s 计,
# 仅此一项就多出约 2 小时。

. (Join-Path $PSScriptRoot 'db.ps1')
. (Join-Path $PSScriptRoot 'cells.ps1')

# 压测专用 Redis 库白名单。用户的 db0(ruoyi 默认)与 db10(axum 默认)里
# 有别的数据 —— 扫描时分别有 2 / 9 个 key,绝不能 flush。
function Test-BenchRedisDb([int]$RedisDb) { $RedisDb -eq 14 -or $RedisDb -eq 15 }

# 生成复位 SQL。纯函数,可单测(bench/tests/dbreset.tests.ps1)。
function New-ResetSql {
    param(
        [Parameter(Mandatory)][ValidateSet('mysql', 'pg')][string]$Engine,
        [Parameter(Mandatory)][AllowEmptyCollection()][string[]]$Tables
    )
    if ($Tables.Count -eq 0) { throw "[$Engine] 表清单为空,拒绝生成复位 SQL" }

    $sb = [System.Text.StringBuilder]::new()

    if ($Engine -eq 'mysql') {
        # 关外键检查只是保险 —— 本 schema 无外键约束,但靠它兜底,
        # 免得将来加了约束这里悄悄失败。
        [void]$sb.AppendLine('SET FOREIGN_KEY_CHECKS=0;')
        foreach ($t in $Tables) {
            [void]$sb.AppendLine("TRUNCATE TABLE $t;")
            [void]$sb.AppendLine("INSERT INTO $t SELECT * FROM $script:MysqlTpl.$t;")
        }
        [void]$sb.AppendLine('SET FOREIGN_KEY_CHECKS=1;')
        # TRUNCATE 把表统计清零。InnoDB 的 innodb_stats_auto_recalc 是异步的,
        # 不显式 ANALYZE 就可能带着「表是空的」的统计跑压测,执行计划完全走样。
        [void]$sb.AppendLine("ANALYZE TABLE $($Tables -join ', ');")
        # MySQL 的 AUTO_INCREMENT 无需手工修:InnoDB 在带显式值的 INSERT 后
        # 会把计数器推到 max+1。PG 的序列不会,见下面的 setval。
    }
    else {
        # 一条 TRUNCATE 带全部表:比逐表 TRUNCATE 少 39 次往返,也免去顺序问题。
        $qualified = ($Tables | ForEach-Object { "public.$_" }) -join ', '
        [void]$sb.AppendLine("TRUNCATE TABLE $qualified CASCADE;")
        foreach ($t in $Tables) {
            [void]$sb.AppendLine("INSERT INTO public.$t SELECT * FROM $script:PgTplSchema.$t;")
        }
        # VACUUM 而不是只 ANALYZE —— 这是公平性问题,不是优化。
        #
        # 复位往 39 张表回灌约 60 万行。批量写入后表上积着大量待清理状态,
        # 而 autovacuum_naptime 是 60 秒 —— 于是**自动清理正好在测量窗口里跑**,
        # 抢 I/O 和 CPU,把长尾打上去。实测一轮 30 秒的读场景期间自动清理跑了
        # 3 次,而检查点是 0 次(所以不是检查点的锅)。
        #
        # MySQL 没有 autovacuum,不吃这个惩罚;不显式 VACUUM 就等于让 PG 白白
        # 背一个由复位方式造成的开销。ANALYZE 顺带做掉(VACUUM 不更新统计)。
        foreach ($t in $Tables) {
            [void]$sb.AppendLine("VACUUM (ANALYZE) public.$t;")
        }
        # 序列复位:TRUNCATE ... RESTART IDENTITY 会复位到序列的 start value(1),
        # 而模板里已有数据 —— 两者不等,下次插入就撞主键。必须 setval 到实际 max。
        # 用 pg_depend 动态找全部序列,避免将来加了新序列这里漏掉。
        [void]$sb.AppendLine(@'
DO $$
DECLARE r RECORD;
BEGIN
  FOR r IN
    SELECT c.relname AS seq, t.relname AS tbl, a.attname AS col
    FROM pg_class c
    JOIN pg_depend d ON d.objid = c.oid AND d.deptype = 'a'
    JOIN pg_class t ON t.oid = d.refobjid
    JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = d.refobjsubid
    WHERE c.relkind = 'S' AND t.relnamespace = 'public'::regnamespace
  LOOP
    EXECUTE format('SELECT setval(%L, COALESCE((SELECT MAX(%I) FROM public.%I), 0) + 1, false)',
                   r.seq, r.col, r.tbl);
  END LOOP;
END $$;
'@)
    }

    $sb.ToString()
}

# sys_user 可变列的内容摘要。
#
# 单靠行数不够:write_mix 的 PUT /system/user 只改 nick_name / email /
# phone_number / status,行数一点不动。复位若失败,纯行数指纹会照样「通过」,
# 而后续轮次实际带着上一轮的脏数据在跑。
#
# 两个引擎的表达式刻意做到结果可比(实测同为
# 9a49bd6f4cd61ed6fb42bc0e617ae246),所以这个摘要顺带也是跨引擎一致性证据。
# 只取压测数据段 [1000001, 1200000],避开种子里 admin/lmy 那两行可能的 NULL——
# MySQL 的 concat() 与 PG 的 || 碰上 NULL 都会整体返回 NULL,摘要就废了。
function Get-UserContentDigest {
    param(
        [Parameter(Mandatory)][ValidateSet('mysql', 'pg')][string]$Engine,
        # PG 上有两个库(ruoyi / ruoyi_tz),库名必须能指定。见 cells.ps1 文件头。
        [string]$DbName = $script:DbName
    )
    if ($Engine -eq 'mysql') {
        # group_concat_max_len 默认 1024,不抬高会把 20 万行截断成前 1024 字节的
        # md5 —— 两边都截断到同一处时甚至会假阳性通过。SET SESSION 是会话级,
        # 不违反「不改数据库配置」。
        (Invoke-MysqlQ @'
SET SESSION group_concat_max_len = 1073741824;
SELECT md5(group_concat(concat(user_id,user_name,phone_number,nick_name,email,status)
       ORDER BY user_id SEPARATOR ','))
FROM sys_user WHERE user_id BETWEEN 1000001 AND 1200000;
'@ | Select-Object -Last 1).Trim()
    }
    else {
        (Invoke-PgQ @'
SELECT md5(string_agg(user_id::text||user_name||phone_number||nick_name||email||status,
       ',' ORDER BY user_id))
FROM sys_user WHERE user_id BETWEEN 1000001 AND 1200000;
'@ | Select-Object -Last 1).Trim()
    }
}

# 库状态指纹 = 39 张表的行数 + sys_user 可变列内容摘要,两者一起取 md5。
# 复位前后比对它,就能证明「本轮起点与种子状态一致」——行数层面和内容层面都是。
function Get-StateFingerprint {
    param(
        [Parameter(Mandatory)][ValidateSet('mysql', 'pg')][string]$Engine,
        [string]$DbName = $script:DbName
    )
    $counts = Get-TableCounts $Engine $DbName
    $joined = (($counts.Keys | Sort-Object) | ForEach-Object { "$_=$($counts[$_])" }) -join ';'
    $joined += '|content=' + (Get-UserContentDigest $Engine $DbName)
    $md5 = [System.Security.Cryptography.MD5]::Create()
    try {
        $bytes = $md5.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($joined))
        ([BitConverter]::ToString($bytes) -replace '-', '').ToLower()
    }
    finally { $md5.Dispose() }
}

function Reset-BenchState {
    param(
        [Parameter(Mandatory)][hashtable]$Cell,
        [switch]$Quiet
    )
    $engine = $Cell.Db
    # PG 上 axum 与 RuoYi 各连一个库(ruoyi_tz / ruoyi),复位必须打到本单元
    # 实际连的那个 —— 打错库等于「复位了一个没人用的库」,而真正在跑的那个
    # 带着上一轮的脏数据,且不会有任何报错。
    $db = Get-CellDbName $Cell
    $sw = [System.Diagnostics.Stopwatch]::StartNew()

    $tables = Get-BizTables $engine $db
    $sql = New-ResetSql -Engine $engine -Tables $tables

    # SQL 走临时文件而非 -e:39 张表的复位语句加上 PG 的 DO 块有几 KB,
    # 命令行参数长度和引号转义都不可靠。
    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) "bench-reset-$engine-$db.sql"
    [System.IO.File]::WriteAllText($tmp, $sql, [System.Text.UTF8Encoding]::new($false))
    try {
        if ($engine -eq 'mysql') { Invoke-MysqlFile $tmp $db } else { Invoke-PgFile $tmp $db }
    }
    finally { Remove-Item $tmp -ErrorAction SilentlyContinue }

    # Redis:只 flush 压测专用库(db14/db15)。绝不 FLUSHALL。
    #
    # 压测与用户共用同一个 6379 实例,所以**库号白名单是唯一的防线** ——
    # db0(ruoyi 默认)和 db10(axum 默认)里有用户的数据,flush 掉就没了。
    $rdb = Get-CellRedisDb $Cell
    if (-not (Test-BenchRedisDb $rdb)) { throw "拒绝 flush 非压测库 db$rdb(白名单仅 14/15)" }
    $rport = Get-CellRedisPort $Cell
    $rpass = Get-CellRedisPass $Cell
    $cliArgs = @('-h', '127.0.0.1', '-p', $rport)
    if ($rpass) { $cliArgs += @('-a', $rpass, '--no-auth-warning') }
    $cliArgs += @('-n', $rdb, 'FLUSHDB')
    & $script:RedisCli @cliArgs | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "redis FLUSHDB :$rport db$rdb 失败" }

    $sw.Stop()
    $fp = Get-StateFingerprint $engine $db
    if (-not $Quiet) {
        Write-Host ("  复位完成 {0:N1}s  {1}/{2} {3} 张表  Redis db{4}  指纹 {5}" -f `
                $sw.Elapsed.TotalSeconds, $engine, $db, $tables.Count, $rdb, $fp.Substring(0, 12)) -ForegroundColor DarkGray
    }
    $fp
}
