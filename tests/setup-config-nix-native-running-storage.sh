#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
HOST_LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
WORK_DIR="${NIXOME_SETUP_CONFIG_NIX_NATIVE_RUNNING_STORAGE_DIR:-$HOST_LOG_DIR/setup-config-nix-native-running-storage}"
BIN_DIR="$WORK_DIR/bin"
TOPOLOGY="$WORK_DIR/topology.json"
CAPABILITIES="$WORK_DIR/xen-capabilities"
LOG_DIR="$WORK_DIR/logs"
POLICY_DIR="$WORK_DIR/policy.d"
POLICY_FILE="$POLICY_DIR/90-nixome.policy"
SYSTEM_STATE_DIR="$WORK_DIR/state"
MUTATION_LOG="$WORK_DIR/mutations.log"
OLD_SYSTEM_SOURCE="$WORK_DIR/old/system-source"
OLD_ROOT_SOURCE="$WORK_DIR/old/root-source"
OLD_STORE_SOURCE="$WORK_DIR/old/store-source"
NEW_SYSTEM_SOURCE="$WORK_DIR/new/system-source"
NEW_ROOT_SOURCE="$WORK_DIR/new/root-source"
NEW_STORE_SOURCE="$WORK_DIR/new/store-source"

rm -rf "$WORK_DIR"
mkdir -p \
  "$BIN_DIR" \
  "$LOG_DIR" \
  "$POLICY_DIR" \
  "$SYSTEM_STATE_DIR" \
  "$OLD_SYSTEM_SOURCE" \
  "$OLD_ROOT_SOURCE" \
  "$OLD_STORE_SOURCE" \
  "$NEW_SYSTEM_SOURCE" \
  "$NEW_ROOT_SOURCE" \
  "$NEW_STORE_SOURCE"

printf 'control_d\n' > "$CAPABILITIES"
printf '#!/bin/sh\nexec /init\n' > "$OLD_SYSTEM_SOURCE/init"
printf '#!/bin/sh\nexec /init\n' > "$NEW_SYSTEM_SOURCE/init"
chmod 0555 "$OLD_SYSTEM_SOURCE/init" "$NEW_SYSTEM_SOURCE/init"
printf 'old-root\n' > "$OLD_ROOT_SOURCE/root.img"
printf 'old-store\n' > "$OLD_STORE_SOURCE/store.img"
printf 'new-root\n' > "$NEW_ROOT_SOURCE/root.img"
printf 'new-store\n' > "$NEW_STORE_SOURCE/store.img"
: > "$MUTATION_LOG"

cat > "$POLICY_FILE" <<'EOF'
# Managed by Nixome setup-config.
# Source: nixome.qubes.topology.qrexec_policy.
# Bootstrap config transport policy remains in 30-nixome-config.policy.
EOF

cat > "$TOPOLOGY" <<EOF
{
  "schema": "nixome.qubes-topology.v1",
  "templates": {},
  "kernels": {},
  "domains": {
    "config-staging": {
      "class": "StandaloneVM",
      "template": null,
      "label": "green",
      "netvm": null,
      "prefs": {
        "autostart": false,
        "memory": 1024,
        "virt_mode": "pvh"
      },
      "features": {
        "nixome-managed": "1",
        "nixome-role": "config-staging",
        "qrexec": "1"
      },
      "services": {},
      "backend": {
        "type": "nix-closure",
        "template": null,
        "system": {
          "source": "$NEW_SYSTEM_SOURCE",
          "source_ref": null
        },
        "kernel": {
          "name": "old-kernel",
          "source": null,
          "source_ref": null
        },
        "initrd": {
          "name": null,
          "source": null,
          "source_ref": null
        },
        "modules": {
          "name": null,
          "source": null,
          "source_ref": null
        },
        "root": {
          "mode": "qubes-root-image",
          "source": "$NEW_ROOT_SOURCE",
          "source_ref": null,
          "read_only": false
        },
        "store": {
          "mode": "closure-image",
          "source": "$NEW_STORE_SOURCE",
          "source_ref": null,
          "mountpoint": "/nix/store",
          "device": "/dev/xvde",
          "read_only": true
        },
        "state": {
          "persistent": "config-staging-private",
          "volatile": null
        }
      }
    }
  },
  "prefs": {},
  "qrexec_policy": {
    "rules": []
  }
}
EOF

cat > "$BIN_DIR/qvm-ls" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  *"--fields NAME,STATE"*)
    printf 'config-staging|Running\n'
    ;;
  *"--fields NAME,CLASS"*)
    printf 'config-staging|StandaloneVM\n'
    ;;
  *)
    printf 'config-staging\n'
    ;;
esac
EOF

cat > "$BIN_DIR/qvm-prefs" <<EOF
#!/usr/bin/env bash
if [ "\$#" -eq 2 ]; then
  case "\$1:\$2" in
    config-staging:label) printf 'green\\n' ;;
    config-staging:netvm) printf 'none\\n' ;;
    config-staging:autostart) printf 'False\\n' ;;
    config-staging:memory) printf '1024\\n' ;;
    config-staging:virt_mode) printf 'pvh\\n' ;;
    config-staging:kernel) printf 'old-kernel\\n' ;;
    config-staging:kernelopts) printf 'console=hvc0 init=/sbin/init nixome.system=$OLD_SYSTEM_SOURCE nixome.storedev=/dev/xvde\\n' ;;
    *) exit 1 ;;
  esac
  exit 0
fi
printf 'qvm-prefs %s\\n' "\$*" >> "$MUTATION_LOG"
EOF

cat > "$BIN_DIR/qvm-features" <<EOF
#!/usr/bin/env bash
if [ "\$#" -eq 2 ]; then
  case "\$1:\$2" in
    config-staging:nixome-backend) printf 'nix-closure\\n' ;;
    config-staging:nixome-system-source) printf '$OLD_SYSTEM_SOURCE\\n' ;;
    config-staging:nixome-root-source) printf '$OLD_ROOT_SOURCE\\n' ;;
    config-staging:nixome-store-source) printf '$OLD_STORE_SOURCE\\n' ;;
    config-staging:nixome-store-image) printf '$OLD_STORE_SOURCE/store.img\\n' ;;
    config-staging:nixome-store-mountpoint) printf '/nix/store\\n' ;;
    config-staging:nixome-store-device) printf '/dev/xvde\\n' ;;
    config-staging:nixome-store-read-only) printf 'True\\n' ;;
    config-staging:nixome-managed) printf '1\\n' ;;
    config-staging:nixome-role) printf 'config-staging\\n' ;;
    config-staging:qrexec) printf '1\\n' ;;
    *) exit 1 ;;
  esac
  exit 0
fi
printf 'qvm-features %s\\n' "\$*" >> "$MUTATION_LOG"
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

for command in qvm-create qvm-template-postprocess qvm-volume nixome-nix-domain-storage; do
  cat > "$BIN_DIR/$command" <<'EOF'
#!/usr/bin/env bash
printf 'unexpected mutation: %s %s\n' "$0" "$*" >> "$MUTATION_LOG"
exit 97
EOF
done

cat > "$BIN_DIR/qvm-service" <<'EOF'
#!/usr/bin/env bash
if [ "$#" -eq 2 ]; then
  exit 1
fi
printf 'qvm-service %s\n' "$*" >> "$MUTATION_LOG"
EOF

cat > "$BIN_DIR/qubes-prefs" <<'EOF'
#!/usr/bin/env bash
if [ "$#" -eq 1 ]; then
  exit 1
fi
printf 'qubes-prefs %s\n' "$*" >> "$MUTATION_LOG"
EOF

cat > "$BIN_DIR/qubesd-query" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF

for command in awk chmod cmp cp grep jq mkdir python3 rm sed tr; do
  if command -v "$command" >/dev/null 2>&1; then
    ln -s "$(command -v "$command")" "$BIN_DIR/$command"
  fi
done

find "$BIN_DIR" -maxdepth 1 -type f -exec chmod 0755 {} +

PATH="$BIN_DIR:$PATH" \
  MUTATION_LOG="$MUTATION_LOG" \
  NIXOME_QUBES_POLICY_DIR="$POLICY_DIR" \
  NIXOME_SYSTEM_STATE_DIR="$SYSTEM_STATE_DIR" \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  "$BASH" "$ROOT/scripts/setup-config.sh" --apply --topology "$TOPOLOGY" \
  > "$LOG_DIR/stdout.log" 2> "$LOG_DIR/stderr.log"

jq -e \
  --arg system "$NEW_SYSTEM_SOURCE" \
  --arg root "$NEW_ROOT_SOURCE" \
  --arg store "$NEW_STORE_SOURCE" \
  '
  .schema == "nixome.setup-config.v1" and
  .status == "passed" and
  .mode == "apply" and
  .domains_changed == 0 and
  .root_images_changed == 0 and
  .store_volumes_changed == 0 and
  .store_images_changed == 0 and
  .restart_required == true and
  .live_actions == [] and
  (.restart_actions | length) == 3 and
  any(.restart_actions[]; .type == "domain-root" and .action == "restart" and .domain == "config-staging" and .field == "source" and .value == $root and .state == "Running") and
  any(.restart_actions[]; .type == "domain-store" and .action == "restart" and .domain == "config-staging" and .field == "source" and .value == $store and .state == "Running") and
  any(.restart_actions[]; .type == "domain-system" and .action == "restart" and .domain == "config-staging" and .field == "source" and .value == $system and .state == "Running") and
  all(.restart_actions[]; .reason == "domain is running; stop it and re-run setup-config for this storage change to take effect")
  ' "$LOG_DIR/setup-config.json" >/dev/null

jq -e '
  .schema == "nixome.setup-config.v1" and
  .status == "passed" and
  .restart_required == true and
  (.restart_actions | length) == 3
' "$SYSTEM_STATE_DIR/current-setup.json" >/dev/null

if [ -s "$MUTATION_LOG" ]; then
  printf 'setup-config mutated a running nix-closure domain storage identity:\n' >&2
  cat "$MUTATION_LOG" >&2
  exit 1
fi

if grep -F '+ qvm-volume import config-staging:root' "$LOG_DIR/setup-config.commands.log" >/dev/null; then
  printf 'setup-config planned root import for a running domain\n' >&2
  exit 1
fi

if grep -F '+ nixome-nix-domain-storage ensure-store config-staging' "$LOG_DIR/setup-config.commands.log" >/dev/null; then
  printf 'setup-config called store helper for a running domain with a changed store\n' >&2
  exit 1
fi

if grep -F '+ qvm-prefs config-staging kernelopts' "$LOG_DIR/setup-config.commands.log" >/dev/null; then
  printf 'setup-config changed kernelopts before the new store image was imported\n' >&2
  exit 1
fi

printf 'setup-config nix-native running storage deferral test ok\n'
