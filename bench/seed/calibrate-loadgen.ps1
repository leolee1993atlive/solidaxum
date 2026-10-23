<#
.SYNOPSIS
  发压侧标定:k6 绑在 loadgen 核组上,能发出多少 rps、每 1000 rps 吃几核。
.DESCRIPTION
  极限档下 k6 会自己抢 CPU,不先量清楚它的能力,测出来的「极限」可能是
  压测机的极限(设计 §4.2)。判据:发压能力必须 ≥ 预期极限的 2 倍。
  打的是后端的健康端点(不查库、不走鉴权),量的因此是 k6 与网络栈本身。
.EXAMPLE
  pwsh bench/seed/calibrate-loadgen.ps1 -Cell axum-pg -Rate 20000
#>
param(
    [string]$Cell = 'axum-pg',
    [int]$Rate = 20000,
    [string]$Duration = '60s',
    [switch]$NoAffinity
)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
. (Join-Path $here '../lib/cells.ps1')
. (Join-Path $here '../lib/backends.ps1')
. (Join-Path $here '../lib/sampler.ps1')
. (Join-Path $here '../lib/affinity.ps1')
# Get-K6Exe 在 gates.ps1 里(它顺带也管三道门禁),标定只借这一个函数。
. (Join-Path $here '../lib/gates.ps1')

$c = Get-Cell $Cell
$probe = Get-CellProbeUrl $c
$outDir = Join-Path $here '../results'
$k6 = Get-K6Exe

# 内联一个只打健康端点的脚本 —— 它不进 k6/ 目录,因为它不是压测场景,
# 只是量发压器自己的天花板。
$js = @"
import http from 'k6/http';
export const options = {
  scenarios: { probe: { executor: 'constant-arrival-rate', rate: $Rate,
    timeUnit: '1s', duration: '$Duration', preAllocatedVUs: 500, maxVUs: 6000 } },
  thresholds: {},
  summaryTrendStats: ['med', 'p(95)', 'p(99)'],
};
export default function () { http.get('$probe'); }
export function handleSummary(d) {
  return { '$($outDir -replace '\\','/')/calibrate.summary.json': JSON.stringify(d, null, 2) };
}
"@
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) 'bench-calibrate.js'
Set-Content -Path $tmp -Value $js -Encoding utf8

$handle = Start-Cell $c
$sampler = $null
try {
    if (-not $NoAffinity) {
        Set-BenchServicesAffinity
        Set-ProcGroupAffinity -ProcName (Get-BackendProcName $c.Id) | Out-Null
    }
    $sampler = Start-ResourceSampler -OutCsv (Join-Path $outDir 'calibrate.res.csv') -IntervalMs 1000
    $p = Start-Process -FilePath $k6 -ArgumentList @('run', $tmp) -NoNewWindow -PassThru
    if (-not $NoAffinity) { Set-ProcessAffinityByGroup -Process $p -Group 'loadgen' }
    $p.WaitForExit()
    $agg = Stop-ResourceSampler $sampler
    $sampler = $null

    $d = Get-Content (Join-Path $outDir 'calibrate.summary.json') -Raw | ConvertFrom-Json
    $achieved = [math]::Round($d.metrics.http_reqs.values.rate, 0)
    $k6Cpu = if ($agg.k6) { $agg.k6.CpuAvgPct } else { 0 }
    $cores = @(Get-GroupCores 'loadgen').Count

    Write-Host "`n===== 发压侧标定 =====" -ForegroundColor Cyan
    Write-Host "目标 $Rate rps → 实际达成 $achieved rps"
    Write-Host "k6 平均占用 $k6Cpu(核数×100),loadgen 组共 $cores 个逻辑处理器"
    Write-Host "占该组比例: $([math]::Round($k6Cpu / ($cores * 100) * 100, 1))%,饱和阈值 $(Get-LoadGenSaturationPct)"
    if ($achieved -gt 0) {
        Write-Host "每 1000 rps 约吃 $([math]::Round($k6Cpu / $achieved * 1000 / 100, 2)) 核"
    }
    Write-Host "`n判据:发压能力必须 ≥ 预期极限的 2 倍。达不到就回 affinity.ps1 重切核组," -ForegroundColor Yellow
    Write-Host "原则是先保发压侧、再等分被测侧,重切后两个后端、两个数据库仍各自相同。" -ForegroundColor Yellow
}
finally {
    if ($sampler) { Stop-ResourceSampler $sampler | Out-Null }
    Stop-Cell $handle
    Remove-Item $tmp -ErrorAction SilentlyContinue
}
