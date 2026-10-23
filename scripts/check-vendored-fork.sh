#!/usr/bin/env bash
# vendored sea-orm fork 生效性断言。违反则退出非零,供 CI / 本地使用。
#
# 守的是什么:
#   solidaxum-rs 用 [patch.crates-io] 把 sea-orm 指向 vendor/sea-orm(达梦后端所在)。
#   若有人只改根 Cargo.toml 的版本要求而不同步 vendor(`cargo upgrade` 这类工具最容易),
#   **cargo 不会报错** —— 它只发一条 warning:
#       warning: patch `sea-orm vX (…/vendor/sea-orm)` was not used in the crate graph
#   然后静默回退到 crates.io 的原版,整个达梦后端连同 fork 一起消失。
#   后续会因 `DbBackend::Dm` 不存在而编译失败,但报错离真因隔着好几层。
#
# 两条断言:
#   1. 根 Cargo.toml 声明的版本 == vendor/sea-orm 的版本(改动当下即可发现)
#   2. Cargo.lock 里 sea-orm 无 `source` 字段(= 解析到本地路径,patch 确实生效)
#
# 用法: check-vendored-fork.sh [solidaxum-rs-path]   (默认: 脚本同级的 ../solidaxum-rs)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${1:-$SCRIPT_DIR/../solidaxum-rs}"

for f in Cargo.toml Cargo.lock vendor/sea-orm/Cargo.toml; do
  if [ ! -f "$ROOT/$f" ]; then
    echo "❌ 找不到 $ROOT/$f —— 请传入正确的 solidaxum-rs 路径" >&2
    exit 2
  fi
done

# ---- 断言 1:声明版本 vs vendored 版本 ----

# 根 Cargo.toml 的 [workspace.dependencies] 里形如:
#   sea-orm = { version = "2.0.2", features = [ … ]
declared=$(grep -m1 -E '^sea-orm[[:space:]]*=[[:space:]]*\{' "$ROOT/Cargo.toml" \
           | sed -E 's/.*version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/')

# vendor/sea-orm/Cargo.toml 的 [package] version(取文件里第一个顶格 version)
vendored=$(grep -m1 -E '^version[[:space:]]*=' "$ROOT/vendor/sea-orm/Cargo.toml" \
           | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$declared" ] || [ -z "$vendored" ]; then
  echo "❌ 无法解析版本(declared='$declared' vendored='$vendored')—— 脚本的解析假设可能已过时" >&2
  exit 2
fi

if [ "$declared" != "$vendored" ]; then
  cat >&2 <<EOF
❌ sea-orm 版本不一致:
     根 Cargo.toml 声明 : $declared
     vendor/sea-orm     : $vendored

   cargo 不会为此报错,只会静默丢弃 patch、回退到 crates.io 原版,
   达梦后端随之消失。要升级请两边同步改 —— 重新 vendor 上游源码后再改版本号,
   步骤见仓库根目录 AGENTS.md「Working with the vendored sea-orm fork」。
EOF
  exit 1
fi

# ---- 断言 2:Cargo.lock 确实解析到本地路径 ----

# path / patched 依赖在 Cargo.lock 中没有 source 字段;registry 依赖则有
#   source = "registry+https://github.com/rust-lang/crates.io-index"
# 取 name = "sea-orm" 那个 package 块(从 name 行到下一个 [[package]] 为止)
lock_block=$(awk '
  /^\[\[package\]\]/       { inblock = 0 }
  /^name = "sea-orm"$/     { inblock = 1 }
  inblock                  { print }
' "$ROOT/Cargo.lock")

if [ -z "$lock_block" ]; then
  echo "❌ Cargo.lock 里找不到 sea-orm —— 依赖可能已被移除" >&2
  exit 1
fi

if echo "$lock_block" | grep -q '^source = '; then
  cat >&2 <<EOF
❌ Cargo.lock 里的 sea-orm 来自 registry,而非 vendor/sea-orm —— patch 未生效:
$(echo "$lock_block" | head -3 | sed 's/^/     /')

   跑一次 \`cargo update -p sea-orm\` 或删掉 Cargo.lock 重新解析。
EOF
  exit 1
fi

echo "✅ vendored sea-orm fork 生效(版本 $vendored,Cargo.lock 解析到本地路径)"
