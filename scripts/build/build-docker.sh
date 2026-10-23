#!/usr/bin/env bash
# Docker 构建脚本
# 用法: ./build-docker.sh [--version 1.0.0] [--suffix proj1] [--force] [--skip-host-build]

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PAYLOAD_DIR="$REPO_ROOT/scripts/installer-payloads/docker"
DOCKER_DIR="$REPO_ROOT/docker"
STAGING_DIR="$SCRIPT_DIR/staging"
DIST_DIR="$REPO_ROOT/dist"

VERSION=""
SUFFIX=""
FORCE=false
SKIP_HOST_BUILD=false
ORIGINAL_ARGS=("$@")
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)         VERSION="$2"; shift 2 ;;
    --suffix)          SUFFIX="$2"; shift 2 ;;
    --force)           FORCE=true; shift ;;
    --skip-host-build) SKIP_HOST_BUILD=true; shift ;;
    -h|--help)         grep '^# 用法' "$0" | head -n1; exit 0 ;;
    *) echo "未知参数: $1" >&2; exit 2 ;;
  esac
done
SVC_SUFFIX="${SUFFIX:+-$SUFFIX}"

# REPO_NAME 只用于产物命名(镜像/服务名);目录是 monorepo 固定布局,
# 不随 repo 名走(后端恒 solidaxum-rs,前端恒 solidaxum-ui)。
BACKEND_DIR="$REPO_ROOT/solidaxum-rs"
REPO_NAME="$(bash "$REPO_ROOT/scripts/lib/repo-name.sh" "$BACKEND_DIR")"
FRONTEND_DIR="$REPO_ROOT/solidaxum-ui"
DOCKER_IMAGE="${REPO_NAME}${SVC_SUFFIX}"

if [[ -z "$VERSION" ]]; then
  if git -C "$REPO_ROOT" rev-parse 2>/dev/null >/dev/null; then
    VERSION=$(git -C "$REPO_ROOT" describe --always --dirty 2>/dev/null || echo "0.0.0+local")
  else
    VERSION="0.0.0+local"
  fi
fi

# ---------- Docker 预检 ----------
command -v docker >/dev/null || { echo "未装 docker" >&2; exit 1; }
if ! docker info >/dev/null 2>&1; then
  if [[ $(id -u) -ne 0 ]]; then
    echo "docker 需要 root 权限, 自动用 sudo 重新执行..."
    exec sudo --preserve-env=PATH,HOME "$0" "${ORIGINAL_ARGS[@]}"
  fi
  echo "docker daemon 不可用" >&2
  exit 1
fi

# ---------- 清理 / 准备 ----------
cleanup() {
  rm -rf "$STAGING_DIR"
  if [[ -f "$REPO_ROOT/.dockerignore.bak" ]]; then
    mv "$REPO_ROOT/.dockerignore.bak" "$REPO_ROOT/.dockerignore"
  elif [[ -f "$REPO_ROOT/.dockerignore" && "${MOVED_DOCKERIGNORE:-}" == "true" ]]; then
    rm -f "$REPO_ROOT/.dockerignore"
  fi
}
trap cleanup EXIT
rm -rf "$STAGING_DIR"
mkdir -p "$STAGING_DIR/docker/images" "$DIST_DIR"

echo "Version: $VERSION"

# ---------- 宿主机构建 ----------
if [[ "$SKIP_HOST_BUILD" != "true" ]]; then
  echo "===== 宿主机构建前端 ====="
  command -v pnpm >/dev/null || { echo "未装 pnpm" >&2; exit 1; }
  # VITE_APP_NAME 让 UI 品牌跟随产物名(brand.ts 兜底 SolidAxum)
  ( cd "$FRONTEND_DIR" && export VITE_APP_NAME="$REPO_NAME" && \
    pnpm install --frozen-lockfile && pnpm build )
  test -f "$FRONTEND_DIR/build/index.html" \
    || { echo "前端构建失败" >&2; exit 1; }

  echo "===== 宿主机构建后端 ====="
  command -v cargo >/dev/null || { echo "未装 cargo" >&2; exit 1; }
  ( cd "$BACKEND_DIR" && cargo build --release )
  # cargo 的 bin 名恒为 solidaxum;镜像内按 REPO_NAME 改名落位(见 Dockerfile 模板)
  test -x "$BACKEND_DIR/target/release/solidaxum" \
    || { echo "后端构建失败" >&2; exit 1; }
else
  echo "===== 跳过宿主机构建 (--skip-host-build) ====="
fi

# 确认产物存在
test -f "$FRONTEND_DIR/build/index.html" \
  || { echo "前端产物缺失: $FRONTEND_DIR/build/index.html" >&2; exit 1; }
test -x "$BACKEND_DIR/target/release/solidaxum" \
  || { echo "后端产物缺失: $BACKEND_DIR/target/release/solidaxum" >&2; exit 1; }

# ---------- 渲染 Docker 构建文件 ----------
mkdir -p "$STAGING_DIR"
sed -e "s|@REPO_NAME@|$REPO_NAME|g" \
    "$DOCKER_DIR/backend.Dockerfile.template" \
    > "$STAGING_DIR/Dockerfile"

MOVED_DOCKERIGNORE=false
if [[ -f "$REPO_ROOT/.dockerignore" ]]; then
  cp "$REPO_ROOT/.dockerignore" "$REPO_ROOT/.dockerignore.bak"
fi
sed -e "s|@REPO_NAME@|$REPO_NAME|g" \
    "$DOCKER_DIR/.dockerignore.template" \
    > "$REPO_ROOT/.dockerignore"
MOVED_DOCKERIGNORE=true

# ---------- 构建 Docker 镜像 ----------
echo "===== 构建 Docker 镜像 ====="
if docker image inspect "${DOCKER_IMAGE}:${VERSION}" >/dev/null 2>&1; then
  echo "删除旧镜像 ${DOCKER_IMAGE}:${VERSION}"
  docker rmi "${DOCKER_IMAGE}:${VERSION}" >/dev/null
fi
docker build -f "$STAGING_DIR/Dockerfile" \
  -t "${DOCKER_IMAGE}:${VERSION}" "$REPO_ROOT"

REDIS_IMAGE="redis:7-alpine"

# ---------- 导出镜像 ----------
echo "===== 导出镜像 ====="
mkdir -p "$STAGING_DIR/docker/images"

docker save "${DOCKER_IMAGE}:${VERSION}" | gzip > "$STAGING_DIR/docker/images/backend.tar.gz"

if ! docker image inspect "$REDIS_IMAGE" >/dev/null 2>&1; then
  echo "拉取 $REDIS_IMAGE ..."
  docker pull "$REDIS_IMAGE"
fi
echo "导出 $REDIS_IMAGE ..."
docker save "$REDIS_IMAGE" | gzip > "$STAGING_DIR/docker/images/redis.tar.gz"

# ---------- 渲染 docker-compose.yml 并复制辅助文件 ----------
echo "===== 渲染 docker-compose.yml ====="
sed -e "s|@VERSION@|${VERSION}|g" \
    -e "s|@SVC_SUFFIX@|${SVC_SUFFIX}|g" \
    -e "s|@REPO_NAME@|${REPO_NAME}|g" \
    "$DOCKER_DIR/docker-compose.yml.template" \
    > "$STAGING_DIR/docker/docker-compose.yml"

install -m 644 "$SCRIPT_DIR/templates/config.toml.example" "$STAGING_DIR/docker/config.toml.example"
install -m 755 "$PAYLOAD_DIR/load-and-start.sh"            "$STAGING_DIR/docker/load-and-start.sh"
install -m 755 "$PAYLOAD_DIR/stop.sh"                      "$STAGING_DIR/docker/stop.sh"
install -m 644 "$PAYLOAD_DIR/README.txt"                   "$STAGING_DIR/docker/README.txt"
echo "$VERSION" > "$STAGING_DIR/docker/VERSION"

# ---------- 打包 ----------
ZIP="$DIST_DIR/${REPO_NAME}-docker${SVC_SUFFIX}-${VERSION}.zip"
if [[ -e "$ZIP" && "$FORCE" != "true" ]]; then
  echo "已存在: $ZIP (用 --force 覆盖)" >&2; exit 1
fi
rm -f "$ZIP" "$ZIP.sha256"
( cd "$STAGING_DIR" && zip -qr "$ZIP" docker/ )
( cd "$DIST_DIR" && sha256sum "$(basename "$ZIP")" > "$ZIP.sha256" )

echo "✓ $ZIP"
