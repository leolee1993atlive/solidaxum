# vendored sea-orm fork 生效性断言(PowerShell 版)。违反则退出非零。
#
# 守的是什么、为何需要:见 check-vendored-fork.sh 的头注释。
# 一句话:只改根 Cargo.toml 的版本要求而不同步 vendor,cargo **不报错**,
# 只发一条 warning 就静默回退到 crates.io 原版,达梦后端随之消失。
#
# 用法: ./scripts/check-vendored-fork.ps1 [-Root <solidaxum-rs-path>]
[CmdletBinding()]
param([string]$Root = (Join-Path $PSScriptRoot '..\solidaxum-rs'))

$ErrorActionPreference = 'Stop'

foreach ($f in @('Cargo.toml', 'Cargo.lock', 'vendor\sea-orm\Cargo.toml')) {
  if (-not (Test-Path (Join-Path $Root $f))) {
    Write-Error "找不到 $Root\$f —— 请传入 -Root 指向正确的 solidaxum-rs 路径"
    exit 2
  }
}

# ---- 断言 1:声明版本 vs vendored 版本 ----

$declaredLine = Get-Content (Join-Path $Root 'Cargo.toml') |
  Where-Object { $_ -match '^sea-orm\s*=\s*\{' } | Select-Object -First 1
$declared = if ($declaredLine -match 'version\s*=\s*"([^"]+)"') { $Matches[1] } else { '' }

$vendoredLine = Get-Content (Join-Path $Root 'vendor\sea-orm\Cargo.toml') |
  Where-Object { $_ -match '^version\s*=' } | Select-Object -First 1
$vendored = if ($vendoredLine -match '"([^"]+)"') { $Matches[1] } else { '' }

if (-not $declared -or -not $vendored) {
  Write-Error "无法解析版本(declared='$declared' vendored='$vendored')—— 脚本的解析假设可能已过时"
  exit 2
}

if ($declared -ne $vendored) {
  Write-Output "❌ sea-orm 版本不一致:"
  Write-Output "     根 Cargo.toml 声明 : $declared"
  Write-Output "     vendor/sea-orm     : $vendored"
  Write-Output ""
  Write-Output "   cargo 不会为此报错,只会静默丢弃 patch、回退到 crates.io 原版,"
  Write-Output "   达梦后端随之消失。要升级请两边同步改 —— 重新 vendor 上游源码后再改版本号,"
  Write-Output "   步骤见仓库根目录 AGENTS.md「Working with the vendored sea-orm fork」。"
  exit 1
}

# ---- 断言 2:Cargo.lock 确实解析到本地路径 ----
# path / patched 依赖在 Cargo.lock 中没有 source 字段;registry 依赖则有。

$lock = Get-Content (Join-Path $Root 'Cargo.lock')
$inBlock = $false
$block = @()
foreach ($line in $lock) {
  if ($line -match '^\[\[package\]\]') { $inBlock = $false }
  if ($line -match '^name = "sea-orm"$') { $inBlock = $true }
  if ($inBlock) { $block += $line }
}

if ($block.Count -eq 0) {
  Write-Output "❌ Cargo.lock 里找不到 sea-orm —— 依赖可能已被移除"
  exit 1
}

if ($block | Where-Object { $_ -match '^source = ' }) {
  Write-Output "❌ Cargo.lock 里的 sea-orm 来自 registry,而非 vendor/sea-orm —— patch 未生效:"
  $block | Select-Object -First 3 | ForEach-Object { "     $_" }
  Write-Output ""
  Write-Output "   跑一次 ``cargo update -p sea-orm`` 或删掉 Cargo.lock 重新解析。"
  exit 1
}

Write-Output "✅ vendored sea-orm fork 生效(版本 $vendored,Cargo.lock 解析到本地路径)"
