#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
RESULT_JSON="$LOG_DIR/qubes-bootstrap.json"
COMMAND_LOG="$LOG_DIR/qubes-bootstrap.commands.log"

TEMPLATE_NAME="${NIXOME_QUBES_TEMPLATE:-fedora-43-xfce}"
TEMPLATE_URL="${NIXOME_QUBES_TEMPLATE_URL:-https://ftp.qubes-os.org/repo/yum/r4.3/templates-itl/rpm/qubes-template-fedora-43-xfce-4.3.0-202601041226.noarch.rpm}"
TEMPLATE_RPM="${NIXOME_QUBES_TEMPLATE_RPM:-}"

KERNEL_NAME="${NIXOME_QUBES_KERNEL_NAME:-6.12.64-1.fc41}"
KERNEL_VERSION="${NIXOME_QUBES_KERNEL_VERSION:-6.12.64-1}"
KERNEL_FEDORA="${NIXOME_QUBES_KERNEL_FEDORA:-fc41}"
KERNEL_URL="${NIXOME_QUBES_KERNEL_URL:-https://ftp.qubes-os.org/repo/yum/r4.3/current/dom0/fc41/rpm/kernel-qubes-vm-6.12.64-1.qubes.fc41.x86_64.rpm}"
KERNEL_MODULES_URL="${NIXOME_QUBES_KERNEL_MODULES_URL:-https://ftp.qubes-os.org/repo/yum/r4.3/current/dom0/fc41/rpm/kernel-modules-6.12.64-1.qubes.fc41.x86_64.rpm}"
KERNEL_RPM="${NIXOME_QUBES_KERNEL_RPM:-}"
KERNEL_MODULES_RPM="${NIXOME_QUBES_KERNEL_MODULES_RPM:-}"

GPG_KEY_URL="${NIXOME_QUBES_GPG_KEY_URL:-https://keys.qubes-os.org/keys/qubes-release-4.3-signing-key.asc}"
GPG_KEY="${NIXOME_QUBES_GPG_KEY:-/var/lib/nixome/qubes-release-4.3-signing-key.asc}"
VERIFY_RPMS="${NIXOME_QUBES_VERIFY_RPMS:-1}"

DEFAULT_DVM="${NIXOME_QUBES_DEFAULT_DVM:-default-dvm}"
UPDATEVM="${NIXOME_QUBES_UPDATEVM:-$TEMPLATE_NAME}"

mkdir -p "$LOG_DIR"
: > "$COMMAND_LOG"

exec > >(tee -a "$COMMAND_LOG") 2>&1

json_escape() {
  printf '%s' "$1" | sed \
    -e 's/\\/\\\\/g' \
    -e 's/"/\\"/g' \
    -e ':a;N;$!ba;s/\n/\\n/g'
}

json_string() {
  printf '"'
  json_escape "$1"
  printf '"'
}

write_result() {
  status="$1"
  reason="$2"
  kernel_installed="$3"
  template_installed="$4"

  {
    printf '{'
    printf '"schema":"nixome.qubes-bootstrap.v1"'
    printf ',"status":'
    json_string "$status"
    printf ',"reason":'
    json_string "$reason"
    printf ',"template":'
    json_string "$TEMPLATE_NAME"
    printf ',"template_url":'
    json_string "$TEMPLATE_URL"
    printf ',"template_rpm":'
    json_string "$TEMPLATE_RPM"
    printf ',"template_installed":%s' "$template_installed"
    printf ',"kernel":'
    json_string "$KERNEL_NAME"
    printf ',"kernel_rpm":'
    json_string "$KERNEL_RPM"
    printf ',"kernel_modules_rpm":'
    json_string "$KERNEL_MODULES_RPM"
    printf ',"kernel_installed":%s' "$kernel_installed"
    printf ',"rpm_signature_verification":'
    if [ "$VERIFY_RPMS" = "1" ]; then
      printf true
    else
      printf false
    fi
    printf ',"default_dvm":'
    json_string "$DEFAULT_DVM"
    printf ',"updatevm":'
    json_string "$UPDATEVM"
    printf ',"command_log":'
    json_string "$COMMAND_LOG"
    printf '}\n'
  } > "$RESULT_JSON"
}

fail() {
  reason="$1"
  write_result failed "$reason" false false
  printf 'qubes bootstrap failed: %s\n' "$reason" >&2
  exit 1
}

require_root() {
  if [ "$(id -u)" != "0" ]; then
    fail "must run as root"
  fi
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing command: $1"
  fi
}

verify_rpm() {
  rpm_file="$1"

  if [ "$VERIFY_RPMS" != "1" ]; then
    return 0
  fi

  rpmdb_dir="$(mktemp -d)"
  rpmkeys --dbpath="$rpmdb_dir" --import "$GPG_KEY"
  rpmkeys --dbpath="$rpmdb_dir" \
    --define='_pkgverify_level all' \
    --define='_pkgverify_flags 0x0' \
    --checksig "$rpm_file"
}

service_active() {
  systemctl is-active --quiet "$1"
}

template_exists() {
  qvm-ls --raw-data --fields NAME --no-spinner 2>/dev/null | grep -Fx "$TEMPLATE_NAME" >/dev/null 2>&1
}

domain_exists() {
  qvm-ls --raw-data --fields NAME --no-spinner 2>/dev/null | grep -Fx "$1" >/dev/null 2>&1
}

require_root

for cmd in \
  curl \
  grep \
  nixos-qubes-install-kernel-rpm \
  nixos-qubes-install-kernel-url \
  nixos-qubes-install-template-rpm \
  nixos-qubes-install-template-url \
  qubes-prefs \
  qvm-create \
  qvm-ls \
  qvm-prefs \
  systemctl; do
  require_command "$cmd"
done

if [ "$VERIFY_RPMS" = "1" ] && {
  [ -n "$KERNEL_RPM" ] || [ -n "$KERNEL_MODULES_RPM" ] || [ -n "$TEMPLATE_RPM" ];
}; then
  require_command rpmkeys
fi

service_active qubes-db-dom0.service || fail "qubes-db-dom0.service is not active"
service_active qubesd.service || fail "qubesd.service is not active"
service_active qubes-qrexec-policy-daemon.service || fail "qubes-qrexec-policy-daemon.service is not active"

key_arg=()
if [ "$VERIFY_RPMS" = "1" ]; then
  install -d -m 0755 "$(dirname "$GPG_KEY")"
  if [ ! -s "$GPG_KEY" ]; then
    printf 'downloading Qubes RPM signing key: %s\n' "$GPG_KEY_URL"
    tmp_key="$GPG_KEY.tmp"
    curl --fail --location --silent --show-error -o "$tmp_key" "$GPG_KEY_URL"
    chmod 0644 "$tmp_key"
    mv "$tmp_key" "$GPG_KEY"
  fi
  key_arg=("$GPG_KEY")
else
  printf 'WARNING: RPM signature verification disabled by NIXOME_QUBES_VERIFY_RPMS=%s\n' "$VERIFY_RPMS" >&2
fi

if [ -n "$KERNEL_RPM" ] || [ -n "$KERNEL_MODULES_RPM" ]; then
  [ -n "$KERNEL_RPM" ] || fail "NIXOME_QUBES_KERNEL_MODULES_RPM is set but NIXOME_QUBES_KERNEL_RPM is empty"
  [ -n "$KERNEL_MODULES_RPM" ] || fail "NIXOME_QUBES_KERNEL_RPM is set but NIXOME_QUBES_KERNEL_MODULES_RPM is empty"
  [ -f "$KERNEL_RPM" ] || fail "kernel RPM is missing: $KERNEL_RPM"
  [ -f "$KERNEL_MODULES_RPM" ] || fail "kernel modules RPM is missing: $KERNEL_MODULES_RPM"
fi

if [ -n "$TEMPLATE_RPM" ]; then
  [ -f "$TEMPLATE_RPM" ] || fail "template RPM is missing: $TEMPLATE_RPM"
fi

kernel_installed=false
if [ -d "/var/lib/qubes/vm-kernels/$KERNEL_NAME" ]; then
  printf 'kernel already installed: %s\n' "$KERNEL_NAME"
else
  printf 'installing Qubes VM kernel: %s\n' "$KERNEL_NAME"
  if [ -n "$KERNEL_RPM" ]; then
    verify_rpm "$KERNEL_RPM"
    verify_rpm "$KERNEL_MODULES_RPM"
    nixos-qubes-install-kernel-rpm \
      "$KERNEL_NAME" \
      "$KERNEL_VERSION" \
      "$KERNEL_FEDORA" \
      "$KERNEL_RPM" \
      "$KERNEL_MODULES_RPM"
  else
    nixos-qubes-install-kernel-url \
      "$KERNEL_NAME" \
      "$KERNEL_VERSION" \
      "$KERNEL_FEDORA" \
      "$KERNEL_URL" \
      "$KERNEL_MODULES_URL" \
      "${key_arg[@]}"
  fi
  kernel_installed=true
fi

printf 'setting default_kernel: %s\n' "$KERNEL_NAME"
qubes-prefs default_kernel "$KERNEL_NAME"

template_installed=false
if template_exists || [ -d "/var/lib/qubes/vm-templates/$TEMPLATE_NAME" ]; then
  printf 'template already installed: %s\n' "$TEMPLATE_NAME"
else
  printf 'installing Qubes template: %s\n' "$TEMPLATE_NAME"
  if [ -n "$TEMPLATE_RPM" ]; then
    verify_rpm "$TEMPLATE_RPM"
    nixos-qubes-install-template-rpm "$TEMPLATE_NAME" "$TEMPLATE_RPM"
  else
    nixos-qubes-install-template-url \
      "$TEMPLATE_NAME" \
      "$TEMPLATE_URL" \
      "${key_arg[@]}"
  fi
  template_installed=true
fi

printf 'setting default_template: %s\n' "$TEMPLATE_NAME"
qubes-prefs default_template "$TEMPLATE_NAME"

if [ -n "$DEFAULT_DVM" ]; then
  if domain_exists "$DEFAULT_DVM"; then
    printf 'default disposable template already exists: %s\n' "$DEFAULT_DVM"
  else
    printf 'creating default disposable template: %s\n' "$DEFAULT_DVM"
    qvm-create --template "$TEMPLATE_NAME" --label red "$DEFAULT_DVM"
  fi
  qvm-prefs "$DEFAULT_DVM" template_for_dispvms True
  qubes-prefs default_dispvm "$DEFAULT_DVM"
fi

if [ -n "$UPDATEVM" ]; then
  printf 'setting updatevm: %s\n' "$UPDATEVM"
  qubes-prefs updatevm "$UPDATEVM"
fi

qvm-ls
write_result passed "Qubes kernel/template bootstrap succeeded" "$kernel_installed" "$template_installed"
cat "$RESULT_JSON"
