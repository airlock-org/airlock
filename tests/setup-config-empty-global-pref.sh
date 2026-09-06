#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
HOST_LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
WORK_DIR="${NIXOME_SETUP_CONFIG_EMPTY_GLOBAL_PREF_DIR:-$HOST_LOG_DIR/setup-config-empty-global-pref}"
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

cat > "$TOPOLOGY" <<'EOF'
{
  "schema": "nixome.qubes-topology.v1",
  "templates": {},
  "kernels": {},
  "domains": {},
  "prefs": {
    "default_kernel": ""
  }
}
EOF

cat > "$BIN_DIR/systemctl" <<'EOF'
#!/usr/bin/env bash
if [ "$1" = "is-active" ]; then
  exit 0
fi
if [ "$1" = "reload" ] || [ "$1" = "try-restart" ]; then
  exit 0
fi
exit 1
EOF

cat > "$BIN_DIR/qvm-ls" <<'EOF'
#!/usr/bin/env bash
printf 'dom0\n'
EOF

cat > "$BIN_DIR/qubes-prefs" <<'EOF'
#!/usr/bin/env bash
if [ "$#" -eq 1 ] && [ "$1" = "default_kernel" ]; then
  printf '\n'
  exit 0
fi
{
  printf 'qubes-prefs'
  for arg in "$@"; do
    printf ' <%s>' "$arg"
  done
  printf '\n'
} >> "$MUTATION_LOG"
EOF

for command in qvm-create qvm-features qvm-prefs qvm-service qvm-template-postprocess qubesd-query; do
  cat > "$BIN_DIR/$command" <<'EOF'
#!/usr/bin/env bash
printf 'unexpected command execution: %s\n' "$0" >&2
exit 97
EOF
done

for command in cmp grep jq; do
  if command -v "$command" >/dev/null 2>&1; then
    ln -s "$(command -v "$command")" "$BIN_DIR/$command"
  fi
done

find "$BIN_DIR" -maxdepth 1 -type f -exec chmod 0755 {} +

PATH="$BIN_DIR:$PATH" \
  MUTATION_LOG="$MUTATION_LOG" \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_QUBES_POLICY_DIR="$POLICY_DIR" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  "$BASH" "$ROOT/scripts/setup-config.sh" --apply --topology "$TOPOLOGY" \
  > "$LOG_DIR/stdout.log" 2> "$LOG_DIR/stderr.log"

jq -e '
  .schema == "nixome.setup-config.v1" and
  .status == "passed" and
  .mode == "apply" and
  .prefs_changed == 1 and
  .qrexec_policy_changed == 1
' "$LOG_DIR/setup-config.json" >/dev/null

grep -Fx 'qubes-prefs <default_kernel> <>' "$MUTATION_LOG" >/dev/null

printf 'setup-config empty-global-pref test ok\n'
