<#
.SYNOPSIS
  扫 bench/results/ 汇总所有轮次,输出对比表(可选落 Markdown)。
.EXAMPLE
  pwsh bench/report.ps1
  pwsh bench/report.ps1 -IncludeCalibration          # 连 rep0 的标定轮次一起看
  pwsh bench/report.ps1 -OutMarkdown dist/benchmark-data.md
#>
param(
    [string]$OutMarkdown,
    [switch]$IncludeCalibration   # rep0 是短时标定,默认排除(不进报告)
)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$resDir = Join-Path $here 'results'
. (Join-Path $here 'lib/sampler.ps1')
. (Join-Path $here 'lib/slo.ps1')
. (Join-Path $here 'lib/affinity.ps1')

$rows = foreach ($f in Get-ChildItem $resDir -Filter '*.summary.json' -ErrorAction SilentlyContinue) {
    # 文件名: <cell>.<scenario>.r<rate>.rep<n>.summary.json
    if ($f.Name -notmatch '^(?<cell>[a-z-]+)\.(?<sc>[a-z_]+)\.r(?<rate>\d+)\.rep(?<rep>\d+)\.summary\.json$') { continue }
    $m = $Matches
    if (-not $IncludeCalibration -and [int]$m.rep -eq 0) { continue }

    $d = Get-Content $f.FullName -Raw | ConvertFrom-Json
    $dur = $d.metrics.http_req_duration.values

    # 资源汇总(同名 .res.json)。里面还带着本轮的库名与复位指纹。
    #
    # **它同时是这一轮的有效性标记**:k6 即使 setup 失败(exit=107)也会调用
    # handleSummary 写出一个空壳 summary.json,而 run.ps1 在那种情况下会抛异常、
    # 不写 .res.json。只认 summary.json 就会把作废轮次当成 0 请求的有效数据
    # 混进对比 —— 批次 6 里 axum-pg / axum-pgb 的六轮正是如此。
    $resFile = Join-Path $resDir ($f.Name -replace '\.summary\.json$', '.res.json')
    if (-not (Test-Path $resFile)) {
        Write-Host "  [跳过] $($f.Name) 无对应 .res.json —— 该轮被 run.ps1 判定作废" -ForegroundColor DarkYellow
        continue
    }
    $res = Get-Content $resFile -Raw | ConvertFrom-Json
    $beName = Get-BackendProcName $m.cell
    $be = if ($res.Procs.$beName) { $res.Procs.$beName } else { $null }
    $dbName = if ($m.cell -like '*mysql*') { 'mysqld' } else { 'postgres' }
    $db = if ($res.Procs.$dbName) { $res.Procs.$dbName } else { $null }
    $k6 = if ($res.Procs.k6) { $res.Procs.k6 } else { $null }

    # 目标速率的单位是**迭代/秒**,而 http_reqs 数的是 HTTP 请求数。
    # read_mix 每次迭代发 1-3 个请求(列表 100% + 详情 25% + 部门 15%,
    # 平均约 1.47),所以拿 http_reqs 去比 TargetRate 会系统性判错:
    # 一个刚好达标的轮次看起来「超出目标 47%」。
    # 达标与否只能看 iterations,吞吐量才看 http_reqs。
    $iterRate = if ($d.metrics.iterations) { $d.metrics.iterations.values.rate } else { $null }
    $dropped = if ($d.metrics.dropped_iterations) { $d.metrics.dropped_iterations.values.count } else { 0 }

    # **业务成功率(k6 check)才是两个后端可比的口径。**
    #
    # http_req_failed 只看 HTTP 状态码,而两个后端表达业务失败的方式不同:
    #   axum  —— 返回非 2xx
    #   RuoYi —— 返回 HTTP 200,失败写在 body 的 code 字段里
    # 于是同一批被拒绝的请求,在 axum 记作错误、在 RuoYi 记作成功。
    # 实测(写场景 40 迭代/秒):HTTP 错误率 8.33% vs 0.00%,看着差距悬殊;
    # 而 check 通过率是 91.67% vs 91.68% —— **两边其实一模一样**。
    # 只看 HTTP 错误率就会得出完全错误的结论。
    $checks = if ($d.metrics.checks) { [math]::Round($d.metrics.checks.values.rate * 100, 2) } else { $null }

    [pscustomobject]@{
        Cell       = $m.cell
        Scenario   = $m.sc
        TargetRate = [int]$m.rate
        Rep        = [int]$m.rep
        Database   = $res.Database
        IterRate   = if ($null -ne $iterRate) { [math]::Round($iterRate, 1) } else { $null }
        Dropped    = $dropped
        RPS        = [math]::Round($d.metrics.http_reqs.values.rate, 1)
        ErrorPct   = [math]::Round($d.metrics.http_req_failed.values.rate * 100, 2)
        BizOkPct   = $checks
        P50        = [math]::Round($dur.med, 1)
        P95        = [math]::Round($dur.'p(95)', 1)
        P99        = [math]::Round($dur.'p(99)', 1)
        BeCpu      = if ($be) { $be.CpuAvgPct } else { $null }
        BeMemMax   = if ($be) { $be.WsMaxMb } else { $null }
        DbCpu      = if ($db) { $db.CpuAvgPct } else { $null }
        K6Cpu      = if ($k6) { $k6.CpuAvgPct } else { $null }
    }
}

if (-not $rows) { Write-Host '结果目录里没有可汇总的轮次。' -ForegroundColor Yellow; return }

$rows = $rows | Sort-Object Scenario, TargetRate, Cell, Rep
$rows | Format-Table -AutoSize

# ---- 两类不可信轮次,主动点名 ----
Write-Host "`n过载轮次(达成迭代率 < 目标 95%、有迭代被丢弃、或业务成功率 < 99%)—— 延迟数据不可用于并排比较:" -ForegroundColor Yellow
$bad = $rows | Where-Object {
    ($null -ne $_.IterRate -and $_.IterRate -lt ($_.TargetRate * 0.95)) -or $_.Dropped -gt 0 -or ($null -ne $_.BizOkPct -and $_.BizOkPct -lt 99)
}
if ($bad) { $bad | Format-Table Cell, Scenario, TargetRate, IterRate, Dropped, ErrorPct, BizOkPct, P99, DbCpu -AutoSize }
else { Write-Host '  (无)' -ForegroundColor Green }

# HTTP 错误率与业务成功率**说法不一致**的轮次,单独点名 —— 这类轮次若只看
# 前者会得出反向结论(axum 返回非 2xx,RuoYi 返回 200 + body code)。
$mismatch = $rows | Where-Object {
    $null -ne $_.BizOkPct -and [math]::Abs((100 - $_.BizOkPct) - $_.ErrorPct) -gt 1
}
if ($mismatch) {
    Write-Host 'HTTP 错误率与业务成功率不一致的轮次(两个后端表达失败的方式不同,以业务成功率为准):' -ForegroundColor Yellow
    $mismatch | Format-Table Cell, Scenario, TargetRate, ErrorPct, BizOkPct -AutoSize
}

# 绑核之后阈值跟着 loadgen 组走(见 affinity.ps1)。这一块在绑核跑的轮次里
# 应当为空 —— 不空说明 loadgen 组不够用,那些档的「极限」只是下界。
$satPct = Get-LoadGenSaturationPct
Write-Host "k6 自身 CPU 过高的轮次(≥ $satPct)—— 发压侧饱和,该点的「极限」只是下界(风险 R3):" -ForegroundColor Yellow
$k6hot = $rows | Where-Object { $null -ne $_.K6Cpu -and $_.K6Cpu -ge $satPct }
if ($k6hot) { $k6hot | Format-Table Cell, Scenario, TargetRate, K6Cpu -AutoSize }
else { Write-Host '  (无)' -ForegroundColor Green }

# ---- 最大稳定吞吐(设计 §3.2)----
#
# 三条件同时满足才算达标,取最后一个达标档。爬坡轮次(rep0)不参与 ——
# 它每个瞬时点样本少、资源采样对不上具体速率,只用来定阶梯档位。
$thr = Get-SloThresholds
Write-Host "`n最大稳定吞吐(业务成功率 ≥ $($thr.BizOkPct)%、达成率 ≥ $($thr.RateRatio)、p95 ≤ $($thr.P95Ms)ms):" -ForegroundColor Cyan
$ladder = $rows | Where-Object { $_.Rep -gt 0 -and $_.Scenario -in @('read_mix', 'write_mix') }
$maxTable = foreach ($g in ($ladder | Group-Object Cell, Scenario)) {
    $best = Get-MaxStableThroughput -Rows @($g.Group)
    $tried = @($g.Group | Sort-Object TargetRate | Select-Object -ExpandProperty TargetRate)
    $cell = $g.Group[0].Cell
    [pscustomobject]@{
        Cell      = $cell
        Scenario  = $g.Group[0].Scenario
        # 「口径」一列让双基线的两组数在同一张表里能分辨。
        CountMode = if ($cell -like '*-exact') { 'exact' } elseif ($cell -like 'axum-*') { 'capped' } else { 'n/a' }
        MaxStable = if ($best) { $best.IterRate } else { $null }
        AtTarget  = if ($best) { $best.TargetRate } else { $null }
        P50       = if ($best) { $best.P50 } else { $null }
        P95       = if ($best) { $best.P95 } else { $null }
        BeCpu     = if ($best) { $best.BeCpu } else { $null }
        BeMemMax  = if ($best) { $best.BeMemMax } else { $null }
        # 达标档恰好是试过的最高档 → 上限没探到,这个数是下界而不是极限。
        Probed    = if ($best -and $tried.Count -gt 0 -and $best.TargetRate -eq $tried[-1]) { '未探顶' } else { '已探顶' }
    }
}
$maxTable | Sort-Object Scenario, Cell | Format-Table -AutoSize

# 一个达标档都没有的配置要单独说 —— 空白格容易被读成「没跑」。
$none = @($maxTable | Where-Object { $null -eq $_.MaxStable })
if ($none) {
    Write-Host '以下配置在所有试过的档位上都不达标(不是没跑):' -ForegroundColor Yellow
    $none | Format-Table Cell, Scenario -AutoSize
}

# ---- 重复采样:中位数汇总 + 离散度 ----
#
# 资源列必须取**中位数而非平均值**。实测教训:批次 1 里 axum-pg 的三次
# DB CPU 是 65.0 / 464.9 / 65.2 —— rep2 被一次后台活动(几乎可以肯定是
# autovacuum)污染,平均值被拉到 198.4,直接得出「axum 让数据库多花 2.3 倍
# CPU」的错误结论,而中位数 65.0 才是真相(复测 62.1 / 65.4 印证)。
#
# 离散度也必须覆盖资源列,不能只看 p95:延迟很稳(极差 0.2-3.1%)不代表
# 资源列同样稳,那次污染在 p95 上完全看不出来。
function Get-Median { param([double[]]$Values)
    if (-not $Values -or $Values.Count -eq 0) { return $null }
    $s = $Values | Sort-Object
    if ($s.Count % 2 -eq 1) { $s[[int](($s.Count - 1) / 2)] }
    else { ($s[$s.Count / 2 - 1] + $s[$s.Count / 2]) / 2 }
}
function Get-SpreadPct { param([double[]]$Values)
    $med = Get-Median $Values
    if (-not $med -or $med -eq 0) { return 0 }
    [math]::Round((($Values | Measure-Object -Max).Maximum - ($Values | Measure-Object -Min).Minimum) / $med * 100, 1)
}

$reps = $rows | Group-Object Cell, Scenario, TargetRate | Where-Object { $_.Count -gt 1 }
if ($reps) {
    Write-Host "`n重复采样中位数(资源列取中位数,平均值会被单次后台活动污染):" -ForegroundColor Cyan
    $med = foreach ($g in $reps) {
        [pscustomobject]@{
            Cell     = $g.Group[0].Cell
            Scenario = $g.Group[0].Scenario
            Rate     = $g.Group[0].TargetRate
            N        = $g.Count
            P50      = Get-Median @($g.Group.P50)
            P95      = Get-Median @($g.Group.P95)
            P99      = Get-Median @($g.Group.P99)
            BeCpu    = Get-Median @($g.Group.BeCpu)
            BeMemMax = Get-Median @($g.Group.BeMemMax)
            DbCpu    = Get-Median @($g.Group.DbCpu)
        }
    }
    $med | Sort-Object Scenario, Rate, Cell | Format-Table -AutoSize

    Write-Host '离散度(极差 / 中位数;> 20% 说明该列噪声过大,单点数据不可信):' -ForegroundColor Cyan
    foreach ($g in $reps) {
        $sp95 = Get-SpreadPct @($g.Group.P95)
        $sBe = Get-SpreadPct @($g.Group.BeCpu)
        $sDb = Get-SpreadPct @($g.Group.DbCpu)
        $flags = @()
        if ($sp95 -gt 20) { $flags += 'p95' }
        if ($sBe -gt 20) { $flags += '后端CPU' }
        if ($sDb -gt 20) { $flags += 'DB CPU' }
        $mark = if ($flags) { "  <-- 噪声过大: $($flags -join ', ')" } else { '' }
        Write-Host ("  {0,-32} n={1}  p95 {2,5}%  后端CPU {3,6}%  DB CPU {4,6}%{5}" -f `
                $g.Name, $g.Count, $sp95, $sBe, $sDb, $mark)
    }
}

if ($OutMarkdown) {
    $md = New-Object System.Collections.Generic.List[string]
    $md.Add('# 压测原始数据')
    $md.Add('')
    $md.Add('> cpu 列口径为「核数 × 100」:250 = 2.5 个核。')
    $md.Add('')
    $md.Add('| 单元 | 库 | 场景 | 目标迭代/s | rep | 达成迭代/s | 丢弃 | HTTP req/s | 错误% | p50 | p95 | p99 | 后端CPU | 后端内存MB | DB CPU | k6 CPU |')
    $md.Add('|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|')
    foreach ($r in $rows) {
        $md.Add("| $($r.Cell) | $($r.Database) | $($r.Scenario) | $($r.TargetRate) | $($r.Rep) | $($r.IterRate) | $($r.Dropped) | $($r.RPS) | $($r.ErrorPct) | $($r.P50) | $($r.P95) | $($r.P99) | $($r.BeCpu) | $($r.BeMemMax) | $($r.DbCpu) | $($r.K6Cpu) |")
    }
    $dir = Split-Path -Parent $OutMarkdown
    if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    $md -join "`n" | Set-Content $OutMarkdown -Encoding utf8
    Write-Host "`n已写出: $OutMarkdown" -ForegroundColor Green
}
