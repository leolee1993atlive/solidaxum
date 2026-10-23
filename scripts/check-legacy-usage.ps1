# app_data::legacy 引用守卫(PowerShell 版)。
# 违反则退出非零。
#
# 语义(与 check-legacy-usage.sh 一致):
#   ① 模块文件 crates/app-data/src/legacy.rs 必须不存在;
#   ② 全仓(src + tests)零引用,无允许面。
# vendor/、target/ 显式排除。
#
# 用法: ./scripts/check-legacy-usage.ps1 [-Root <solidaxum-rs-path>]   (默认: 脚本同级的 ..\solidaxum-rs)
[CmdletBinding()]
param([string]$Root = (Join-Path $PSScriptRoot '..\solidaxum-rs'))

$ErrorActionPreference = 'Stop'

if (-not (Test-Path (Join-Path $Root 'crates'))) {
  Write-Error "找不到 $Root\crates —— 请传入 -Root 指向正确的 solidaxum-rs 路径"
}

# 零容忍第一面:模块本体必须不存在
if (Test-Path (Join-Path $Root 'crates\app-data\src\legacy.rs')) {
  # 先打明细再 Write-Error:EAP=Stop 时 Write-Error 即终止
  Write-Output "❌ crates/app-data/src/legacy.rs 仍存在 —— 该模块应已删除"
  Write-Error "❌ legacy 模块文件仍存在"
}

# 零容忍第二面:全仓(src + tests)零引用
$hits = Get-ChildItem -Path "$Root\crates" -Recurse -Filter '*.rs' -File |
  Where-Object { $_.FullName -notmatch '[\/]vendor[\/]' -and $_.FullName -notmatch '[\/]target[\/]' } |
  Select-String -Pattern 'app_data::legacy'

if ($hits) {
  # 先打明细再 Write-Error:EAP=Stop 时 Write-Error 即终止,反写会吞掉 file:line;
  # Write-Error 后不写任何代码——EAP=Stop 下即抛出终止,其后的 exit 是死码
  $hits | ForEach-Object { "{0}:{1}: {2}" -f $_.Path, $_.LineNumber, $_.Line.Trim() }
  Write-Error "❌ 检出 app_data::legacy 引用 —— legacy 模块已删除,全仓零容忍"
}

Write-Output "✅ app_data::legacy 零容忍通过:模块不存在,全仓(含 tests)零引用"
