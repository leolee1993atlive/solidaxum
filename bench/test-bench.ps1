<#
.SYNOPSIS
  bench/ 里纯函数的单测入口。跑它,退出码即结果。
.EXAMPLE
  pwsh bench/test-bench.ps1
#>
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path

. (Join-Path $here 'lib/test-lib.ps1')

Get-ChildItem (Join-Path $here 'tests') -Filter '*.tests.ps1' | ForEach-Object {
    Write-Host "`n=== $($_.Name) ===" -ForegroundColor Yellow
    . $_.FullName
}

summary
