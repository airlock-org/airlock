#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
HOST_LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
WORK_DIR="${NIXOME_SETUP_CONFIG_PREF_ORDER_DIR:-$HOST_LOG_DIR/setup-config-pref-order}"
BIN_DIR="$WORK_DIR/bin"
TOPOLOGY="$WORK_DIR/topology.json"
CAPABILITIES="$WORK_DIR/xen-capabilities"
LOG_DIR="$WORK_DIR/logs"
POLICY_DIR="$WORK_DIR/policy.d"
MUTATION_LOG="$WORK_DIR/mutations.log"
MEMORY_SET_MARKER="$WORK_DIR/memory-set"
SYSTEM_STATE_DIR="$WORK_DIR/state"

rm -rf "$WORK_DIR"
mkdir -p "$BIN_DIR" "$LOG_DIR" "$POLICY_DIR" "$SYSTEM_STATE_DIR"

printf 'control_d\n' > "$CAPABILITIES"
: > "$MUTATION_LOG"

cat > "$TOPOLOGY" <<'EOF'
{
  "schema": "nixome.qubes-topology.v1",
  "templates": {},
  "kernels": {},
  "domains": {
    "config-staging": {
      "class": "AppVM",
      "template": "fedora-44-xfce",
      "label": "gray",
      "netvm": null,
      "prefs": {
        "autostart": false,
        "maxmem": 4096,
        "memory": 2048,
        "vcpus": 2
      },
      "features": {},
      "services": {}
    }
  },
  "prefs": {}
}
EOF

cat > "$BIN_DIR/qvm-ls" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  *NAME,STATE*)
    printf 'dom0|Running\nconfig-staging|Halted\n'
    ;;
  *)
    printf 'dom0\nconfig-staging\n'
    ;;
esac
EOF

cat > "$BIN_DIR/qvm-prefs" <<'EOF'
#!/usr/bin/env bash
if [ "$#" -eq 2 ]; then
  case "$1:$2" in
    config-staging:label) printf 'gray\n' ;;
    config-staging:netvm) printf 'none\n' ;;
    config-staging:autostart) printf 'True\n' ;;
    config-staging:memory) printf '400\n' ;;
    config-staging:maxmem) printf '4000\n' ;;
    config-staging:vcpus) printf '1\n' ;;
    *) exit 1 ;;
  esac
  exit 0
fi

if [ "$#" -eq 3 ] && [ "$1" = "config-staging" ]; then
  if [ "$2" = "maxmem" ] && [ ! -e "$MEMORY_SET_MARKER" ]; then
    printf 'maxmem was set before memory\n' >&2
    exit 42
  fi
  if [ "$2" = "memory" ]; then
    : > "$MEMORY_SET_MARKER"
  fi
  printf 'qvm-prefs %s\n' "$*" >> "$MUTATION_LOG"
  exit 0
fi

exit 1
EOF

cat > "$BIN_DIR/systemctl" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  is-active|reload|try-restart)
    exit 0
    ;;
esac
exit 1
EOF

for command in qubes-prefs qvm-create qvm-features qvm-service qvm-template-postprocess qvm-volume qubesd-query; do
  cat > "$BIN_DIR/$command" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
done

find "$BIN_DIR" -maxdepth 1 -type f -exec chmod 0755 {} +

PATH="$BIN_DIR:$PATH" \
  MEMORY_SET_MARKER="$MEMORY_SET_MARKER" \
  MUTATION_LOG="$MUTATION_LOG" \
  NIXOME_QUBES_POLICY_DIR="$POLICY_DIR" \
  NIXOME_SYSTEM_STATE_DIR="$SYSTEM_STATE_DIR" \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  bash "$ROOT/scripts/setup-config.sh" --apply --topology "$TOPOLOGY" \
  > "$LOG_DIR/stdout.log" 2> "$LOG_DIR/stderr.log"

memory_line="$(grep -nF '+ qvm-prefs config-staging memory 2048' "$LOG_DIR/setup-config.commands.log" | cut -d: -f1)"
maxmem_line="$(grep -nF '+ qvm-prefs config-staging maxmem 4096' "$LOG_DIR/setup-config.commands.log" | cut -d: -f1)"

[ -n "$memory_line" ] && [ -n "$maxmem_line" ]
[ "$memory_line" -lt "$maxmem_line" ]

jq -e '
  .schema == "nixome.setup-config.v1" and
  .status == "passed" and
  .prefs_changed == 4
' "$LOG_DIR/setup-config.json" >/dev/null

printf 'setup-config pref order test ok\n'
