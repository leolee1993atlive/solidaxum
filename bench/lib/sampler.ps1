# 资源采样器。每 IntervalMs 采一次被测后端 / 数据库 / pgbouncer / k6 的
# CPU 与内存,落 CSV,与 k6 结果同名关联。
#
# 为什么这是报告里最有说服力的一列:「同样 300 RPS,Rust 用 0.8 核 / 90MB,
# JVM 用 3.2 核 / 1.4GB」比裸 RPS 更能说明问题,且不受「谁先撞到 CPU 墙」干扰。
# 同时它也是判断 k6 是否成为瓶颈的唯一依据(风险 R3)。
#
# 数据源:Win32_PerfFormattedData_PerfProc_Process(CIM 性能数据类)。
#
# 为什么不是 Get-Process 的 CPU 累计秒差分(第一版用的就是它):
# **它读不到别的账号下的进程**。mysqld / postgres / redis-server 都跑在
# Windows 服务账号下,Get-Process 对它们的 CPU 属性一律返回 0(不是 null,
# 所以连报错都没有)—— 实测在 MySQL 满负载时仍然是 0,而报告里最关键的
# 「同等 RPS 下各自吃多少 CPU」正需要 DB 那一列。
#
# 为什么不是 Get-Counter:'\Process(name)\% Processor Time' 的计数器路径在
# 中文系统上会本地化,且要自己处理 name#1/name#2 的多实例后缀。CIM 类用的是
# 英文类名/属性名,不受区域设置影响,一次查询就拿到全部进程的 CPU 与内存。
# (两者实测都能读到服务进程:满负载时分别报 75.7% 与 71%。)
#
# cpu_pct 的口径:**核数 × 100**,与该类的 PercentProcessorTime 语义一致
# (跨线程累加,多核进程会超过 100)。250 表示占用 2.5 个核。
# 不用「占全机百分比」是因为本机 24 线程,那个口径下所有数字都挤在个位数,
# 两个后端的差距看不出来。

# 关心的进程名 → 角色。postgres/mysqld 有多个子进程,按名聚合。
$script:WatchProcs = @{
    'solidaxum' = 'backend'
    'java'          = 'backend'
    'mysqld'        = 'db'
    'postgres'      = 'db'
    'pgbouncer'     = 'proxy'
    'k6'            = 'loadgen'
    'redis-server'  = 'cache'
}

# k6 占到 4 个核以上就认为压测机自身成了瓶颈,该速率点的延迟数据不可信。
$script:LoadGenSaturationPct = 400

function Get-BackendProcName([Parameter(Mandatory)][string]$CellId) {
    if ($CellId -like 'axum-*') { 'solidaxum' } else { 'java' }
}

function Test-LoadGenSaturated([double]$CpuPct) { $CpuPct -ge $script:LoadGenSaturationPct }

# 把采样行聚合成 proc → 统计。纯函数,可单测。
function Get-SamplerAggregate($Rows) {
    $agg = @{}
    if (-not $Rows -or @($Rows).Count -eq 0) { return $agg }
    foreach ($g in (@($Rows) | Group-Object proc)) {
        $cpu = @($g.Group | ForEach-Object { [double]$_.cpu_pct })
        $ws = @($g.Group | ForEach-Object { [double]$_.ws_mb })
        $agg[$g.Name] = @{
            Role      = $g.Group[0].role
            CpuAvgPct = [math]::Round(($cpu | Measure-Object -Average).Average, 1)
            CpuMaxPct = [math]::Round(($cpu | Measure-Object -Maximum).Maximum, 1)
            WsAvgMb   = [math]::Round(($ws | Measure-Object -Average).Average, 1)
            WsMaxMb   = [math]::Round(($ws | Measure-Object -Maximum).Maximum, 1)
            Samples   = $g.Count
        }
    }
    $agg
}

function Start-ResourceSampler {
    param(
        [Parameter(Mandatory)][string]$OutCsv,
        [int]$IntervalMs = 1000
    )

    $dir = Split-Path -Parent $OutCsv
    if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }

    $job = Start-Job -ArgumentList $OutCsv, $IntervalMs, $script:WatchProcs -ScriptBlock {
        param($OutCsv, $IntervalMs, $Watch)

        # 数字一律走 F1 + InvariantCulture,**绝不能用 N1**:
        # N1 会插千位分隔符,22987.7 变成 "22,987.7",直接把一列劈成两列 ——
        # 而恰恰是最关键的两个值会超过 1000(可用内存约 22000MB、RuoYi 的
        # -Xmx2g 常驻 1400MB+),corrupt 的正好是要拿来做结论的数据。
        # InvariantCulture 另外挡住区域设置把小数点写成逗号的情况。
        $inv = [cultureinfo]::InvariantCulture
        $fmt = { param($v) ([double]$v).ToString('F1', $inv) }

        'ts,proc,role,cpu_pct,ws_mb' | Out-File -FilePath $OutCsv -Encoding utf8

        while ($true) {
            Start-Sleep -Milliseconds $IntervalMs
            $now = Get-Date

            $lines = New-Object System.Collections.Generic.List[string]
            try {
                # 一次拿到全部进程。Name 形如 postgres、postgres#1、postgres#2 ——
                # 按 # 前的基名聚合,postgres 的十几个子进程算作一个。
                $all = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -ErrorAction Stop |
                    Where-Object { $_.Name -ne '_Total' -and $_.Name -ne 'Idle' }
                $byBase = $all | Group-Object { ($_.Name -split '#')[0] }
            }
            catch { $byBase = @() }

            foreach ($g in $byBase) {
                $role = $Watch[$g.Name]
                if (-not $role) { continue }
                $cpu = ($g.Group | Measure-Object PercentProcessorTime -Sum).Sum
                $ws = ($g.Group | Measure-Object WorkingSet -Sum).Sum
                $lines.Add(('{0:o},{1},{2},{3},{4}' -f $now, $g.Name, $role,
                        (& $fmt $cpu), (& $fmt ($ws / 1MB))))
            }

            # 系统总览:CPU 用全机百分比(与进程行的口径不同,列名已区分)
            try {
                $sysCpu = (Get-CimInstance Win32_Processor -ErrorAction Stop |
                        Measure-Object -Property LoadPercentage -Average).Average
                $freeMb = (Get-CimInstance Win32_OperatingSystem -ErrorAction Stop).FreePhysicalMemory / 1KB
                $lines.Add(('{0:o},_system,system,{1},{2}' -f $now, (& $fmt $sysCpu), (& $fmt $freeMb)))
            }
            catch { }

            if ($lines.Count -gt 0) { $lines | Add-Content -Path $OutCsv -Encoding utf8 }
        }
    }

    @{ Job = $job; Csv = $OutCsv }
}

# CSV 完整性守卫:每行必须恰好 5 个字段,数值字段必须能解析成数字。
#
# 加它的理由:第一版用 N1 格式化,千位分隔符把 22987.7 写成 "22,987.7",
# 一列劈成两列。这种损坏的外在表现只是「可用内存莫名其妙只有 20MB」——
# 不 assert 的话会一路带进报告,而受害的恰恰是最关键的两个值
# (可用内存、RuoYi 的 -Xmx2g 常驻内存)。
function Test-SamplerCsvWellFormed([Parameter(Mandatory)][string]$Path) {
    $bad = New-Object System.Collections.Generic.List[string]
    $lineNo = 0
    foreach ($line in (Get-Content $Path)) {
        $lineNo++
        if (-not $line) { continue }
        $f = $line -split ','
        if ($f.Count -ne 5) { $bad.Add("行 $lineNo 有 $($f.Count) 个字段(应为 5): $line"); continue }
        if ($lineNo -eq 1) { continue }   # 表头
        $n = 0.0
        if (-not [double]::TryParse($f[3], [ref]$n)) { $bad.Add("行 $lineNo cpu_pct 不是数字: $($f[3])") }
        if (-not [double]::TryParse($f[4], [ref]$n)) { $bad.Add("行 $lineNo ws_mb 不是数字: $($f[4])") }
    }
    @{ Ok = ($bad.Count -eq 0); Problems = $bad }
}

function Stop-ResourceSampler([Parameter(Mandatory)][hashtable]$Handle) {
    Stop-Job -Job $Handle.Job -ErrorAction SilentlyContinue
    Remove-Job -Job $Handle.Job -Force -ErrorAction SilentlyContinue

    if (-not (Test-Path $Handle.Csv)) { return @{} }

    $check = Test-SamplerCsvWellFormed $Handle.Csv
    if (-not $check.Ok) {
        throw "采样 CSV 损坏($($Handle.Csv)):`n" + (($check.Problems | Select-Object -First 5) -join "`n")
    }

    Get-SamplerAggregate (Import-Csv $Handle.Csv)
}

# 把聚合结果打成人读的表。run.ps1 与 report.ps1 共用,避免两处格式不一。
function Format-SamplerAggregate([hashtable]$Agg) {
    if (-not $Agg -or $Agg.Count -eq 0) { return '  (无采样数据)' }
    $lines = New-Object System.Collections.Generic.List[string]
    foreach ($k in ($Agg.Keys | Sort-Object)) {
        $a = $Agg[$k]
        $lines.Add(('  {0,-14} {1,-8} CPU 均 {2,7} 峰 {3,7}   内存 均 {4,8}MB 峰 {5,8}MB  ({6} 点)' -f `
                    $k, $a.Role, $a.CpuAvgPct, $a.CpuMaxPct, $a.WsAvgMb, $a.WsMaxMb, $a.Samples))
    }
    $lines -join "`n"
}
