#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
HOST_LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
WORK_DIR="${NIXOME_SETUP_CONFIG_NIX_NATIVE_UNSUPPORTED_DIR:-$HOST_LOG_DIR/setup-config-nix-native-unsupported}"
BIN_DIR="$WORK_DIR/bin"
TOPOLOGY="$WORK_DIR/topology.json"
CAPABILITIES="$WORK_DIR/xen-capabilities"
PLAN_LOG_DIR="$WORK_DIR/plan-logs"
APPLY_LOG_DIR="$WORK_DIR/apply-logs"
POLICY_DIR="$WORK_DIR/policy.d"
KERNEL_BASE="$WORK_DIR/vm-kernels"

rm -rf "$WORK_DIR"
mkdir -p "$BIN_DIR" "$PLAN_LOG_DIR" "$APPLY_LOG_DIR" "$POLICY_DIR" "$KERNEL_BASE"

printf 'control_d\n' > "$CAPABILITIES"

cat > "$TOPOLOGY" <<'EOF'
{
  "schema": "nixome.qubes-topology.v1",
  "templates": {},
  "kernels": {},
  "domains": {
    "nix-native-work": {
      "class": "StandaloneVM",
      "template": null,
      "label": "green",
      "netvm": null,
      "prefs": {
        "autostart": false,
        "include_in_backups": false,
        "memory": 1536
      },
      "features": {
        "nixome-managed": "1",
        "nixome-role": "nix-native"
      },
      "services": {},
      "backend": {
        "type": "nix-closure",
        "template": null,
        "system": {
          "source": "/nix/store/test-system",
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
        "store": {
          "mode": "closure-image",
          "source": "/nix/store/test-store.img",
          "source_ref": null,
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

for command in qubes-prefs qvm-create qvm-features qvm-prefs qvm-service qvm-template-postprocess qubesd-query; do
  cat > "$BIN_DIR/$command" <<'EOF'
#!/usr/bin/env bash
printf 'unexpected command execution: %s\n' "$0" >&2
exit 97
EOF
  chmod 0755 "$BIN_DIR/$command"
done

cat > "$BIN_DIR/qvm-ls" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod 0755 "$BIN_DIR/qvm-ls"

cat > "$BIN_DIR/systemctl" <<'EOF'
#!/usr/bin/env bash
if [ "$1" = "is-active" ]; then
  exit 0
fi
exit 1
EOF
chmod 0755 "$BIN_DIR/systemctl"

PATH="$BIN_DIR:$PATH" \
  NIXOME_QUBES_KERNEL_BASE="$KERNEL_BASE" \
  NIXOME_QUBES_POLICY_DIR="$POLICY_DIR" \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_LOG_DIR="$PLAN_LOG_DIR" \
  bash "$ROOT/scripts/setup-config.sh" --plan --topology "$TOPOLOGY" \
  > "$PLAN_LOG_DIR/stdout.log" 2> "$PLAN_LOG_DIR/stderr.log"

jq -e '
  .schema == "nixome.setup-config.v1" and
  .status == "passed" and
  .mode == "plan" and
  .domains_changed == 0 and
  .unsupported_domains_count == 1 and
  .unsupported_domains == [
    {
      "domain": "nix-native-work",
      "backend": "nix-closure",
      "reason": "nix-closure domain materializer is not implemented yet"
    }
  ]
' "$PLAN_LOG_DIR/setup-config.json" >/dev/null
grep -F 'skipping unsupported nix-closure domain backend in plan: nix-native-work' \
  "$PLAN_LOG_DIR/setup-config.commands.log" >/dev/null

if PATH="$BIN_DIR:$PATH" \
  NIXOME_QUBES_KERNEL_BASE="$KERNEL_BASE" \
  NIXOME_QUBES_POLICY_DIR="$POLICY_DIR" \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_LOG_DIR="$APPLY_LOG_DIR" \
  bash "$ROOT/scripts/setup-config.sh" --apply --topology "$TOPOLOGY" \
  > "$APPLY_LOG_DIR/stdout.log" 2> "$APPLY_LOG_DIR/stderr.log"; then
  printf 'setup-config apply unexpectedly accepted nix-closure backend\n' >&2
  exit 1
fi

jq -e '
  .schema == "nixome.setup-config.v1" and
  .status == "failed" and
  .mode == "apply" and
  (.reason | contains("nix-closure domain materializer is not implemented yet: nix-native-work")) and
  .domains_changed == 0 and
  .qrexec_policy_changed == 0 and
  .unsupported_domains_count == 1
' "$APPLY_LOG_DIR/setup-config.json" >/dev/null
grep -F 'nixome setup-config failed: nix-closure domain materializer is not implemented yet: nix-native-work' \
  "$APPLY_LOG_DIR/setup-config.commands.log" >/dev/null

printf 'setup-config nix-native unsupported test ok\n'
