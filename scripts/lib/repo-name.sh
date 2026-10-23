#!/usr/bin/env bash
set -euo pipefail

# 产物名(repo 名)解析,供二进制/服务/安装包/镜像命名统一取值。
# 优先级:
#   ① 仓库根的 .repo-name 覆盖文件(一行产品名) —— fork 项目让产物跟随
#      自己的牌子而非上游 remote 时用,整个项目只改这一处;
#   ② git remote URL(剥离 .git):origin → 任意第一个 remote;
#   ③ 目录名兜底。

resolve_from_dir() {
    local dir="$1" root override url first_remote
    root="$(git -C "$dir" rev-parse --show-toplevel 2>/dev/null || true)"
    if [ -n "$root" ] && [ -f "$root/.repo-name" ]; then
        override="$(tr -d ' \t\r\n' < "$root/.repo-name")"
        if [ -n "$override" ]; then
            printf '%s' "$override"
            return
        fi
    fi
    url="$(git -C "$dir" remote get-url origin 2>/dev/null || true)"
    if [ -z "$url" ]; then
        first_remote="$(git -C "$dir" remote 2>/dev/null | head -n1 || true)"
        if [ -n "$first_remote" ]; then
            url="$(git -C "$dir" remote get-url "$first_remote" 2>/dev/null || true)"
        fi
    fi
    if [ -n "$url" ]; then
        local name
        name="$(basename "$url")"
        printf '%s' "${name%.git}"
    else
        printf '%s' "$(basename "$dir")"
    fi
}

if [ -n "${1:-}" ]; then
    resolve_from_dir "$1"
else
    repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
    resolve_from_dir "$repo_root"
fi
