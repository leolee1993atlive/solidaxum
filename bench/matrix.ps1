<#
.SYNOPSIS
  批次编排。每批次跑完即停,交人看结果后再决定下一批。
.EXAMPLE
  pwsh bench/matrix.ps1 -Batch 0                        # 就绪门禁 + 短时标定
  pwsh bench/matrix.ps1 -Batch 1 -RefRate 100 -Reps 3   # 参考点重复采样
  pwsh bench/matrix.ps1 -Batch 2 -Rates 25,50,150
#>
param(
    [Parameter(Mandatory)][ValidateRange(0, 9)][int]$Batch,
    # 逗号分隔的字符串而非 [int[]]:`pwsh -File script.ps1 -Rates 1,2,3` 会把
    # 整串当成一个字符串传进来(-File 不做 PowerShell 的数组解析),声明成
    # [int[]] 会直接报 "Cannot convert value 1,2,3 to type System.Int32"。
    # 用字符串自行切分,-File 与 -Command 两种调用方式都能用。
    [string]$Rates = '25,50,100,150',
    [int]$RefRate = 50,
    [int]$Reps = 3,
    [int]$CooldownSec = 15,
    [string]$Duration = '60s',
    [int]$RampTarget = 2000,        # 读场景爬坡终点速率(迭代/秒)
    # 写场景单独一个终点:读的量级在千,写在百,共用一个终点必然有一边
    # 把全部分辨率浪费在拐点之外 —— 顶太低够不到拐点,顶太高第一档就已过载。
    [int]$RampWriteTarget = 400,    # 写场景爬坡终点速率(迭代/秒)
    [string]$RampStep = '30s',      # 每档时长
    [int]$RampSteps = 10,           # 档数
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
. (Join-Path $here 'lib/cells.ps1')
. (Join-Path $here 'lib/gates.ps1')
. (Join-Path $here 'lib/backends.ps1')
. (Join-Path $here 'lib/dbreset.ps1')
. (Join-Path $here 'lib/ladders.ps1')

$AllCells = Get-CellIds
$RateList = @($Rates -split ',' | ForEach-Object { [int]$_.Trim() } | Where-Object { $_ -gt 0 })
if ($RateList.Count -eq 0) { throw "速率阶梯为空:-Rates '$Rates'" }
Write-Host "速率阶梯: $($RateList -join ', ') 迭代/秒" -ForegroundColor DarkGray

# 交替顺序(F10):避免「先跑完 3 个 axum 再跑 3 个 ruoyi」——
# CPU 热漂移(降频)会系统性偏向后跑的一方。
function Get-AlternatingOrder([string[]]$Ids) {
    $a = @($Ids | Where-Object { $_ -like 'axum-*' })
    $r = @($Ids | Where-Object { $_ -like 'ruoyi-*' })
    $out = New-Object System.Collections.Generic.List[string]
    for ($i = 0; $i -lt [Math]::Max($a.Count, $r.Count); $i++) {
        if ($i -lt $a.Count) { $out.Add($a[$i]) }
        if ($i -lt $r.Count) { $out.Add($r[$i]) }
    }
    $out
}

function Invoke-Round {
    param([string]$CellId, [string]$Scenario, [int]$Rate, [int]$Rep, [string]$Dur = $Duration, [string]$Warmup = '60s')
    $desc = "$CellId / $Scenario / ${Rate}rps / rep$Rep / $Dur"
    if ($DryRun) { Write-Host "[DRY] $desc" -ForegroundColor DarkGray; return }
    Write-Host "`n>>> $desc" -ForegroundColor Magenta
    try {
        & (Join-Path $here 'run.ps1') -Cell $CellId -Scenario $Scenario -Rate $Rate -Rep $Rep -Duration $Dur -Warmup $Warmup
    }
    catch {
        # 单轮失败不中断整批:记下来继续,报告阶段按 BLOCKED 处理。
        # 中断整批的代价太大(前面几十分钟白跑),而缺一个点不影响其余数据。
        Write-Host "[轮次失败] $desc`n  $($_.Exception.Message.Split([char]10)[0])" -ForegroundColor Red
    }
    Write-Host "--- 冷却 ${CooldownSec}s ---" -ForegroundColor DarkGray
    Start-Sleep -Seconds $CooldownSec
}

# 逐单元档位的批次编排。顺序由 Get-LadderSchedule 决定:按档位序号推进,
# 每一轮内 axum / ruoyi 交替(设计 F10 的热漂移防护)。
function Invoke-LadderBatch {
    param(
        [Parameter(Mandatory)][string[]]$CellIds,
        [Parameter(Mandatory)][string]$Scenario
    )
    $sched = @(Get-LadderSchedule -CellIds $CellIds -Scenario $Scenario `
            -Alternate { param($ids) Get-AlternatingOrder $ids })
    Write-Host "共 $($sched.Count) 轮,逐单元档位:" -ForegroundColor DarkGray
    foreach ($id in $CellIds) {
        Write-Host "  $id : $((Get-CellLadder -CellId $id -Scenario $Scenario) -join ', ')" -ForegroundColor DarkGray
    }
    foreach ($s in $sched) {
        Invoke-Round $s.CellId $Scenario $s.Rate 1
    }
}

# 批次 0 的等价性门禁:每个 DB 路径下,axum 与 ruoyi 各采一次快照后比对
function Invoke-EquivalenceGates {
    foreach ($pair in @(@('axum-mysql', 'ruoyi-mysql'), @('axum-pg', 'ruoyi-pg'), @('axum-pgb', 'ruoyi-pgb'))) {
        Write-Host "`n--- 等价性: $($pair[0]) vs $($pair[1]) ---" -ForegroundColor Cyan
        $snaps = @{}
        foreach ($id in $pair) {
            $c = Get-Cell $id
            try {
                Reset-BenchState $c -Quiet | Out-Null
                $h = Start-Cell $c
                try {
                    if (-not (Test-GateHealth $c)) { throw '门禁1 失败' }
                    if (-not (Test-GateSmoke $c)) { throw '门禁2 失败' }
                    $snaps[$id] = Get-EquivSnapshot $c
                }
                finally { Stop-Cell $h }
            }
            catch { Write-Host "  [$id] BLOCKED: $($_.Exception.Message.Split([char]10)[0])" -ForegroundColor Red }
        }
        if ($snaps[$pair[0]] -and $snaps[$pair[1]]) {
            Test-GateEquivalence -AxumSnap $snaps[$pair[0]] -RuoyiSnap $snaps[$pair[1]] | Out-Null
        }
        else { Write-Host '  快照缺失,无法比对' -ForegroundColor Red }
    }
}

switch ($Batch) {
    0 {
        Write-Host '=== 批次 0:就绪门禁 + 短时标定 ===' -ForegroundColor Cyan
        Invoke-EquivalenceGates
        # 短时标定:20s 采样、20s 预热,只为选参考速率,**不进报告**(rep0)
        Write-Host "`n--- 短时标定(20s,结果不进报告)---" -ForegroundColor Cyan
        foreach ($id in (Get-AlternatingOrder $AllCells)) {
            foreach ($r in $RateList) { Invoke-Round $id 'read_mix' $r 0 '20s' '20s' }
        }
    }
    1 {
        Write-Host "=== 批次 1:参考点重复采样 @ $RefRate rps × $Reps 次 ===" -ForegroundColor Cyan
        for ($rep = 1; $rep -le $Reps; $rep++) {
            foreach ($id in (Get-AlternatingOrder $AllCells)) { Invoke-Round $id 'read_mix' $RefRate $rep }
        }
    }
    2 { foreach ($r in ($RateList | Where-Object { $_ -ne $RefRate })) { foreach ($id in (Get-AlternatingOrder @('axum-mysql', 'ruoyi-mysql'))) { Invoke-Round $id 'read_mix' $r 1 } } }
    3 { foreach ($r in ($RateList | Where-Object { $_ -ne $RefRate })) { foreach ($id in (Get-AlternatingOrder @('axum-pg', 'ruoyi-pg'))) { Invoke-Round $id 'read_mix' $r 1 } } }
    4 { foreach ($r in ($RateList | Where-Object { $_ -ne $RefRate })) { foreach ($id in (Get-AlternatingOrder @('axum-pgb', 'ruoyi-pgb'))) { Invoke-Round $id 'read_mix' $r 1 } } }
    5 { foreach ($r in $RateList) { foreach ($id in (Get-AlternatingOrder $AllCells)) { Invoke-Round $id 'login' $r 1 } } }
    6 { foreach ($r in $RateList) { foreach ($id in (Get-AlternatingOrder $AllCells)) { Invoke-Round $id 'write_mix' $r 1 } } }
    7 {
        Write-Host '=== 批次 7:爬坡定区间(结果不进报告正文)===' -ForegroundColor Cyan
        # 总时长 = 档数 × 每档时长。爬坡轮次记 rep0,与批次 0 的短时标定同例 ——
        # report.ps1 默认跳过 rep0,不会混进正文。
        $stepSec = [int]($RampStep -replace '\D', '')
        $rampDur = "$($RampSteps * $stepSec)s"
        Write-Host "读爬坡:0 → $RampTarget 迭代/秒,$RampSteps 档 × $RampStep = $rampDur" -ForegroundColor DarkGray
        Write-Host "写爬坡:0 → $RampWriteTarget 迭代/秒,同档数与档时长" -ForegroundColor DarkGray
        foreach ($id in (Get-AlternatingOrder (Get-ReadBaselineCellIds))) {
            Invoke-Round $id 'ramp_read' $RampTarget 0 $rampDur '0'
        }
        foreach ($id in (Get-AlternatingOrder $AllCells)) {
            Invoke-Round $id 'ramp_write' $RampWriteTarget 0 $rampDur '0'
        }
    }
    8 {
        Write-Host '=== 批次 8:读 · 阶梯定数(双基线,9 配置)===' -ForegroundColor Cyan
        # 档位逐单元取(lib/ladders.ps1):九个配置的天花板跨度近 10 倍,
        # 一组统一档位必然两头落空 —— 最快的全部通过只能标「未探顶」,
        # 最慢的一档都不达标。-Rates 对本批次不起作用。
        Invoke-LadderBatch -CellIds (Get-ReadBaselineCellIds) -Scenario 'read_mix'
    }
    9 {
        Write-Host '=== 批次 9:写 · 阶梯定数(6 单元)===' -ForegroundColor Cyan
        Invoke-LadderBatch -CellIds $AllCells -Scenario 'write_mix'
    }
}

Write-Host "`n批次 $Batch 完成。汇总:pwsh bench/report.ps1" -ForegroundColor Green
