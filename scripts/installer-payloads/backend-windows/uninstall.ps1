# @REPO_NAME@ backend - Windows uninstall(只注销服务,保留文件)
[CmdletBinding()]
param(
  [string]$Suffix = ''
)

$ErrorActionPreference = 'Stop'

$Prefix   = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoName = Split-Path -Leaf $Prefix
$ServiceName = "${RepoName}$(if ($Suffix) { '-' + $Suffix } else { '' })"

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { throw "需要管理员身份运行 PowerShell" }

if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
  Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
  $winsw = "$Prefix\service\$ServiceName.exe"
  if (Test-Path $winsw) {
    & $winsw uninstall | Out-Null
  } else {
    Write-Warning "未找到 $winsw,改用 sc.exe 注销服务 $ServiceName"
    & sc.exe delete $ServiceName | Out-Null
  }
  Write-Host "OK 已注销服务 $ServiceName"
} else {
  Write-Host "服务 $ServiceName 未注册,跳过。"
}

Write-Host ""
Write-Host "OK 卸载完成(文件全部保留)"
Write-Host "如需彻底清理,手工删除整个文件夹: $Prefix"
