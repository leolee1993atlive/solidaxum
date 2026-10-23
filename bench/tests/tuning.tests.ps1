# tuning.ps1 的单测。文本变换是纯函数,可在开发机上跑;
# Test-TuningEffective 要连库,不在单测范围。
. (Join-Path $PSScriptRoot '../lib/tuning.ps1')

section '两库的调优块都含各自的四类项'
$my = Get-TuningBlock -Engine mysql
$pg = Get-TuningBlock -Engine pg
assert_match 'MySQL 缓存'   'innodb_buffer_pool_size = 8G'  $my
assert_match 'MySQL 日志'   'innodb_redo_log_capacity = 4G' $my
# 10000 / 20000 是 MySQL 8.4 的默认值(8.0 是 200 / 2000)。设计 §5.2 按 8.0
# 写的 2000 会把本机的 IO 能力降到 1/5,断言跟着钉在实际值上。
assert_match 'MySQL IO'     'innodb_io_capacity = 10000'    $my
assert_notmatch 'MySQL IO 不得降到 8.0 的数字' 'innodb_io_capacity = 2000' $my
assert_match 'PG 缓存'      'shared_buffers = 8GB'          $pg
assert_match 'PG 缓存估计'  'effective_cache_size = 16GB'   $pg
assert_match 'PG 日志'      'max_wal_size = 4GB'            $pg
assert_match 'PG IO'        'random_page_cost = 1.1'        $pg

section '持久性级别必须对等 —— 这是写场景可比的前提'
# 任何一边降级都能换来大幅写吞吐,但那时两边就不是在比同一件事(设计 §5.3)。
assert_match 'MySQL 每次提交落盘' 'innodb_flush_log_at_trx_commit = 1' $my
assert_match 'PG 每次提交落盘'    'synchronous_commit = on'            $pg
assert_notmatch 'MySQL 不得降级到 2'   'innodb_flush_log_at_trx_commit = 2' $my
assert_notmatch 'MySQL 不得降级到 0'   'innodb_flush_log_at_trx_commit = 0' $my
assert_notmatch 'PG 不得降级到 off'    'synchronous_commit = off'           $pg
assert_notmatch 'PG 不得降级到 local'  'synchronous_commit = local'         $pg

section '日志容量必须与缓存一起调'
# 只提 buffer 不提日志会系统性偏向 PG:MySQL 的 redo 默认只有 100M,
# PG 的 max_wal_size 默认 1G。漏掉这项,写场景测的是两边默认值的不对称。
assert_match 'MySQL 块里有 redo 容量' 'innodb_redo_log_capacity' $my
assert_match 'PG 块里有 WAL 容量'     'max_wal_size'             $pg

section '幂等:应用两次与一次结果相同'
$orig = "key_a = 1`nkey_b = 2`n"
$once = Add-TuningBlockToText -Text $orig -Block $pg
$twice = Add-TuningBlockToText -Text $once -Block $pg
assert_eq '两次与一次相同' $once $twice

section '回滚后与原文逐字相同'
# 不是「差不多」——压测机上那是别人的数据库配置文件,多一个空行都算污染。
assert_eq '还原' $orig (Remove-TuningBlockFromText -Text $once)

section '原文没有末尾换行也能正确追加与回滚'
$noNewline = 'key_a = 1'
$applied = Add-TuningBlockToText -Text $noNewline -Block $pg
assert_match '块被追加' 'shared_buffers = 8GB' $applied
assert_eq '回滚回无换行原文' $noNewline (Remove-TuningBlockFromText -Text $applied).TrimEnd("`n")

section '块内容变了,重新应用要整块替换而不是叠加'
$old = Add-TuningBlockToText -Text $orig -Block $pg
$new = Add-TuningBlockToText -Text $old  -Block $my
assert_notmatch '旧块被替换掉' 'shared_buffers = 8GB'         $new
assert_match     '新块在'      'innodb_buffer_pool_size = 8G' $new

section '期望生效值与配置块一一对应'
# 配置块里写了项、期望表里漏了项,生效核对就查不到它,配错了也发现不了。
$myKeys = @((Get-ExpectedEffective -Engine mysql).Keys)
$pgKeys = @((Get-ExpectedEffective -Engine pg).Keys)
foreach ($k in $myKeys) { assert_match "MySQL 块含 $k" $k $my }
foreach ($k in $pgKeys) { assert_match "PG 块含 $k"    $k $pg }
assert_eq 'MySQL 项数一致' 5 $myKeys.Count
assert_eq 'PG 项数一致'    5 $pgKeys.Count
