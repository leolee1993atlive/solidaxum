# 一轮压测「达标」与否的判定,以及从一组阶梯里取「最大稳定吞吐」。
# 纯函数,不碰文件、不碰进程,由 tests/slo.tests.ps1 覆盖。
#
# 阈值见设计 §3.2。三条一起看的理由:任何单独一条都能被绕过 ——
# 只看延迟,后端可以靠丢弃迭代维持低延迟;只看达成率,可以在 5 秒延迟下
# 「达成」目标;只看成功率,过载时排队但最终成功的请求也算成功。

$script:SloBizOkPct = 99.0   # 业务成功率下限(k6 checks 口径)
$script:SloRateRatio = 0.95   # 达成迭代率 / 目标迭代率 的下限
$script:SloP95Ms = 200.0  # p95 上限,毫秒

function Get-SloThresholds {
    @{ BizOkPct = $script:SloBizOkPct; RateRatio = $script:SloRateRatio; P95Ms = $script:SloP95Ms }
}

function Test-RoundMeetsSlo {
    param(
        [Parameter(Mandatory)][double]$BizOkPct,
        [Parameter(Mandatory)][double]$IterRate,
        [Parameter(Mandatory)][double]$TargetRate,
        [Parameter(Mandatory)][double]$P95Ms
    )
    $reasons = New-Object System.Collections.Generic.List[string]

    if ($BizOkPct -lt $script:SloBizOkPct) {
        $reasons.Add("业务成功率 $BizOkPct% < $script:SloBizOkPct%")
    }
    # 目标速率为 0 的轮次(爬坡场景的占位行)不做除法,视为不设该条约束。
    if ($TargetRate -gt 0) {
        $ratio = [math]::Round($IterRate / $TargetRate, 3)
        if ($ratio -lt $script:SloRateRatio) {
            $reasons.Add("达成迭代率 $IterRate / 目标 $TargetRate = $ratio < $script:SloRateRatio")
        }
    }
    if ($P95Ms -gt $script:SloP95Ms) {
        $reasons.Add("p95 ${P95Ms}ms > $($script:SloP95Ms)ms")
    }

    @{ Pass = ($reasons.Count -eq 0); Reasons = @($reasons) }
}

# 一组同单元同场景的阶梯里,**最后一个**达标档。
#
# 取最后一个而不是「第一个不达标档的前一个」:后面还有更高的达标档,
# 说明中间那次不达标是偶然(一次后台活动、一次 GC),按前者会白白低估上限。
# 反过来,若真的是拐点,后面就不会再有达标档,两种取法结果相同。
function Get-MaxStableThroughput {
    param([Parameter(Mandatory)][AllowEmptyCollection()][object[]]$Rows)
    $passing = @($Rows | Sort-Object TargetRate | Where-Object {
            (Test-RoundMeetsSlo -BizOkPct $_.BizOkPct -IterRate $_.IterRate `
                    -TargetRate $_.TargetRate -P95Ms $_.P95).Pass
        })
    if ($passing.Count -eq 0) { return $null }
    $passing[-1]
}
