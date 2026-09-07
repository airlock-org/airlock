#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"

WORK_DIR="${NIXOME_CONFIG_STATUS_TEST_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/config-status}"
BIN_DIR="$WORK_DIR/bin"
LOG_DIR="$WORK_DIR/logs"
STATE_DIR="$WORK_DIR/state"
POLICY_DIR="$WORK_DIR/policy.d"
TOPOLOGY_JSON="$WORK_DIR/topology.json"
RESULT_JSON="$WORK_DIR/status.json"

rm -rf "$WORK_DIR"
mkdir -p "$BIN_DIR" "$LOG_DIR" "$STATE_DIR" "$POLICY_DIR"

cat > "$TOPOLOGY_JSON" <<'EOF'
{
  "schema": "nixome.qubes-topology.v1",
  "templates": {
    "nixome-nixos-work": {
      "import": true,
      "source": "/nix/store/example-template/nixome-nixos-work"
    }
  },
  "kernels": {},
  "domains": {
    "config-vault": {
      "class": "AppVM",
      "template": "nixome-nixos-config-system"
    },
    "nixome-work": {
      "class": "AppVM",
      "template": "nixome-nixos-work"
    }
  },
  "prefs": {},
  "qrexec_policy": {
    "rules": [
      {
        "service": "nixome.PolicyProbe",
        "argument": "*",
        "source": "dom0",
        "target": "config-vault",
        "action": "allow"
      }
    ]
  }
}
EOF

cat > "$LOG_DIR/setup-config.json" <<'EOF'
{"schema":"nixome.setup-config.v1","status":"passed","domains_changed":2}
EOF

cat > "$LOG_DIR/config-apply.json" <<'EOF'
{"schema":"nixome.config-apply.v1","status":"passed","candidate_commit":"abc123"}
EOF

cat > "$STATE_DIR/current-system.json" <<'EOF'
{"schema":"nixome.system.v1","dom0":{"profile":{"message":"from-test"}}}
EOF

cat > "$STATE_DIR/current-plan.json" <<'EOF'
{"live_actions":[],"reboot_required":false,"reboot_reasons":[]}
EOF

cat > "$STATE_DIR/last-apply.json" <<'EOF'
{"schema":"nixome.system-apply.v1","status":"passed","host_system_status":"passed"}
EOF

cat > "$POLICY_DIR/90-nixome.policy" <<'EOF'
# Managed by Nixome setup-config.
nixome.PolicyProbe * dom0 config-vault allow
nixome.PolicyProbe * @anyvm @anyvm deny
EOF

cat > "$BIN_DIR/qvm-ls" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

case "$*" in
  "--raw-data --fields NAME,CLASS,STATE,TEMPLATE,LABEL --no-spinner")
    cat <<'QVM'
config-vault|AppVM|Running|nixome-nixos-config-system|black
nixome-work|AppVM|Halted|nixome-nixos-work|green
other|AppVM|Halted|fedora-43-xfce|red
QVM
    ;;
  *)
    printf 'unexpected qvm-ls args: %s\n' "$*" >&2
    exit 2
    ;;
esac
EOF
chmod 0755 "$BIN_DIR/qvm-ls"

out="$(nix build "$ROOT#packages.x86_64-linux.nixome-config-status" --no-link --print-out-paths)"
wrapper="$(nixome_physical_store_path "$out")/bin/nixome-config-status"

env \
  PATH="$BIN_DIR:$PATH" \
  NIXOME_QVM_LS_BIN="$BIN_DIR/qvm-ls" \
  NIXOME_CONFIG_STATUS_TOPOLOGY_JSON="$TOPOLOGY_JSON" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  NIXOME_SYSTEM_STATE_DIR="$STATE_DIR" \
  NIXOME_QUBES_POLICY_DIR="$POLICY_DIR" \
  "$wrapper" > "$RESULT_JSON"

jq -e '
  .schema == "nixome.config-status.v1" and
  .status == "applied" and
  .topology.present == true and
  (.topology.domain_names | index("config-vault")) and
  (.topology.template_names | index("nixome-nixos-work")) and
  .topology.qrexec_policy_rule_count == 1 and
  .qvm.available == true and
  .qvm.status == "ok" and
  (.qvm.domains | length) == 3 and
  (.qvm.managed_domains | length) == 2 and
  .qrexec_policy.exists == true and
  .qrexec_policy.rule_count == 2 and
  .setup.status == "passed" and
  .apply.status == "passed" and
  .system.current_system.dom0.profile.message == "from-test" and
  .system.current_plan.reboot_required == false and
  .system.last_apply.host_system_status == "passed"
' "$RESULT_JSON" >/dev/null

printf 'config status test ok\n'
