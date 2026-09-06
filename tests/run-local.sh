#!/usr/bin/env bash
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
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
run_logged config-apply-package nix build "$ROOT#packages.x86_64-linux.nixome-config-apply" --print-build-logs --show-trace
run_logged config-archive-package nix build "$ROOT#packages.x86_64-linux.nixome-config-archive-candidate" --print-build-logs --show-trace
run_logged config-vms-smoke-package nix build "$ROOT#packages.x86_64-linux.nixome-config-vms-smoke" --print-build-logs --show-trace
run_logged observe-package nix build "$ROOT#packages.x86_64-linux.nixome-observe-dom0" --print-build-logs --show-trace
run_logged setup-config-package nix build "$ROOT#packages.x86_64-linux.nixome-setup-config" --print-build-logs --show-trace
run_logged setup-config-runtime-path "$ROOT/tests/setup-config-runtime-path.sh"
run_logged config-topology-eval "$ROOT/tests/config-topology-eval.sh"
run_logged qrexec-policy-path "$ROOT/tests/qrexec-policy-path.sh"
run_logged qube-guest-policy-probe "$ROOT/tests/qube-guest-policy-probe.sh"
run_logged topology-schema "$ROOT/tests/topology-schema.sh"
run_logged system-schema "$ROOT/tests/system-schema.sh"
run_logged config-candidate-flake "$ROOT/tests/config-candidate-flake.sh"
run_logged config-validator-candidate "$ROOT/tests/config-validator-candidate.sh"
run_logged config-apply-template-build-ref "$ROOT/tests/config-apply-template-build-ref.sh"
run_logged config-system-apply "$ROOT/tests/config-system-apply.sh"
run_logged host-capabilities "$ROOT/tests/host-capabilities.sh" --json "$LOG_DIR/host-capabilities.json"
run_logged setup-config-static "$ROOT/tests/setup-config-static.sh"
run_logged setup-config-qrexec-policy "$ROOT/tests/setup-config-qrexec-policy.sh"
run_logged setup-config-idempotent "$ROOT/tests/setup-config-idempotent.sh"
run_logged setup-config-empty-global-pref "$ROOT/tests/setup-config-empty-global-pref.sh"

if [ "${NIXOME_RUN_QUBES_SMOKE:-0}" = "1" ]; then
  run_logged qubes-smoke "$ROOT/tests/qubes-smoke.sh"
else
  printf '{"schema":"nixome.qubes-smoke.v1","status":"skipped","reason":"set NIXOME_RUN_QUBES_SMOKE=1 on a Xen dom0 target"}\n' > "$LOG_DIR/qubes-smoke.json"
  printf 'skipped: qubes-smoke (set NIXOME_RUN_QUBES_SMOKE=1 on a Xen dom0 target)\n'
fi

printf 'logs: %s\n' "$LOG_DIR"
exit "$STATUS"
