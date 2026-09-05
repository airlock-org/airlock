#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${NIXOME_RECONCILE_STATIC_DIR:-$ROOT/.nixome-logs/reconcile-static}"
BIN_DIR="$WORK_DIR/bin"
KERNEL_DIR="$WORK_DIR/kernel/6.12.64-1.fc41"
TEMPLATE_DIR="$WORK_DIR/template/nixome-nixos-test"
TOPOLOGY="$WORK_DIR/topology.json"
CAPABILITIES="$WORK_DIR/xen-capabilities"
LOG_DIR="$WORK_DIR/logs"
KERNEL_BASE="$WORK_DIR/vm-kernels"

mkdir -p "$BIN_DIR" "$KERNEL_DIR" "$TEMPLATE_DIR" "$LOG_DIR"

printf 'control_d\n' > "$CAPABILITIES"
: > "$KERNEL_DIR/vmlinuz"
: > "$KERNEL_DIR/initramfs"
: > "$KERNEL_DIR/modules.img"
: > "$TEMPLATE_DIR/root.img"
printf 'qrexec=1\nvirt-mode=pvh\n' > "$TEMPLATE_DIR/template.conf"

cat > "$TOPOLOGY" <<EOF
{
  "schema": "nixome.qubes-topology.v1",
  "templates": {
    "nixome-nixos-test": {
      "import": true,
      "skip_start": true,
      "source": "$TEMPLATE_DIR"
    }
  },
  "kernels": {
    "6.12.64-1.fc41": {
      "source": "$KERNEL_DIR"
    }
  },
  "domains": {
    "sys-config-vault": {
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

for command in qubes-prefs qvm-create qvm-features qvm-prefs qvm-service qvm-template-postprocess; do
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
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  bash "$ROOT/scripts/reconcile-qubes-topology.sh" --plan --topology "$TOPOLOGY" \
  > "$LOG_DIR/stdout.log" 2> "$LOG_DIR/stderr.log"

jq -e '
  .schema == "nixome.qubes-reconcile.v1" and
  .status == "passed" and
  .mode == "plan" and
  .templates_changed == 1 and
  .kernels_changed == 1 and
  .domains_changed == 1 and
  .prefs_changed == 1
' "$LOG_DIR/qubes-reconcile.json" >/dev/null

printf 'reconcile topology static test ok\n'
