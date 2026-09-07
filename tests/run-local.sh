#!/usr/bin/env bash
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
STATUS=0
RESULTS_JSONL="$LOG_DIR/test-results.jsonl"
SUMMARY_JSON="$LOG_DIR/test-summary.json"
TOTAL=0
PASSED=0
FAILED=0
SKIPPED=0

mkdir -p "$LOG_DIR"
: > "$RESULTS_JSONL"

json_escape() {
  printf '%s' "$1" | sed \
    -e 's/\\/\\\\/g' \
    -e 's/"/\\"/g' \
    -e ':a;N;$!ba;s/\n/\\n/g'
}

json_string() {
  printf '"'
  json_escape "$1"
  printf '"'
}

remember_failure() {
  rc="$1"

  if [ "$STATUS" -eq 0 ]; then
    STATUS="$rc"
  fi
}

record_result() {
  name="$1"
  status="$2"
  rc="$3"
  log="$4"

  {
    printf '{"name":'
    json_string "$name"
    printf ',"status":'
    json_string "$status"
    printf ',"exit_code":%s,"log":' "$rc"
    json_string "$log"
    printf '}\n'
  } >> "$RESULTS_JSONL"
}

run_logged() {
  name="$1"
  shift

  log="$LOG_DIR/$name.log"
  TOTAL=$((TOTAL + 1))
  printf '==> %s\n' "$name"

  if "$@" > "$log" 2>&1; then
    printf 'ok: %s\n' "$name"
    PASSED=$((PASSED + 1))
    record_result "$name" passed 0 "$log"
  else
    rc="$?"
    remember_failure "$rc"
    FAILED=$((FAILED + 1))
    printf 'failed: %s (exit %s)\n' "$name" "$rc" >&2
    printf 'log: %s\n' "$log" >&2
    tail -n 80 "$log" >&2
    record_result "$name" failed "$rc" "$log"
  fi
}

record_skipped() {
  name="$1"
  reason="$2"
  log="$3"

  TOTAL=$((TOTAL + 1))
  SKIPPED=$((SKIPPED + 1))
  {
    printf '{"name":'
    json_string "$name"
    printf ',"status":"skipped","exit_code":0,"reason":'
    json_string "$reason"
    printf ',"log":'
    json_string "$log"
    printf '}\n'
  } >> "$RESULTS_JSONL"
}

write_summary() {
  status="passed"
  [ "$FAILED" -eq 0 ] || status="failed"

  {
    printf '{"schema":"nixome.local-test-summary.v1"'
    printf ',"status":'
    json_string "$status"
    printf ',"total":%s' "$TOTAL"
    printf ',"passed":%s' "$PASSED"
    printf ',"failed":%s' "$FAILED"
    printf ',"skipped":%s' "$SKIPPED"
    printf ',"log_dir":'
    json_string "$LOG_DIR"
    printf ',"results_jsonl":'
    json_string "$RESULTS_JSONL"
    printf ',"results":['
    first=1
    while IFS= read -r line; do
      [ -n "$line" ] || continue
      if [ "$first" = "1" ]; then
        first=0
      else
        printf ','
      fi
      printf '%s' "$line"
    done < "$RESULTS_JSONL"
    printf ']}\n'
  } > "$SUMMARY_JSON"
}

run_logged flake-check-no-build nix flake check "$ROOT" --no-build --show-trace
run_logged module-contract nix build "$ROOT#checks.x86_64-linux.module-contract" --print-build-logs --show-trace
run_logged config-apply-package nix build "$ROOT#packages.x86_64-linux.nixome-config-apply" --print-build-logs --show-trace
run_logged config-archive-package nix build "$ROOT#packages.x86_64-linux.nixome-config-archive-candidate" --print-build-logs --show-trace
run_logged config-boot-verify-package nix build "$ROOT#packages.x86_64-linux.nixome-config-boot-verify" --print-build-logs --show-trace
run_logged config-status-package nix build "$ROOT#packages.x86_64-linux.nixome-config-status" --print-build-logs --show-trace
run_logged config-vms-smoke-package nix build "$ROOT#packages.x86_64-linux.nixome-config-vms-smoke" --print-build-logs --show-trace
run_logged observe-package nix build "$ROOT#packages.x86_64-linux.nixome-observe-dom0" --print-build-logs --show-trace
run_logged setup-config-package nix build "$ROOT#packages.x86_64-linux.nixome-setup-config" --print-build-logs --show-trace
run_logged setup-config-runtime-path "$ROOT/tests/setup-config-runtime-path.sh"
run_logged gc-nixome-store "$ROOT/tests/gc-nixome-store.sh"
run_logged config-topology-eval "$ROOT/tests/config-topology-eval.sh"
run_logged config-staging-validate "$ROOT/tests/config-staging-validate.sh"
run_logged qrexec-policy-path "$ROOT/tests/qrexec-policy-path.sh"
run_logged qube-guest-policy-probe "$ROOT/tests/qube-guest-policy-probe.sh"
run_logged topology-schema "$ROOT/tests/topology-schema.sh"
run_logged system-schema "$ROOT/tests/system-schema.sh"
run_logged config-candidate-flake "$ROOT/tests/config-candidate-flake.sh"
run_logged config-validator-candidate "$ROOT/tests/config-validator-candidate.sh"
run_logged config-vault-deployment-archive "$ROOT/tests/config-vault-deployment-archive.sh"
run_logged config-boot-verify "$ROOT/tests/config-boot-verify.sh"
run_logged config-status "$ROOT/tests/config-status.sh"
run_logged config-apply-template-build-ref "$ROOT/tests/config-apply-template-build-ref.sh"
run_logged config-system-apply "$ROOT/tests/config-system-apply.sh"
run_logged config-system-apply-boot "$ROOT/tests/config-system-apply-boot.sh"
run_logged run-nested-gui-smoke-static "$ROOT/tests/run-nested-gui-smoke-static.sh"
run_logged host-capabilities "$ROOT/tests/host-capabilities.sh" --json "$LOG_DIR/host-capabilities.json"
run_logged setup-config-static "$ROOT/tests/setup-config-static.sh"
run_logged setup-config-nix-native-unsupported "$ROOT/tests/setup-config-nix-native-unsupported.sh"
run_logged setup-config-qrexec-policy "$ROOT/tests/setup-config-qrexec-policy.sh"
run_logged setup-config-idempotent "$ROOT/tests/setup-config-idempotent.sh"
run_logged setup-config-restart-actions "$ROOT/tests/setup-config-restart-actions.sh"
run_logged setup-config-pref-order "$ROOT/tests/setup-config-pref-order.sh"
run_logged setup-config-empty-global-pref "$ROOT/tests/setup-config-empty-global-pref.sh"

if [ "${NIXOME_RUN_QUBES_SMOKE:-0}" = "1" ]; then
  run_logged qubes-smoke "$ROOT/tests/qubes-smoke.sh"
else
  skip_reason="set NIXOME_RUN_QUBES_SMOKE=1 on a Xen dom0 target"
  printf '{"schema":"nixome.qubes-smoke.v1","status":"skipped","reason":"%s"}\n' "$skip_reason" > "$LOG_DIR/qubes-smoke.json"
  printf 'skipped: qubes-smoke (set NIXOME_RUN_QUBES_SMOKE=1 on a Xen dom0 target)\n'
  record_skipped qubes-smoke "$skip_reason" "$LOG_DIR/qubes-smoke.json"
fi

write_summary
printf 'logs: %s\n' "$LOG_DIR"
printf 'summary: %s\n' "$SUMMARY_JSON"
exit "$STATUS"
