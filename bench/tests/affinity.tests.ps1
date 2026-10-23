# affinity.ps1 的单测。掩码计算与核组划分是纯函数;真正设 affinity 的部分
# 要有进程,不在单测范围。
. (Join-Path $PSScriptRoot '../lib/affinity.ps1')

section '掩码计算'
assert_eq '核 0 → 1'             1        (Get-AffinityMask -Cores @(0))
assert_eq '核 0-7 → 0xFF'        255      (Get-AffinityMask -Cores @(0..7))
assert_eq '核 8-15 → 0xFF00'     65280    (Get-AffinityMask -Cores @(8..15))
assert_eq '核 16-23 → 0xFF0000'  16711680 (Get-AffinityMask -Cores @(16..23))
assert_eq '不连续核集合'          9        (Get-AffinityMask -Cores @(0, 3))
assert_throws '空集合抛异常'   { Get-AffinityMask -Cores @() }
assert_throws '越界核号抛异常' { Get-AffinityMask -Cores @(24) }
assert_throws '负核号抛异常'   { Get-AffinityMask -Cores @(-1) }

section '核组划分 —— 隔离的两个前提'
# 重叠就等于没隔离;漏掉的核会被 Windows 随意调度,「后端只有 N 核」这个
# 前提就不成立了。两条缺一不可。
assert_eq '三组互不重叠'           $true (Test-CoreGroupsDisjoint)
assert_eq '并集盖满全部逻辑处理器' $true (Test-CoreGroupsCoverAll)

section '两个后端、两个数据库各自拿到完全相同的预算'
# 这是整个极限对比的公平性根基:结论是「同样 N 核下谁跑得多」,
# 不是「谁抢到的核多」。
assert_eq 'axum 与 java 同组'       (Get-ProcCoreGroup 'java')     (Get-ProcCoreGroup 'solidaxum')
assert_eq 'mysqld 与 postgres 同组' (Get-ProcCoreGroup 'postgres') (Get-ProcCoreGroup 'mysqld')
assert_eq 'pgbouncer 归数据库预算'  'db'                            (Get-ProcCoreGroup 'pgbouncer')
assert_eq '后端与数据库核数相同' (Get-GroupCores 'db').Count (Get-GroupCores 'backend').Count

section '未分组的进程名必须抛异常,不能静默不绑'
# 静默不绑 = 那个进程在全部 24 核上跑,隔离形同虚设,而日志上什么都看不到。
assert_throws '未知进程名' { Set-ProcGroupAffinity -ProcName 'notepad' }
assert_throws '未知核组'   { Get-GroupCores 'nope' }

section '绑核失败必须能被判出来,不能只回成功数'
# 数据库与 Redis 以服务账户运行,非提权会话设它们的 ProcessorAffinity 会得到
# 「拒绝访问」。那时进程仍在全部 24 核上跑,而结果文件看起来完全正常 ——
# 设计 §4.3「两个数据库拿到相同且受限的核集合」这个前提就悄悄没了。
$mixed = @(
    [pscustomobject]@{ Name = 'mysqld'; Group = 'db'; Ok = 2; Failed = 0 }
    [pscustomobject]@{ Name = 'postgres'; Group = 'db'; Ok = 0; Failed = 20 }
    [pscustomobject]@{ Name = 'pgbouncer'; Group = 'db'; Ok = 1; Failed = 0 }
)
assert_eq '挑出有失败的那一个' 1 (Get-AffinityFailures -Results $mixed).Count
assert_eq '挑出的是 postgres' 'postgres' (Get-AffinityFailures -Results $mixed)[0].Name
assert_eq '部分成功也算失败' 1 (Get-AffinityFailures -Results @(
        [pscustomobject]@{ Name = 'postgres'; Group = 'db'; Ok = 19; Failed = 1 })).Count
assert_eq '全绑上则为空' 0 (Get-AffinityFailures -Results @(
        [pscustomobject]@{ Name = 'mysqld'; Group = 'db'; Ok = 2; Failed = 0 })).Count
assert_eq '一个进程都没找到不算失败' 0 (Get-AffinityFailures -Results @(
        [pscustomobject]@{ Name = 'pgbouncer'; Group = 'db'; Ok = 0; Failed = 0 })).Count
assert_eq '空输入不抛异常' 0 (Get-AffinityFailures -Results @()).Count

section '发压侧饱和阈值跟着核组走,不是写死的 400'
# 写死的 400 是「k6 在整机 24 线程上随便跑」的经验值。绑核后它只有 loadgen
# 组那么多核,阈值与组大小脱节的话,二道门禁要么永不触发、要么见轮就报。
assert_eq 'loadgen 组 8 核 → 720' 720 (Get-LoadGenSaturationPct)
