#!/usr/bin/env bash
# app_data::legacy 引用守卫:legacy 过渡模块已删除,全仓零容忍。
# 违反则退出非零,供 CI / 本地使用。
#
# 语义:模块已删除 —— 全仓(生产 src
# 与 tests)任何 `app_data::legacy` 引用即红;模块文件本身存在亦红
# (复活模块而守卫不改,等于裸奔)。引用即编译错,本守卫做双保险。
#
# Windows Git-Bash 注意:本机 grep 可能是 Windows 原生版(ugrep WIN64),
# 递归输出反斜杠路径,'[/tests/]' 类过滤会被 '\tests\' 绕过——过滤前先
# tr 归一化为 '/'。vendor/、target/ 不在 crates/ 扫描范围内,仍显式兜底。
#
# 用法: check-legacy-usage.sh [solidaxum-rs-path]   (默认: 脚本同级的 ../solidaxum-rs)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${1:-$SCRIPT_DIR/../solidaxum-rs}"

if [ ! -d "$ROOT/crates" ]; then
  echo "❌ 找不到 $ROOT/crates —— 请传入正确的 solidaxum-rs 路径" >&2
  exit 2
fi

# 零容忍第一面:模块本体必须不存在
if [ -f "$ROOT/crates/app-data/src/legacy.rs" ]; then
  echo "❌ crates/app-data/src/legacy.rs 仍存在 —— 该模块应已删除" >&2
  exit 1
fi

# 零容忍第二面:全仓(src + tests)零引用,无允许面
hits=$(grep -rn 'app_data::legacy' "$ROOT/crates" --include='*.rs' \
    | tr '\\' '/' \
    | grep -v '/vendor/\|/target/' \
    || true)

if [ -n "$hits" ]; then
  echo "❌ 检出 app_data::legacy 引用 —— legacy 模块已删除,全仓零容忍:" >&2
  echo "$hits" >&2
  exit 1
fi

echo "✅ app_data::legacy 零容忍通过:模块不存在,全仓(含 tests)零引用"
