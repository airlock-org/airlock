#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
HOST_LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
WORK_DIR="${NIXOME_SETUP_CONFIG_EXTERNAL_NETVM_DIR:-$HOST_LOG_DIR/setup-config-external-netvm}"
BIN_DIR="$WORK_DIR/bin"
TOPOLOGY="$WORK_DIR/topology.json"
CAPABILITIES="$WORK_DIR/xen-capabilities"
LOG_DIR="$WORK_DIR/logs"
MUTATION_LOG="$WORK_DIR/mutations.log"
POLICY_DIR="$WORK_DIR/policy.d"

rm -rf "$WORK_DIR"
mkdir -p "$BIN_DIR" "$LOG_DIR" "$POLICY_DIR"

printf 'control_d\n' > "$CAPABILITIES"
: > "$MUTATION_LOG"
export MUTATION_LOG

cat > "$TOPOLOGY" <<'EOF'
{
  "schema": "nixome.qubes-topology.v1",
  "templates": {},
  "kernels": {},
  "domains": {
    "dev": {
      "class": "StandaloneVM",
      "template": null,
      "label": "green",
      "netvm": "sys-firewall",
      "prefs": {
        "autostart": false
      },
      "features": {
        "nixome-role": "dev"
      },
      "services": {}
    }
  },
  "prefs": {
    "default_kernel": ""
  }
}
EOF

cat > "$BIN_DIR/qvm-ls" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  "--raw-data --fields NAME --no-spinner")
    printf 'dom0\n'
    printf 'dev\n'
    ;;
  "--raw-data --fields NAME,STATE --no-spinner")
    printf 'dom0|Running\n'
    printf 'dev|Halted\n'
    ;;
  "--raw-data --fields NAME,CLASS --no-spinner")
    printf 'dom0|AdminVM\n'
    printf 'dev|StandaloneVM\n'
    ;;
  *)
    printf 'unexpected qvm-ls args: %s\n' "$*" >&2
    exit 1
    ;;
esac
EOF
chmod 0755 "$BIN_DIR/qvm-ls"

cat > "$BIN_DIR/systemctl" <<'EOF'
#!/usr/bin/env bash
if [ "$1" = "is-active" ]; then
  exit 0
fi
if [ "$1" = "reload" ] || [ "$1" = "try-restart" ]; then
  printf 'systemctl %s\n' "$*" >> "$MUTATION_LOG"
  exit 0
fi
exit 1
EOF
chmod 0755 "$BIN_DIR/systemctl"

cat > "$BIN_DIR/qubes-prefs" <<'EOF'
#!/usr/bin/env bash
if [ "$#" -eq 1 ] && [ "$1" = "default_kernel" ]; then
  printf '\n'
  exit 0
fi
printf 'qubes-prefs %s\n' "$*" >> "$MUTATION_LOG"
exit 0
EOF
chmod 0755 "$BIN_DIR/qubes-prefs"

cat > "$BIN_DIR/qvm-create" <<'EOF'
#!/usr/bin/env bash
printf 'qvm-create %s\n' "$*" >> "$MUTATION_LOG"
exit 0
EOF
chmod 0755 "$BIN_DIR/qvm-create"

cat > "$BIN_DIR/qvm-prefs" <<'EOF'
#!/usr/bin/env bash
if [ "$#" -eq 2 ]; then
  case "$1:$2" in
    dev:label) printf 'green\n' ;;
    dev:autostart) printf 'False\n' ;;
    *) exit 1 ;;
  esac
  exit 0
fi

if [ "$#" -eq 3 ] && [ "$1:$2:$3" = "dev:netvm:sys-firewall" ]; then
  printf 'setup-config should defer missing external sys-firewall instead of setting it\n' >&2
  exit 97
fi

printf 'qvm-prefs %s\n' "$*" >> "$MUTATION_LOG"
exit 0
EOF
chmod 0755 "$BIN_DIR/qvm-prefs"

for command in qvm-appmenus qvm-features qvm-service qvm-template-postprocess qvm-volume qubesd-query runuser; do
  cat > "$BIN_DIR/$command" <<'EOF'
#!/usr/bin/env bash
printf '%s %s\n' "$(basename "$0")" "$*" >> "$MUTATION_LOG"
exit 0
EOF
  chmod 0755 "$BIN_DIR/$command"
done

PATH="$BIN_DIR:$PATH" \
  NIXOME_QUBES_POLICY_DIR="$POLICY_DIR" \
  NIXOME_SETUP_CONFIG_PERSIST_STATE=0 \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  bash "$ROOT/scripts/setup-config.sh" --apply --topology "$TOPOLOGY" \
  > "$LOG_DIR/stdout.log" 2> "$LOG_DIR/stderr.log"

jq -e '
  .schema == "nixome.setup-config.v1" and
  .status == "passed" and
  .domains_changed == 0 and
  .prefs_changed == 1 and
  any(.live_actions[]; .type == "domain-pref" and .action == "defer" and .domain == "dev" and .field == "netvm" and .value == "sys-firewall")
' "$LOG_DIR/setup-config.json" >/dev/null

! grep -Fx 'qvm-prefs dev netvm sys-firewall' "$MUTATION_LOG" >/dev/null

printf 'setup-config external netvm test ok\n'
