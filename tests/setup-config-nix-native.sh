#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
HOST_LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
WORK_DIR="${NIXOME_SETUP_CONFIG_NIX_NATIVE_DIR:-$HOST_LOG_DIR/setup-config-nix-native}"
BIN_DIR="$WORK_DIR/bin"
TOPOLOGY="$WORK_DIR/topology.json"
CAPABILITIES="$WORK_DIR/xen-capabilities"
PLAN_LOG_DIR="$WORK_DIR/plan-logs"
APPLY_LOG_DIR="$WORK_DIR/apply-logs"
POLICY_DIR="$WORK_DIR/policy.d"
KERNEL_BASE="$WORK_DIR/vm-kernels"
SYSTEM_SOURCE="$WORK_DIR/system-source"
ROOT_SOURCE="$WORK_DIR/root-source"
STORE_SOURCE="$WORK_DIR/store-source"
ROOT_IMAGE="$ROOT_SOURCE/root.img"
MUTATION_LOG="$WORK_DIR/mutations.log"

rm -rf "$WORK_DIR"
mkdir -p \
  "$BIN_DIR" \
  "$PLAN_LOG_DIR" \
  "$APPLY_LOG_DIR" \
  "$POLICY_DIR" \
  "$KERNEL_BASE" \
  "$SYSTEM_SOURCE" \
  "$ROOT_SOURCE" \
  "$STORE_SOURCE"

printf 'control_d\n' > "$CAPABILITIES"
printf '#!/bin/sh\nexec /init\n' > "$SYSTEM_SOURCE/init"
chmod 0555 "$SYSTEM_SOURCE/init"
: > "$ROOT_IMAGE"
printf '%s\n' "$SYSTEM_SOURCE" > "$STORE_SOURCE/store-paths"
printf '{"schema":"nixome.store-closure-manifest.v1","name":"test-store","read_only":true}\n' \
  > "$STORE_SOURCE/manifest.json"
: > "$MUTATION_LOG"

cat > "$TOPOLOGY" <<EOF
{
  "schema": "nixome.qubes-topology.v1",
  "templates": {},
  "kernels": {},
  "domains": {
    "legacy-fedora-app": {
      "class": "AppVM",
      "template": "fedora-44-xfce",
      "label": "red",
      "netvm": null,
      "prefs": {
        "autostart": false
      },
      "features": {
        "nixome-managed": "1",
        "nixome-role": "legacy-fedora-app"
      },
      "services": {},
      "backend": {
        "type": "qubes-template",
        "template": "fedora-44-xfce"
      }
    },
    "nix-native-work": {
      "class": "StandaloneVM",
      "template": null,
      "label": "green",
      "netvm": null,
      "prefs": {
        "autostart": false,
        "include_in_backups": true,
        "kernelopts": "console=hvc0 init=/sbin/init",
        "memory": 1536,
        "virt_mode": "pvh"
      },
      "features": {
        "nixome-managed": "1",
        "nixome-role": "nix-native-work",
        "qrexec": "1"
      },
      "services": {},
      "backend": {
        "type": "nix-closure",
        "template": null,
        "system": {
          "source": "$SYSTEM_SOURCE",
          "source_ref": null
        },
        "kernel": {
          "name": "6.12.64-1.fc41",
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
          "source": "$ROOT_SOURCE",
          "source_ref": null,
          "read_only": false
        },
        "store": {
          "mode": "shared-closure",
          "source": "$STORE_SOURCE",
          "source_ref": null,
          "view_backend": "erofs-blobmap",
          "blob_device": "/dev/xvdf",
          "chunk_size": 65536,
          "mountpoint": "/nix/store",
          "read_only": true
        },
        "state": {
          "persistent": "nix-native-work-private",
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
exit 0
EOF

cat > "$BIN_DIR/qvm-create" <<'EOF'
#!/usr/bin/env bash
printf 'qvm-create %s\n' "$*" >> "$MUTATION_LOG"
EOF

cat > "$BIN_DIR/qvm-volume" <<'EOF'
#!/usr/bin/env bash
printf 'qvm-volume %s\n' "$*" >> "$MUTATION_LOG"
EOF

cat > "$BIN_DIR/qvm-prefs" <<'EOF'
#!/usr/bin/env bash
if [ "$#" = 2 ]; then
  exit 1
fi
printf 'qvm-prefs %s\n' "$*" >> "$MUTATION_LOG"
EOF

cat > "$BIN_DIR/qvm-features" <<'EOF'
#!/usr/bin/env bash
if [ "$#" = 2 ]; then
  exit 1
fi
printf 'qvm-features %s\n' "$*" >> "$MUTATION_LOG"
EOF

cat > "$BIN_DIR/qvm-service" <<'EOF'
#!/usr/bin/env bash
if [ "$#" = 2 ]; then
  exit 1
fi
printf 'qvm-service %s\n' "$*" >> "$MUTATION_LOG"
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

for command in qubes-prefs qvm-template-postprocess qubesd-query; do
  cat > "$BIN_DIR/$command" <<'EOF'
#!/usr/bin/env bash
printf 'unexpected command execution: %s\n' "$0" >&2
exit 97
EOF
done

cat > "$BIN_DIR/nixome-nix-domain-storage" <<'EOF'
#!/usr/bin/env bash
printf 'nixome-nix-domain-storage %s\n' "$*" >> "$MUTATION_LOG"
cat <<JSON
{"schema":"nixome.nix-domain-storage.v1","domain":"$2","volume":"store","mode":"shared-closure","source":"$4","read_only":true,"volume_created":true,"volumes_created":["store","store-data"],"image_imported":false,"view_created":true,"metadata_changed":true,"qubesd_restart_required":false,"qubesd_restarted":true,"generation":1,"closure_hash":"fixture","filesystem":"erofs","view_backend":"erofs-blobmap","view_image":"/var/lib/nixome/closure-store/domains/$2/generations/1/store.erofs","metadata_image":"/var/lib/nixome/closure-store/domains/$2/generations/1/store.erofs","blob_map":"/var/lib/nixome/closure-store/domains/$2/generations/1/store.blob-map.json","blob_volume":"store-data","blob_device":"/dev/xvdf","blob_logical_size":65536}
JSON
EOF

chmod 0755 "$BIN_DIR"/*

PATH="$BIN_DIR:$PATH" \
  MUTATION_LOG="$MUTATION_LOG" \
  NIXOME_QUBES_KERNEL_BASE="$KERNEL_BASE" \
  NIXOME_QUBES_POLICY_DIR="$POLICY_DIR" \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_LOG_DIR="$PLAN_LOG_DIR" \
  NIXOME_SYSTEM_STATE_DIR="$WORK_DIR/state-plan" \
  bash "$ROOT/scripts/setup-config.sh" --plan --topology "$TOPOLOGY" \
  > "$PLAN_LOG_DIR/stdout.log" 2> "$PLAN_LOG_DIR/stderr.log"

jq -e '
  .schema == "nixome.setup-config.v1" and
  .status == "passed" and
  .mode == "plan" and
  .domains_changed == 2 and
  .root_images_changed == 1 and
  .store_volumes_changed == 2 and
  .store_images_changed == 1 and
  .unsupported_domains_count == 0
' "$PLAN_LOG_DIR/setup-config.json" >/dev/null
grep -F '+ qvm-create --class StandaloneVM --label green nix-native-work' \
  "$PLAN_LOG_DIR/setup-config.commands.log" >/dev/null
grep -F '+ qvm-create --class AppVM --label red --template fedora-44-xfce legacy-fedora-app' \
  "$PLAN_LOG_DIR/setup-config.commands.log" >/dev/null
grep -F "+ qvm-volume import nix-native-work:root $ROOT_IMAGE" \
  "$PLAN_LOG_DIR/setup-config.commands.log" >/dev/null
grep -F "+ nixome-nix-domain-storage ensure-store nix-native-work $STORE_SOURCE $STORE_SOURCE --mode shared-closure --system $SYSTEM_SOURCE --root $ROOT_SOURCE --kernel 6.12.64-1.fc41 --device /dev/xvde --blob-device /dev/xvdf --mountpoint /nix/store --view-backend erofs-blobmap --chunk-size 65536" \
  "$PLAN_LOG_DIR/setup-config.commands.log" >/dev/null
[ "$(grep -c '^+ nixome-nix-domain-storage ensure-store ' "$PLAN_LOG_DIR/setup-config.commands.log")" = "1" ]

PATH="$BIN_DIR:$PATH" \
  MUTATION_LOG="$MUTATION_LOG" \
  NIXOME_QUBES_KERNEL_BASE="$KERNEL_BASE" \
  NIXOME_QUBES_POLICY_DIR="$POLICY_DIR" \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_LOG_DIR="$APPLY_LOG_DIR" \
  NIXOME_SYSTEM_STATE_DIR="$WORK_DIR/state-apply" \
  bash "$ROOT/scripts/setup-config.sh" --apply --topology "$TOPOLOGY" \
  > "$APPLY_LOG_DIR/stdout.log" 2> "$APPLY_LOG_DIR/stderr.log"

jq -e '
  .schema == "nixome.setup-config.v1" and
  .status == "passed" and
  .mode == "apply" and
  .domains_changed == 2 and
  .root_images_changed == 1 and
  .store_volumes_changed == 2 and
  .store_images_changed == 1 and
  .qubesd_restarts == 1 and
  .unsupported_domains_count == 0 and
  any(.live_actions[]; .type == "domain-root" and .action == "import" and .domain == "nix-native-work") and
  any(.live_actions[]; .type == "domain-store-view" and .action == "materialize" and .domain == "nix-native-work") and
  any(.live_actions[]; .type == "domain-store-volume" and .action == "create" and .domain == "nix-native-work" and .value == "store") and
  any(.live_actions[]; .type == "domain-store-volume" and .action == "create" and .domain == "nix-native-work" and .value == "store-data") and
  any(.live_actions[]; .type == "systemd" and .action == "restart" and .field == "service" and .value == "qubesd.service")
' "$APPLY_LOG_DIR/setup-config.json" >/dev/null

grep -Fx 'qvm-create --class AppVM --label red --template fedora-44-xfce legacy-fedora-app' "$MUTATION_LOG" >/dev/null
grep -Fx 'qvm-create --class StandaloneVM --label green nix-native-work' "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-volume import nix-native-work:root $ROOT_IMAGE" "$MUTATION_LOG" >/dev/null
grep -Fx 'qvm-prefs nix-native-work kernel 6.12.64-1.fc41' "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-prefs nix-native-work kernelopts console=hvc0 init=/sbin/init nixome.system=$SYSTEM_SOURCE nixome.storedev=/dev/xvde nixome.storefmt=erofs nixome.storeblobdev=/dev/xvdf" "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-features nix-native-work nixome-root-source $ROOT_SOURCE" "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-features nix-native-work nixome-store-source $STORE_SOURCE" "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-features nix-native-work nixome-store-image $STORE_SOURCE" "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-features nix-native-work nixome-store-mode shared-closure" "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-features nix-native-work nixome-store-view-backend erofs-blobmap" "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-features nix-native-work nixome-store-filesystem erofs" "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-features nix-native-work nixome-store-generation 1" "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-features nix-native-work nixome-store-closure-hash fixture" "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-features nix-native-work nixome-store-view-image /var/lib/nixome/closure-store/domains/nix-native-work/generations/1/store.erofs" "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-features nix-native-work nixome-store-blob-map /var/lib/nixome/closure-store/domains/nix-native-work/generations/1/store.blob-map.json" "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-features nix-native-work nixome-store-blob-volume store-data" "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-features nix-native-work nixome-store-device /dev/xvde" "$MUTATION_LOG" >/dev/null
grep -Fx "qvm-features nix-native-work nixome-store-blob-device /dev/xvdf" "$MUTATION_LOG" >/dev/null
grep -Fx "nixome-nix-domain-storage ensure-store nix-native-work $STORE_SOURCE $STORE_SOURCE --mode shared-closure --system $SYSTEM_SOURCE --root $ROOT_SOURCE --kernel 6.12.64-1.fc41 --device /dev/xvde --blob-device /dev/xvdf --mountpoint /nix/store --view-backend erofs-blobmap --chunk-size 65536" "$MUTATION_LOG" >/dev/null
[ "$(grep -c '^nixome-nix-domain-storage ensure-store ' "$MUTATION_LOG")" = "1" ]

printf 'setup-config nix-native materialization test ok\n'
