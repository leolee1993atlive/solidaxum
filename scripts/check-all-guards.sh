#!/usr/bin/env bash
# 守卫聚合入口:依次跑四个 .sh 守卫,汇总 ✅/❌ 与退出码,
# 任一红则总体退出 1。供 CI / 本地一键自检。
#
# 聚合清单(四个守卫,见各自头注释的语义与白名单):
#   check-legacy-usage.sh          app_data::legacy 过渡命名空间引用
#   check-no-raw-sql.sh            业务 crate 原生 SQL 归零
#   check-sea-orm-uniqueness.sh    sea-orm 依赖唯一性(app-data 漏斗)
#   check-dm-backend-usage.sh      DbBackend::Dm 标识符唯一性(方言收编点)
#
# 说明:不因首个红中断(用 set -u 不用 -e)——汇总表需要全量结果;
# .ps1 版本是 Windows 本地等价面,不在此聚合内(有 pwsh 的环境可逐个跑)。
#
# 用法: check-all-guards.sh [solidaxum-rs-path]   (默认: 脚本同级的 ../solidaxum-rs)
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${1:-$SCRIPT_DIR/../solidaxum-rs}"

if [ ! -d "$ROOT/crates" ]; then
  echo "❌ 找不到 $ROOT/crates —— 请传入正确的 solidaxum-rs 路径" >&2
  exit 2
fi

guards=(
  check-legacy-usage.sh
  check-no-raw-sql.sh
  check-sea-orm-uniqueness.sh
  check-dm-backend-usage.sh
)

results=()
failed=0
for g in "${guards[@]}"; do
  echo "━━ $g ━━"
  if bash "$SCRIPT_DIR/$g" "$ROOT"; then
    results+=("✅ $g")
  else
    rc=$?
    results+=("❌ $g(退出码 $rc)")
    failed=1
  fi
done

echo
echo "━━ 守卫汇总 ━━"
printf '%s\n' "${results[@]}"
if [ "$failed" -ne 0 ]; then
  echo "❌ 存在红色守卫,详见上方明细"
  exit 1
fi
echo "✅ 全部 ${#guards[@]} 个守卫绿"
