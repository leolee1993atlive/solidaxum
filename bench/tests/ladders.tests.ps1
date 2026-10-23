# ladders.ps1 的单测。档位表是纯数据,但它决定整个批次 8/9 跑什么,
# 配错一档就是几十分钟白跑,所以逐条钉住。
. (Join-Path $PSScriptRoot '../lib/ladders.ps1')
. (Join-Path $PSScriptRoot '../lib/cells.ps1')

section '每个配置都有档位,不能漏'
# 漏一个配置 = 批次里静默少跑一路,而汇总表上只会显示为空行。
foreach ($id in (Get-ReadBaselineCellIds)) {
    assert_eq "$id 有读档位" $true ((Get-CellLadder -CellId $id -Scenario 'read_mix').Count -ge 2)
}
foreach ($id in (Get-CellIds)) {
    assert_eq "$id 有写档位" $true ((Get-CellLadder -CellId $id -Scenario 'write_mix').Count -ge 2)
}

section '档位必须严格递增'
# Get-MaxStableThroughput 取「最后一个达标档」,靠的是按 TargetRate 排序;
# 档位里有重复值会让两轮产出同名文件,后一轮直接覆盖前一轮。
foreach ($sc in 'read_mix', 'write_mix') {
    $ids = if ($sc -eq 'read_mix') { Get-ReadBaselineCellIds } else { Get-CellIds }
    foreach ($id in $ids) {
        $l = @(Get-CellLadder -CellId $id -Scenario $sc)
        $sorted = @($l | Sort-Object -Unique)
        assert_eq "$id/$sc 严格递增且无重复" ($l -join ',') ($sorted -join ',')
    }
}

section '档位的量级要贴着爬坡测出的天花板'
# 「拐点两侧各留至少一档」是**经验结果**,静态断言证不了 —— 真正的验收在
# report.ps1 的「未探顶」一列:最大稳定档若恰好是试过的最高档,就说明没探到顶。
# 这里能钉的是量级:最高档写成 48000 或 48 都是打字错误,而那种错误要等
# 几十分钟跑完才发现。
#
# 上界取 0.9×:SLO 拐点(p95 <= 200ms)本来就低于原始吞吐天花板 —— 延迟先炸、
# 吞吐后平 —— 所以最高档到天花板的九成就足以不达标,不必真的超过它。
# 下界取 0.5×:最低档要稳稳落在达标区里,否则一档都不达标。
$ceil = @{
    'axum-pg'       = 4895; 'axum-pgb' = 2531; 'axum-mysql' = 2074
    'axum-pg-exact' = 726; 'axum-pgb-exact' = 691
    'ruoyi-pg'      = 622; 'ruoyi-pgb' = 614
}
foreach ($id in $ceil.Keys) {
    $l = @(Get-CellLadder -CellId $id -Scenario 'read_mix')
    assert_eq "$id 最高档 >= 天花板 $($ceil[$id]) 的九成" $true ($l[-1] -ge $ceil[$id] * 0.9)
    assert_eq "$id 最低档 <= 天花板 $($ceil[$id]) 的一半" $true ($l[0] -le $ceil[$id] * 0.5)
}

section '爬坡没测到天花板的两个配置,档位必须往下探'
# axum-mysql-exact 与 ruoyi-mysql 在爬坡首档(500)就已过载,真实天花板在
# 500 以下、本轮未测得。对它们只能钉「整条档位都在已知的过载点以下」,
# 拿 500 当天花板去套上面那两条断言是把上界当成测量值。
foreach ($id in 'axum-mysql-exact', 'ruoyi-mysql') {
    $l = @(Get-CellLadder -CellId $id -Scenario 'read_mix')
    assert_eq "$id 最高档 < 已知过载点 500" $true ($l[-1] -lt 500)
    assert_eq "$id 最低档 <= 100(真实天花板可能很低)" $true ($l[0] -le 100)
}

section '写场景:补索引后六个单元同档,约束换成业务成功率'
# 吞吐天花板全部越过 3200,不再相差数量级;真正的拐点是防重复提交造成的
# 业务失败 —— 1200 时六个单元都 100%,2400 时降到 82-87%。
foreach ($id in (Get-CellIds)) {
    $l = @(Get-CellLadder -CellId $id -Scenario 'write_mix')
    assert_eq "$id 写档位统一" '800,1600,3200,4800,6400,8000' ($l -join ',')
    # 最低档要稳稳落在两边都全绿的区间(实测 800 时两边都 100%)
    assert_eq "$id 最低档 <= 800" $true ($l[0] -le 800)
    # 最高档要越过较快一方的天花板(axum-pg 爬坡推算 7324),否则拿不到不达标档
    assert_eq "$id 最高档 >= 7324" $true ($l[-1] -ge 7324)
}

section '双基线的两个单元用各自的档位,不能共用'
# 同口径比现状慢一个数量级(4895 vs 726),共用档位必然一头落空。
$capped = (Get-CellLadder -CellId 'axum-pg' -Scenario 'read_mix') -join ','
$exact = (Get-CellLadder -CellId 'axum-pg-exact' -Scenario 'read_mix') -join ','
assert_eq 'axum-pg 与 exact 变体档位不同' $false ($capped -eq $exact)

section '未知配置/场景要抛异常,不能静默给空档位'
# 静默返回空 = 该配置一轮不跑,而批次日志上什么都看不出来。
assert_throws '未知单元' { Get-CellLadder -CellId 'nope' -Scenario 'read_mix' }
assert_throws '未知场景' { Get-CellLadder -CellId 'axum-pg' -Scenario 'login' }
assert_throws 'ruoyi 没有 exact 变体的写档位' { Get-CellLadder -CellId 'axum-pg-exact' -Scenario 'write_mix' }

section '轮次总数'
$readRounds = (@(Get-ReadBaselineCellIds) | ForEach-Object { (Get-CellLadder -CellId $_ -Scenario 'read_mix').Count } | Measure-Object -Sum).Sum
$writeRounds = (@(Get-CellIds) | ForEach-Object { (Get-CellLadder -CellId $_ -Scenario 'write_mix').Count } | Measure-Object -Sum).Sum
# 41 = 初版 36 轮 + 补档 5 轮(axum-pg 三档、axum-pgb 两档)。
# 补档的理由见 ladders.ps1 里 axum-pg 那条注释:爬坡推算是下界,
# 批次 8 跑完发现这两个配置在最高档仍未被压垮。
assert_eq '批次 8 共 41 轮' 41 $readRounds
assert_eq '批次 9 共 36 轮' 36 $writeRounds
