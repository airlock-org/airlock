#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
source "$ROOT/lifecycle/misc/tools/scripts/lib/airlock-env.sh"

exec python3 "$ROOT/lifecycle/misc/tools/scripts/repository-source.py" nix -- "$@"
