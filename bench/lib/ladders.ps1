# 批次 8/9 的阶梯档位,逐单元一份。
#
# 为什么不能一组档位跑全部:九个读配置的天花板跨度接近 10 倍(≤500 到 4895,
# 见就绪文档 §6.2)。统一档位必然两头落空 —— 给最快单元用的档位对最慢单元
# 全是过载点(一档都不达标,汇总表上是空格),反过来对最快单元全部通过
# (只能标「未探顶」)。两种结果都不是结论。
#
# 档位来自批次 7 的爬坡:每个配置取自己天花板的约 25% / 50% / 75% / 100%,
# 保证「拐点两侧各留至少一档」——Get-MaxStableThroughput 取最后一个达标档,
# 没有不达标档就说明上限没探到。
#
# SLO 拐点(p95 <= 200ms)通常低于原始吞吐天花板:延迟先炸、吞吐后平。
# 所以重心压在天花板的 25%-100%,而不是围绕天花板对称展开。

$script:Ladders = @{
    read_mix  = [ordered]@{
        # 现状口径(capped)
        #
        # 这两条比爬坡推算的天花板高出一截,是**批次 8 跑完后补的**。
        #
        # 教训不是「爬坡不准」——爬坡给 axum-pg 推算 4895,补档实测的原始吞吐
        # 平台是约 5100(6400/8000/9600 三档的达成率都停在这里),只差 4%。
        # 真正的问题是**档位不能顶到推算值就收手**:SLO 拐点(p95 <= 200ms)
        # 可能恰好落在原始平台之下一点点,于是「等于推算值」的那一档仍然达标,
        # 拿不到不达标档,只能标「未探顶」。
        #
        # 所以最高档要明确越过爬坡推算值。axum-pg 在 4800 时 p95 才 4.2ms、
        # 后端只用掉 474/800 的核预算,到 6400 就是 p95 287ms、CPU 656 —— 拐点
        # 陡得很,跨过去只需要一档。
        'axum-pg'          = @(1200, 2400, 3600, 4800, 6400, 8000, 9600)  # 爬坡推算 4895
        'axum-pgb'         = @(600, 1200, 1800, 2400, 3200, 4000)         # 爬坡推算 2531
        'axum-mysql'       = @(600, 1200, 1800, 2400)    # 天花板 2074
        # 同口径(exact)—— 与 RuoYi 相同的全量计数,慢一个数量级
        'axum-pg-exact'    = @(200, 400, 600, 800)       # 天花板 726
        'axum-pgb-exact'   = @(200, 400, 600, 800)       # 天花板 691
        'axum-mysql-exact' = @(50, 100, 200, 400)        # 天花板 <= 500,爬坡首档即过载
        # RuoYi
        'ruoyi-pg'         = @(200, 400, 600, 800)       # 天花板 622
        'ruoyi-pgb'        = @(200, 400, 600, 800)       # 天花板 614
        'ruoyi-mysql'      = @(50, 100, 200, 400)        # 天花板 <= 500,爬坡首档即过载
    }
    write_mix = [ordered]@{
        # 写场景不走分页计数,不需要双基线(设计 §1)。
        #
        # 补上 sys_user(email) 索引、并把唯一性校验改成只判存在之后,六个单元的
        # 吞吐天花板**全部越过 3200**(修复前是 226 / 521 / 804 / >400),彼此
        # 不再相差数量级,因此改用统一档位。
        #
        # 档位在 VU 耦合偏差修掉之后重定(见 k6/lib/flows.js 里 writeMixIteration
        # 的注释)。修偏差前测出的 axum 1199 / RuoYi 1751-2324 是错的,方向还是
        # 反的 —— 那组数字由 k6 的 VU 分配策略与旧取值公式的耦合造出来。
        #
        # 重新爬坡(两边同顶 12800):axum-pg 天花板 7324(业务 100%),
        # ruoyi-pg 4761(业务 99.44%、p95 136ms)。axum 约为 RuoYi 的 1.54 倍。
        #
        # 一条必须记住的教训:定这些数之前的一次探测漏了预热,RuoYi 冷 JIT 撞上
        # 突增负载在 1800 就大量 http=0,险些被当成它的真实极限。爬坡自带渐进热身,
        # 阶梯轮次由 run.ps1 统一预热 60 秒 —— 任何不带预热的短探测都不能用来
        # 给 JVM 侧下结论。
        'axum-pg'     = @(800, 1600, 3200, 4800, 6400, 8000)
        'axum-pgb'    = @(800, 1600, 3200, 4800, 6400, 8000)
        'axum-mysql'  = @(800, 1600, 3200, 4800, 6400, 8000)
        'ruoyi-pg'    = @(800, 1600, 3200, 4800, 6400, 8000)
        'ruoyi-pgb'   = @(800, 1600, 3200, 4800, 6400, 8000)
        'ruoyi-mysql' = @(800, 1600, 3200, 4800, 6400, 8000)
    }
}

# 取某个配置在某场景下的档位。
#
# 查不到一律抛异常,不返回空数组:静默给空档位等于该配置一轮不跑,
# 而批次日志上什么都看不出来,汇总表上只是一行空格。
function Get-CellLadder {
    param(
        [Parameter(Mandatory)][string]$CellId,
        [Parameter(Mandatory)][string]$Scenario
    )
    if (-not $script:Ladders.Contains($Scenario)) {
        throw "场景 $Scenario 没有档位表(可选: $($script:Ladders.Keys -join ', '))"
    }
    $byCell = $script:Ladders[$Scenario]
    if (-not $byCell.Contains($CellId)) {
        throw "单元 $CellId 在场景 $Scenario 下没有档位 —— 补进 ladders.ps1。"
    }
    # 不做 `, @(...)` 的逗号包裹(cells.ps1 里 Get-CellIds 那种写法)。
    # 调用方普遍会再套一层 @(),包裹后就成了「数组套数组」:.Count 变成 1、
    # 下标取到的是整个数组。档位至少两档(见 tests),不存在单元素被展开的风险。
    @($byCell[$CellId])
}

# 批次里的轮次顺序:按档位序号推进,每一轮内 axum / ruoyi 交替。
#
# 两件事一起满足:同一个配置的各档分散在整场里(热漂移对所有配置一视同仁),
# 且任意时刻两个后端交替出现(设计 F10 —— 连着跑完一方再跑另一方,
# CPU 降频会系统性偏向后跑的那个)。
function Get-LadderSchedule {
    param(
        [Parameter(Mandatory)][string[]]$CellIds,
        [Parameter(Mandatory)][string]$Scenario,
        [Parameter(Mandatory)][scriptblock]$Alternate
    )
    $ordered = @(& $Alternate $CellIds)
    $max = 0
    foreach ($id in $ordered) {
        $n = (Get-CellLadder -CellId $id -Scenario $Scenario).Count
        if ($n -gt $max) { $max = $n }
    }
    for ($i = 0; $i -lt $max; $i++) {
        foreach ($id in $ordered) {
            $l = @(Get-CellLadder -CellId $id -Scenario $Scenario)
            if ($i -lt $l.Count) {
                [pscustomobject]@{ CellId = $id; Rate = $l[$i]; Rung = $i + 1 }
            }
        }
    }
}
