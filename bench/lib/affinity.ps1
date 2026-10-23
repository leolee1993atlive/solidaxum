# CPU 亲和性:把发压器、被测后端、数据库钉在互不重叠的核集合上。
#
# 为什么需要:极限档下 k6 自己会吃掉大量 CPU,不隔离的话测出来的「极限」
# 是压测机的极限(设计 §4.1)。绑核之后结论从「整机极限」变成「同样 N 核
# 预算下两边能跑多少」——后者更可引用,也更接近容器化部署里给 CPU limit
# 的现实。
#
# i7-13700K 在 Windows 上的逻辑处理器编号:0-15 是 8 个 P 核(每核两个
# 超线程),16-23 是 8 个 E 核。
#
# 这份划分是**初始候选**,由 bench/seed/calibrate-loadgen.ps1 的标定结果
# 确认或推翻(设计 §4.2)。改这里之后必须重跑 bench/test-bench.ps1。

$script:TotalLogicalCores = 24

$script:CoreGroups = @{
    backend = @(0..7)    # 4 个物理 P 核 / 8 线程
    db      = @(8..15)   # 4 个物理 P 核 / 8 线程
    loadgen = @(16..23)  # 8 个 E 核
}

# 进程名 → 核组。名字与 sampler.ps1 的 $WatchProcs 同一批,但那边分的是
# 采样角色、这边分的是 CPU 预算,两者不是一回事:pgbouncer 采样上算 proxy,
# 预算上归数据库侧(它是数据库路径的一部分,不该跟发压器抢核)。
#
# redis-server 归 loadgen 组是权衡的结果:它是关键路径组件,理应有独立预算,
# 但机器只有三组核可分。Redis 单线程、第一轮实测占用极低,与 k6 共享 8 个
# E 核的风险小于让它跟后端或数据库抢 P 核。若标定显示 E 核组吃紧,
# 优先把 redis 挪出去,而不是动 backend / db 两组。
$script:ProcCoreGroup = @{
    'solidaxum' = 'backend'
    'java'          = 'backend'
    'mysqld'        = 'db'
    'postgres'      = 'db'
    'pgbouncer'     = 'db'
    'redis-server'  = 'loadgen'
    'k6'            = 'loadgen'
}

# 核号数组 → affinity 位掩码。
function Get-AffinityMask {
    param([Parameter(Mandatory)][AllowEmptyCollection()][int[]]$Cores)
    $mask = [int64]0
    foreach ($c in $Cores) {
        if ($c -lt 0 -or $c -ge $script:TotalLogicalCores) {
            throw "核号 $c 超出 0..$($script:TotalLogicalCores - 1)"
        }
        $mask = $mask -bor ([int64]1 -shl $c)
    }
    # 空掩码会让进程无核可用。宁可在这里炸,也不要在压测中途发现。
    if ($mask -eq 0) { throw '核集合不能为空' }
    $mask
}

function Get-GroupCores {
    param([Parameter(Mandatory)][string]$Group)
    if (-not $script:CoreGroups.ContainsKey($Group)) { throw "未知核组: $Group" }
    @($script:CoreGroups[$Group])
}

function Get-ProcCoreGroup {
    param([Parameter(Mandatory)][string]$ProcName)
    $script:ProcCoreGroup[$ProcName]
}

function Test-CoreGroupsDisjoint {
    $seen = @{}
    foreach ($g in $script:CoreGroups.Keys) {
        foreach ($c in $script:CoreGroups[$g]) {
            if ($seen.ContainsKey($c)) { return $false }
            $seen[$c] = $g
        }
    }
    $true
}

function Test-CoreGroupsCoverAll {
    $all = @($script:CoreGroups.Values | ForEach-Object { $_ }) | Sort-Object -Unique
    $all.Count -eq $script:TotalLogicalCores
}

# 发压侧饱和阈值,单位与 sampler 的 cpu_pct 一致(核数 × 100)。
#
# sampler.ps1 里原有的固定值 400 是「k6 在整机 24 线程上随便跑」时的经验值。
# 绑核之后 k6 只有 loadgen 组那么多核,400 可能已经是满载、也可能还很闲,
# 与实际组大小脱节 —— 阈值必须跟着组走,否则设计 §4.4 那道二道门禁要么
# 永不触发、要么见轮就报。取组容量的 90%。
function Get-LoadGenSaturationPct {
    [int][math]::Round((Get-GroupCores 'loadgen').Count * 100 * 0.9, 0)
}

# 给一个已拿到的 Process 对象设亲和性。k6 由 run.ps1 用 Start-Process
# -PassThru 拉起,拿得到对象,走这条。
function Set-ProcessAffinityByGroup {
    param(
        [Parameter(Mandatory)][System.Diagnostics.Process]$Process,
        [Parameter(Mandatory)][string]$Group
    )
    $Process.ProcessorAffinity = [IntPtr](Get-AffinityMask -Cores (Get-GroupCores $Group))
}

# 按进程名绑该名字下的全部实例。postgres / mysqld 有多个子进程,按名字全绑。
# 找不到进程不算错(某一轮可能没起 pgbouncer)。
#
# 返回 @{ Name; Group; Ok; Failed }。**失败个数必须带出来**,不能只回成功数:
# 数据库与 Redis 以服务账户运行,非提权会话设它们的 ProcessorAffinity 会得到
# 「拒绝访问」。那时该进程仍在全部 $TotalLogicalCores 核上跑,而结果文件看起来
# 完全正常 —— 与第一轮 §8 那六处测量缺陷是同一类问题:产出看似合理、方向却是错的。
function Set-ProcGroupAffinity {
    param([Parameter(Mandatory)][string]$ProcName)
    $group = Get-ProcCoreGroup $ProcName
    if (-not $group) {
        throw "进程 $ProcName 未分配核组 —— 补进 affinity.ps1 的 ProcCoreGroup。静默不绑等于它在全部 $script:TotalLogicalCores 核上跑,隔离形同虚设。"
    }
    $mask = [IntPtr](Get-AffinityMask -Cores (Get-GroupCores $group))
    $ok = 0
    $failed = 0
    foreach ($p in @(Get-Process -Name $ProcName -ErrorAction SilentlyContinue)) {
        try { $p.ProcessorAffinity = $mask; $ok++ }
        catch {
            $failed++
            Write-Host "  [绑核失败] $ProcName pid=$($p.Id): $($_.Exception.Message.Split([char]10)[0])" -ForegroundColor Yellow
        }
    }
    [pscustomobject]@{ Name = $ProcName; Group = $group; Ok = $ok; Failed = $failed }
}

# 从一组绑核结果里挑出「有进程没绑上」的。纯函数,由 tests/affinity.tests.ps1 覆盖。
function Get-AffinityFailures {
    param([Parameter(Mandatory)][AllowEmptyCollection()][object[]]$Results)
    @($Results | Where-Object { $_.Failed -gt 0 })
}

# 一轮开始前把常驻服务全绑一遍。服务重启后亲和性会丢,所以每轮都做。
# 返回逐进程的结果,由调用方用 Get-AffinityFailures 判定是否放行。
function Set-BenchServicesAffinity {
    foreach ($n in @('mysqld', 'postgres', 'pgbouncer', 'redis-server')) {
        $r = Set-ProcGroupAffinity -ProcName $n
        if ($r.Ok -gt 0) {
            Write-Host "  [绑核] $n × $($r.Ok) → $($r.Group) 组" -ForegroundColor DarkGray
        }
        $r
    }
}
