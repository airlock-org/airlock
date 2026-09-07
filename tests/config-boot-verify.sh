#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${NIXOME_CONFIG_BOOT_VERIFY_TEST_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/config-boot-verify}"
BIN_DIR="$WORK_DIR/bin"
LOG_DIR="$WORK_DIR/logs"
STATUS_JSON="$WORK_DIR/status.json"
FAILED_STATUS_JSON="$WORK_DIR/status-failed.json"
RESULT_JSON="$LOG_DIR/config-boot-verify.json"

rm -rf "$WORK_DIR"
mkdir -p "$BIN_DIR" "$LOG_DIR"

cat > "$BIN_DIR/nixome-config-status" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cat "$NIXOME_FAKE_CONFIG_STATUS"
EOF
chmod 0755 "$BIN_DIR/nixome-config-status"

cat > "$STATUS_JSON" <<'EOF'
{
  "schema": "nixome.config-status.v1",
  "pending_reboot": {
    "required": false,
    "actions": [],
    "reasons": []
  },
  "system": {
    "last_apply": {
      "host_system_source": "/nix/store/new-system"
    },
    "host_system_profile_resolved_target": "/nix/store/new-system",
    "run_current_system_resolved_target": "/nix/store/new-system"
  }
}
EOF

env \
  PATH="$BIN_DIR:$PATH" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  NIXOME_FAKE_CONFIG_STATUS="$STATUS_JSON" \
  NIXOME_EXPECTED_HOST_SYSTEM_SOURCE="/nix/store/new-system" \
  bash "$ROOT/scripts/config-boot-verify.sh" > "$WORK_DIR/stdout.log"

jq -e '
  .schema == "nixome.config-boot-verify.v1" and
  .status == "passed" and
  .expected_host_system_source == "/nix/store/new-system" and
  .last_apply_host_system_source == "/nix/store/new-system" and
  .host_system_profile_resolved_target == "/nix/store/new-system" and
  .run_current_system_resolved_target == "/nix/store/new-system" and
  .pending_reboot.required == false and
  .status_json.schema == "nixome.config-status.v1"
' "$RESULT_JSON" >/dev/null

cat > "$FAILED_STATUS_JSON" <<'EOF'
{
  "schema": "nixome.config-status.v1",
  "pending_reboot": {
    "required": true,
    "actions": [
      {
        "type": "dom0-nixos-system",
        "switch_action": "boot",
        "reason": "install next dom0 boot generation"
      }
    ],
    "reasons": ["install next dom0 boot generation"]
  },
  "system": {
    "last_apply": {
      "host_system_source": "/nix/store/new-system"
    },
    "host_system_profile_resolved_target": "/nix/store/new-system",
    "run_current_system_resolved_target": "/nix/store/old-system"
  }
}
EOF

if env \
  PATH="$BIN_DIR:$PATH" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  NIXOME_FAKE_CONFIG_STATUS="$FAILED_STATUS_JSON" \
  NIXOME_EXPECTED_HOST_SYSTEM_SOURCE="/nix/store/new-system" \
  bash "$ROOT/scripts/config-boot-verify.sh" > "$WORK_DIR/failed-stdout.log" 2> "$WORK_DIR/failed-stderr.log"; then
  printf 'config boot verifier unexpectedly passed\n' >&2
  exit 1
fi

jq -e '
  .schema == "nixome.config-boot-verify.v1" and
  .status == "failed" and
  .reason == "running system does not point at expected generation" and
  .pending_reboot.required == true
' "$RESULT_JSON" >/dev/null

printf 'config boot verify test ok\n'
