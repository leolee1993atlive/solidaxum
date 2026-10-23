#!/usr/bin/env bash
# sea-orm 依赖唯一性守卫。违反则退出非零,供 CI / 本地使用。
#
# 语义:业务 crate 的 Cargo.toml 里,任何 section([dependencies] /
# [dev-dependencies] / [features] / 其他)出现 sea-orm / sea_orm 依赖行即红;
# 注释行(#)不判。白名单(设计 6.1 + 普查):
#   ① crates/app-data/Cargo.toml      本尊——整个 workspace 唯一的 sea-orm
#                                     [dependencies] 声明处,全段允许
#   ② crates/dm/Cargo.toml 仅 [dev-dependencies] 段
#     e2e 例外(注释锚:该文件 :32-34):vendored sea-orm(fork)依赖 dm
#     crate,正向依赖会成环;Cargo 允许 dev-dependency 反向成环,故只有
#     e2e 测试可以经 SeaORM 走达梦。
# 根 Cargo.toml 的 [workspace.dependencies] 是声明处而非消费点,不在扫描范围。
#
# 解析形态:除「键 = 值」依赖行(含 dotted key,如 sea-orm.workspace = true)
# 外,内联表头 [dependencies.sea-orm] 同样判——它等价于在该段声明依赖;
# 表头按点号拆分,首段为 section 名、末段为依赖键(如
# [dev-dependencies.sea-orm] → 段 dev-dependencies / 键 sea-orm)。
#
# 与 grep 管道守卫的差异:按段判定要逐行跟踪 section,故用 awk 逐文件解析,
# 文件路径来自 shell glob 展开(POSIX 分隔符),不经 grep 递归输出——
# check-legacy-usage.sh 头注的 ugrep WIN64 反斜杠路径坑在此结构下不存在,
# 因此无 tr 归一化步骤(ps1 版对应位置用 [\\/] 双分隔符正则,同理)。
#
# 用法: check-sea-orm-uniqueness.sh [solidaxum-rs-path]   (默认: 脚本同级的 ../solidaxum-rs)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${1:-$SCRIPT_DIR/../solidaxum-rs}"

if [ ! -d "$ROOT/crates" ]; then
  echo "❌ 找不到 $ROOT/crates —— 请传入正确的 solidaxum-rs 路径" >&2
  exit 2
fi

tomls=( "$ROOT"/crates/*/Cargo.toml )
if [ ! -f "${tomls[0]}" ]; then
  echo "❌ $ROOT/crates 下未发现任何 */Cargo.toml" >&2
  exit 2
fi

# 逐行跟踪 section;命中即输出「文件:行号: [段] 原文」,白名单在 awk 内判
hits=$(awk '
  function hit(base) {
    f = FILENAME
    sub(/.*crates\//, "", f)          # 贪婪匹配:锚定路径中最后一个 crates/
    sub(/\/Cargo\.toml$/, "", f)      # f = crate 名
    allowed = (f == "app-data") || (f == "dm" && base == "dev-dependencies")
    if (!allowed) {
      printf "%s:%d: [%s] %s\n", FILENAME, FNR, base, $0
      bad = 1
    }
  }
  /^[[:space:]]*#/ { next }           # 注释行不判
  {
    line = $0
    sub(/^[[:space:]]+/, "", line)
    if (line ~ /^\[/) {               # 段头(含内联表头 [段.键])
      sec = line
      gsub(/^\[+|\]+[[:space:]]*(#.*)?$/, "", sec)
      n = split(sec, parts, ".")
      key = parts[n]
      base = (n > 1) ? parts[1] : sec
      if (key == "sea-orm" || key == "sea_orm") hit(base)
      next
    }
    if (match(line, /^[^=]+=/)) {     # 「键 = 值」依赖行(dotted key 取首段)
      key = substr(line, 1, RLENGTH - 1)
      sub(/[[:space:]]+$/, "", key)
      gsub(/"/, "", key)
      split(key, parts, ".")
      if (parts[1] == "sea-orm" || parts[1] == "sea_orm") hit(sec)
      next
    }
  }
  END { if (bad) exit 1 }
' "${tomls[@]}" || true)

if [ -n "$hits" ]; then
  echo "❌ 业务 crate 的 Cargo.toml 出现 sea-orm 依赖(数据访问必须经 app-data 漏斗," >&2
  echo "   白名单仅:app-data 全段 / dm [dev-dependencies]):" >&2
  echo "$hits" >&2
  exit 1
fi

echo "✅ sea-orm 依赖唯一(app-data 本尊 + dm [dev-dependencies] e2e 例外)"
