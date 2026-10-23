# solidaxum-rs build script - Windows
# Usage: .\build-windows.ps1 [-Backend] [-Frontend] [-All] [-Version '1.0.0'] [-Suffix 'demo'] [-Force]
#
# -All     → 全量交互式 setup.exe(Inno Setup,内含 web/)+ 前端 zip
# -Backend → 纯后端更新 zip(${RepoName}-backend-*.zip,现场停服解压覆盖用,
#            不含 web/,不需要 ISCC)
# -Frontend→ 前端更新 zip(${RepoName}-ui-*.zip)
# 后端 setup.exe 需要 Inno Setup 6(https://jrsoftware.org/isdl.php)。

[CmdletBinding()]
param(
    [switch]$Backend,
    [switch]$Frontend,
    [switch]$All,
    [string]$Version = '',
    [string]$Suffix = '',
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

if ($PSVersionTable.OS -and $PSVersionTable.OS -notmatch 'Windows') {
    throw 'This script must run on Windows'
}

$RepoRoot     = (Resolve-Path "$PSScriptRoot\..\..").Path
# RepoName 只用于产物命名(二进制/服务/包名);目录是 monorepo 固定布局,
# 不随 repo 名走(后端恒 solidaxum-rs,前端恒 solidaxum-ui)。
$RepoName     = & "$RepoRoot\scripts\lib\repo-name.ps1" -ProjectDir "$RepoRoot\solidaxum-rs"
$BackendDir   = "$RepoRoot\solidaxum-rs"
$FrontendDir  = "$RepoRoot\solidaxum-ui"

$TemplatesDir = "$PSScriptRoot\templates"
$PayloadDir   = "$RepoRoot\scripts\installer-payloads\backend-windows"
$IssFile      = "$PSScriptRoot\installer\solidaxum.iss"
$WinSWUrl     = 'https://github.com/winsw/winsw/releases/download/v3.0.0-alpha.11/WinSW-x64.exe'
$StagingDir   = "$PSScriptRoot\staging"
$DistDir      = "$RepoRoot\dist"

$SvcSuffix = if ($Suffix) { "-$Suffix" } else { '' }

# Default -All
if (-not ($Backend -or $Frontend -or $All)) { $All = $true }
if ($All) { $Backend = $true; $Frontend = $true }

# 实例后缀现在是安装向导里的输入,不再是构建期概念:一个 setup.exe 能装任意
# 后缀的实例。-Suffix 只剩下给前端 zip 命名的作用,别让它静默失效。
if ($Suffix -and $Backend) {
    Write-Warning "-Suffix 只影响前端 zip 的文件名;后端 setup.exe 的实例后缀在安装向导里填。"
}

# setup.exe 是后端 + 前端的综合包(.iss 的 [Files] 无条件引用 web\*),而
# $StagingDir 每次运行都清空 —— web\ 只可能来自本次的 Build-Frontend。
# 区分:web\ 在 → 编译 setup.exe;不在 → 改产纯后端更新 zip(见 Build-Backend)。

# Version
if ([string]::IsNullOrWhiteSpace($Version)) {
    Push-Location $RepoRoot
    try {
        if (git rev-parse 2>$null) {
            $Version = (git describe --always --dirty 2>$null)
        }
    } catch { }
    finally { Pop-Location }
    if ([string]::IsNullOrWhiteSpace($Version)) { $Version = '0.0.0+local' }
}

# Directories
New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
if (Test-Path $StagingDir) { Remove-Item $StagingDir -Recurse -Force }
New-Item -ItemType Directory -Path $StagingDir | Out-Null

# ---------- Helpers ----------
function Get-BuildTimestamp { (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ') }

function Write-VersionFile([string]$Path) {
    "$Version+$(Get-BuildTimestamp)" | Set-Content -Path $Path -Encoding UTF8
}

function Test-ZipCollision([string]$Path) {
    if (Test-Path $Path) {
        if ($Force) {
            Remove-Item $Path -Force
            Remove-Item "$Path.sha256.txt" -ErrorAction SilentlyContinue
        } else {
            throw "Already exists: $Path (use -Force to overwrite)"
        }
    }
}

function Write-Sha256([string]$Path) {
    (Get-FileHash $Path -Algorithm SHA256).Hash + '  ' + (Split-Path $Path -Leaf) |
        Set-Content -Path "$Path.sha256.txt" -Encoding ASCII
}

function Ensure-WinSW {
    $exe = Join-Path $env:TEMP 'WinSW-x64.exe'
    if (-not (Test-Path $exe)) {
        Write-Host 'Downloading WinSW...'
        Invoke-WebRequest -Uri $WinSWUrl -OutFile $exe
    } else {
        Write-Host 'WinSW cache hit'
    }
    return $exe
}

function Find-ISCC {
    # 优先用 PATH 里的 ISCC;否则查默认安装路径(Inno Setup 6)
    $cmd = Get-Command ISCC -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $default = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
    if (Test-Path $default) { return $default }
    return $null
}

# ---------- Backend ----------
# 注:Windows 侧不需要 Linux 那样的 unixODBC 预检。odbc-sys 在 Windows 上链接的是
# odbc32(`link(name = "odbc32")`),odbc32.lib 随 Windows SDK(MSVC 工具链必备),
# odbc32.dll 是系统组件 —— 构建期与运行期都没有额外依赖。
function Build-Backend {
    Write-Host '===== Building backend ====='

    # cargo 的 bin 名恒为 solidaxum(Cargo.toml 静态声明);产物按 RepoName 改名落位
    Push-Location $BackendDir
    try { cargo build --release } finally { Pop-Location }

    $bin = "$BackendDir\target\release\solidaxum.exe"
    if (-not (Test-Path $bin)) { throw "Backend binary not found: $bin" }

    $stage = "$StagingDir\backend"
    New-Item -ItemType Directory -Path "$stage\templates\winsw" -Force | Out-Null

    # 模板源文件名恒为 solidaxum.*(与 cargo bin 同理);包内按 RepoName 落位,
    # setup.exe 按 templates\winsw\{#MyRepoName}.xml 取用(.iss:667/:731)。
    Copy-Item $bin "$stage\$RepoName.exe"
    Copy-Item "$TemplatesDir\winsw\solidaxum.xml" "$stage\templates\winsw\$RepoName.xml"
    Copy-Item "$PayloadDir\README.txt" "$stage\README.txt"
    (Get-Content "$stage\README.txt" -Raw).Replace('@REPO_NAME@', $RepoName) |
        Set-Content "$stage\README.txt" -Encoding UTF8 -NoNewline
    Write-VersionFile "$stage\VERSION"

    if (-not (Test-Path "$StagingDir\web")) {
        # 纯后端更新 zip:现场流程 = 停服务(Net stop/<svc> 或 services.msc)
        # → 解压覆盖安装目录里的 $RepoName.exe → 起服务。不含 web/ 与安装向导。
        $zip = "$DistDir\${RepoName}-backend-$Version-windows-x64.zip"
        Test-ZipCollision $zip
        Compress-Archive -Path "$stage\$RepoName.exe", "$stage\VERSION", "$stage\README.txt" `
            -DestinationPath $zip
        Write-Sha256 $zip
        Write-Host "Created $zip"
        return
    }

    # 全量路径:交互式 setup.exe(ISCC 编译 .iss)。
    $iscc = Find-ISCC
    if (-not $iscc) {
        throw @'
未找到 Inno Setup 编译器(ISCC.exe)。
安装 Inno Setup 6: https://jrsoftware.org/isdl.php
安装后确保 ISCC.exe 在 PATH,或装在默认路径 %ProgramFiles(x86)%\Inno Setup 6\。
'@
    }
    Write-Host "Using ISCC: $iscc"

    $winsw = Ensure-WinSW
    Copy-Item $winsw "$stage\WinSW-x64.exe"

    # 以下都是 .iss 的 [Files] 段引用的文件。旧 zip 时代的 install.ps1 /
    # uninstall.ps1 不再入包 —— 服务注册改由 setup.exe 自己做;遗留 zip 用户
    # 要跑的是他们旧安装目录里那份,不是这里这份。

    # 无条件搬:.iss 的 [Files] 引用 web\*,缺了编译不过。
    # 「必须与前端同批构建」由上面的纯后端 zip 分支保证不会走到缺 web 的 ISCC。
    Copy-Item "$StagingDir\web" "$stage\web" -Recurse

    # 交互式 setup.exe:调 ISCC 编译 .iss。
    # 文件名不带后缀,必须与 .iss 的 OutputBaseFilename 逐字一致 —— 对不上时
    # ISCC 照样成功,只有下面的 Write-Sha256 会以一句莫名其妙的报错收场。
    $setup = "$DistDir\${RepoName}-setup-$Version-windows-x64.exe"
    Test-ZipCollision $setup
    # AppName:从 repo 名派生(参考实现→ bl haems)
    $appName = ($RepoName -replace '-', ' ')
    & $iscc /Q `
        "/DMyVersion=$Version" `
        "/DMySourceDir=$stage" `
        "/DMyRepoName=$RepoName" `
        "/DMyAppName=$appName" `
        $IssFile
    if ($LASTEXITCODE -ne 0) { throw "ISCC 编译失败 (exit $LASTEXITCODE)" }
    if (-not (Test-Path $setup)) {
        throw "ISCC 未产出 $setup(检查 .iss 的 OutputDir / OutputBaseFilename 是否与本脚本一致)"
    }
    Write-Sha256 $setup
    Write-Host "Created $setup"
}

# ---------- Frontend (static SPA) ----------
function Build-Frontend {
    Write-Host '===== Building frontend ====='

    # VITE_APP_NAME 让 UI 品牌(标题/侧栏/登录页)跟随产物名(brand.ts 兜底 SolidAxum)
    $env:VITE_APP_NAME = $RepoName
    Push-Location $FrontendDir
    try {
        pnpm install --frozen-lockfile
        pnpm build
    } finally { Pop-Location }

    $build = "$FrontendDir\build"
    if (-not (Test-Path "$build\index.html")) { throw 'Frontend build missing index.html' }
    if (Test-Path "$build\server") { throw 'Detected build\server; frontend is not a static SPA' }

    $web = "$StagingDir\web"
    New-Item -ItemType Directory -Path $web -Force | Out-Null
    Copy-Item "$build\*" $web -Recurse

    $zip = "$DistDir\${RepoName}-ui${SvcSuffix}-$Version-windows-x64.zip"
    Test-ZipCollision $zip
    Push-Location $StagingDir
    try { Compress-Archive -Path 'web\*' -DestinationPath $zip }
    finally { Pop-Location }
    Write-Sha256 $zip
    Write-Host "Created $zip"
}

# ---------- Main ----------
Write-Host "Version: $Version"
try {
    if ($Frontend) { Build-Frontend }
    if ($Backend)  { Build-Backend }
    Write-Host "Done. Artifacts in $DistDir\"
}
finally {
    if (Test-Path $StagingDir) { Remove-Item $StagingDir -Recurse -Force }
}
