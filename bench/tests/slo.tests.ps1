# slo.ps1 的单测。由 bench/test-bench.ps1 dot-source(断言函数已在作用域内)。
. (Join-Path $PSScriptRoot '../lib/slo.ps1')

section '三条件的边界值必须算达标(阈值是 ≥ / ≤,不是 > / <)'
assert_eq '恰好 99% 业务成功率' $true (Test-RoundMeetsSlo -BizOkPct 99.0 -IterRate 100 -TargetRate 100 -P95Ms 10).Pass
assert_eq '恰好 95% 达成率'     $true (Test-RoundMeetsSlo -BizOkPct 100 -IterRate 95 -TargetRate 100 -P95Ms 10).Pass
assert_eq '恰好 200ms p95'      $true (Test-RoundMeetsSlo -BizOkPct 100 -IterRate 100 -TargetRate 100 -P95Ms 200.0).Pass

section '各差一点即不达标'
assert_eq '98.99% 业务成功率' $false (Test-RoundMeetsSlo -BizOkPct 98.99 -IterRate 100 -TargetRate 100 -P95Ms 10).Pass
assert_eq '94.9% 达成率'      $false (Test-RoundMeetsSlo -BizOkPct 100 -IterRate 94.9 -TargetRate 100 -P95Ms 10).Pass
assert_eq '200.1ms p95'       $false (Test-RoundMeetsSlo -BizOkPct 100 -IterRate 100 -TargetRate 100 -P95Ms 200.1).Pass

section '不达标要列出全部原因'
# 只报第一条会让人反复重跑才凑齐原因 —— 一轮 3.5 分钟,凑不起。
$r = Test-RoundMeetsSlo -BizOkPct 50 -IterRate 10 -TargetRate 100 -P95Ms 999
assert_eq '三条全列' 3 $r.Reasons.Count
assert_match '原因里点名业务成功率' '业务成功率' ($r.Reasons -join '|')
assert_match '原因里点名达成率'     '达成迭代率' ($r.Reasons -join '|')
assert_match '原因里点名 p95'       'p95'        ($r.Reasons -join '|')

section '目标速率为 0 时不做除法'
assert_eq '不抛异常' $true (Test-RoundMeetsSlo -BizOkPct 100 -IterRate 0 -TargetRate 0 -P95Ms 10).Pass

section '最大稳定吞吐 = 最后一个达标档'
$rows = @(
    [pscustomobject]@{ TargetRate = 100; IterRate = 100; BizOkPct = 100; P95 = 10 }
    [pscustomobject]@{ TargetRate = 200; IterRate = 199; BizOkPct = 100; P95 = 20 }
    [pscustomobject]@{ TargetRate = 400; IterRate = 250; BizOkPct = 100; P95 = 900 }
)
assert_eq '取 200 那档' 200 (Get-MaxStableThroughput -Rows $rows).TargetRate

section '中间档偶发不达标,但后面还有达标档 —— 取最后一个'
# 这不是纵容噪声:后面更高的档都达标了,中间那档的失败必然是偶然
# (一次后台活动、一次 GC),按「第一个不达标档的前一个」会白白低估上限。
$noisy = @(
    [pscustomobject]@{ TargetRate = 100; IterRate = 100; BizOkPct = 100; P95 = 10 }
    [pscustomobject]@{ TargetRate = 200; IterRate = 120; BizOkPct = 100; P95 = 20 }
    [pscustomobject]@{ TargetRate = 400; IterRate = 399; BizOkPct = 100; P95 = 30 }
)
assert_eq '取 400 那档' 400 (Get-MaxStableThroughput -Rows $noisy).TargetRate

section '一档都不达标'
assert_eq '返回 null' $null (Get-MaxStableThroughput -Rows @(
        [pscustomobject]@{ TargetRate = 100; IterRate = 10; BizOkPct = 100; P95 = 10 }))

section '乱序输入也要按速率排序后取'
$shuffled = @(
    [pscustomobject]@{ TargetRate = 400; IterRate = 399; BizOkPct = 100; P95 = 30 }
    [pscustomobject]@{ TargetRate = 100; IterRate = 100; BizOkPct = 100; P95 = 10 }
)
assert_eq '仍取 400' 400 (Get-MaxStableThroughput -Rows $shuffled).TargetRate

section '阈值可被 report.ps1 读出来印在表头'
$thr = Get-SloThresholds
assert_eq '成功率阈值' 99.0  $thr.BizOkPct
assert_eq '达成率阈值' 0.95  $thr.RateRatio
assert_eq 'p95 阈值'   200.0 $thr.P95Ms
