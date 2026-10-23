# 业务 crate 原生 SQL 归零断言(PowerShell 版)。违反则退出非零,供 Windows 本地/CI 使用。
#
# 归零范围与排除清单与 check-no-raw-sql.sh 一致(见该脚本头注释)。
# 维护记录:把通用 testkit 豁免收窄锚到 app-data testkit,
# 并补齐此前缺失的 app-data dialect/context.rs 排除(与 .sh 同口径)。
#
# 用法: ./scripts/check-no-raw-sql.ps1 [-Root <solidaxum-rs-path>]   (默认: 脚本同级的 ..\solidaxum-rs)
[CmdletBinding()]
param([string]$Root = (Join-Path $PSScriptRoot '..\solidaxum-rs'))

$ErrorActionPreference = 'Stop'

if (-not (Test-Path (Join-Path $Root 'crates'))) {
  Write-Error "找不到 $Root\crates —— 请传入 -Root 指向正确的 solidaxum-rs 路径"
  exit 2
}

$pattern = 'Statement::from_sql_and_values|Statement::from_string|Statement::from_sql_and_tuples|execute_unprepared'

$hits = Get-ChildItem -Path "$Root\crates" -Recurse -Filter '*.rs' -File |
  Select-String -Pattern $pattern |
  Where-Object {
    $_.Path -notmatch '[\\/]tests[\\/]' -and
    $_.Path -notmatch '[\\/]crates[\\/]app-data[\\/]src[\\/]testkit\.rs$' -and
    $_.Path -notmatch '[\\/]codegen[\\/]service[\\/]db_meta\.rs$' -and
    $_.Path -notmatch '[\\/]crates[\\/]dm[\\/]' -and
    $_.Path -notmatch '[\\/]crates[\\/]app-data[\\/]src[\\/]dialect[\\.\\/]' -and
    $_.Path -notmatch '[\\/]crates[\\/]app-data[\\/]src[\\/]context\.rs$'
  }

if ($hits) {
  # 先打明细再 Write-Error:EAP=Stop 时 Write-Error 即终止,反写会吞掉 file:line
  # (Write-Error 后不写任何代码——那是死码)
  $hits | ForEach-Object { "{0}:{1}: {2}" -f $_.Path, $_.LineNumber, $_.Line.Trim() }
  Write-Error "❌ 业务 crate 仍有原生 SQL(应转 SeaORM 结构化 API)"
}

Write-Output "✅ 业务 crate 原生 SQL 归零(db_meta / crates/dm / app-data testkit / tests 已排除)"
