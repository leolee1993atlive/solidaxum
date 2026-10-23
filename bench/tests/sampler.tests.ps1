# sampler.ps1 里纯函数的单测。
# 采样本身要起后台 job、跨秒采集,属集成级,由冒烟步骤验证。
. (Join-Path $PSScriptRoot '../lib/sampler.ps1')

section 'CSV 行解析与聚合'
$rows = @(
    [pscustomobject]@{ ts = '1'; proc = 'k6';   role = 'loadgen'; cpu_pct = '120.0'; ws_mb = '200.0' }
    [pscustomobject]@{ ts = '2'; proc = 'k6';   role = 'loadgen'; cpu_pct = '180.0'; ws_mb = '260.0' }
    [pscustomobject]@{ ts = '1'; proc = 'java'; role = 'backend'; cpu_pct = '300.0'; ws_mb = '1400.0' }
    [pscustomobject]@{ ts = '2'; proc = 'java'; role = 'backend'; cpu_pct = '500.0'; ws_mb = '1600.0' }
)
$agg = Get-SamplerAggregate $rows
assert_eq 'k6 CPU 均值'   150 $agg.k6.CpuAvgPct
assert_eq 'k6 CPU 峰值'   180 $agg.k6.CpuMaxPct
assert_eq 'java 内存峰值' 1600 $agg.java.WsMaxMb
assert_eq 'java 采样点数' 2   $agg.java.Samples

section '空数据不炸'
$empty = Get-SamplerAggregate @()
assert_eq '空聚合为空' 0 $empty.Count

section '被测后端进程名映射'
# axum 的进程名是二进制名 solidaxum,RuoYi 跑在 java 里
assert_eq 'axum → solidaxum' 'solidaxum' (Get-BackendProcName 'axum-mysql')
assert_eq 'ruoyi → java'         'java'          (Get-BackendProcName 'ruoyi-pgb')

section 'CSV 完整性守卫'
# 回归测试:第一版用 N1 格式化,千位分隔符把 22987.7 写成 "22,987.7",
# 把一列劈成两列。外在表现只是「可用内存莫名其妙只有 20MB」,不 assert
# 就会一路带进报告 —— 而受害的恰恰是 RuoYi 的 -Xmx2g 常驻内存。
$good = Join-Path ([System.IO.Path]::GetTempPath()) 'sampler-good.csv'
$bad = Join-Path ([System.IO.Path]::GetTempPath()) 'sampler-bad.csv'
@(
    'ts,proc,role,cpu_pct,ws_mb'
    '2026-09-05T22:00:00.0000000+08:00,java,backend,320.5,1487.2'
    '2026-09-05T22:00:01.0000000+08:00,_system,system,12.0,22987.7'
) | Set-Content $good -Encoding utf8
@(
    'ts,proc,role,cpu_pct,ws_mb'
    '2026-09-05T22:00:00.0000000+08:00,_system,system,12.0,22,987.7'   # 千位分隔符劈列
) | Set-Content $bad -Encoding utf8

assert_eq '四位数内存不误判'   $true  (Test-SamplerCsvWellFormed $good).Ok
assert_eq '千位分隔符被抓到'   $false (Test-SamplerCsvWellFormed $bad).Ok
assert_match '报出字段数不符' '有 6 个字段' (((Test-SamplerCsvWellFormed $bad).Problems) -join ' ')
Remove-Item $good, $bad -ErrorAction SilentlyContinue

section 'k6 是否成为瓶颈的判定'
# cpu_pct 是「核数 × 100」,400 即占 4 个核。本机 24 线程,k6 吃掉 4 个核
# 以上时该速率点的延迟数据已不可信(风险 R3)。
assert_eq 'k6 380 未过线' $false (Test-LoadGenSaturated 380)
assert_eq 'k6 400 过线'   $true  (Test-LoadGenSaturated 400)
assert_eq 'k6 900 过线'   $true  (Test-LoadGenSaturated 900)
