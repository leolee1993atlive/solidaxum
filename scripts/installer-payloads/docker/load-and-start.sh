#!/usr/bin/env bash
# @REPO_NAME@ Docker bundle - load images and start services
# Usage: ./load-and-start.sh [--suffix proj1]
set -euo pipefail

SUFFIX=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --suffix) SUFFIX="$2"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

REPO_NAME="$(basename "$(cd "$(dirname "$0")" && pwd)")"
SVC_SUFFIX="${SUFFIX:+-$SUFFIX}"
export COMPOSE_PROJECT_NAME="${REPO_NAME}${SVC_SUFFIX}"

cd "$(dirname "$0")"

[[ -f config.toml ]] || { echo "Please copy config.toml.example to config.toml and fill in the configuration first."; exit 1; }

command -v docker >/dev/null || { echo "docker not found"; exit 1; }
docker info >/dev/null 2>&1 || { echo "docker daemon is not reachable; please start docker or add your user to the docker group"; exit 1; }

mkdir -p data logs

# The backend container runs as fixed uid 10001 (see backend.Dockerfile),
# so bind-mounted data/ and logs/ must be writable by that user.
# Align ownership to 10001; idempotent if already correct.
if [[ "$(stat -c %u data)" != "10001" || "$(stat -c %u logs)" != "10001" ]]; then
  echo "Aligning data/logs ownership to container runtime user (uid 10001)..."
  if [[ $(id -u) -eq 0 ]]; then
    chown -R 10001:10001 data logs
  elif command -v sudo >/dev/null; then
    sudo chown -R 10001:10001 data logs
  else
    echo "Warning: not running as root and sudo is unavailable. If the container reports permission errors, run manually:" >&2
    echo "      chown -R 10001:10001 data logs" >&2
  fi
fi

echo "Loading images..."
# Extract image names from compose and remove any stale local copies so the loaded images are used.
IMAGES=$(grep -oP 'image:\s*\K\S+' docker-compose.yml 2>/dev/null || true)
for img in $IMAGES; do
  if docker image inspect "$img" >/dev/null 2>&1; then
    echo "Removing old local image $img"
    docker rmi "$img" >/dev/null 2>&1 || true
  fi
done

docker load -i images/backend.tar.gz
docker load -i images/redis.tar.gz

echo "Starting containers..."
docker compose up -d

echo ""
docker compose ps
echo ""
echo "Started. View logs with: docker compose logs -f"
