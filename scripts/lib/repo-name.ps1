#!/usr/bin/env pwsh
param(
    [string]$ProjectDir
)

$ErrorActionPreference = 'Stop'

# 产物名(repo 名)解析,供二进制/服务/安装包/镜像命名统一取值。
# 优先级:
#   ① 仓库根的 .repo-name 覆盖文件(一行产品名) —— fork 项目让产物跟随
#      自己的牌子而非上游 remote 时用,整个项目只改这一处;
#   ② git remote URL(剥离 .git):origin → 任意第一个 remote;
#   ③ 目录名兜底。
#
# 必须走 Write-Output(PowerShell 输出流),不能用 [Console]::Out.Write ——
# 后者直接写进程 stdout 句柄,绕过管道,调用方的 $x = & repo-name.ps1 会拿到
# $null,名字只是打在屏幕上。这与 repo-name.sh 的 printf 不同:bash 的 $(...)
# 捕获的就是 stdout,直译过来会失效。

function Resolve-RepoName([string]$Dir) {
    $root = $null
    try { $root = (git -C "$Dir" rev-parse --show-toplevel 2>$null) } catch { $root = $null }

    if ($root -and (Test-Path (Join-Path $root '.repo-name'))) {
        $override = (Get-Content (Join-Path $root '.repo-name') -Raw).Trim()
        if ($override) {
            Write-Output $override
            return
        }
    }

    $url = $null
    try { $url = git -C "$Dir" remote get-url origin 2>$null } catch { $url = $null }
    if (-not $url) {
        try {
            $first = (git -C "$Dir" remote 2>$null | Select-Object -First 1)
            if ($first) { $url = git -C "$Dir" remote get-url $first 2>$null }
        } catch { $url = $null }
    }
    if ($url) {
        $name = Split-Path -Leaf $url
        Write-Output ($name -replace '\.git$', '')
    } else {
        Write-Output (Split-Path -Leaf $Dir)
    }
}

if ($ProjectDir) {
    Resolve-RepoName $ProjectDir
} else {
    $repo_root = (Resolve-Path "$PSScriptDir\..\..").Path
    Resolve-RepoName $repo_root
}
