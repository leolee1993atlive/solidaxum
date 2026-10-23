# @REPO_NAME@ backend - Windows install(就地安装)
# 用法: .\install.ps1 [-Suffix 'proj1']
# 要求: 管理员身份的 PowerShell;勿在临时/可移动目录安装

[CmdletBinding()]
param(
  [string]$Suffix = ''
)

$ErrorActionPreference = 'Stop'

# ---------- 状态(用于回滚) ----------
$script:CreatedItems     = @()
$script:ServiceInstalled = $false

$Prefix   = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoName = Split-Path -Leaf $Prefix
$ServiceName = "${RepoName}$(if ($Suffix) { '-' + $Suffix } else { '' })"

function Cleanup-OnFail {
  Write-Host ""
  Write-Warning "安装失败,开始回滚..."
  if ($script:ServiceInstalled) {
    & "$Prefix\service\$ServiceName.exe" uninstall 2>$null | Out-Null
  }
  foreach ($p in ($script:CreatedItems | Sort-Object -Descending)) {
    if (Test-Path $p) { Remove-Item $p -Recurse -Force -ErrorAction SilentlyContinue }
  }
  Write-Warning "回滚完成(仅清理本次生成的文件,未触碰解压的原始文件)"
}

# ---------- 预检 ----------
Write-Host "[1/5] 预检..."
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { throw "需要管理员身份运行 PowerShell" }

if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
  throw "服务已注册: $ServiceName(先运行 .\uninstall.ps1)"
}

$RequiredFiles = @(
  "$Prefix\$RepoName.exe",
  "$Prefix\config.toml.example",
  "$Prefix\templates\winsw\$RepoName.xml",
  "$Prefix\WinSW-x64.exe"
)
foreach ($f in $RequiredFiles) {
  if (-not (Test-Path $f)) { throw "缺失文件: $f(请确认在完整解压目录内运行)" }
}

try {
  # ---------- 建运行目录(原地) ----------
  Write-Host "[2/5] 创建运行目录..."
  foreach ($d in @("$Prefix\data", "$Prefix\logs", "$Prefix\service")) {
    if (-not (Test-Path $d)) {
      New-Item -ItemType Directory -Path $d | Out-Null
      $script:CreatedItems += $d
    }
  }

  # ---------- 渲染 WinSW xml(原地) ----------
  Write-Host "[3/5] 渲染 WinSW 配置..."
  $xml = Get-Content "$Prefix\templates\winsw\$RepoName.xml" -Raw
  $xml = $xml.Replace('@PREFIX@', $Prefix).Replace('@SVC_NAME@', $ServiceName).Replace('@REPO_NAME@', $RepoName)
  Set-Content -Path "$Prefix\service\$ServiceName.xml" -Value $xml -Encoding UTF8
  Copy-Item "$Prefix\WinSW-x64.exe" "$Prefix\service\$ServiceName.exe"

  # ---------- 注册服务 ----------
  Write-Host "[4/5] 注册 Windows 服务 $ServiceName..."
  & "$Prefix\service\$ServiceName.exe" install
  if ($LASTEXITCODE -ne 0) { throw "WinSW install 失败" }
  $script:ServiceInstalled = $true

  # ---------- 提示 ----------
  Write-Host "[5/5] 完成。"
  Write-Host ""
  Write-Host "OK 安装完成(就地):$Prefix"
  Write-Host "! 请勿移动/重命名/删除此文件夹,否则服务会启动失败。"
  Write-Host "v 下一步(必须):"
  Write-Host "  1. Copy-Item '$Prefix\config.toml.example' '$Prefix\config.toml'"
  Write-Host "  2. notepad '$Prefix\config.toml'  # 填 DB / Redis / JWT secret / encryption key"
  Write-Host "  3. icacls '$Prefix\config.toml' /inheritance:r /grant:r `"$env:USERNAME:F`""
  Write-Host "  4. Start-Service $ServiceName"
  Write-Host "  5. Get-EventLog -LogName Application -Source $ServiceName  # 或看 $Prefix\logs"
  Write-Host ""
  Write-Host "卸载(只注销服务,不删文件): .\uninstall.ps1"
}
catch {
  Cleanup-OnFail
  throw
}
