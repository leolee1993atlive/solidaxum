# sea-orm 依赖唯一性守卫(PowerShell 版)。违反则退出非零。
#
# 白名单与解析口径与 check-sea-orm-uniqueness.sh 一致(见该脚本头注释):
# app-data 全段(本尊)+ dm 仅 [dev-dependencies](e2e 例外);注释行不判;
# 内联表头 [dependencies.sea-orm] 同判;根 Cargo.toml 不在扫描范围。
#
# 路径说明:文件经 Get-ChildItem 枚举、crate 名取自目录名,无 grep 递归
# 输出,不存在 ugrep WIN64 反斜杠路径问题(.sh 版头注的坑在此结构不适用)。
#
# 用法: ./scripts/check-sea-orm-uniqueness.ps1 [-Root <solidaxum-rs-path>]   (默认: 脚本同级的 ..\solidaxum-rs)
[CmdletBinding()]
param([string]$Root = (Join-Path $PSScriptRoot '..\solidaxum-rs'))

$ErrorActionPreference = 'Stop'

if (-not (Test-Path (Join-Path $Root 'crates'))) {
  Write-Error "找不到 $Root\crates —— 请传入 -Root 指向正确的 solidaxum-rs 路径"
}

# 白名单:app-data 全段;dm 仅 [dev-dependencies]
function Test-Whitelisted([string]$crate, [string]$base) {
  return ($crate -eq 'app-data') -or ($crate -eq 'dm' -and $base -eq 'dev-dependencies')
}

# 枚举注意:必须用 "$Root\crates\*\Cargo.toml" 通配形态。若写成
# -Path "$Root\crates\*" -Filter 'Cargo.toml',`*` 匹配到的是 crate 目录名,
# -Filter 对目录名过滤 → 枚举结果为空 → 静默假绿(临时目录自验抓到的坑)。
$tomls = Get-ChildItem -Path "$Root\crates\*\Cargo.toml" -File
if (-not $tomls) {
  Write-Error "$Root\crates 下未发现任何 \*\Cargo.toml"
}

$hits = foreach ($toml in $tomls) {
  $crate = $toml.Directory.Name
  $section = ''
  $lineNo = 0
  foreach ($line in Get-Content $toml.FullName) {
    $lineNo++
    $t = $line.Trim()
    if ($t -eq '' -or $t.StartsWith('#')) { continue }   # 注释行不判
    if ($t -match '^\[(.+)\]$') {                        # 段头(含内联表头 [段.键])
      $section = $Matches[1]
      $parts = $section -split '\.'
      if (($parts[-1] -eq 'sea-orm' -or $parts[-1] -eq 'sea_orm') -and -not (Test-Whitelisted $crate $parts[0])) {
        '{0}:{1}: [{2}] {3}' -f $toml.FullName, $lineNo, $section, $t
      }
      continue
    }
    if ($t -match '^([^=]+)=') {                         # 「键 = 值」依赖行(dotted key 取首段)
      $key = (($Matches[1].Trim() -replace '"', '') -split '\.')[0]
      if (($key -eq 'sea-orm' -or $key -eq 'sea_orm') -and -not (Test-Whitelisted $crate $section)) {
        '{0}:{1}: [{2}] {3}' -f $toml.FullName, $lineNo, $section, $t
      }
    }
  }
}

if ($hits) {
  # 先打明细再 Write-Error:EAP=Stop 时 Write-Error 即终止,反写会吞掉 file:line
  # (Write-Error 后不写任何代码——那是死码)
  $hits | ForEach-Object { $_ }
  Write-Error "❌ 业务 crate 的 Cargo.toml 出现 sea-orm 依赖(数据访问必须经 app-data 漏斗;白名单仅:app-data 全段 / dm [dev-dependencies])"
}

Write-Output "✅ sea-orm 依赖唯一(app-data 本尊 + dm [dev-dependencies] e2e 例外)"
