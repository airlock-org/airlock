#!/usr/bin/env bash
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
STATUS=0

mkdir -p "$LOG_DIR"

remember_failure() {
  rc="$1"

  if [ "$STATUS" -eq 0 ]; then
    STATUS="$rc"
  fi
}

run_logged() {
  name="$1"
  shift

  log="$LOG_DIR/$name.log"
  printf '==> %s\n' "$name"

  if "$@" > "$log" 2>&1; then
    printf 'ok: %s\n' "$name"
  else
    rc="$?"
    remember_failure "$rc"
    printf 'failed: %s (exit %s)\n' "$name" "$rc" >&2
    printf 'log: %s\n' "$log" >&2
    tail -n 80 "$log" >&2
  fi
}

run_logged flake-check-no-build nix flake check "$ROOT" --no-build --show-trace
run_logged module-contract nix build "$ROOT#checks.x86_64-linux.module-contract" --print-build-logs --show-trace
run_logged config-archive-package nix build "$ROOT#packages.x86_64-linux.nixome-config-archive-candidate" --print-build-logs --show-trace
run_logged config-vms-smoke-package nix build "$ROOT#packages.x86_64-linux.nixome-config-vms-smoke" --print-build-logs --show-trace
run_logged observe-package nix build "$ROOT#packages.x86_64-linux.nixome-observe-dom0" --print-build-logs --show-trace
run_logged reconcile-package nix build "$ROOT#packages.x86_64-linux.nixome-qubes-reconcile" --print-build-logs --show-trace
run_logged config-topology-eval "$ROOT/tests/config-topology-eval.sh"
run_logged host-capabilities "$ROOT/tests/host-capabilities.sh" --json "$LOG_DIR/host-capabilities.json"
run_logged reconcile-static "$ROOT/tests/reconcile-topology-static.sh"

if [ "${NIXOME_RUN_QUBES_SMOKE:-0}" = "1" ]; then
  run_logged qubes-smoke "$ROOT/tests/qubes-smoke.sh"
else
  printf '{"schema":"nixome.qubes-smoke.v1","status":"skipped","reason":"set NIXOME_RUN_QUBES_SMOKE=1 on a Xen dom0 target"}\n' > "$LOG_DIR/qubes-smoke.json"
  printf 'skipped: qubes-smoke (set NIXOME_RUN_QUBES_SMOKE=1 on a Xen dom0 target)\n'
fi

printf 'logs: %s\n' "$LOG_DIR"
exit "$STATUS"
