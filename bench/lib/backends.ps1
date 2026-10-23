# 后端构建/启停助手(被 run.ps1、matrix.ps1 dot-source)。
#
# 设计要点:
# - axum 按单元切换靠**工作目录**:crates/admin/src/main.rs:84 是
#   File::with_name("config.toml"),相对 CWD,既无 --config 也无环境变量覆盖。
# - RuoYi 按单元切换靠**生成的 application-bench.yml** + --spring.profiles.active=dev,bench,
#   走标准 java -jar(PG 驱动已进 pom,见 RuoYi 仓 48a50bdb0)。
# - 启动日志落 bench/results/<cell>.server.log。

. (Join-Path $PSScriptRoot 'cells.ps1')

$script:BenchRoot = Split-Path -Parent $PSScriptRoot
$script:RepoRoot = Split-Path -Parent $script:BenchRoot
$script:AxumRoot = Join-Path $script:RepoRoot 'solidaxum-rs'
$script:RuoyiRoot = Join-Path $script:RepoRoot 'RuoYi-Vue-Plus'
$script:LogDir = Join-Path $script:BenchRoot 'results'

# JDK21 路径可由 BENCH_JDK 覆盖。
$script:DefaultJdk = if ($env:BENCH_JDK) { $env:BENCH_JDK } else { 'C:\Program Files\Microsoft\jdk-21.0.11.10-hotspot' }

function Wait-Backend {
    param([string]$Url, [int]$TimeoutSec = 180, [System.Diagnostics.Process]$Proc)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        # 进程已退出就别再等满超时 —— 直接返回,让调用方去看日志
        if ($Proc -and $Proc.HasExited) { return $false }
        try {
            $r = Invoke-WebRequest -Uri $Url -Method Get -TimeoutSec 3 -SkipHttpErrorCheck
            if ($r.StatusCode -eq 200) { return $true }
        }
        catch { }
        Start-Sleep -Milliseconds 500
    }
    $false
}

# 写该单元的配置文件。axum → config.toml;ruoyi → application-bench.yml。
function Initialize-CellWorkdir([Parameter(Mandatory)][hashtable]$Cell) {
    $dir = Get-CellWorkDir $Cell
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    if ($Cell.Backend -eq 'axum') {
        Get-CellAxumConfig $Cell | Set-Content (Join-Path $dir 'config.toml') -Encoding utf8
    }
    else {
        Get-CellRuoyiYaml $Cell | Set-Content (Join-Path $dir 'application-bench.yml') -Encoding utf8
    }
    $dir
}

function Build-Backends {
    param([switch]$SkipAxum, [switch]$SkipRuoyi, [string]$Jdk = $script:DefaultJdk)

    if (-not $SkipAxum) {
        Write-Host '[axum] cargo build --release' -ForegroundColor Cyan
        Push-Location $script:AxumRoot
        try {
            & cargo build --release
            if ($LASTEXITCODE -ne 0) { throw 'cargo build --release 失败' }
        }
        finally { Pop-Location }
    }

    if (-not $SkipRuoyi) {
        if (-not (Test-Path $Jdk)) { throw "找不到 JDK21: $Jdk" }
        $env:JAVA_HOME = $Jdk
        Write-Host '[ruoyi] mvnw package' -ForegroundColor Cyan
        Push-Location $script:RuoyiRoot
        try {
            & "$script:RuoyiRoot\mvnw.cmd" -pl ruoyi-admin -am package -DskipTests -q
            if ($LASTEXITCODE -ne 0) { throw 'mvnw package 失败' }
        }
        finally { Pop-Location }
    }
}

function Start-Cell {
    param(
        [Parameter(Mandatory)][hashtable]$Cell,
        [string]$Jdk = $script:DefaultJdk
    )

    $probe = Get-CellProbeUrl $Cell
    if (Wait-Backend -Url $probe -TimeoutSec 1) {
        throw "[$($Cell.Id)] 端口已被占用($probe 已返回 200)。矩阵编排要求每轮独占启停 —— 但占用它的实例连的未必是本单元的库,继续跑会静默测错对象。请先停掉现有后端。"
    }

    if (-not (Test-Path $script:LogDir)) { New-Item -ItemType Directory -Path $script:LogDir -Force | Out-Null }
    $outLog = Join-Path $script:LogDir "$($Cell.Id).server.log"
    $errLog = Join-Path $script:LogDir "$($Cell.Id).server.err.log"
    $dir = Initialize-CellWorkdir $Cell

    if ($Cell.Backend -eq 'axum') {
        $exe = Join-Path $script:AxumRoot 'target\release\solidaxum.exe'
        if (-not (Test-Path $exe)) { throw "未见 release 二进制: $exe(先跑 Build-Backends)" }

        # 压到 warn:默认过滤器是 info,admin=debug,system=debug,...(main.rs:29-34),
        # 且同时挂控制台 + 文件两个 sink。不关等于自残,和当年 p6spy 同一类坑。
        $env:RUST_LOG = 'warn,sqlx=warn'
        $proc = Start-Process -FilePath $exe -WorkingDirectory $dir -PassThru `
            -WindowStyle Hidden -RedirectStandardOutput $outLog -RedirectStandardError $errLog
        $boot = 60
    }
    else {
        if (-not (Test-Path $Jdk)) { throw "找不到 JDK21: $Jdk" }
        $env:JAVA_HOME = $Jdk
        $jar = Join-Path $script:RuoyiRoot 'ruoyi-admin\target\ruoyi-admin.jar'
        if (-not (Test-Path $jar)) { throw "未见 jar: $jar(先跑 Build-Backends)" }

        $java = Join-Path $Jdk 'bin\java.exe'
        # 业务配置全在生成的 application-bench.yml 里,命令行只剩堆大小与配置定位。
        # -Xms=-Xmx 固定堆:不固定则堆自适应引入轮次间方差(F5)。
        $jarArgs = @(
            '-Xms2g', '-Xmx2g',
            '-jar', $jar,
            '--spring.profiles.active=dev,bench',
            "--spring.config.additional-location=file:$dir\"
        )
        $proc = Start-Process -FilePath $java -ArgumentList $jarArgs `
            -WorkingDirectory (Join-Path $script:RuoyiRoot 'ruoyi-admin') -PassThru `
            -WindowStyle Hidden -RedirectStandardOutput $outLog -RedirectStandardError $errLog
        $boot = 180
    }

    Write-Host "[$($Cell.Id)] 等待就绪(最多 ${boot}s,探活 $probe)..." -ForegroundColor DarkGray
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    if (-not (Wait-Backend -Url $probe -TimeoutSec $boot -Proc $proc)) {
        $handle = @{ Process = $proc; Cell = $Cell }
        Stop-Cell $handle
        $tail = if (Test-Path $outLog) { (Get-Content $outLog -Tail 12) -join "`n" } else { '(无日志)' }
        throw "[$($Cell.Id)] 启动失败/超时。日志尾部:`n$tail"
    }
    $sw.Stop()
    Write-Host ("[$($Cell.Id)] 已就绪 pid=$($proc.Id) 用时 {0:N1}s" -f $sw.Elapsed.TotalSeconds) -ForegroundColor Green
    @{ Process = $proc; Cell = $Cell }
}

function Stop-Cell([Parameter(Mandatory)][hashtable]$Handle) {
    $p = $Handle.Process
    if (-not $p) { return }
    if (-not $p.HasExited) {
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        $deadline = (Get-Date).AddSeconds(30)
        while ((Get-Date) -lt $deadline -and -not $p.HasExited) { Start-Sleep -Milliseconds 200 }
    }
    # 等端口真的释放,否则下一轮 Start-Cell 会撞「端口被占用」
    $probe = Get-CellProbeUrl $Handle.Cell
    $deadline = (Get-Date).AddSeconds(20)
    while ((Get-Date) -lt $deadline) {
        if (-not (Wait-Backend -Url $probe -TimeoutSec 1)) { break }
        Start-Sleep -Milliseconds 300
    }
    Start-Sleep -Seconds 1
}
