#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
HOST_LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
WORK_DIR="${NIXOME_SETUP_CONFIG_RESTART_ACTIONS_DIR:-$HOST_LOG_DIR/setup-config-restart-actions}"
BIN_DIR="$WORK_DIR/bin"
KERNEL_DIR="$WORK_DIR/kernel/6.12.64-1.fc41"
KERNEL_BASE="$WORK_DIR/vm-kernels"
KERNEL_TARGET="$KERNEL_BASE/6.12.64-1.fc41"
TOPOLOGY="$WORK_DIR/topology.json"
CAPABILITIES="$WORK_DIR/xen-capabilities"
LOG_DIR="$WORK_DIR/logs"
MUTATION_LOG="$WORK_DIR/mutations.log"
QUBES_XML="$WORK_DIR/qubes.xml"
POLICY_DIR="$WORK_DIR/policy.d"
POLICY_FILE="$POLICY_DIR/90-nixome.policy"
SYSTEM_STATE_DIR="$WORK_DIR/state"

rm -rf "$WORK_DIR"
mkdir -p "$BIN_DIR" "$KERNEL_DIR" "$KERNEL_TARGET" "$LOG_DIR" "$POLICY_DIR" "$SYSTEM_STATE_DIR"

printf 'control_d\n' > "$CAPABILITIES"
for file in vmlinuz initramfs modules.img; do
  printf '%s\n' "$file" > "$KERNEL_DIR/$file"
  cp "$KERNEL_DIR/$file" "$KERNEL_TARGET/$file"
done
{
  printf 'source=%s\n' "$KERNEL_DIR"
  printf 'schema=nixome.qubes-kernel.v1\n'
} > "$KERNEL_TARGET/MANAGED_BY_NIXOME"
: > "$MUTATION_LOG"

cat > "$QUBES_XML" <<'EOF'
<qubes>
  <properties>
    <property name="default_kernel"></property>
  </properties>
</qubes>
EOF

cat > "$POLICY_FILE" <<'EOF'
# Managed by Nixome setup-config.
# Source: nixome.qubes.topology.qrexec_policy.
# Bootstrap config transport policy remains in 30-nixome-config.policy.
EOF

cat > "$TOPOLOGY" <<EOF
{
  "schema": "nixome.qubes-topology.v1",
  "templates": {},
  "kernels": {
    "6.12.64-1.fc41": {
      "source": "$KERNEL_DIR"
    }
  },
  "domains": {
    "config-vault": {
      "class": "AppVM",
      "template": "nixome-nixos-test",
      "label": "black",
      "netvm": null,
      "prefs": {
        "autostart": false,
        "memory": 1024
      },
      "features": {
        "nixome-role": "config-vault"
      },
      "services": {
        "network-manager": false
      }
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
  *"--fields NAME,STATE"*)
    printf 'dom0|Running\n'
    printf 'nixome-nixos-test|Halted\n'
    printf 'config-vault|Running\n'
    ;;
  *)
    printf 'dom0\n'
    printf 'nixome-nixos-test\n'
    printf 'config-vault\n'
    ;;
esac
EOF

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

cat > "$BIN_DIR/qubes-prefs" <<'EOF'
#!/usr/bin/env bash
if [ "$#" -eq 1 ] && [ "$1" = "default_kernel" ]; then
  printf '\n'
  exit 0
fi
printf 'qubes-prefs %s\n' "$*" >> "$MUTATION_LOG"
exit 0
EOF

cat > "$BIN_DIR/qvm-prefs" <<'EOF'
#!/usr/bin/env bash
if [ "$#" -eq 2 ]; then
  case "$1:$2" in
    config-vault:label) printf 'black\n' ;;
    config-vault:netvm) printf 'none\n' ;;
    config-vault:autostart) printf 'False\n' ;;
    config-vault:memory) printf '512\n' ;;
    *) exit 1 ;;
  esac
  exit 0
fi
printf 'qvm-prefs %s\n' "$*" >> "$MUTATION_LOG"
exit 0
EOF

cat > "$BIN_DIR/qvm-features" <<'EOF'
#!/usr/bin/env bash
if [ "$#" -eq 2 ] && [ "$1:$2" = "config-vault:nixome-role" ]; then
  printf 'config-vault\n'
  exit 0
fi
printf 'qvm-features %s\n' "$*" >> "$MUTATION_LOG"
exit 0
EOF

cat > "$BIN_DIR/qvm-service" <<'EOF'
#!/usr/bin/env bash
if [ "$#" -eq 2 ] && [ "$1:$2" = "config-vault:network-manager" ]; then
  printf 'on\n'
  exit 0
fi
printf 'qvm-service %s\n' "$*" >> "$MUTATION_LOG"
exit 0
EOF

for command in qvm-create qvm-template-postprocess; do
  cat > "$BIN_DIR/$command" <<'EOF'
#!/usr/bin/env bash
printf '%s %s\n' "$0" "$*" >> "$MUTATION_LOG"
exit 99
EOF
done

cat > "$BIN_DIR/qubesd-query" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF

for command in awk chmod cmp cp grep jq mkdir python3 rm; do
  if command -v "$command" >/dev/null 2>&1; then
    ln -s "$(command -v "$command")" "$BIN_DIR/$command"
  fi
done

find "$BIN_DIR" -maxdepth 1 -type f -exec chmod 0755 {} +

PATH="$BIN_DIR:$PATH" \
  MUTATION_LOG="$MUTATION_LOG" \
  NIXOME_QUBES_KERNEL_BASE="$KERNEL_BASE" \
  NIXOME_QUBES_XML="$QUBES_XML" \
  NIXOME_QUBES_POLICY_DIR="$POLICY_DIR" \
  NIXOME_SYSTEM_STATE_DIR="$SYSTEM_STATE_DIR" \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  "$BASH" "$ROOT/scripts/setup-config.sh" --apply --topology "$TOPOLOGY" \
  > "$LOG_DIR/stdout.log" 2> "$LOG_DIR/stderr.log"

jq -e '
  .schema == "nixome.setup-config.v1" and
  .status == "passed" and
  .mode == "apply" and
  .templates_changed == 0 and
  .kernels_changed == 0 and
  .domains_changed == 0 and
  .prefs_changed == 1 and
  .features_changed == 0 and
  .services_changed == 1 and
  .qrexec_policy_changed == 0 and
  .restart_required == true and
  (.live_actions | length) == 2 and
  (.restart_actions | length) == 2 and
  (.live_actions[] | select(
    .type == "domain-pref" and
    .action == "set" and
    .domain == "config-vault" and
    .field == "memory" and
    .value == "1024"
  )) and
  (.live_actions[] | select(
    .type == "domain-service" and
    .action == "set" and
    .domain == "config-vault" and
    .field == "network-manager" and
    .value == "off"
  )) and
  (.restart_actions[] | select(
    .type == "domain-pref" and
    .action == "restart" and
    .domain == "config-vault" and
    .field == "memory" and
    .value == "1024" and
    .state == "Running"
  )) and
  (.restart_actions[] | select(
    .type == "domain-service" and
    .action == "restart" and
    .domain == "config-vault" and
    .field == "network-manager" and
    .value == "off" and
    .state == "Running"
  ))
' "$LOG_DIR/setup-config.json" >/dev/null

jq -e '
  .schema == "nixome.setup-config.v1" and
  .status == "passed" and
  .restart_required == true and
  (.restart_actions | length) == 2
' "$SYSTEM_STATE_DIR/current-setup.json" >/dev/null
jq -e '
  .schema == "nixome.qubes-topology.v1" and
  .domains."config-vault".prefs.memory == 1024 and
  .domains."config-vault".services."network-manager" == false
' "$SYSTEM_STATE_DIR/current-topology.json" >/dev/null

jq -e '
  .restart_actions[]
  | select(.domain == "config-vault")
  | .reason == "domain is running; restart required for this change to fully take effect"
' "$LOG_DIR/setup-config.json" >/dev/null

if ! grep -Fx 'qvm-prefs config-vault memory 1024' "$MUTATION_LOG" >/dev/null; then
  printf 'missing qvm-prefs mutation\n' >&2
  cat "$MUTATION_LOG" >&2
  exit 1
fi

if ! grep -Fx 'qvm-service config-vault network-manager off' "$MUTATION_LOG" >/dev/null; then
  printf 'missing qvm-service mutation\n' >&2
  cat "$MUTATION_LOG" >&2
  exit 1
fi

printf 'setup-config restart actions test ok\n'
