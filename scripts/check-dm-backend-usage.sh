#!/usr/bin/env bash
# DbBackend::Dm 唯一性守卫。违反则退出非零,供 CI / 本地使用。
#
# 语义:DatabaseBackend::Dm / DbBackend::Dm 标识符只允许出现在达梦方言收编点,
# 其余任何 crate 的 .rs 命中即红。白名单(设计 9 节 + 口径修正):
#   ① crates/app-data/src/dialect.rs  方言出口(设计原文)
#   ② crates/app-data/src/meta.rs     口径修正披露:设计原文只列 dialect,
#      2026-08-19 普查实况含 MetadataReader 的 4 处 DbBackend::Dm 分支
#      (设计步 3 的合法消费),按实况补入
#   ③ crates/app-data/tests/          app-data 自身测试(meta_dm_test 等)
#   ④ crates/dm/tests/                dm crate 的 e2e / 单测
# vendor/、target/ 排除。
#
# Windows Git-Bash 注意:本机 grep 可能是 Windows 原生版(ugrep WIN64),
# 递归输出反斜杠路径(实测如 D:/.../crates\app-data\src\...),'/tests/' 类
# 过滤会被 '\tests\' 绕过——过滤前先 tr 归一化为 '/'(处置与坑注释照抄
# check-legacy-usage.sh)。
#
# 用法: check-dm-backend-usage.sh [solidaxum-rs-path]   (默认: 脚本同级的 ../solidaxum-rs)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${1:-$SCRIPT_DIR/../solidaxum-rs}"

if [ ! -d "$ROOT/crates" ]; then
  echo "❌ 找不到 $ROOT/crates —— 请传入正确的 solidaxum-rs 路径" >&2
  exit 2
fi

# 路径归一化(反斜杠 -> 正斜杠)后再套白名单过滤
# count.rs 豁免口径:其内联 #[cfg(test)] 里有 `sql(DbBackend::Dm).is_none()`
# 负断言(达梦不猜 SQL 的守恒测试),与 tests/ 目录同属"app-data 自身测试"。
hits=$(grep -rnE 'DatabaseBackend::Dm|DbBackend::Dm' "$ROOT/crates" --include='*.rs' \
    | tr '\\' '/' \
    | grep -v '/vendor/\|/target/' \
    | grep -v '/crates/app-data/src/dialect\.rs:' \
    | grep -v '/crates/app-data/src/meta\.rs:' \
    | grep -v '/crates/app-data/src/count\.rs:' \
    | grep -v '/crates/app-data/tests/' \
    | grep -v '/crates/dm/tests/' \
    || true)

if [ -n "$hits" ]; then
  echo "❌ DbBackend::Dm 出现在白名单之外(达梦分支收编点仅:app-data dialect/meta / app-data tests / dm tests):" >&2
  echo "$hits" >&2
  exit 1
fi

echo "✅ DbBackend::Dm 使用合规(允许面:app-data dialect+meta(MetadataReader,口径修正)/ app-data tests / dm tests)"
