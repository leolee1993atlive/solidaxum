#!/usr/bin/env bash
# @REPO_NAME@ Docker bundle - stop services
# Usage: ./stop.sh [--suffix proj1]
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

docker compose down

echo "Stopped. Data is preserved in ./data/ and ./logs/."
