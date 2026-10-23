#!/usr/bin/env bash
# solidaxum 构建脚本 - Linux
# 用法: ./build-linux.sh [--backend] [--frontend] [--all] [--version 1.0.0] [--suffix proj1] [--force]

set -euo pipefail

# ---------- 常量 ----------

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TEMPLATES_DIR="$SCRIPT_DIR/templates"
PAYLOAD_DIR="$REPO_ROOT/scripts/installer-payloads"
STAGING_DIR="$SCRIPT_DIR/staging"
DIST_DIR="$REPO_ROOT/dist"

# REPO_NAME 只用于产物命名(二进制/服务/包名);目录是 monorepo 固定布局,
# 不随 repo 名走(后端恒 solidaxum-rs,前端恒 solidaxum-ui)。
BACKEND_DIR="$REPO_ROOT/solidaxum-rs"
REPO_NAME="$(bash "$REPO_ROOT/scripts/lib/repo-name.sh" "$BACKEND_DIR")"
FRONTEND_DIR="$REPO_ROOT/solidaxum-ui"

# ---------- 参数 ----------
DO_BACKEND=false
DO_FRONTEND=false
VERSION=""
SUFFIX=""
FORCE=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --backend)  DO_BACKEND=true; shift ;;
    --frontend) DO_FRONTEND=true; shift ;;
    --all)      DO_BACKEND=true; DO_FRONTEND=true; shift ;;
    --version)  VERSION="$2"; shift 2 ;;
    --suffix)   SUFFIX="$2"; shift 2 ;;
    --force)    FORCE=true; shift ;;
    -h|--help)
      grep '^# 用法' "$0" | head -n1; exit 0 ;;
    *) echo "未知参数: $1" >&2; exit 2 ;;
  esac
done
SVC_SUFFIX="${SUFFIX:+-$SUFFIX}"
$DO_BACKEND || $DO_FRONTEND || { DO_BACKEND=true; DO_FRONTEND=true; }

# 版本号
if [[ -z "$VERSION" ]]; then
  if git -C "$REPO_ROOT" rev-parse 2>/dev/null >/dev/null; then
    VERSION=$(git -C "$REPO_ROOT" describe --always --dirty 2>/dev/null || echo "0.0.0+local")
  else
    VERSION="0.0.0+local"
  fi
fi

# 清理 staging
trap 'rm -rf "$STAGING_DIR"' EXIT
rm -rf "$STAGING_DIR"
mkdir -p "$STAGING_DIR" "$DIST_DIR"

# ---------- 工具函数 ----------
build_timestamp() { date -u +'%Y-%m-%dT%H:%M:%SZ'; }

write_version_file() {
  local out="$1"
  echo "${VERSION}+$(build_timestamp)" > "$out"
}

check_dist_collision() {
  local zip_path="$1"
  if [[ -e "$zip_path" && "$FORCE" != "true" ]]; then
    echo "已存在: $zip_path (用 --force 覆盖)" >&2
    exit 1
  fi
  rm -f "$zip_path" "${zip_path}.sha256"
}

sha256_file() {
  ( cd "$(dirname "$1")" && sha256sum "$(basename "$1")" ) > "$1.sha256"
}

# ---------- 后端 ----------
# 用 cargo-zigbuild pin glibc 到 2.28,覆盖 openEuler 20+/RHEL 8+/Ubuntu 20.04+。
BUILD_TARGET="x86_64-unknown-linux-gnu"
GLIBC_MIN="2.28"

ensure_zigbuild() {
  command -v cargo-zigbuild >/dev/null \
    || { echo "缺少 cargo-zigbuild。装:cargo install --locked cargo-zigbuild" >&2; exit 1; }
  # zigbuild 找 zig 顺序:CARGO_ZIGBUILD_ZIG_PATH → PATH 里 zig → python -m ziglang
  if ! command -v zig >/dev/null \
     && [[ -z "${CARGO_ZIGBUILD_ZIG_PATH:-}" ]] \
     && ! python3 -c "import ziglang" 2>/dev/null; then
    # ziglang ≥ 0.16 的 entry point 是 python-zig 不是 zig,自动兜底
    if command -v python-zig >/dev/null; then
      export CARGO_ZIGBUILD_ZIG_PATH="$(command -v python-zig)"
    else
      echo "缺少 zig。装法二选一:" >&2
      echo "  uv tool install ziglang  &&  ln -sf ~/.local/bin/python-zig ~/.local/bin/zig" >&2
      echo "  或从 https://ziglang.org/download/ 下 zig tarball 放 PATH" >&2
      exit 1
    fi
  fi
  rustup target list --installed 2>/dev/null | grep -q "^${BUILD_TARGET}$" \
    || rustup target add "$BUILD_TARGET"
}

# dm crate 无条件链进二进制,链接期需要 unixODBC 的开发库(libodbc.so 符号链接)。
# 缺了的话 zig cc 只会甩一句 "unable to find library -lodbc",对构建者不是人话。
ensure_unixodbc() {
  pkg-config --exists odbc 2>/dev/null && return 0
  [[ -e /usr/include/sqlext.h ]] && return 0
  echo "缺少 unixODBC 开发库(dm crate 需要链接 libodbc)。" >&2
  echo "  装:yum install unixODBC-devel   或   apt install unixodbc-dev" >&2
  exit 1
}

build_backend() {
  echo "===== 构建后端 (target=${BUILD_TARGET}.${GLIBC_MIN}) ====="
  ensure_zigbuild
  ensure_unixodbc

  ( cd "$BACKEND_DIR" && \
    cargo zigbuild --release --target "${BUILD_TARGET}.${GLIBC_MIN}" )
  # cargo 的 bin 名恒为 solidaxum(Cargo.toml 静态声明);产物按 REPO_NAME 改名落位
  local bin="$BACKEND_DIR/target/${BUILD_TARGET}/release/solidaxum"
  [[ -x "$bin" ]] || { echo "后端编译产物不存在: $bin" >&2; exit 1; }

  local stage="$STAGING_DIR/${REPO_NAME}${SVC_SUFFIX}"
  mkdir -p "$stage/templates/systemd"
  install -m 755 "$bin" "$stage/$REPO_NAME"
  install -m 644 "$TEMPLATES_DIR/config.toml.example" "$stage/config.toml.example"
  # 模板源文件名恒为 solidaxum.*(与 cargo bin 同理);包内按 REPO_NAME 落位,
  # install.sh 按 $PAYLOAD_DIR/templates/systemd/${REPO_NAME}.service 取用
  install -m 644 "$TEMPLATES_DIR/systemd/solidaxum.service" "$stage/templates/systemd/$REPO_NAME.service"
  install -m 755 "$PAYLOAD_DIR/backend-linux/install.sh"   "$stage/install.sh"
  install -m 755 "$PAYLOAD_DIR/backend-linux/uninstall.sh" "$stage/uninstall.sh"
  install -m 644 "$PAYLOAD_DIR/backend-linux/README.txt"   "$stage/README.txt"
  sed -i "s|@REPO_NAME@|$REPO_NAME|g" "$stage/README.txt"
  write_version_file "$stage/VERSION"

  # 若同次也跑了前端,把 staging/web 折进综合包的 web/
  if [[ -d "$STAGING_DIR/web" ]]; then
    cp -a "$STAGING_DIR/web" "$stage/web"
  fi

  # 命名约定:全量包(同批构了前端,web/ 已折进包)不带段;纯后端包带
  # -backend 段 —— 两类包可同版本共存,互不覆盖(现场:全量首装,后端包
  # 解压覆盖更新)。
  local kind=""
  if [[ ! -d "$STAGING_DIR/web" ]]; then
    kind="-backend"
  fi
  local zip="$DIST_DIR/${REPO_NAME}${SVC_SUFFIX}${kind}-${VERSION}-linux-x64.zip"
  check_dist_collision "$zip"
  ( cd "$STAGING_DIR" && zip -qr "$zip" "${REPO_NAME}${SVC_SUFFIX}/" )
  sha256_file "$zip"
  echo "✓ $zip"
}

# ---------- 前端(SPA 静态) ----------
build_frontend() {
  echo "===== 构建前端(SPA) ====="

  grep -q "adapter-static" "$FRONTEND_DIR/svelte.config.js" \
    || { echo "svelte.config.js 必须是 adapter-static(SPA)" >&2; exit 1; }

  # VITE_APP_NAME 让 UI 品牌(标题/侧栏/登录页)跟随产物名(brand.ts 兜底 SolidAxum)
  ( cd "$FRONTEND_DIR" && export VITE_APP_NAME="$REPO_NAME" && \
    pnpm install --frozen-lockfile && pnpm build )

  local build="$FRONTEND_DIR/build"
  [[ -f "$build/index.html" ]] || { echo "前端构建产物缺 index.html" >&2; exit 1; }
  [[ ! -d "$build/server" ]]   || { echo "检测到 build/server,前端不是纯 SPA" >&2; exit 1; }

  # 静态产物放入 staging/web,供综合包与 UI 包共用
  local web="$STAGING_DIR/web"
  mkdir -p "$web"
  cp -a "$build"/. "$web/"

  # UI 包(前端单独更新用)
  local zip="$DIST_DIR/${REPO_NAME}-ui-${VERSION}-linux-x64.zip"
  check_dist_collision "$zip"
  ( cd "$STAGING_DIR" && zip -qr "$zip" web/ )
  sha256_file "$zip"
  echo "✓ $zip"
}

# ---------- 主流程 ----------
echo "Repo: $REPO_NAME"
echo "Version: $VERSION"
$DO_FRONTEND && build_frontend
$DO_BACKEND  && build_backend
echo "完成。产物在 $DIST_DIR/"
