#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
WORK_DIR="$LOG_DIR/config-topology"
DOMAINS_JSON="$WORK_DIR/domains.json"
KERNELS_JSON="$WORK_DIR/kernels.json"
TEMPLATES_JSON="$WORK_DIR/templates.json"
RECONCILE_ON_BOOT="$WORK_DIR/reconcile-on-boot"
RECONCILE_AFTER_JSON="$WORK_DIR/reconcile-after.json"
RECONCILE_TIMER_WANTED_BY_JSON="$WORK_DIR/reconcile-timer-wanted-by.json"
RECONCILE_TIMER_ON_BOOT="$WORK_DIR/reconcile-timer-on-boot"
GUEST_INIT_WANTED_BY_JSON="$WORK_DIR/guest-init-wanted-by.json"
GUEST_INIT_AFTER_JSON="$WORK_DIR/guest-init-after.json"
GUEST_INIT_BEFORE_JSON="$WORK_DIR/guest-init-before.json"
GUEST_QREXEC_REQUIRES_JSON="$WORK_DIR/guest-qrexec-requires.json"
GUEST_QREXEC_AFTER_JSON="$WORK_DIR/guest-qrexec-after.json"

mkdir -p "$WORK_DIR"

nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.nixome.qubes.domains" > "$DOMAINS_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.nixome.qubes.kernels" > "$KERNELS_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.nixome.qubes.templates" > "$TEMPLATES_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.nixome.qubes.reconcileOnBoot" > "$RECONCILE_ON_BOOT"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.services.nixome-qubes-reconcile.after" > "$RECONCILE_AFTER_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.timers.nixome-qubes-reconcile.wantedBy" > "$RECONCILE_TIMER_WANTED_BY_JSON"
nix eval --raw "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.timers.nixome-qubes-reconcile.timerConfig.OnBootSec" > "$RECONCILE_TIMER_ON_BOOT"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-system.config.systemd.services.nixome-config-guest-init.wantedBy" > "$GUEST_INIT_WANTED_BY_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-system.config.systemd.services.nixome-config-guest-init.after" > "$GUEST_INIT_AFTER_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-system.config.systemd.services.nixome-config-guest-init.before" > "$GUEST_INIT_BEFORE_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-system.config.systemd.services.qubes-qrexec-agent.requires" > "$GUEST_QREXEC_REQUIRES_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-system.config.systemd.services.qubes-qrexec-agent.after" > "$GUEST_QREXEC_AFTER_JSON"

jq -e '
  ."config-vault".class == "AppVM" and
  ."config-vault".template == "nixome-nixos-config-vault" and
  ."config-vault".netvm == null and
  ."config-vault".features."nixome-role" == "config-vault" and
  ."config-mod".class == "AppVM" and
  ."config-mod".template == "nixome-nixos-config-mod" and
  ."config-mod".netvm == null and
  ."config-mod".features."nixome-role" == "config-mod"
' "$DOMAINS_JSON" >/dev/null

jq -e '
  ."6.12.64-1.fc41".source | test("qubes-kernel-6.12.64-1.fc41")
' "$KERNELS_JSON" >/dev/null

jq -e '
  ."nixome-nixos-config-vault".import == true and
  ."nixome-nixos-config-vault".skipStart == true and
  (."nixome-nixos-config-vault".source | test("nixome-nixos-config-vault")) and
  ."nixome-nixos-config-mod".import == true and
  ."nixome-nixos-config-mod".skipStart == true and
  (."nixome-nixos-config-mod".source | test("nixome-nixos-config-mod"))
' "$TEMPLATES_JSON" >/dev/null

jq -e '. == true' "$RECONCILE_ON_BOOT" >/dev/null

jq -e '
  index("timers.target") != null
' "$RECONCILE_TIMER_WANTED_BY_JSON" >/dev/null

grep -qx '90s' "$RECONCILE_TIMER_ON_BOOT"

jq -e '
  index("qubes-db-dom0.service") != null and
  index("qubesd.service") != null and
  index("qubes-qrexec-policy-daemon.service") != null
' "$RECONCILE_AFTER_JSON" >/dev/null

jq -e '
  index("multi-user.target") != null
' "$GUEST_INIT_WANTED_BY_JSON" >/dev/null

jq -e '
  index("nixome-qubes-rw-init.service") != null and
  index("qubes-db.service") != null
' "$GUEST_INIT_AFTER_JSON" >/dev/null

jq -e '
  index("qubes-qrexec-agent.service") != null
' "$GUEST_INIT_BEFORE_JSON" >/dev/null

jq -e '
  index("nixome-config-guest-init.service") != null
' "$GUEST_QREXEC_REQUIRES_JSON" >/dev/null

jq -e '
  index("nixome-config-guest-init.service") != null
' "$GUEST_QREXEC_AFTER_JSON" >/dev/null

printf 'config topology eval ok\n'
