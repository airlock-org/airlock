#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
mkdir -p "$LOG_DIR"

SSH_PORT="${NIXOME_VM_SSH_PORT:-2222}"
SSH_USER="${NIXOME_VM_SSH_USER:-nixome}"
SSH_PASSWORD="${NIXOME_VM_SSH_PASSWORD:-nixome}"
SSH_WAIT_SECONDS="${NIXOME_VM_SSH_WAIT_SECONDS:-240}"
VM_TIMEOUT="${NIXOME_VM_RUN_TIMEOUT:-14400}"
VM_START_WAIT_SECONDS="${NIXOME_VM_START_WAIT_SECONDS:-$VM_TIMEOUT}"
SETUP_CONFIG_BOOT_WAIT_SECONDS="${NIXOME_SETUP_CONFIG_BOOT_WAIT_SECONDS:-900}"
IMAGE_ATTR="${NIXOME_VM_IMAGE_ATTR:-nixome-vm-gui-image}"
DISPLAY_MODE="${NIXOME_VM_DISPLAY:-gtk}"
KEEP_VM="${NIXOME_KEEP_VM:-0}"
AUTO_GC_AFTER_RUN="${NIXOME_AUTO_GC_AFTER_RUN:-0}"
SYNC_DOM0_TOOLS="${NIXOME_SYNC_DOM0_TOOLS:-0}"
REMOTE_WARM_BIN_DIR="${NIXOME_NESTED_WARM_BIN_DIR:-/run/nixome-warm/bin}"
SETUP_CONFIG_COMMAND="${NIXOME_SETUP_CONFIG_COMMAND:-nixome-setup-config}"
NIX_CLOSURE_DOMAIN_SMOKE_COMMAND="${NIXOME_NIX_CLOSURE_DOMAIN_SMOKE_COMMAND:-nixome-nix-closure-domain-smoke}"

BOOTSTRAP_QUBES="${NIXOME_BOOTSTRAP_QUBES:-1}"
RUN_SETUP_CONFIG="${NIXOME_RUN_SETUP_CONFIG:-0}"
RUN_CONFIG_VMS_SMOKE="${NIXOME_RUN_CONFIG_VMS_SMOKE:-0}"
RUN_CONFIG_BOOT_VERIFY="${NIXOME_RUN_CONFIG_BOOT_VERIFY:-0}"
RUN_NIX_CLOSURE_SMOKE="${NIXOME_RUN_NIX_CLOSURE_SMOKE:-0}"
RUN_QREXEC_SMOKE="${NIXOME_RUN_QUBES_SMOKE:-1}"
RUN_GUI_SMOKE="${NIXOME_RUN_GUI_SMOKE:-1}"
CACHE_QUBES_ASSETS="${NIXOME_CACHE_QUBES_ASSETS:-1}"
CACHE_DIR="${NIXOME_QUBES_CACHE_DIR:-$ROOT/.nixome-cache/rpms}"
GUEST_CACHE_DIR="/var/cache/nixome/qubes-rpms"
SHUTDOWN_VM_ON_SUCCESS="${NIXOME_SHUTDOWN_VM_ON_SUCCESS:-0}"
SHUTDOWN_WAIT_SECONDS="${NIXOME_VM_SHUTDOWN_WAIT_SECONDS:-180}"

TEMPLATE="${NIXOME_QUBES_TEMPLATE:-fedora-43-xfce}"
TEMPLATE_URL="${NIXOME_QUBES_TEMPLATE_URL:-https://ftp.qubes-os.org/repo/yum/r4.3/templates-itl/rpm/qubes-template-fedora-43-xfce-4.3.0-202601041226.noarch.rpm}"
QUBE="${NIXOME_TEST_QUBE:-test-nixome-smoke}"

KERNEL_URL="${NIXOME_QUBES_KERNEL_URL:-https://ftp.qubes-os.org/repo/yum/r4.3/current/dom0/fc41/rpm/kernel-qubes-vm-6.12.64-1.qubes.fc41.x86_64.rpm}"
KERNEL_MODULES_URL="${NIXOME_QUBES_KERNEL_MODULES_URL:-https://ftp.qubes-os.org/repo/yum/r4.3/current/dom0/fc41/rpm/kernel-modules-6.12.64-1.qubes.fc41.x86_64.rpm}"
GPG_KEY_URL="${NIXOME_QUBES_GPG_KEY_URL:-https://keys.qubes-os.org/keys/qubes-release-4.3-signing-key.asc}"

QEMU_LOG="$LOG_DIR/vm-gui-smoke-qemu.log"
OBSERVE_JSON="$LOG_DIR/vm-gui-observe.json"
SERVICES_LOG="$LOG_DIR/vm-gui-services.log"
FAILED_UNITS_LOG="$LOG_DIR/vm-gui-failed-units.log"
JOURNAL_LOG="$LOG_DIR/vm-gui-journal.log"
BOOTSTRAP_JSON="$LOG_DIR/vm-gui-bootstrap.json"
BOOTSTRAP_STDOUT="$LOG_DIR/vm-gui-bootstrap.stdout.log"
BOOTSTRAP_STDERR="$LOG_DIR/vm-gui-bootstrap.stderr.log"
SETUP_CONFIG_BOOT_JSON="$LOG_DIR/vm-gui-setup-config-boot.json"
SETUP_CONFIG_BOOT_COMMANDS_LOG="$LOG_DIR/vm-gui-setup-config-boot.commands.log"
SETUP_CONFIG_JSON="$LOG_DIR/vm-gui-setup-config.json"
SETUP_CONFIG_STDOUT="$LOG_DIR/vm-gui-setup-config.stdout.log"
SETUP_CONFIG_STDERR="$LOG_DIR/vm-gui-setup-config.stderr.log"
CONFIG_VMS_JSON="$LOG_DIR/vm-gui-config-vms-smoke.json"
CONFIG_VMS_STDOUT="$LOG_DIR/vm-gui-config-vms-smoke.stdout.log"
CONFIG_VMS_STDERR="$LOG_DIR/vm-gui-config-vms-smoke.stderr.log"
CONFIG_VMS_COMMANDS_LOG="$LOG_DIR/vm-gui-config-vms-smoke.commands.log"
CONFIG_VMS_STAGING_UPDATE_LOG="$LOG_DIR/vm-gui-config-vms-config-staging-update.log"
CONFIG_VMS_STAGING_UPDATE_SCRIPT="$LOG_DIR/vm-gui-config-vms-config-staging-update.sh"
CONFIG_BOOT_VERIFY_JSON="$LOG_DIR/vm-gui-config-boot-verify.json"
CONFIG_BOOT_VERIFY_STDOUT="$LOG_DIR/vm-gui-config-boot-verify.stdout.log"
CONFIG_BOOT_VERIFY_STDERR="$LOG_DIR/vm-gui-config-boot-verify.stderr.log"
NIX_CLOSURE_JSON="$LOG_DIR/vm-gui-nix-closure-domain-smoke.json"
NIX_CLOSURE_STDOUT="$LOG_DIR/vm-gui-nix-closure-domain-smoke.stdout.log"
NIX_CLOSURE_STDERR="$LOG_DIR/vm-gui-nix-closure-domain-smoke.stderr.log"
NIX_CLOSURE_COMMANDS_LOG="$LOG_DIR/vm-gui-nix-closure-domain-smoke.commands.log"
SYNC_DOM0_TOOLS_JSON="$LOG_DIR/vm-gui-sync-dom0-tools.json"
SYNC_DOM0_TOOLS_STDOUT="$LOG_DIR/vm-gui-sync-dom0-tools.stdout.log"
SYNC_DOM0_TOOLS_STDERR="$LOG_DIR/vm-gui-sync-dom0-tools.stderr.log"
QREXEC_JSON="$LOG_DIR/vm-gui-qrexec-smoke.json"
QREXEC_STDERR="$LOG_DIR/vm-gui-qrexec-smoke.stderr.log"
GUI_JSON="$LOG_DIR/vm-gui-window-smoke.json"
GUI_STDERR="$LOG_DIR/vm-gui-window-smoke.stderr.log"
QVM_LS_LOG="$LOG_DIR/vm-gui-qvm-ls.log"

vm_pid=""
cached_gpg_key_host=""
cached_gpg_key_guest=""
cached_kernel_host=""
cached_kernel_guest=""
cached_kernel_modules_host=""
cached_kernel_modules_guest=""
cached_template_host=""
cached_template_guest=""

rm -f \
  "$OBSERVE_JSON" \
  "$SERVICES_LOG" \
  "$FAILED_UNITS_LOG" \
  "$JOURNAL_LOG" \
  "$BOOTSTRAP_JSON" \
  "$BOOTSTRAP_STDOUT" \
  "$BOOTSTRAP_STDERR" \
  "$SETUP_CONFIG_BOOT_JSON" \
  "$SETUP_CONFIG_BOOT_COMMANDS_LOG" \
  "$SETUP_CONFIG_JSON" \
  "$SETUP_CONFIG_STDOUT" \
  "$SETUP_CONFIG_STDERR" \
  "$CONFIG_VMS_JSON" \
  "$CONFIG_VMS_STDOUT" \
  "$CONFIG_VMS_STDERR" \
  "$CONFIG_VMS_COMMANDS_LOG" \
  "$CONFIG_VMS_STAGING_UPDATE_LOG" \
  "$CONFIG_VMS_STAGING_UPDATE_SCRIPT" \
  "$CONFIG_BOOT_VERIFY_JSON" \
  "$CONFIG_BOOT_VERIFY_STDOUT" \
  "$CONFIG_BOOT_VERIFY_STDERR" \
  "$NIX_CLOSURE_JSON" \
  "$NIX_CLOSURE_STDOUT" \
  "$NIX_CLOSURE_STDERR" \
  "$NIX_CLOSURE_COMMANDS_LOG" \
  "$SYNC_DOM0_TOOLS_JSON" \
  "$SYNC_DOM0_TOOLS_STDOUT" \
  "$SYNC_DOM0_TOOLS_STDERR" \
  "$QREXEC_JSON" \
  "$QREXEC_STDERR" \
  "$GUI_JSON" \
  "$GUI_STDERR" \
  "$QVM_LS_LOG"

cleanup() {
  if [ "$KEEP_VM" = "1" ]; then
    return
  fi

  if [ -n "$vm_pid" ] && kill -0 "$vm_pid" >/dev/null 2>&1; then
    kill "$vm_pid" >/dev/null 2>&1 || true
    wait "$vm_pid" >/dev/null 2>&1 || true
  fi

  case "$AUTO_GC_AFTER_RUN" in
    1|true|yes)
      "$ROOT/scripts/gc-nixome-store.sh" auto || true
      ;;
    0|false|no)
      ;;
    *)
      printf 'warning: ignoring unsupported NIXOME_AUTO_GC_AFTER_RUN value: %s\n' "$AUTO_GC_AFTER_RUN" >&2
      ;;
  esac
}

trap cleanup EXIT INT TERM

shell_quote() {
  printf '%q' "$1"
}

filename_from_url() {
  local url="${1%%\?*}"
  url="${url%/}"
  local filename="${url##*/}"

  if [ -z "$filename" ]; then
    printf 'could not determine filename from URL: %s\n' "$1" >&2
    exit 1
  fi

  printf '%s' "$filename"
}

require_host_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'missing host command: %s\n' "$1" >&2
    exit 1
  fi
}

download_cached() {
  local url="$1"
  local filename="$2"
  local dest="$CACHE_DIR/$filename"
  local partial="$dest.part"
  local curl_resume=()

  mkdir -p "$CACHE_DIR"

  if [ -s "$dest" ]; then
    printf 'cache: using %s\n' "$dest" >&2
    printf '%s' "$dest"
    return 0
  fi

  if [ -s "$partial" ]; then
    curl_resume=(--continue-at -)
    printf 'cache: resuming %s\n' "$url" >&2
  else
    printf 'cache: downloading %s\n' "$url" >&2
  fi

  curl \
    --fail \
    --location \
    --silent \
    --show-error \
    "${curl_resume[@]}" \
    --output "$partial" \
    "$url"

  chmod 0644 "$partial"
  mv "$partial" "$dest"
  printf 'cache: stored %s\n' "$dest" >&2
  printf '%s' "$dest"
}

prepare_cached_bootstrap_assets() {
  if [ "$BOOTSTRAP_QUBES" != "1" ] || [ "$CACHE_QUBES_ASSETS" != "1" ]; then
    return 0
  fi

  require_host_command curl
  require_host_command stat

  printf 'cache: host Qubes asset cache: %s\n' "$CACHE_DIR"

  if [ -z "${NIXOME_QUBES_GPG_KEY:-}" ] && [ "${NIXOME_QUBES_VERIFY_RPMS:-1}" = "1" ]; then
    local gpg_key_filename
    gpg_key_filename="$(filename_from_url "$GPG_KEY_URL")"
    cached_gpg_key_host="$(download_cached "$GPG_KEY_URL" "$gpg_key_filename")"
    cached_gpg_key_guest="$GUEST_CACHE_DIR/$gpg_key_filename"
    NIXOME_QUBES_GPG_KEY="$cached_gpg_key_guest"
  fi

  if [ -n "${NIXOME_QUBES_KERNEL_RPM:-}" ] || [ -n "${NIXOME_QUBES_KERNEL_MODULES_RPM:-}" ]; then
    if [ -z "${NIXOME_QUBES_KERNEL_RPM:-}" ] || [ -z "${NIXOME_QUBES_KERNEL_MODULES_RPM:-}" ]; then
      printf 'NIXOME_QUBES_KERNEL_RPM and NIXOME_QUBES_KERNEL_MODULES_RPM must be set together\n' >&2
      exit 1
    fi
  else
    local kernel_filename
    local kernel_modules_filename
    kernel_filename="$(filename_from_url "$KERNEL_URL")"
    kernel_modules_filename="$(filename_from_url "$KERNEL_MODULES_URL")"
    cached_kernel_host="$(download_cached "$KERNEL_URL" "$kernel_filename")"
    cached_kernel_guest="$GUEST_CACHE_DIR/$kernel_filename"
    cached_kernel_modules_host="$(download_cached "$KERNEL_MODULES_URL" "$kernel_modules_filename")"
    cached_kernel_modules_guest="$GUEST_CACHE_DIR/$kernel_modules_filename"
    NIXOME_QUBES_KERNEL_RPM="$cached_kernel_guest"
    NIXOME_QUBES_KERNEL_MODULES_RPM="$cached_kernel_modules_guest"
  fi

  if [ -z "${NIXOME_QUBES_TEMPLATE_RPM:-}" ]; then
    local template_filename
    template_filename="$(filename_from_url "$TEMPLATE_URL")"
    cached_template_host="$(download_cached "$TEMPLATE_URL" "$template_filename")"
    cached_template_guest="$GUEST_CACHE_DIR/$template_filename"
    NIXOME_QUBES_TEMPLATE_RPM="$cached_template_guest"
  fi
}

copy_to_guest_cache() {
  local host_path="$1"
  local guest_path="$2"
  local guest_partial="$guest_path.part"
  local host_size

  if [ -z "$host_path" ]; then
    return 0
  fi

  host_size="$(stat -c %s "$host_path")"

  if remote "test -f $(shell_quote "$guest_path") && [ \"\$(stat -c %s $(shell_quote "$guest_path"))\" = $(shell_quote "$host_size") ]" >/dev/null 2>&1; then
    printf 'cache: guest already has %s\n' "$guest_path"
    return 0
  fi

  printf 'cache: copying %s to guest:%s\n' "$host_path" "$guest_path"
  remote "sudo -n install -d -m 0755 $(shell_quote "$GUEST_CACHE_DIR")"
  ssh_command "sudo -n tee $(shell_quote "$guest_partial") >/dev/null" < "$host_path"
  remote "sudo -n chmod 0644 $(shell_quote "$guest_partial") && sudo -n mv $(shell_quote "$guest_partial") $(shell_quote "$guest_path")"
}

copy_cached_bootstrap_assets_to_guest() {
  if [ "$BOOTSTRAP_QUBES" != "1" ] || [ "$CACHE_QUBES_ASSETS" != "1" ]; then
    return 0
  fi

  copy_to_guest_cache "$cached_gpg_key_host" "$cached_gpg_key_guest"
  copy_to_guest_cache "$cached_kernel_host" "$cached_kernel_guest"
  copy_to_guest_cache "$cached_kernel_modules_host" "$cached_kernel_modules_guest"
  copy_to_guest_cache "$cached_template_host" "$cached_template_guest"
}

remote_env_prefix() {
  local prefix="env"
  local name

  for name in \
    NIXOME_QUBES_TEMPLATE \
    NIXOME_QUBES_TEMPLATE_URL \
    NIXOME_QUBES_TEMPLATE_RPM \
    NIXOME_QUBES_KERNEL_NAME \
    NIXOME_QUBES_KERNEL_VERSION \
    NIXOME_QUBES_KERNEL_FEDORA \
    NIXOME_QUBES_KERNEL_URL \
    NIXOME_QUBES_KERNEL_MODULES_URL \
    NIXOME_QUBES_KERNEL_RPM \
    NIXOME_QUBES_KERNEL_MODULES_RPM \
    NIXOME_QUBES_GPG_KEY_URL \
    NIXOME_QUBES_GPG_KEY \
    NIXOME_QUBES_VERIFY_RPMS \
    NIXOME_QUBES_DEFAULT_DVM \
    NIXOME_QUBES_UPDATEVM; do
    if [ -n "${!name+x}" ]; then
      prefix="$prefix $name=$(shell_quote "${!name}")"
    fi
  done

  printf '%s' "$prefix"
}

config_vms_env_prefix() {
  local prefix="env"
  local name

  for name in \
    NIXOME_CONFIG_DOM0_SYSTEM_REASON \
    NIXOME_CONFIG_DOM0_SYSTEM_SWITCH_ACTION \
    NIXOME_CONFIG_STAGING_VM \
    NIXOME_CONFIG_VALIDATOR_VM \
    NIXOME_CONFIG_VAULT_VM; do
    if [ -n "${!name+x}" ]; then
      prefix="$prefix $name=$(shell_quote "${!name}")"
    fi
  done

  printf '%s' "$prefix"
}

config_boot_verify_env_prefix() {
  local prefix="env"
  local name

  for name in \
    NIXOME_EXPECTED_APPLIED_MESSAGE \
    NIXOME_EXPECTED_HOST_SYSTEM_SOURCE; do
    if [ -n "${!name+x}" ]; then
      prefix="$prefix $name=$(shell_quote "${!name}")"
    fi
  done

  printf '%s' "$prefix"
}

nix_closure_env_prefix() {
  local prefix="env"
  local name

  for name in \
    NIXOME_CONFIG_APPLY_NIX_OFFLINE \
    NIXOME_CONFIG_STAGING_VM \
    NIXOME_CONFIG_VALIDATOR_VM \
    NIXOME_CONFIG_VAULT_VM \
    NIXOME_NIX_CLOSURE_APPLY_MESSAGE \
    NIXOME_NIX_CLOSURE_DOMAIN \
    NIXOME_NIX_CLOSURE_GUI_PROBE \
    NIXOME_NIX_CLOSURE_QREXEC_TIMEOUT_SECONDS \
    NIXOME_NIX_CLOSURE_WAIT_SECONDS \
    NIXOME_QUBE_USER; do
    if [ -n "${!name+x}" ]; then
      prefix="$prefix $name=$(shell_quote "${!name}")"
    fi
  done

  printf '%s' "$prefix"
}

ssh_command() {
  SSHPASS="$SSH_PASSWORD" "$ROOT/scripts/nixome-nix.sh" shell nixpkgs#sshpass -c sshpass -e \
    ssh \
      -F /dev/null \
      -o LogLevel=ERROR \
      -o StrictHostKeyChecking=no \
      -o UserKnownHostsFile=/dev/null \
      -o ConnectTimeout=5 \
      -o NumberOfPasswordPrompts=1 \
      -o PubkeyAuthentication=no \
      -p "$SSH_PORT" \
      "$SSH_USER@127.0.0.1" \
      "$@"
}

remote() {
  ssh_command "$1"
}

sync_dom0_tools() {
  case "$SYNC_DOM0_TOOLS" in
    1|true|yes)
      ;;
    0|false|no)
      printf '{"schema":"nixome.nested-dom0-tool-sync.v1","status":"skipped","reason":"NIXOME_SYNC_DOM0_TOOLS=0"}\n' > "$SYNC_DOM0_TOOLS_JSON"
      return 0
      ;;
    *)
      printf 'unsupported NIXOME_SYNC_DOM0_TOOLS value: %s\n' "$SYNC_DOM0_TOOLS" >&2
      exit 1
      ;;
  esac

  printf 'warm-sync: syncing current dom0 tools into nested VM\n'
  if ! NIXOME_LOG_DIR="$LOG_DIR" "$ROOT/scripts/sync-nested-dom0-tools.sh" > "$SYNC_DOM0_TOOLS_STDOUT" 2> "$SYNC_DOM0_TOOLS_STDERR"; then
    collect_diagnostics
    cat "$SYNC_DOM0_TOOLS_STDERR" >&2 || true
    exit 1
  fi
  if ! extract_json_result "nixome.nested-dom0-tool-sync.v1" "$SYNC_DOM0_TOOLS_STDOUT" "$SYNC_DOM0_TOOLS_JSON"; then
    collect_diagnostics
    printf 'warm-sync did not emit result JSON; see %s\n' "$SYNC_DOM0_TOOLS_STDOUT" >&2
    exit 1
  fi

  SETUP_CONFIG_COMMAND="$REMOTE_WARM_BIN_DIR/nixome-setup-config"
  NIX_CLOSURE_DOMAIN_SMOKE_COMMAND="$REMOTE_WARM_BIN_DIR/nixome-nix-closure-domain-smoke"
}

collect_diagnostics() {
  remote 'systemctl --failed --no-legend || true' > "$FAILED_UNITS_LOG" 2>/dev/null || true
  remote 'journalctl -b --no-pager -n 320 -u qubesd -u qubes-core -u libvirtd -u qubes-db-dom0 -u qubes-qrexec-policy-daemon -u nixome-qubes-store-init -u nixome-setup-config || true' > "$JOURNAL_LOG" 2>/dev/null || true
  remote 'qvm-ls || true' > "$QVM_LS_LOG" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/qubes-bootstrap.commands.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-bootstrap.commands.log" 2>/dev/null || true
  [ -s "$SETUP_CONFIG_BOOT_JSON" ] ||
    remote 'sudo -n cat /var/log/nixome/setup-config.json 2>/dev/null || true' > "$SETUP_CONFIG_BOOT_JSON" 2>/dev/null || true
  [ -s "$SETUP_CONFIG_BOOT_COMMANDS_LOG" ] ||
    remote 'sudo -n cat /var/log/nixome/setup-config.commands.log 2>/dev/null || true' > "$SETUP_CONFIG_BOOT_COMMANDS_LOG" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/setup-config.commands.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-setup-config.commands.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-vms-smoke.commands.log 2>/dev/null || true' > "$CONFIG_VMS_COMMANDS_LOG" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-vms-config-staging-update.log 2>/dev/null || true' > "$CONFIG_VMS_STAGING_UPDATE_LOG" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-vms-config-staging-update.sh 2>/dev/null || true' > "$CONFIG_VMS_STAGING_UPDATE_SCRIPT" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-archive.json 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-archive.json" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-archive.commands.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-archive.commands.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-apply.json 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-apply.json" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-apply.commands.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-apply.commands.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-apply-template-builds.json 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-apply-template-builds.json" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-apply-template-builds.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-apply-template-builds.log" 2>/dev/null || true
  remote 'sudo -n sh -c '"'"'for f in /var/log/nixome/config-apply-template-build-*.log; do [ -f "$f" ] && printf "== %s ==\n" "$f" && cat "$f"; done'"'"' 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-apply-template-build-details.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-apply-system-builds.json 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-apply-system-builds.json" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-apply-system-builds.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-apply-system-builds.log" 2>/dev/null || true
  remote 'sudo -n sh -c '"'"'for f in /var/log/nixome/config-apply-system-build-*.log; do [ -f "$f" ] && printf "== %s ==\n" "$f" && cat "$f"; done'"'"' 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-apply-system-build-details.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-apply-nix-domain-builds.json 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-apply-nix-domain-builds.json" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-apply-nix-domain-builds.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-apply-nix-domain-builds.log" 2>/dev/null || true
  remote 'sudo -n sh -c '"'"'for f in /var/log/nixome/config-apply-nix-domain-build-*.log; do [ -f "$f" ] && printf "== %s ==\n" "$f" && cat "$f"; done'"'"' 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-apply-nix-domain-build-details.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-apply-nix-domain-materializations.json 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-apply-nix-domain-materializations.json" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-system-apply.json 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-system-apply.json" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-system-apply.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-system-apply.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-system-profile-activation.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-system-profile-activation.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-vms-journal.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-vms-journal.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-vms-libxl.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-vms-libxl.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-vms-qvm-prefs.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-vms-qvm-prefs.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-vms-config-status.json 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-vms-config-status.json" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-vms-config-vault-console.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-vms-config-vault-console.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-vms-config-staging-console.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-vms-config-staging-console.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-vms-config-validator-console.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-vms-config-validator-console.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-boot-verify.json 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-boot-verify.json" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-boot-verify-status.json 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-boot-verify-status.json" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/config-boot-verify-marker.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-config-boot-verify-marker.log" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/nix-closure-domain-smoke.json 2>/dev/null || true' > "$NIX_CLOSURE_JSON" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/nix-closure-domain-smoke.commands.log 2>/dev/null || true' > "$NIX_CLOSURE_COMMANDS_LOG" 2>/dev/null || true
  remote 'sudo -n sh -c '"'"'for f in /var/log/nixome/nix-closure-domain-*; do [ -f "$f" ] && printf "== %s ==\n" "$f" && cat "$f"; done'"'"' 2>/dev/null || true' > "$LOG_DIR/vm-gui-nix-closure-domain-details.log" 2>/dev/null || true
}

extract_json_result() {
  local schema="$1"
  local input="$2"
  local output="$3"

  awk -v schema="$schema" '
    index($0, "\"schema\":\"" schema "\"") { line = $0 }
    END {
      if (line != "") {
        print line
        exit 0
      }
      exit 1
    }
  ' "$input" > "$output"
}

wait_for_setup_config_boot_unit() {
  if ! remote 'systemctl cat nixome-setup-config.service >/dev/null 2>&1'; then
    return 0
  fi

  printf 'setup-config: waiting for boot setup unit\n'
  if ! remote "deadline=\$((SECONDS + $SETUP_CONFIG_BOOT_WAIT_SECONDS)); while [ \"\$(systemctl show -p ActiveState --value nixome-setup-config.service 2>/dev/null || printf missing)\" = activating ]; do if [ \"\$SECONDS\" -ge \"\$deadline\" ]; then systemctl --no-pager status nixome-setup-config.service || true; exit 124; fi; sleep 2; done; state=\"\$(systemctl show -p ActiveState --value nixome-setup-config.service 2>/dev/null || printf missing)\"; if [ \"\$state\" = failed ]; then systemctl --no-pager status nixome-setup-config.service || true; exit 1; fi"; then
    remote 'sudo -n cat /var/log/nixome/setup-config.json 2>/dev/null || true' > "$SETUP_CONFIG_JSON" 2>/dev/null || true
    collect_diagnostics
    exit 1
  fi

  copy_boot_setup_config_artifacts || true
}

copy_boot_setup_config_artifacts() {
  local copied=1

  if remote 'sudo -n test -r /var/log/nixome/setup-config.json' >/dev/null 2>&1; then
    remote 'sudo -n cat /var/log/nixome/setup-config.json' > "$SETUP_CONFIG_BOOT_JSON" 2>/dev/null || true
    copied=0
  fi

  if remote 'sudo -n test -r /var/log/nixome/setup-config.commands.log' >/dev/null 2>&1; then
    remote 'sudo -n cat /var/log/nixome/setup-config.commands.log' > "$SETUP_CONFIG_BOOT_COMMANDS_LOG" 2>/dev/null || true
    copied=0
  fi

  return "$copied"
}

copy_boot_setup_config_result() {
  copy_boot_setup_config_artifacts || return 1

  if grep -q '"schema":"nixome.setup-config.v1"' "$SETUP_CONFIG_BOOT_JSON"; then
    cp "$SETUP_CONFIG_BOOT_JSON" "$SETUP_CONFIG_JSON"
    return 0
  fi

  return 1
}

shutdown_vm_on_success() {
  case "$SHUTDOWN_VM_ON_SUCCESS" in
    1|true|yes)
      ;;
    0|false|no)
      return 0
      ;;
    *)
      printf 'warning: ignoring unsupported NIXOME_SHUTDOWN_VM_ON_SUCCESS value: %s\n' "$SHUTDOWN_VM_ON_SUCCESS" >&2
      return 0
      ;;
  esac

  if [ "$KEEP_VM" = "1" ] || [ -z "$vm_pid" ] || ! kill -0 "$vm_pid" >/dev/null 2>&1; then
    return 0
  fi

  printf 'vm: requesting guest shutdown\n'
  remote 'sudo -n systemctl poweroff' >/dev/null 2>&1 || true

  local deadline
  deadline=$((SECONDS + SHUTDOWN_WAIT_SECONDS))
  while [ "$SECONDS" -lt "$deadline" ]; do
    if ! kill -0 "$vm_pid" >/dev/null 2>&1; then
      wait "$vm_pid" >/dev/null 2>&1 || true
      vm_pid=""
      return 0
    fi
    sleep 2
  done

  printf 'warning: VM did not shut down within %s seconds; cleanup will terminate it\n' "$SHUTDOWN_WAIT_SECONDS" >&2
}

prepare_cached_bootstrap_assets

printf 'starting nested GUI VM smoke run\n'
printf 'logs: %s\n' "$LOG_DIR"

timeout -k 20s "$VM_TIMEOUT" env \
  NIXOME_VM_IMAGE_ATTR="$IMAGE_ATTR" \
  NIXOME_VM_DISPLAY="$DISPLAY_MODE" \
  NIXOME_VM_SSH_PORT="$SSH_PORT" \
  "$ROOT/scripts/run-nested-vm.sh" > "$QEMU_LOG" 2>&1 &
vm_pid="$!"

vm_start_deadline=$((SECONDS + VM_START_WAIT_SECONDS))
while [ "$SECONDS" -lt "$vm_start_deadline" ]; do
  if grep -q '^starting VM$' "$QEMU_LOG" 2>/dev/null; then
    break
  fi

  if ! kill -0 "$vm_pid" >/dev/null 2>&1; then
    tail -n 120 "$QEMU_LOG" >&2 || true
    exit 1
  fi

  sleep 3
done

if ! grep -q '^starting VM$' "$QEMU_LOG" 2>/dev/null; then
  printf 'VM did not start within %s seconds; see %s\n' "$VM_START_WAIT_SECONDS" "$QEMU_LOG" >&2
  tail -n 120 "$QEMU_LOG" >&2 || true
  exit 1
fi

deadline=$((SECONDS + SSH_WAIT_SECONDS))
while [ "$SECONDS" -lt "$deadline" ]; do
  if remote 'echo ssh-ready' >/dev/null 2>&1; then
    printf 'ssh: ready\n'
    break
  fi

  if ! kill -0 "$vm_pid" >/dev/null 2>&1; then
    tail -n 120 "$QEMU_LOG" >&2 || true
    exit 1
  fi

  sleep 3
done

if ! remote 'echo ssh-ready' >/dev/null 2>&1; then
  tail -n 120 "$QEMU_LOG" >&2 || true
  exit 1
fi

if ! remote 'nixome-observe-dom0' > "$OBSERVE_JSON"; then
  collect_diagnostics
  exit 1
fi

if ! remote 'systemctl is-active libvirtd qubesd qubes-db-dom0 qubes-qrexec-policy-daemon' > "$SERVICES_LOG"; then
  collect_diagnostics
  cat "$SERVICES_LOG" >&2 || true
  exit 1
fi

wait_for_setup_config_boot_unit
sync_dom0_tools

if [ "$BOOTSTRAP_QUBES" = "1" ]; then
  printf 'bootstrap: installing/checking Qubes VM kernel and template\n'
  copy_cached_bootstrap_assets_to_guest
  bootstrap_env="$(remote_env_prefix)"
  if ! remote "sudo -n $bootstrap_env NIXOME_LOG_DIR=/var/log/nixome nixome-qube-bootstrap" > "$BOOTSTRAP_STDOUT" 2> "$BOOTSTRAP_STDERR"; then
    collect_diagnostics
    cat "$BOOTSTRAP_STDERR" >&2 || true
    exit 1
  fi
  if ! extract_json_result "nixome.qubes-bootstrap.v1" "$BOOTSTRAP_STDOUT" "$BOOTSTRAP_JSON"; then
    collect_diagnostics
    printf 'bootstrap did not emit result JSON; see %s\n' "$BOOTSTRAP_STDOUT" >&2
    exit 1
  fi
else
  printf '{"schema":"nixome.qubes-bootstrap.v1","status":"skipped","reason":"NIXOME_BOOTSTRAP_QUBES=0"}\n' > "$BOOTSTRAP_JSON"
fi

if [ "$RUN_SETUP_CONFIG" = "1" ]; then
  printf 'setup-config: applying declared Qubes topology\n'
  setup_config_command="$(shell_quote "$SETUP_CONFIG_COMMAND")"
  if ! remote "sudo -n env NIXOME_LOG_DIR=/var/log/nixome $setup_config_command --apply" > "$SETUP_CONFIG_STDOUT" 2> "$SETUP_CONFIG_STDERR"; then
    remote 'sudo -n cat /var/log/nixome/setup-config.json 2>/dev/null || true' > "$SETUP_CONFIG_JSON" 2>/dev/null || true
    collect_diagnostics
    cat "$SETUP_CONFIG_STDERR" >&2 || true
    exit 1
  fi
  if ! extract_json_result "nixome.setup-config.v1" "$SETUP_CONFIG_STDOUT" "$SETUP_CONFIG_JSON"; then
    remote 'sudo -n cat /var/log/nixome/setup-config.json 2>/dev/null || true' > "$SETUP_CONFIG_JSON" 2>/dev/null || true
    if ! grep -q '"schema":"nixome.setup-config.v1"' "$SETUP_CONFIG_JSON"; then
      collect_diagnostics
      printf 'setup-config did not emit result JSON; see %s\n' "$SETUP_CONFIG_STDOUT" >&2
      exit 1
    fi
  fi
else
  if ! copy_boot_setup_config_result; then
    printf '{"schema":"nixome.setup-config.v1","status":"skipped","reason":"NIXOME_RUN_SETUP_CONFIG=0"}\n' > "$SETUP_CONFIG_JSON"
  fi
fi

if [ "$RUN_CONFIG_VMS_SMOKE" = "1" ]; then
  printf 'config-vms: booting declared NixOS config VMs and archiving a validated config candidate\n'
  config_vms_env="$(config_vms_env_prefix)"
  if ! remote "sudo -n $config_vms_env NIXOME_LOG_DIR=/var/log/nixome nixome-config-vms-smoke" > "$CONFIG_VMS_STDOUT" 2> "$CONFIG_VMS_STDERR"; then
    remote 'sudo -n cat /var/log/nixome/config-vms-smoke.json 2>/dev/null || true' > "$CONFIG_VMS_JSON" 2>/dev/null || true
    collect_diagnostics
    cat "$CONFIG_VMS_STDERR" >&2 || true
    exit 1
  fi
  if ! extract_json_result "nixome.config-vms-smoke.v1" "$CONFIG_VMS_STDOUT" "$CONFIG_VMS_JSON"; then
    remote 'sudo -n cat /var/log/nixome/config-vms-smoke.json 2>/dev/null || true' > "$CONFIG_VMS_JSON" 2>/dev/null || true
    if ! grep -q '"schema":"nixome.config-vms-smoke.v1"' "$CONFIG_VMS_JSON"; then
      collect_diagnostics
      printf 'config VMs smoke did not emit result JSON; see %s\n' "$CONFIG_VMS_STDOUT" >&2
      exit 1
    fi
  fi
else
  printf '{"schema":"nixome.config-vms-smoke.v1","status":"skipped","reason":"NIXOME_RUN_CONFIG_VMS_SMOKE=0"}\n' > "$CONFIG_VMS_JSON"
fi

if [ "$RUN_CONFIG_BOOT_VERIFY" = "1" ]; then
  printf 'config-boot: verifying dom0 booted into staged generation\n'
  config_boot_verify_env="$(config_boot_verify_env_prefix)"
  if ! remote "sudo -n $config_boot_verify_env NIXOME_LOG_DIR=/var/log/nixome nixome-config-boot-verify" > "$CONFIG_BOOT_VERIFY_STDOUT" 2> "$CONFIG_BOOT_VERIFY_STDERR"; then
    remote 'sudo -n cat /var/log/nixome/config-boot-verify.json 2>/dev/null || true' > "$CONFIG_BOOT_VERIFY_JSON" 2>/dev/null || true
    collect_diagnostics
    cat "$CONFIG_BOOT_VERIFY_STDERR" >&2 || true
    exit 1
  fi
  if ! extract_json_result "nixome.config-boot-verify.v1" "$CONFIG_BOOT_VERIFY_STDOUT" "$CONFIG_BOOT_VERIFY_JSON"; then
    remote 'sudo -n cat /var/log/nixome/config-boot-verify.json 2>/dev/null || true' > "$CONFIG_BOOT_VERIFY_JSON" 2>/dev/null || true
    if ! grep -q '"schema":"nixome.config-boot-verify.v1"' "$CONFIG_BOOT_VERIFY_JSON"; then
      collect_diagnostics
      printf 'config boot verify did not emit result JSON; see %s\n' "$CONFIG_BOOT_VERIFY_STDOUT" >&2
      exit 1
    fi
  fi
else
  printf '{"schema":"nixome.config-boot-verify.v1","status":"skipped","reason":"NIXOME_RUN_CONFIG_BOOT_VERIFY=0"}\n' > "$CONFIG_BOOT_VERIFY_JSON"
fi

if [ "$RUN_NIX_CLOSURE_SMOKE" = "1" ]; then
  printf 'nix-closure: running standalone closure-backed domain lifecycle test\n'
  nix_closure_env="$(nix_closure_env_prefix)"
  nix_closure_domain_smoke_command="$(shell_quote "$NIX_CLOSURE_DOMAIN_SMOKE_COMMAND")"
  if ! remote "sudo -n $nix_closure_env NIXOME_LOG_DIR=/var/log/nixome $nix_closure_domain_smoke_command" > "$NIX_CLOSURE_STDOUT" 2> "$NIX_CLOSURE_STDERR"; then
    remote 'sudo -n cat /var/log/nixome/nix-closure-domain-smoke.json 2>/dev/null || true' > "$NIX_CLOSURE_JSON" 2>/dev/null || true
    collect_diagnostics
    cat "$NIX_CLOSURE_STDERR" >&2 || true
    exit 1
  fi
  if ! extract_json_result "nixome.nix-closure-domain-smoke.v1" "$NIX_CLOSURE_STDOUT" "$NIX_CLOSURE_JSON"; then
    remote 'sudo -n cat /var/log/nixome/nix-closure-domain-smoke.json 2>/dev/null || true' > "$NIX_CLOSURE_JSON" 2>/dev/null || true
    if ! grep -q '"schema":"nixome.nix-closure-domain-smoke.v1"' "$NIX_CLOSURE_JSON"; then
      collect_diagnostics
      printf 'nix-closure domain smoke did not emit result JSON; see %s\n' "$NIX_CLOSURE_STDOUT" >&2
      exit 1
    fi
  fi
else
  printf '{"schema":"nixome.nix-closure-domain-smoke.v1","status":"skipped","reason":"NIXOME_RUN_NIX_CLOSURE_SMOKE=0"}\n' > "$NIX_CLOSURE_JSON"
fi

if [ "$RUN_QREXEC_SMOKE" = "1" ]; then
  printf 'qrexec: running uname smoke test\n'
  if ! remote "env NIXOME_LOG_DIR=/tmp/nixome-logs NIXOME_TEST_TEMPLATE=$(shell_quote "$TEMPLATE") NIXOME_TEST_QUBE=$(shell_quote "$QUBE") NIXOME_TEST_CLEANUP=0 nixome-qubes-smoke" > "$QREXEC_JSON" 2> "$QREXEC_STDERR"; then
    collect_diagnostics
    cat "$QREXEC_STDERR" >&2 || true
    exit 1
  fi
else
  printf '{"schema":"nixome.qubes-smoke.v1","status":"skipped","reason":"NIXOME_RUN_QUBES_SMOKE=0"}\n' > "$QREXEC_JSON"
fi

if [ "$RUN_GUI_SMOKE" = "1" ]; then
  printf 'gui: running AppVM window smoke test\n'
  if ! remote "env NIXOME_LOG_DIR=/tmp/nixome-logs NIXOME_TEST_QUBE=$(shell_quote "$QUBE") NIXOME_GUI_DISPLAY=:0 nixome-qube-gui-smoke" > "$GUI_JSON" 2> "$GUI_STDERR"; then
    collect_diagnostics
    cat "$GUI_STDERR" >&2 || true
    exit 1
  fi
else
  printf '{"schema":"nixome.qubes-gui-smoke.v1","status":"skipped","reason":"NIXOME_RUN_GUI_SMOKE=0"}\n' > "$GUI_JSON"
fi

remote 'qvm-ls || true' > "$QVM_LS_LOG" 2>/dev/null || true
collect_diagnostics
shutdown_vm_on_success

printf 'nested GUI smoke completed\n'
printf 'observe: %s\n' "$OBSERVE_JSON"
printf 'bootstrap: %s\n' "$BOOTSTRAP_JSON"
printf 'setup-config boot: %s\n' "$SETUP_CONFIG_BOOT_JSON"
printf 'setup-config: %s\n' "$SETUP_CONFIG_JSON"
printf 'config-vms: %s\n' "$CONFIG_VMS_JSON"
printf 'config-boot: %s\n' "$CONFIG_BOOT_VERIFY_JSON"
printf 'warm-sync: %s\n' "$SYNC_DOM0_TOOLS_JSON"
printf 'nix-closure: %s\n' "$NIX_CLOSURE_JSON"
printf 'qrexec: %s\n' "$QREXEC_JSON"
printf 'gui: %s\n' "$GUI_JSON"
