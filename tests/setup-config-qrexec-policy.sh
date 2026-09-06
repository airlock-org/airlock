#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
HOST_LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
WORK_DIR="${NIXOME_SETUP_CONFIG_QREXEC_POLICY_DIR:-$HOST_LOG_DIR/setup-config-qrexec-policy}"
BIN_DIR="$WORK_DIR/bin"
TOPOLOGY="$WORK_DIR/topology.json"
CAPABILITIES="$WORK_DIR/xen-capabilities"
LOG_DIR="$WORK_DIR/logs"
POLICY_DIR="$WORK_DIR/policy.d"
POLICY_FILE="$POLICY_DIR/90-nixome.policy"
MUTATION_LOG="$WORK_DIR/mutations.log"
SYSTEMCTL_LOG="$WORK_DIR/systemctl.log"

rm -rf "$WORK_DIR"
mkdir -p "$BIN_DIR" "$LOG_DIR" "$POLICY_DIR"

printf 'control_d\n' > "$CAPABILITIES"
: > "$MUTATION_LOG"
: > "$SYSTEMCTL_LOG"

cat > "$TOPOLOGY" <<'EOF'
{
  "schema": "nixome.qubes-topology.v1",
  "templates": {},
  "kernels": {},
  "domains": {},
  "prefs": {},
  "qrexec_policy": {
    "rules": [
      {
        "service": "nixome.PolicyProbe",
        "argument": "*",
        "source": "dom0",
        "target": "config-vault",
        "action": "allow"
      },
      {
        "service": "nixome.PolicyProbe",
        "argument": "*",
        "source": "@anyvm",
        "target": "@anyvm",
        "action": "deny"
      }
    ]
  }
}
EOF

for command in qvm-create qvm-features qvm-prefs qvm-service qvm-template-postprocess qubes-prefs qubesd-query; do
  cat > "$BIN_DIR/$command" <<'EOF'
#!/usr/bin/env bash
printf 'unexpected command execution: %s %s\n' "$0" "$*" >&2
exit 97
EOF
done

cat > "$BIN_DIR/qvm-ls" <<'EOF'
#!/usr/bin/env bash
printf 'dom0\n'
EOF

cat > "$BIN_DIR/systemctl" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  is-active)
    exit 0
    ;;
  reload)
    printf 'systemctl %s\n' "$*" >> "$SYSTEMCTL_LOG"
    exit 1
    ;;
  try-restart)
    printf 'systemctl %s\n' "$*" >> "$SYSTEMCTL_LOG"
    exit 0
    ;;
esac
exit 1
EOF

for command in cmp cp grep jq mkdir chmod rm; do
  if command -v "$command" >/dev/null 2>&1; then
    ln -s "$(command -v "$command")" "$BIN_DIR/$command"
  fi
done

find "$BIN_DIR" -maxdepth 1 -type f -exec chmod 0755 {} +

PATH="$BIN_DIR:$PATH" \
  MUTATION_LOG="$MUTATION_LOG" \
  SYSTEMCTL_LOG="$SYSTEMCTL_LOG" \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_QUBES_POLICY_DIR="$POLICY_DIR" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  "$BASH" "$ROOT/scripts/setup-config.sh" --apply --topology "$TOPOLOGY" \
  > "$LOG_DIR/first-stdout.log" 2> "$LOG_DIR/first-stderr.log"

jq -e '
  .schema == "nixome.setup-config.v1" and
  .status == "passed" and
  .mode == "apply" and
  .qrexec_policy_changed == 1 and
  .qrexec_policy_file == "'"$POLICY_FILE"'"
' "$LOG_DIR/setup-config.json" >/dev/null

cat > "$WORK_DIR/expected.policy" <<'EOF'
# Managed by Nixome setup-config.
# Source: nixome.qubes.topology.qrexec_policy.
# Bootstrap config transport policy remains in 30-nixome-config.policy.
nixome.PolicyProbe * dom0 config-vault allow
nixome.PolicyProbe * @anyvm @anyvm deny
EOF

cmp -s "$WORK_DIR/expected.policy" "$POLICY_FILE" || {
  printf 'generated qrexec policy did not match expected output\n' >&2
  diff -u "$WORK_DIR/expected.policy" "$POLICY_FILE" >&2 || true
  exit 1
}

grep -Fx 'systemctl reload qubes-qrexec-policy-daemon.service' "$SYSTEMCTL_LOG" >/dev/null
grep -Fx 'systemctl try-restart qubes-qrexec-policy-daemon.service' "$SYSTEMCTL_LOG" >/dev/null

: > "$SYSTEMCTL_LOG"
PATH="$BIN_DIR:$PATH" \
  MUTATION_LOG="$MUTATION_LOG" \
  SYSTEMCTL_LOG="$SYSTEMCTL_LOG" \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_QUBES_POLICY_DIR="$POLICY_DIR" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  "$BASH" "$ROOT/scripts/setup-config.sh" --apply --topology "$TOPOLOGY" \
  > "$LOG_DIR/second-stdout.log" 2> "$LOG_DIR/second-stderr.log"

jq -e '
  .schema == "nixome.setup-config.v1" and
  .status == "passed" and
  .mode == "apply" and
  .qrexec_policy_changed == 0
' "$LOG_DIR/setup-config.json" >/dev/null

if [ -s "$SYSTEMCTL_LOG" ]; then
  printf 'idempotent qrexec policy apply restarted the policy daemon:\n' >&2
  cat "$SYSTEMCTL_LOG" >&2
  exit 1
fi

if [ -s "$MUTATION_LOG" ]; then
  printf 'setup-config mutated non-policy Qubes state:\n' >&2
  cat "$MUTATION_LOG" >&2
  exit 1
fi

printf 'setup-config qrexec policy test ok\n'
