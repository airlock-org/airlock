#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"

mkdir -p "$LOG_DIR"
cd "$ROOT"

if [ "$#" -eq 0 ] || [[ "${1:-}" == -* ]]; then
  set -- tests "$@"
fi

exec uv run pytest "$@" \
  --durations=20 \
  --junitxml "$LOG_DIR/pytest-junit.xml" \
  --json-report \
  --json-report-file "$LOG_DIR/pytest-report.json" \
  "$@"
