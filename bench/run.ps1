<#
.SYNOPSIS
  跑一轮压测:复位 → 启后端 → 门禁 → 预热 → 采样 → 停后端。
.EXAMPLE
  pwsh bench/run.ps1 -Cell axum-mysql -Scenario read_mix -Rate 300
  pwsh bench/run.ps1 -Cell ruoyi-pgb  -Scenario login    -Rate 50 -Rep 2
#>
param(
    [Parameter(Mandatory)][string]$Cell,
    [ValidateSet('read_mix', 'login', 'write_mix', 'ramp_read', 'ramp_write')][string]$Scenario = 'read_mix',
    [Parameter(Mandatory)][int]$Rate,
    [string]$Duration = '60s',
    [string]$Warmup = '60s',
    [int]$WarmupRate = 25,
    [int]$Rep = 1,
    [switch]$NoReset,       # 调试用:跳过 DB 复位
    [switch]$SkipSmoke,     # 调试用:跳过门禁 2(k6 冒烟)
    [switch]$NoAffinity     # 调试用:跳过绑核(正式轮次不要用)
)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
. (Join-Path $here 'lib/cells.ps1')
. (Join-Path $here 'lib/backends.ps1')
. (Join-Path $here 'lib/dbreset.ps1')
. (Join-Path $here 'lib/sampler.ps1')
. (Join-Path $here 'lib/gates.ps1')
. (Join-Path $here 'lib/affinity.ps1')

$c = Get-Cell $Cell
$base = Get-CellBaseUrl $c
$outDir = Join-Path $here 'results'
if (-not (Test-Path $outDir)) { New-Item -ItemType Directory -Path $outDir -Force | Out-Null }

$tag = "$($c.Id).$Scenario.r$Rate.rep$Rep"
$k6 = Get-K6Exe
$script = Join-Path $here "k6/$Scenario.js"

Write-Host "`n===== $tag =====" -ForegroundColor Cyan
Write-Host "地址=$base  库=$($c.Db)/$(Get-CellDbName $c)  速率=$Rate/s  时长=$Duration  预热=$Warmup @ $WarmupRate/s"

# 1) 状态复位(spec §5)。指纹一并落盘 —— 事后要能证明这一轮的起点是干净的。
$fingerprint = $null
if (-not $NoReset) {
    Write-Host '--- 复位数据库与 Redis ---' -ForegroundColor DarkGray
    $fingerprint = Reset-BenchState $c
}

# 2) 启后端
$handle = Start-Cell $c
$sampler = $null
try {
    # 2.5) 绑核。必须在 try 里 —— 它会抛(后端进程找不到就作废本轮),
    #      放在 try 外面 finally 的 Stop-Cell 不执行,后端会残留到下一轮。
    if (-not $NoAffinity) {
        Write-Host '--- 绑核 ---' -ForegroundColor DarkGray
        # 常驻服务的亲和性在服务重启后会丢,所以每轮都设一遍。
        # 服务侧绑核失败同样要作废本轮。设计 §4.3 的结论形式是「同样 N 核预算下
        # 两边能跑多少」,前提正是两个数据库拿到相同且受限的核集合;绑不上时它们
        # 在全部 24 核上跑、与 k6 和后端抢核,测出来的不是那个结论,而结果文件
        # 看起来完全正常。最常见的原因是会话未提权 —— 服务以服务账户运行。
        $svc = @(Set-BenchServicesAffinity)
        $svcFail = Get-AffinityFailures -Results $svc
        if ($svcFail) {
            $names = ($svcFail | ForEach-Object { "$($_.Name)($($_.Failed) 个)" }) -join ', '
            throw "[$($c.Id)] 服务绑核失败: $names —— 本轮作废。多半是会话未提权(数据库/Redis 以服务账户运行),请在管理员终端里跑批次。"
        }
        $beProc = Get-BackendProcName $c.Id
        $r = Set-ProcGroupAffinity -ProcName $beProc
        # 没绑上等于后端在全部 24 核上跑,而结果文件看起来完全正常 —— 必须炸。
        if ($r.Ok -eq 0) { throw "[$($c.Id)] 后端进程 $beProc 没找到,绑核失败 —— 本轮作废" }
        if ($r.Failed -gt 0) { throw "[$($c.Id)] 后端进程 $beProc 有 $($r.Failed) 个实例绑核失败 —— 本轮作废" }
        Write-Host "  [绑核] $beProc × $($r.Ok) → backend 组" -ForegroundColor DarkGray
    }

    # 3) 门禁 1、2(门禁 3 是跨单元的,由 matrix.ps1 统一做)
    if (-not (Test-GateHealth $c)) { throw "[$($c.Id)] 门禁1 失败 —— 标记 BLOCKED" }
    if (-not $SkipSmoke -and -not (Test-GateSmoke $c)) { throw "[$($c.Id)] 门禁2 失败 —— 标记 BLOCKED" }

    # 4) 预热(结果丢弃)。统一 60s @ 25/s:JVM 热 JIT,Rust 热连接池,
    #    两边都热 DB 缓冲池。用低速而非测试速率,避免全速打冷 JVM 造成假过载。
    # 爬坡从低速起步,前几档本身就是预热;再单独跑 60s 是白等。
    $isRamp = $Scenario -like 'ramp_*'
    if ($Warmup -ne '0' -and -not $isRamp) {
        Write-Host "--- 预热 $Warmup @ $WarmupRate/s(不计入结果)---" -ForegroundColor DarkGray
        # 预热也要绑核:发压器若在全部核上跑,预热完成度与正式采样不一致。
        $warmArgs = @(
            'run', '--quiet', '-e', "BASE_URL=$base", '-e', "RATE=$WarmupRate",
            '-e', "DURATION=$Warmup", '-e', 'TAG=_warmup', '-e', "OUTDIR=$outDir", $script
        )
        $wp = Start-Process -FilePath $k6 -ArgumentList $warmArgs -NoNewWindow -PassThru
        if (-not $NoAffinity) { Set-ProcessAffinityByGroup -Process $wp -Group 'loadgen' }
        $wp.WaitForExit()
    }

    # 5) 正式采样(资源采样器与 k6 同起同落)
    Write-Host '--- 正式采样 ---' -ForegroundColor Green
    $sampler = Start-ResourceSampler -OutCsv (Join-Path $outDir "$tag.res.csv") -IntervalMs 1000
    # 用 Start-Process 而不是 & :后者拿不到进程对象,设不了亲和性。
    $k6Args = @(
        'run', '-e', "BASE_URL=$base", '-e', "RATE=$Rate", '-e', "DURATION=$Duration",
        '-e', "TAG=$tag", '-e', "OUTDIR=$outDir", $script
    )
    $k6Proc = Start-Process -FilePath $k6 -ArgumentList $k6Args -NoNewWindow -PassThru
    if (-not $NoAffinity) { Set-ProcessAffinityByGroup -Process $k6Proc -Group 'loadgen' }
    $k6Proc.WaitForExit()
    $k6Exit = $k6Proc.ExitCode
    $agg = Stop-ResourceSampler $sampler
    $sampler = $null

    # k6 的退出码要分开看:
    #   0   正常
    #   99  阈值未达标(错误率超限)—— 这是**合法的过载结果**,要记录并标注,
    #       报告里正是靠它识别拐点,不能当成失败丢掉
    #   其他 脚本异常(setup 失败等)—— 本轮数据无意义,必须响亮地失败,
    #       否则会像上一轮那样产出一份 100% 错误率的 summary.json 被并排比较
    if ($k6Exit -ne 0 -and $k6Exit -ne 99) {
        throw "[$tag] k6 脚本异常退出(exit=$k6Exit)—— 本轮作废。看上面的 GoError 定位。"
    }
    if ($k6Exit -eq 99) {
        Write-Host "`n[注意] 阈值未达标(exit=99)—— 本轮记为过载点,延迟数据仅供参考" -ForegroundColor Yellow
    }

    # k6 自身吃太多核 → 压测机成了瓶颈,该速率点的延迟数据不可信(风险 R3)。
    # 绑核后 k6 只有 loadgen 组那么多核,阈值必须跟着组走(见 affinity.ps1
    # 的 Get-LoadGenSaturationPct)。绑核关掉时退回 sampler 原有的经验值。
    $satPct = if ($NoAffinity) { 400 } else { Get-LoadGenSaturationPct }
    $loadGenSat = [bool]($agg.k6 -and $agg.k6.CpuAvgPct -ge $satPct)
    if ($loadGenSat) {
        Write-Host "`n[警告] k6 自身平均占用 $($agg.k6.CpuAvgPct) ≥ 阈值 $satPct —— 发压侧饱和,本速率点的「极限」只是下界" -ForegroundColor Yellow
    }

    # 6) 资源汇总落盘,与 k6 结果同名关联。带上本轮的库与指纹,
    #    报告阶段不必回头猜这一轮跑的是哪个库、起点干不干净。
    @{
        Cell        = $c.Id
        Scenario    = $Scenario
        Rate        = $Rate
        Rep         = $Rep
        Engine      = $c.Db
        Database    = (Get-CellDbName $c)
        Via         = $c.Via
        Fingerprint = $fingerprint
        K6ExitCode  = $k6Exit
        Affinity    = if ($NoAffinity) { 'off' } else { 'on' }
        LoadGenSat  = $loadGenSat
        CoreGroups  = if ($NoAffinity) { $null } else {
            @{
                backend = (Get-GroupCores 'backend') -join ','
                db      = (Get-GroupCores 'db') -join ','
                loadgen = (Get-GroupCores 'loadgen') -join ','
            }
        }
        Procs       = $agg
    } | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $outDir "$tag.res.json") -Encoding utf8

    Write-Host "`n资源占用(cpu_pct 为「核数×100」,250 = 2.5 个核):" -ForegroundColor Cyan
    Write-Host (Format-SamplerAggregate $agg)
}
finally {
    if ($sampler) { Stop-ResourceSampler $sampler | Out-Null }
    Stop-Cell $handle
}
