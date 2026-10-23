# 两个数据库的调优配置:同等内存预算(各 8 GB)+ 各自社区惯例的配套项。
# 设计 §5。
#
# 文本变换(生成块、应用、回滚)是纯函数,与落盘分离 —— 这样「回滚后逐字
# 还原」这件事可以单测,而不用在压测机上拿真配置文件试。
#
# 压测机上的配置文件是别人的数据库的配置文件。marker 包裹 + 整块替换 +
# 逐字回滚,三条一起才敢动它。

$script:TuneBegin = '# >>> bench round2 begin >>>'
$script:TuneEnd = '# <<< bench round2 end <<<'

# 标注了默认值,是为了让报告能直接引用「从多少调到多少」。
$script:TuningSettings = @{
    mysql = [ordered]@{
        innodb_buffer_pool_size        = '8G'      # 默认 128M
        innodb_redo_log_capacity       = '4G'      # 默认 100M
        innodb_flush_log_at_trx_commit = '1'       # 默认 1,显式写出以防被人调低
        # 设计 §5.2 写的是 2000 / 4000(据「默认 200 / 2000」),那是 8.0 的默认值。
        # MySQL 8.4 已把这两项默认值提到 10000 / 20000,本机实测生效值正是如此。
        # 照 8.0 的数字写下去等于把 MySQL 的 IO 能力降到现值的 1/5,凭空造出一边倒
        # 的写场景 —— 与 §5.3 要防的「测的是默认值不对称」是同一类错误。
        # 故按实际生效值显式钉住:不升不降,只防漂移。
        innodb_io_capacity             = '10000'   # 8.4 默认 10000(8.0 是 200)
        innodb_io_capacity_max         = '20000'   # 8.4 默认 20000(8.0 是 2000)
    }
    pg    = [ordered]@{
        shared_buffers       = '8GB'   # 默认 128MB
        effective_cache_size = '16GB'  # 默认 4GB
        max_wal_size         = '4GB'   # 默认 1GB
        synchronous_commit   = 'on'    # 默认 on,显式写出以防被人关掉
        random_page_cost     = '1.1'   # 默认 4.0,NVMe 上 4.0 会高估随机读
    }
}

# 生效核对的期望值。MySQL 返回字节数,PG 返回带单位的串,故各写各的、
# 不做跨引擎归一 —— 归一函数本身会成为新的错误来源。
$script:ExpectedEffective = @{
    mysql = [ordered]@{
        innodb_buffer_pool_size        = '8589934592'
        innodb_redo_log_capacity       = '4294967296'
        innodb_flush_log_at_trx_commit = '1'
        innodb_io_capacity             = '10000'
        innodb_io_capacity_max         = '20000'
    }
    pg    = [ordered]@{
        shared_buffers       = '8GB'
        effective_cache_size = '16GB'
        max_wal_size         = '4GB'
        synchronous_commit   = 'on'
        random_page_cost     = '1.1'
    }
}

function Get-ExpectedEffective {
    param([Parameter(Mandatory)][ValidateSet('mysql', 'pg')][string]$Engine)
    $script:ExpectedEffective[$Engine]
}

function Get-TuningBlock {
    param([Parameter(Mandatory)][ValidateSet('mysql', 'pg')][string]$Engine)
    $lines = @($script:TuneBegin)
    foreach ($k in $script:TuningSettings[$Engine].Keys) {
        $lines += "$k = $($script:TuningSettings[$Engine][$k])"
    }
    $lines += $script:TuneEnd
    $lines -join "`n"
}

function Get-TuneBlockPattern {
    [regex]::Escape($script:TuneBegin) + '.*?' + [regex]::Escape($script:TuneEnd)
}

# 已有 marker 块就整块替换,没有就追加。返回新全文,不落盘。
function Add-TuningBlockToText {
    param(
        [Parameter(Mandatory)][AllowEmptyString()][string]$Text,
        [Parameter(Mandatory)][string]$Block
    )
    $pattern = Get-TuneBlockPattern
    # 必须带 Singleline:块是多行的,而 -match / 默认的 . 不跨换行 ——
    # 漏了它,已有的块永远检测不到,重复应用就变成一层层往下叠。
    if ([regex]::IsMatch($Text, $pattern, 'Singleline')) {
        # 替换串里的 $ 对 Regex::Replace 有特殊含义,先转义。
        return [regex]::Replace($Text, $pattern, $Block.Replace('$', '$$$$'), 'Singleline')
    }
    $sep = if ($Text.Length -eq 0 -or $Text.EndsWith("`n")) { '' } else { "`n" }
    "$Text$sep$Block`n"
}

# 连同块后的那个换行一起删,这样回滚结果与应用前逐字相同。
function Remove-TuningBlockFromText {
    param([Parameter(Mandatory)][AllowEmptyString()][string]$Text)
    [regex]::Replace($Text, "$(Get-TuneBlockPattern)`n?", '', 'Singleline')
}

# 落盘:先备份一份 .bench-backup(只在不存在时备,重跑不会覆盖初始状态),
# 再写入。返回备份路径。
function Set-TuningApplied {
    param(
        [Parameter(Mandatory)][ValidateSet('mysql', 'pg')][string]$Engine,
        [Parameter(Mandatory)][string]$ConfigPath
    )
    if (-not (Test-Path $ConfigPath)) { throw "配置文件不存在: $ConfigPath" }
    $backup = "$ConfigPath.bench-backup"
    if (-not (Test-Path $backup)) { Copy-Item $ConfigPath $backup }
    $text = Get-Content $ConfigPath -Raw
    $new = Add-TuningBlockToText -Text $text -Block (Get-TuningBlock -Engine $Engine)
    Set-Content -Path $ConfigPath -Value $new -NoNewline -Encoding utf8
    $backup
}

function Remove-TuningApplied {
    param([Parameter(Mandatory)][string]$ConfigPath)
    $text = Get-Content $ConfigPath -Raw
    Set-Content -Path $ConfigPath -Value (Remove-TuningBlockFromText -Text $text) -NoNewline -Encoding utf8
}

# 生效核对。写进配置文件不等于生效:拼错、被后面的同名项覆盖、服务没重启,
# 三种都会让文件与实际不一致,而三种都不报错。
# 返回逐项结果数组,由调用方决定怎么处置。
function Test-TuningEffective {
    param([Parameter(Mandatory)][ValidateSet('mysql', 'pg')][string]$Engine)
    $expected = Get-ExpectedEffective -Engine $Engine
    foreach ($k in $expected.Keys) {
        $actual = if ($Engine -eq 'mysql') {
            $r = Invoke-MysqlQ "SHOW GLOBAL VARIABLES LIKE '$k';" 'mysql'
            (@($r) | Select-Object -Last 1) -replace "^$k\s+", ''
        }
        else {
            (@(Invoke-PgQ "SHOW $k;" 'ruoyi') | Select-Object -Last 1)
        }
        $actual = "$actual".Trim()
        [pscustomobject]@{
            Engine   = $Engine
            Key      = $k
            Expected = $expected[$k]
            Actual   = $actual
            Ok       = ($actual -eq $expected[$k])
        }
    }
}
