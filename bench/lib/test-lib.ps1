# 单测断言框架。由 bench/test-bench.ps1 dot-source。
# 仿 scripts/installer-payloads/backend-linux/test-lib.sh —— 本仓库既有约定是
# 手写断言脚本,不引框架。Pester 本机只有 3.4.0(2016 年的版本),不值得为它装依赖。
# 本文件只定义函数,不执行测试。

$script:PASS = 0
$script:FAIL = 0

function ok([string]$desc) {
    $script:PASS++
    Write-Host "  [OK] $desc" -ForegroundColor Green
}

function no([string]$desc, [string]$expected, [string]$actual) {
    $script:FAIL++
    Write-Host "  [NG] $desc" -ForegroundColor Red
    Write-Host "    期望: $expected"
    Write-Host "    实际: $actual"
}

function assert_eq([string]$desc, $expected, $actual) {
    if ($expected -eq $actual) { ok $desc } else { no $desc "$expected" "$actual" }
}

# 子串包含。用于断言 DSN / config 文本里必须出现的片段。
# 用 [string]::Contains 而非 -like,避免 needle 里的 * ? [ ] 被当成通配符 ——
# JDBC URL 里就有 ? 和 &,用 -like 会静默匹配错。
function assert_match([string]$desc, [string]$needle, [string]$haystack) {
    if ($haystack -and $haystack.Contains($needle)) { ok $desc }
    else { no $desc "包含 '$needle'" "$haystack" }
}

function assert_notmatch([string]$desc, [string]$needle, [string]$haystack) {
    if (-not $haystack -or -not $haystack.Contains($needle)) { ok $desc }
    else { no $desc "不含 '$needle'" "(实际含有)" }
}

function assert_throws([string]$desc, [scriptblock]$block) {
    try {
        & $block | Out-Null
        no $desc '抛出异常' '正常返回'
    }
    catch { ok $desc }
}

function section([string]$title) { Write-Host "`n$title" -ForegroundColor Cyan }

# 每个测试入口末尾调用一次。退出码即整体结果。
function summary {
    Write-Host "`n$script:PASS 通过, $script:FAIL 失败"
    if ($script:FAIL -gt 0) { exit 1 }
    exit 0
}
