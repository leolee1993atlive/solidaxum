<#
.SYNOPSIS
  并排对比两个后端最近一次压测的关键指标。
  先分别 run.ps1 打过 axum 和 ruoyi，再跑这个。

.EXAMPLE
  ./bench/compare.ps1
#>
param(
  [string]$A = 'axum',
  [string]$B = 'ruoyi'
)

$here = Split-Path -Parent $MyInvocation.MyCommand.Path

function Get-Metrics($tag) {
  $f = Join-Path $here "results/$tag.summary.json"
  if (-not (Test-Path $f)) { throw "缺少 $f —— 先用 run.ps1 打一遍 $tag" }
  $d = Get-Content $f -Raw | ConvertFrom-Json
  $dur = $d.metrics.http_req_duration.values
  [pscustomobject]@{
    Backend   = $tag
    RPS       = [math]::Round($d.metrics.http_reqs.values.rate, 1)
    ErrorPct  = [math]::Round($d.metrics.http_req_failed.values.rate * 100, 2)
    Avg_ms    = [math]::Round($dur.avg, 2)
    P50_ms    = [math]::Round($dur.med, 2)
    P95_ms    = [math]::Round($dur.'p(95)', 2)
    P99_ms    = [math]::Round($dur.'p(99)', 2)
    Max_ms    = [math]::Round($dur.max, 2)
  }
}

$rows = @(Get-Metrics $A; Get-Metrics $B)
$rows | Format-Table -AutoSize

# 相对差（以 A 为基准）
$ra = $rows[0]; $rb = $rows[1]
Write-Host "`n相对 $($ra.Backend)（基准）：" -ForegroundColor Cyan
$ratio = { param($x, $y) if ($y -eq 0) { 'n/a' } else { "{0:N2}x" -f ($x / $y) } }
Write-Host ("  吞吐 RPS:  {0} 是 {1} 的 {2}" -f $rb.Backend, $ra.Backend, (& $ratio $rb.RPS $ra.RPS))
Write-Host ("  P95 延迟:  {0} 是 {1} 的 {2}" -f $rb.Backend, $ra.Backend, (& $ratio $rb.P95_ms $ra.P95_ms))
Write-Host ("  P99 延迟:  {0} 是 {1} 的 {2}" -f $rb.Backend, $ra.Backend, (& $ratio $rb.P99_ms $ra.P99_ms))
