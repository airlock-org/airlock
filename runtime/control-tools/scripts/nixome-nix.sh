#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
source "$ROOT/runtime/control-tools/scripts/lib/nixome-env.sh"

exec nix "$@"
