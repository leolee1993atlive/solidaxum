# DbBackend::Dm 唯一性守卫(PowerShell 版)。违反则退出非零。
#
# 白名单与口径与 check-dm-backend-usage.sh 一致(见该脚本头注释,含 meta.rs
# 口径修正披露:设计原文仅 dialect,实况补 MetadataReader 4 处合法消费):
# app-data dialect.rs / meta.rs / app-data tests / dm tests;vendor、target 排除。
#
# 用法: ./scripts/check-dm-backend-usage.ps1 [-Root <solidaxum-rs-path>]   (默认: 脚本同级的 ..\solidaxum-rs)
[CmdletBinding()]
param([string]$Root = (Join-Path $PSScriptRoot '..\solidaxum-rs'))

$ErrorActionPreference = 'Stop'

if (-not (Test-Path (Join-Path $Root 'crates'))) {
  Write-Error "找不到 $Root\crates —— 请传入 -Root 指向正确的 solidaxum-rs 路径"
}

$hits = Get-ChildItem -Path "$Root\crates" -Recurse -Filter '*.rs' -File |
  Where-Object { $_.FullName -notmatch '[\\/]vendor[\\/]' -and $_.FullName -notmatch '[\\/]target[\\/]' } |
  Select-String -Pattern 'DatabaseBackend::Dm|DbBackend::Dm' |
  Where-Object {
    $_.Path -notmatch '[\\/]crates[\\/]app-data[\\/]src[\\/]dialect\.rs$' -and
    $_.Path -notmatch '[\\/]crates[\\/]app-data[\\/]src[\\/]meta\.rs$' -and
    $_.Path -notmatch '[\\/]crates[\\/]app-data[\\/]tests[\\/]' -and
    $_.Path -notmatch '[\\/]crates[\\/]dm[\\/]tests[\\/]'
  }

if ($hits) {
  # 先打明细再 Write-Error:EAP=Stop 时 Write-Error 即终止,反写会吞掉 file:line
  # (Write-Error 后不写任何代码——那是死码)
  $hits | ForEach-Object { "{0}:{1}: {2}" -f $_.Path, $_.LineNumber, $_.Line.Trim() }
  Write-Error "❌ DbBackend::Dm 出现在白名单之外(达梦分支收编点仅:app-data dialect/meta / app-data tests / dm tests)"
}

Write-Output "✅ DbBackend::Dm 使用合规(允许面:app-data dialect+meta(MetadataReader,口径修正)/ app-data tests / dm tests)"
