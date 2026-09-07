#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

bash -n "$ROOT/scripts/run-nested-gui-smoke.sh"
bash -n "$ROOT/scripts/nixos-config-vms-smoke.sh"

grep -F 'NIXOME_CONFIG_BUILD_EXTRA_TEMPLATE' "$ROOT/Makefile" >/dev/null
grep -F 'NIXOME_CONFIG_BUILD_EXTRA_TEMPLATE' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_CONFIG_BUILD_EXTRA_TEMPLATE' "$ROOT/scripts/nixos-config-vms-smoke.sh" >/dev/null
grep -F 'sudo -n $config_vms_env NIXOME_LOG_DIR=/var/log/nixome nixome-config-vms-smoke' \
  "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null

printf '{"schema":"nixome.run-nested-gui-smoke-static.v1","status":"passed"}\n'
