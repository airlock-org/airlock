#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
RESULT_JSON="$LOG_DIR/nix-closure-domain-smoke.json"
COMMAND_LOG="$LOG_DIR/nix-closure-domain-smoke.commands.log"
STAGING_UPDATE_SCRIPT="$LOG_DIR/nix-closure-domain-staging-update.sh"
STAGING_UPDATE_LOG="$LOG_DIR/nix-closure-domain-staging-update.log"
APPLY_LOG="$LOG_DIR/nix-closure-domain-apply.log"
APPLY_RESULT_JSON="$LOG_DIR/nix-closure-domain-config-apply.json"
QVM_LS_LOG="$LOG_DIR/nix-closure-domain-qvm-ls.log"
QVM_PREFS_LOG="$LOG_DIR/nix-closure-domain-qvm-prefs.log"
QVM_FEATURES_LOG="$LOG_DIR/nix-closure-domain-qvm-features.log"
QVM_VOLUMES_LOG="$LOG_DIR/nix-closure-domain-qvm-volumes.log"
ROOT_VOLUME_LOG="$LOG_DIR/nix-closure-domain-root-volume.log"
STORE_VOLUME_LOG="$LOG_DIR/nix-closure-domain-store-volume.log"
PRIVATE_VOLUME_LOG="$LOG_DIR/nix-closure-domain-private-volume.log"
VOLATILE_VOLUME_LOG="$LOG_DIR/nix-closure-domain-volatile-volume.log"
KERNEL_OPTS_LOG="$LOG_DIR/nix-closure-domain-kernelopts.log"
ROOT_IMAGE_CHECK_LOG="$LOG_DIR/nix-closure-domain-root-image-check.log"
STORE_IMAGE_CHECK_LOG="$LOG_DIR/nix-closure-domain-store-image-check.log"
RUNTIME_SERVICE_LOG="$LOG_DIR/nix-closure-domain-runtime-service.log"
RUNTIME_SERVICE_ERR="$LOG_DIR/nix-closure-domain-runtime-service.err.log"
GUEST_PROBE_SCRIPT="$LOG_DIR/nix-closure-domain-guest-probe.sh"
GUEST_PROBE_LOG="$LOG_DIR/nix-closure-domain-guest-probe.log"
GUEST_PROBE_AFTER_REBOOT_LOG="$LOG_DIR/nix-closure-domain-guest-probe-after-reboot.log"
GUEST_PROBE_AFTER_RESTART_LOG="$LOG_DIR/nix-closure-domain-guest-probe-after-restart.log"
GUEST_PERSISTENCE_LOG="$LOG_DIR/nix-closure-domain-persistence.log"
REBOOT_LOG="$LOG_DIR/nix-closure-domain-reboot.log"
SHUTDOWN_LOG="$LOG_DIR/nix-closure-domain-shutdown.log"
RESTART_LOG="$LOG_DIR/nix-closure-domain-restart.log"
GUI_LOG="$LOG_DIR/nix-closure-domain-gui.log"
CONSOLE_LOG="$LOG_DIR/nix-closure-domain-console.log"
JOURNAL_LOG="$LOG_DIR/nix-closure-domain-journal.log"
LIBXL_LOG="$LOG_DIR/nix-closure-domain-libxl.log"
APPLIED_TOPOLOGY_LOG="$LOG_DIR/nix-closure-domain-applied-topology.json"
MATERIALIZATIONS_LOG="$LOG_DIR/nix-closure-domain-materializations.json"
BUILDS_LOG="$LOG_DIR/nix-closure-domain-builds.json"
SETUP_LOG="$LOG_DIR/nix-closure-domain-setup.json"
POLICY_LOG="$LOG_DIR/nix-closure-domain-policy.log"
QUBES_XML_LOG="$LOG_DIR/nix-closure-domain-qubes.xml"

DOMAIN="${NIXOME_NIX_CLOSURE_DOMAIN:-nixome-closure-smoke}"
STAGING="${NIXOME_CONFIG_STAGING_VM:-config-staging}"
VALIDATOR="${NIXOME_CONFIG_VALIDATOR_VM:-config-validator}"
VAULT="${NIXOME_CONFIG_VAULT_VM:-config-vault}"
QUBE_USER="${NIXOME_QUBE_USER:-user}"
APPLY_MESSAGE="${NIXOME_NIX_CLOSURE_APPLY_MESSAGE:-nix-closure-smoke-$(date -u +%Y%m%dT%H%M%SZ)}"
XEN_CAPABILITIES="${NIXOME_XEN_CAPABILITIES:-/proc/xen/capabilities}"
WAIT_SECONDS="${NIXOME_NIX_CLOSURE_WAIT_SECONDS:-240}"
QREXEC_TIMEOUT_SECONDS="${NIXOME_NIX_CLOSURE_QREXEC_TIMEOUT_SECONDS:-45}"
RUN_GUI_PROBE="${NIXOME_NIX_CLOSURE_GUI_PROBE:-0}"
CONFIG_APPLY_NIX_OFFLINE="${NIXOME_CONFIG_APPLY_NIX_OFFLINE:-0}"

result_written=0
root_source=""
root_source_physical=""
store_source=""
store_source_physical=""
store_image=""
store_image_physical=""
store_mode=""
system_source=""
store_device=""
reboot_status="not-run"
shutdown_start_status="not-run"
gui_status="skipped"

mkdir -p "$LOG_DIR"
: > "$COMMAND_LOG"
exec 3>&1
exec >> "$COMMAND_LOG" 2>&1
trap write_unexpected_failure EXIT

json_file_string_or_null() {
  file="$1"

  if [ -r "$file" ]; then
    jq -Rs . < "$file" | tr -d '\n'
  else
    printf 'null'
  fi
}

json_file_value_or_null() {
  file="$1"

  if [ -r "$file" ] && jq -e . "$file" >/dev/null 2>&1; then
    jq -c . "$file" | tr -d '\n'
  else
    printf 'null'
  fi
}

write_result() {
  status="$1"
  reason="$2"

  {
    printf '{'
    printf '"schema":"nixome.nix-closure-domain-smoke.v1"'
    printf ',"status":'
    jq -Rn --arg value "$status" '$value' | tr -d '\n'
    printf ',"reason":'
    jq -Rn --arg value "$reason" '$value' | tr -d '\n'
    printf ',"domain":'
    jq -Rn --arg value "$DOMAIN" '$value' | tr -d '\n'
    printf ',"staging":'
    jq -Rn --arg value "$STAGING" '$value' | tr -d '\n'
    printf ',"validator":'
    jq -Rn --arg value "$VALIDATOR" '$value' | tr -d '\n'
    printf ',"vault":'
    jq -Rn --arg value "$VAULT" '$value' | tr -d '\n'
    printf ',"apply_message":'
    jq -Rn --arg value "$APPLY_MESSAGE" '$value' | tr -d '\n'
    printf ',"system_source":'
    jq -Rn --arg value "$system_source" '$value' | tr -d '\n'
    printf ',"root_source":'
    jq -Rn --arg value "$root_source" '$value' | tr -d '\n'
    printf ',"root_source_physical":'
    jq -Rn --arg value "$root_source_physical" '$value' | tr -d '\n'
    printf ',"store_source":'
    jq -Rn --arg value "$store_source" '$value' | tr -d '\n'
    printf ',"store_source_physical":'
    jq -Rn --arg value "$store_source_physical" '$value' | tr -d '\n'
    printf ',"store_image":'
    jq -Rn --arg value "$store_image" '$value' | tr -d '\n'
    printf ',"store_image_physical":'
    jq -Rn --arg value "$store_image_physical" '$value' | tr -d '\n'
    printf ',"store_mode":'
    jq -Rn --arg value "$store_mode" '$value' | tr -d '\n'
    printf ',"store_device":'
    jq -Rn --arg value "$store_device" '$value' | tr -d '\n'
    printf ',"reboot_status":'
    jq -Rn --arg value "$reboot_status" '$value' | tr -d '\n'
    printf ',"shutdown_start_status":'
    jq -Rn --arg value "$shutdown_start_status" '$value' | tr -d '\n'
    printf ',"gui_status":'
    jq -Rn --arg value "$gui_status" '$value' | tr -d '\n'
    printf ',"logs":{'
    printf '"command":'
    jq -Rn --arg value "$COMMAND_LOG" '$value' | tr -d '\n'
    printf ',"staging_update":'
    jq -Rn --arg value "$STAGING_UPDATE_LOG" '$value' | tr -d '\n'
    printf ',"apply":'
    jq -Rn --arg value "$APPLY_LOG" '$value' | tr -d '\n'
    printf ',"apply_result":'
    jq -Rn --arg value "$APPLY_RESULT_JSON" '$value' | tr -d '\n'
    printf ',"qvm_ls":'
    jq -Rn --arg value "$QVM_LS_LOG" '$value' | tr -d '\n'
    printf ',"qvm_prefs":'
    jq -Rn --arg value "$QVM_PREFS_LOG" '$value' | tr -d '\n'
    printf ',"qvm_features":'
    jq -Rn --arg value "$QVM_FEATURES_LOG" '$value' | tr -d '\n'
    printf ',"qvm_volumes":'
    jq -Rn --arg value "$QVM_VOLUMES_LOG" '$value' | tr -d '\n'
    printf ',"root_volume":'
    jq -Rn --arg value "$ROOT_VOLUME_LOG" '$value' | tr -d '\n'
    printf ',"store_volume":'
    jq -Rn --arg value "$STORE_VOLUME_LOG" '$value' | tr -d '\n'
    printf ',"kernelopts":'
    jq -Rn --arg value "$KERNEL_OPTS_LOG" '$value' | tr -d '\n'
    printf ',"root_image_check":'
    jq -Rn --arg value "$ROOT_IMAGE_CHECK_LOG" '$value' | tr -d '\n'
    printf ',"store_image_check":'
    jq -Rn --arg value "$STORE_IMAGE_CHECK_LOG" '$value' | tr -d '\n'
    printf ',"runtime_service":'
    jq -Rn --arg value "$RUNTIME_SERVICE_LOG" '$value' | tr -d '\n'
    printf ',"guest_probe":'
    jq -Rn --arg value "$GUEST_PROBE_LOG" '$value' | tr -d '\n'
    printf ',"guest_probe_after_reboot":'
    jq -Rn --arg value "$GUEST_PROBE_AFTER_REBOOT_LOG" '$value' | tr -d '\n'
    printf ',"guest_probe_after_restart":'
    jq -Rn --arg value "$GUEST_PROBE_AFTER_RESTART_LOG" '$value' | tr -d '\n'
    printf ',"persistence":'
    jq -Rn --arg value "$GUEST_PERSISTENCE_LOG" '$value' | tr -d '\n'
    printf ',"console":'
    jq -Rn --arg value "$CONSOLE_LOG" '$value' | tr -d '\n'
    printf ',"journal":'
    jq -Rn --arg value "$JOURNAL_LOG" '$value' | tr -d '\n'
    printf '}'
    printf ',"apply_result":'
    json_file_value_or_null "$APPLY_RESULT_JSON"
    printf ',"setup_result":'
    json_file_value_or_null "$SETUP_LOG"
    printf ',"nix_domain_builds":'
    json_file_value_or_null "$BUILDS_LOG"
    printf ',"nix_domain_materializations":'
    json_file_value_or_null "$MATERIALIZATIONS_LOG"
    printf ',"runtime_service_output":'
    json_file_string_or_null "$RUNTIME_SERVICE_LOG"
    printf ',"guest_probe_output":'
    json_file_string_or_null "$GUEST_PROBE_LOG"
    printf '}'
    printf '\n'
  } > "$RESULT_JSON"
  result_written=1
}

collect_diagnostics() {
  qvm-ls --raw-data --fields NAME,STATE,CLASS,LABEL,TEMPLATE > "$QVM_LS_LOG" 2>&1 || true
  cp /var/lib/qubes/qubes.xml "$QUBES_XML_LOG" 2>/dev/null || true
  if qvm-check "$DOMAIN" >/dev/null 2>&1; then
    qvm-prefs "$DOMAIN" > "$QVM_PREFS_LOG" 2>&1 || true
    qvm-features "$DOMAIN" > "$QVM_FEATURES_LOG" 2>&1 || true
    qvm-volume list "$DOMAIN" > "$QVM_VOLUMES_LOG" 2>&1 || true
    qvm-volume info "$DOMAIN:root" > "$ROOT_VOLUME_LOG" 2>&1 || true
    qvm-volume info "$DOMAIN:private" > "$PRIVATE_VOLUME_LOG" 2>&1 || true
    qvm-volume info "$DOMAIN:volatile" > "$VOLATILE_VOLUME_LOG" 2>&1 || true
    qvm-volume info "$DOMAIN:store" > "$STORE_VOLUME_LOG" 2>&1 || true
    qvm-prefs "$DOMAIN" kernelopts > "$KERNEL_OPTS_LOG" 2>&1 || true
  fi
  tail -n 1000 "/var/log/xen/console/guest-$DOMAIN.log" > "$CONSOLE_LOG" 2>&1 || true
  journalctl -b --no-pager -n 320 \
    -u qubesd \
    -u libvirtd \
    -u qubes-db-dom0 \
    -u qubes-qrexec-policy-daemon \
    > "$JOURNAL_LOG" 2>&1 || true
  tail -n 320 /var/log/libvirt/libxl/libxl-driver.log > "$LIBXL_LOG" 2>&1 || true
  cat /var/log/nixome/config-apply-topology.json > "$APPLIED_TOPOLOGY_LOG" 2>/dev/null || true
  cat /var/log/nixome/config-apply.json > "$APPLY_RESULT_JSON" 2>/dev/null || true
  cat /var/log/nixome/config-apply-nix-domain-materializations.json > "$MATERIALIZATIONS_LOG" 2>/dev/null || true
  cat /var/log/nixome/config-apply-nix-domain-builds.json > "$BUILDS_LOG" 2>/dev/null || true
  cat /var/log/nixome/setup-config.json > "$SETUP_LOG" 2>/dev/null || true
  cat /var/lib/nixome/qubes-policy.d/90-nixome.policy > "$POLICY_LOG" 2>/dev/null || true
}

fail() {
  reason="$1"
  collect_diagnostics
  write_result failed "$reason"
  printf 'nix-closure domain smoke failed: %s\n' "$reason" >&2
  cat "$RESULT_JSON" >&3
  exit 1
}

write_unexpected_failure() {
  rc="$?"

  if [ "$rc" != "0" ] && [ "$result_written" != "1" ]; then
    collect_diagnostics
    write_result failed "unexpected command failure; see command_log"
    cat "$RESULT_JSON" >&3
  fi
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing command: $1"
  fi
}

run_or_fail() {
  description="$1"
  output="$2"
  shift 2

  printf '+'
  printf ' %q' "$@"
  printf '\n'

  if ! "$@" > "$output" 2>&1; then
    cat "$output" >&2 || true
    fail "$description failed"
  fi
}

run_qrexec_script_or_fail() {
  vm="$1"
  description="$2"
  output="$3"
  script="$4"
  qvm_run_args=(qvm-run --pass-io)

  if qvm-run --help 2>&1 | grep -q -- '--no-gui'; then
    qvm_run_args+=(--no-gui)
  fi

  printf '+'
  printf ' %q' "${qvm_run_args[@]}" "$vm" "bash -s"
  printf ' < %q\n' "$script"

  if ! "${qvm_run_args[@]}" "$vm" "bash -s" < "$script" > "$output" 2>&1; then
    cat "$output" >&2 || true
    fail "$description failed"
  fi
}

qvm_run_probe() {
  vm="$1"
  output="$2"
  command="$3"
  qvm_run_args=(qvm-run --pass-io)

  if qvm-run --help 2>&1 | grep -q -- '--no-gui'; then
    qvm_run_args+=(--no-gui)
  fi

  timeout "$QREXEC_TIMEOUT_SECONDS" "${qvm_run_args[@]}" "$vm" "$command" > "$output" 2>&1
}

qvm_volume_list_has_volume() {
  vm="$1"
  volume="$2"

  qvm-volume list "$vm" |
    awk -v volume="$volume" '
      NR == 1 { next }
      $3 == volume || $1 ~ "/" volume "$" { found = 1 }
      END { exit found ? 0 : 1 }
    '
}

wait_for_qrexec() {
  vm="$1"
  label="$2"
  output="$3"
  deadline=$((SECONDS + WAIT_SECONDS))

  while [ "$SECONDS" -lt "$deadline" ]; do
    if qvm_run_probe "$vm" "$output" "printf qrexec-ready"; then
      grep -q qrexec-ready "$output" && return 0
    fi
    sleep 3
  done

  cat "$output" >&2 || true
  fail "qrexec did not become ready for $label within ${WAIT_SECONDS}s"
}

start_vm() {
  vm="$1"

  if qvm-check --running "$vm" >/dev/null 2>&1; then
    printf 'VM already running: %s\n' "$vm"
  else
    run_or_fail "qvm-start $vm" "$LOG_DIR/nix-closure-domain-$vm-start.log" qvm-start "$vm"
  fi
}

shutdown_vm() {
  vm="$1"
  output="$2"

  if ! qvm-check --running "$vm" >/dev/null 2>&1; then
    printf 'VM already halted: %s\n' "$vm" > "$output"
    return 0
  fi

  run_or_fail "qvm-shutdown $vm" "$output" qvm-shutdown --wait "$vm"
}

nixome_store_uses_chroot() {
  [ "${NIXOME_USE_HOST_NIX_STORE:-0}" != "1" ] &&
    [ -n "${NIXOME_NIX_ROOT:-}" ] &&
    [ "$NIXOME_NIX_ROOT" != "/" ]
}

nixome_physical_store_path() {
  path="$1"

  if nixome_store_uses_chroot && [ "${path#/nix/store/}" != "$path" ]; then
    printf '%s%s\n' "$NIXOME_NIX_ROOT" "$path"
  else
    printf '%s\n' "$path"
  fi
}

extract_kernelopt() {
  key="$1"
  tr ' ' '\n' < "$KERNEL_OPTS_LOG" |
    sed -n "s|^$key=||p" |
    head -n 1
}

feature_value() {
  qvm-features "$DOMAIN" "$1" 2>/dev/null | tr -d '\n'
}

check_source_paths() {
  root_source="$(feature_value nixome-root-source)"
  store_source="$(feature_value nixome-store-source)"
  store_mode="$(feature_value nixome-store-mode)"
  store_view_backend="$(feature_value nixome-store-view-backend)"
  store_filesystem="$(feature_value nixome-store-filesystem)"
  store_blob_device="$(feature_value nixome-store-blob-device)"
  store_blob_map="$(feature_value nixome-store-blob-map)"
  [ -n "$store_mode" ] || store_mode="closure-image"
  [ -n "$store_view_backend" ] || store_view_backend="ext4-image"
  [ -n "$store_filesystem" ] || store_filesystem="ext4"
  if [ "$store_mode" = "shared-closure" ]; then
    store_image="$(feature_value nixome-store-view-image)"
  else
    store_image="$(feature_value nixome-store-image)"
  fi
  system_source="$(feature_value nixome-system-source)"
  store_device="$(feature_value nixome-store-device)"
  root_source_physical="$(nixome_physical_store_path "$root_source")"
  store_source_physical="$(nixome_physical_store_path "$store_source")"
  store_image_physical="$(nixome_physical_store_path "$store_image")"
  store_blob_map_physical="$(nixome_physical_store_path "$store_blob_map")"

  [ -n "$root_source" ] || fail "missing nixome-root-source feature"
  [ -n "$store_source" ] || fail "missing nixome-store-source feature"
  [ -n "$store_image" ] || fail "missing nixome-store-image feature"
  [ -n "$system_source" ] || fail "missing nixome-system-source feature"
  [ -n "$store_device" ] || fail "missing nixome-store-device feature"
  [ -f "$root_source_physical" ] || fail "root image source is missing: $root_source"
  case "$store_mode" in
    closure-image)
      [ -f "$store_source_physical" ] || fail "store source image is missing: $store_source"
      [ -f "$store_image_physical" ] || fail "store imported image path is missing: $store_image"
      [ "$store_source_physical" = "$store_image_physical" ] ||
        fail "store source and imported image path disagree"
      ;;
    shared-closure)
      [ -d "$store_source_physical" ] || fail "store closure source is missing: $store_source"
      [ -f "$store_source_physical/store-paths" ] ||
        fail "store closure source is missing store-paths: $store_source/store-paths"
      [ -f "$store_source_physical/manifest.json" ] ||
        fail "store closure source is missing manifest.json: $store_source/manifest.json"
      [ -f "$store_image_physical" ] || fail "store view image path is missing: $store_image"
      if [ "$store_view_backend" = "erofs-blobmap" ] || [ "$store_view_backend" = "erofs-objectmap" ]; then
        [ "$store_filesystem" = "erofs" ] ||
          fail "shared closure EROFS filtered-blob backend did not report erofs filesystem"
        [ -n "$store_blob_device" ] || fail "missing nixome-store-blob-device feature"
        [ -n "$store_blob_map" ] || fail "missing nixome-store-blob-map feature"
        [ -f "$store_blob_map_physical" ] || fail "store blob map path is missing: $store_blob_map"
      fi
      ;;
    *)
      fail "unsupported store mode for $DOMAIN: $store_mode"
      ;;
  esac
}

check_root_image_has_no_store() {
  {
    printf 'root_source=%s\n' "$root_source"
    printf 'root_source_physical=%s\n' "$root_source_physical"
    printf 'bytes=%s\n' "$(stat -c %s "$root_source_physical")"
    debugfs -R 'ls -p /nix/store' "$root_source_physical"
  } > "$ROOT_IMAGE_CHECK_LOG" 2>&1

  entries="$(
    debugfs -R 'ls -p /nix/store' "$root_source_physical" 2>/dev/null |
      awk -F/ 'NF >= 6 && $6 != "." && $6 != ".." { print $6 }'
  )"

  [ -z "$entries" ] ||
    fail "root image contains unexpected /nix/store entries: $entries"
}

reconstruct_blob_view() {
  map="$1"
  output="$2"
  python3 - "$map" "$output" <<'PY'
import json
import sys
from pathlib import Path

blob_map = json.loads(Path(sys.argv[1]).read_text())
output = Path(sys.argv[2])
with Path(blob_map["central_blob"]).open("rb") as source, output.open("wb") as target:
    for extent in blob_map["extents"]:
        source.seek(int(extent["central_offset"]))
        data = source.read(int(extent["length"]))
        if len(data) != int(extent["length"]):
            raise SystemExit("central blob extent is truncated")
        target.seek(int(extent["logical_offset"]))
        target.write(data)
    target.truncate(int(blob_map["logical_size"]))
PY
}

check_store_image_contains_system() {
  system_entry="${system_source#/nix/store/}"
  [ "$system_entry" != "$system_source" ] ||
    fail "system source is not in /nix/store: $system_source"

  {
    printf 'store_source=%s\n' "$store_source"
    printf 'store_source_physical=%s\n' "$store_source_physical"
    printf 'store_image=%s\n' "$store_image"
    printf 'store_image_physical=%s\n' "$store_image_physical"
    printf 'system_source=%s\n' "$system_source"
    printf 'system_entry=%s\n' "$system_entry"
    printf 'bytes=%s\n' "$(stat -c %s "$store_image_physical")"
    if [ -f "$store_source_physical/store-paths" ]; then
      grep -Fx "$system_source" "$store_source_physical/store-paths"
    elif [ -f "$(dirname "$store_source_physical")/store-paths" ]; then
      grep -Fx "$system_source" "$(dirname "$store_source_physical")/store-paths"
    fi
    if [ "$store_view_backend" = "erofs-blobmap" ] || [ "$store_view_backend" = "erofs-objectmap" ]; then
      logical_blob="$LOG_DIR/nix-closure-domain-store.blob"
      extract_dir="$LOG_DIR/nix-closure-domain-store-extract"
      chmod -R u+w "$extract_dir" 2>/dev/null || true
      rm -rf "$extract_dir"
      mkdir -p "$extract_dir"
      reconstruct_blob_view "$store_blob_map_physical" "$logical_blob"
      fsck.erofs --device="$logical_blob" --extract="$extract_dir" "$store_image_physical"
      [ -x "$extract_dir/$system_entry/init" ]
    else
      debugfs -R "stat /$system_entry/init" "$store_image_physical"
    fi
  } > "$STORE_IMAGE_CHECK_LOG" 2>&1 ||
    fail "store image does not contain declared system init"
}

run_runtime_service_check() {
  qrexec_args=(qrexec-client -d "$DOMAIN" -T -w 120 "DEFAULT:QUBESRPC nixome.NixStoreRuntimeCheck+ dom0")
  printf '+'
  printf ' %q' "${qrexec_args[@]}"
  printf '\n'

  if ! "${qrexec_args[@]}" > "$RUNTIME_SERVICE_LOG" 2> "$RUNTIME_SERVICE_ERR"; then
    cat "$RUNTIME_SERVICE_ERR" >&2 || true
    fail "nixome.NixStoreRuntimeCheck qrexec service failed"
  fi

  grep -Fx "system=$system_source" "$RUNTIME_SERVICE_LOG" >/dev/null ||
    fail "runtime service did not report expected system source"
  grep -Fx "store_device=$store_device" "$RUNTIME_SERVICE_LOG" >/dev/null ||
    fail "runtime service did not report expected store device"
  grep -Fx "store_format=$store_filesystem" "$RUNTIME_SERVICE_LOG" >/dev/null ||
    fail "runtime service did not report expected store filesystem"
  if [ "$store_filesystem" = "erofs" ]; then
    grep -Fx "store_blob_device=$store_blob_device" "$RUNTIME_SERVICE_LOG" >/dev/null ||
      fail "runtime service did not report expected store blob device"
  fi
  grep -E '^store_options=([^,]+,)*ro(,|$)' "$RUNTIME_SERVICE_LOG" >/dev/null ||
    fail "runtime service did not report read-only store mount"
}

write_guest_probe_script() {
  cat > "$GUEST_PROBE_SCRIPT" <<'EOF'
set -eu

system_path="$(
  tr ' ' '\n' < /proc/cmdline |
    sed -n 's|^nixome.system=||p' |
    head -n 1
)"
store_device="$(
  tr ' ' '\n' < /proc/cmdline |
    sed -n 's|^nixome.storedev=||p' |
    head -n 1
)"
store_format="$(
  tr ' ' '\n' < /proc/cmdline |
    sed -n 's|^nixome.storefmt=||p' |
    head -n 1
)"
store_blob_device="$(
  tr ' ' '\n' < /proc/cmdline |
    sed -n 's|^nixome.storeblobdev=||p' |
    head -n 1
)"
[ -n "$store_format" ] || store_format="ext4"
mount_source="$(findmnt -rn -T /nix/store -o SOURCE)"
mount_options="$(findmnt -rn -T /nix/store -o OPTIONS)"
current_system="$(readlink -f /run/current-system)"
guest_user="$(id -un)"
home_dir="${HOME:-/home/$guest_user}"

printf 'cmdline=%s\n' "$(cat /proc/cmdline)"
printf 'system_path=%s\n' "$system_path"
printf 'store_device=%s\n' "$store_device"
printf 'store_format=%s\n' "$store_format"
printf 'store_blob_device=%s\n' "$store_blob_device"
printf 'mount_source=%s\n' "$mount_source"
printf 'mount_options=%s\n' "$mount_options"
printf 'current_system=%s\n' "$current_system"
printf 'guest_user=%s\n' "$guest_user"
printf 'home_dir=%s\n' "$home_dir"
printf 'system_state=%s\n' "$(systemctl is-system-running 2>/dev/null || true)"
printf 'qubes_name=%s\n' "$(qubesdb-read /name 2>/dev/null || true)"
printf 'domain_role=%s\n' "$(cat /etc/nixome/domain-role)"
printf 'applied_message=%s\n' "$(cat /etc/nixome/applied-message)"

[ -n "$system_path" ]
[ -n "$store_device" ]
[ "$current_system" = "$system_path" ]
[ -x "$system_path/init" ]
[ "$mount_source" = "$store_device" ]
if [ "$store_format" = "erofs" ]; then
  [ -n "$store_blob_device" ]
  [ -b "$store_blob_device" ]
fi
case ",$mount_options," in
  *,ro,*) ;;
  *) echo "/nix/store is not read-only: $mount_options" >&2; exit 1 ;;
esac

printf '\n== findmnt ==\n'
for mount_path in / /nix/store /rw /home /usr/local /run /var "$home_dir"; do
  printf '%s ' "$mount_path"
  findmnt -rn -T "$mount_path" -o TARGET,SOURCE,OPTIONS || true
done
printf '\n== lsblk ==\n'
lsblk -o NAME,RO,SIZE,FSTYPE,MOUNTPOINTS
printf '\n== services ==\n'
systemctl is-active qubes-db.service
systemctl is-active qubes-qrexec-agent.service
systemctl is-active nixome-qubes-rw-init.service

case "$(cat /etc/nixome/domain-role)" in
  nix-closure-smoke) ;;
  *) echo "unexpected domain role" >&2; exit 1 ;;
esac

[ -d /rw ]
[ -d /rw/home ]
[ -d /usr/local ]
[ -d "$home_dir" ]
[ -w "$home_dir" ]
touch "$home_dir/nixome-closure-smoke-home"
test -d /run/qubes
test -d /var/log/qubes

printf 'runtime_probe=passed\n'
EOF
}

run_guest_probe() {
  output="$1"
  run_qrexec_script_or_fail "$DOMAIN" "runtime guest probe in $DOMAIN" "$output" "$GUEST_PROBE_SCRIPT"
  grep -Fx "system_path=$system_source" "$output" >/dev/null ||
    fail "guest probe did not report expected system path"
  grep -Fx "store_device=$store_device" "$output" >/dev/null ||
    fail "guest probe did not report expected store device"
  grep -Fx "mount_source=$store_device" "$output" >/dev/null ||
    fail "guest probe did not mount store from expected device"
  grep -Fx "runtime_probe=passed" "$output" >/dev/null ||
    fail "guest probe did not complete"
}

run_persistence_probe() {
  cat > "$GUEST_PROBE_SCRIPT.persistence" <<'EOF'
set -eu
guest_user="$(id -un)"
home_dir="${HOME:-/home/$guest_user}"
test -d /rw
test -d /rw/home
test -d /usr/local
test -d "$home_dir"
test -w "$home_dir"
test -f "$home_dir/nixome-closure-smoke-home"
findmnt -rn -T /nix/store -o OPTIONS | grep -E '(^|,)ro(,|$)'
printf 'persistence_probe=passed\n'
EOF
  run_qrexec_script_or_fail "$DOMAIN" "persistent state probe in $DOMAIN" "$GUEST_PERSISTENCE_LOG" "$GUEST_PROBE_SCRIPT.persistence"
  grep -Fx "persistence_probe=passed" "$GUEST_PERSISTENCE_LOG" >/dev/null ||
    fail "persistent state probe did not complete"
}

run_reboot_cycle() {
  start_vm "$DOMAIN"
  wait_for_qrexec "$DOMAIN" "$DOMAIN before reboot" "$LOG_DIR/nix-closure-domain-qrexec-before-reboot.log"

  qvm_run_args=(qvm-run --pass-io)
  if qvm-run --help 2>&1 | grep -q -- '--no-gui'; then
    qvm_run_args+=(--no-gui)
  fi

  {
    printf '+'
    printf ' %q' "${qvm_run_args[@]}" "$DOMAIN" "sh -lc 'echo reboot-requested; sleep 1; systemctl reboot'"
    printf '\n'
    "${qvm_run_args[@]}" "$DOMAIN" "sh -lc 'echo reboot-requested; sleep 1; systemctl reboot'"
  } > "$REBOOT_LOG" 2>&1 || true

  sleep 8
  if ! qvm-check --running "$DOMAIN" >/dev/null 2>&1; then
    printf 'domain stopped after guest reboot request; starting it again\n' >> "$REBOOT_LOG"
    qvm-start "$DOMAIN" >> "$REBOOT_LOG" 2>&1
  fi

  wait_for_qrexec "$DOMAIN" "$DOMAIN after reboot" "$LOG_DIR/nix-closure-domain-qrexec-after-reboot.log"
  run_runtime_service_check
  run_guest_probe "$GUEST_PROBE_AFTER_REBOOT_LOG"
  reboot_status="passed"
}

run_shutdown_start_cycle() {
  shutdown_vm "$DOMAIN" "$SHUTDOWN_LOG"
  start_vm "$DOMAIN"
  wait_for_qrexec "$DOMAIN" "$DOMAIN after shutdown/start" "$RESTART_LOG"
  run_runtime_service_check
  run_guest_probe "$GUEST_PROBE_AFTER_RESTART_LOG"
  run_persistence_probe
  shutdown_start_status="passed"
}

run_optional_gui_probe() {
  case "$RUN_GUI_PROBE" in
    1|true|yes)
      ;;
    0|false|no)
      gui_status="skipped"
      printf 'GUI probe skipped by NIXOME_NIX_CLOSURE_GUI_PROBE=%s\n' "$RUN_GUI_PROBE" > "$GUI_LOG"
      return 0
      ;;
    *)
      fail "unsupported NIXOME_NIX_CLOSURE_GUI_PROBE value: $RUN_GUI_PROBE"
      ;;
  esac

  if ! command -v nixome-qube-gui-smoke >/dev/null 2>&1; then
    gui_status="skipped"
    printf 'nixome-qube-gui-smoke is not available\n' > "$GUI_LOG"
    return 0
  fi

  if NIXOME_TEST_QUBE="$DOMAIN" nixome-qube-gui-smoke > "$GUI_LOG" 2>&1; then
    gui_status="passed"
  else
    gui_status="failed"
    cat "$GUI_LOG" >&2 || true
    fail "GUI probe failed for $DOMAIN"
  fi
}

for cmd in \
  awk \
  cat \
  date \
  debugfs \
  findmnt \
  grep \
  jq \
  journalctl \
  nixome-config-apply \
  qrexec-client \
  qvm-check \
  qvm-features \
  qvm-ls \
  qvm-prefs \
  qvm-run \
  qvm-shutdown \
  qvm-start \
  qvm-volume \
  readlink \
  sed \
  stat \
  systemctl \
  tail \
  timeout \
  tr; do
  require_command "$cmd"
done

case "$DOMAIN" in
  ""|*[!A-Za-z0-9_.-]*)
    fail "invalid Nix closure test domain name: $DOMAIN"
    ;;
esac

if ! [ -r "$XEN_CAPABILITIES" ] || ! grep -qw control_d "$XEN_CAPABILITIES"; then
  fail "host is not a Xen control domain"
fi

systemctl is-active --quiet qubes-db-dom0.service || fail "qubes-db-dom0.service is not active"
systemctl is-active --quiet qubesd.service || fail "qubesd.service is not active"
systemctl is-active --quiet qubes-qrexec-policy-daemon.service ||
  fail "qubes-qrexec-policy-daemon.service is not active"

qvm-check "$STAGING" >/dev/null 2>&1 || fail "missing config staging VM: $STAGING"
qvm-check "$VALIDATOR" >/dev/null 2>&1 || fail "missing config validator VM: $VALIDATOR"
qvm-check "$VAULT" >/dev/null 2>&1 || fail "missing config vault VM: $VAULT"

start_vm "$STAGING"
start_vm "$VALIDATOR"
start_vm "$VAULT"
wait_for_qrexec "$STAGING" "$STAGING" "$LOG_DIR/nix-closure-domain-$STAGING-qrexec.log"
wait_for_qrexec "$VALIDATOR" "$VALIDATOR" "$LOG_DIR/nix-closure-domain-$VALIDATOR-qrexec.log"
wait_for_qrexec "$VAULT" "$VAULT" "$LOG_DIR/nix-closure-domain-$VAULT-qrexec.log"

run_qrexec_script_or_fail "$STAGING" "initialize $STAGING config repo" "$LOG_DIR/nix-closure-domain-staging-init.log" <(
  printf 'set -eu\nnixome-config-staging-init\n'
)
run_qrexec_script_or_fail "$VALIDATOR" "initialize $VALIDATOR config tools" "$LOG_DIR/nix-closure-domain-validator-init.log" <(
  printf 'set -eu\nnixome-config-validator-init\n'
)

cat > "$STAGING_UPDATE_SCRIPT" <<EOF
set -eu
cd /home/$QUBE_USER/nixome-config
cat > nixome/machine.nix <<'MACHINENIX'
{
  appliedMessage,
  pkgs ? null,
  system,
  templateBuilders ? null,
  ...
}:
{
  domains = { };

  nixNative.domains."$DOMAIN" = {
    label = "green";
    netvm = null;
    rootImageSizeMiB = 128;
    additionalStoreSpaceMiB = 128;
    prefs = {
      memory = 1024;
      maxmem = 2048;
      qrexec_timeout = 180;
      vcpus = 1;
    };
    features = {
      "nixome-role" = "nix-closure-smoke";
      "nixome-test" = "nix-closure-domain";
    };
    services = {
      "network-manager" = false;
    };
    configuration = {
      environment.etc."nixome/domain-role".text = "nix-closure-smoke\\n";
      environment.etc."nixome/applied-message".text = "$APPLY_MESSAGE\\n";
    };
  };

  topology = {
    extraTemplates = { };
    extraKernels = { };
    extraPrefs = { };
    extraDomains = { };
    extraQrexecRules = [
      {
        service = "nixome.NixStoreRuntimeCheck";
        argument = "*";
        source = "dom0";
        target = "$DOMAIN";
        action = "allow";
      }
    ];
  };

  configVms = {
    guests = {
      config-vault = { };
      config-staging = { };
      config-validator = { };
    };
    domains = { };
  };

  dom0.host = {
    apply.mode = "auto";
    overlays = [ ];
    packages = [ ];
    etc = { };
    systemd = { };
    configuration = { };
    modules = [ ];
  };

  dom0.extraModules = [ ];
  system.args = { };
}
MACHINENIX
git add nixome/machine.nix
git -c user.name='Nixome Closure Smoke' -c user.email='nixome-closure-smoke@localhost' \
  commit -m 'add nix closure smoke domain'
git status --short --branch
EOF

run_qrexec_script_or_fail \
  "$STAGING" \
  "stage nix-closure domain config in $STAGING" \
  "$STAGING_UPDATE_LOG" \
  "$STAGING_UPDATE_SCRIPT"

run_or_fail "apply nix-closure domain config through $VALIDATOR" "$APPLY_LOG" \
  env \
    NIXOME_LOG_DIR="$LOG_DIR" \
    NIXOME_CONFIG_STAGING_VM="$STAGING" \
    NIXOME_CONFIG_VALIDATOR_VM="$VALIDATOR" \
    NIXOME_CONFIG_VAULT_VM="$VAULT" \
    NIXOME_CONFIG_APPLY_NIX_OFFLINE="$CONFIG_APPLY_NIX_OFFLINE" \
  nixome-config-apply --apply --staging "$STAGING" --validator "$VALIDATOR" --vault "$VAULT"

collect_diagnostics

jq -e --arg domain "$DOMAIN" '
  .schema == "nixome.config-apply.v1" and
  .status == "passed" and
  .setup.status == "passed" and
  any(.nix_domain_materializations_result.domains[]; .domain == $domain)
' "$APPLY_RESULT_JSON" >/dev/null ||
  fail "config apply result did not contain a successful nix-closure materialization"

qvm-check "$DOMAIN" >/dev/null 2>&1 || fail "Qubes did not create domain: $DOMAIN"
qvm-ls --raw-data --fields NAME,CLASS | grep -Fx "$DOMAIN|StandaloneVM" >/dev/null ||
  fail "domain is not a StandaloneVM"
qvm-volume info "$DOMAIN:store" >/dev/null 2>&1 ||
  fail "Qubes store volume was not created"
qvm_volume_list_has_volume "$DOMAIN" store ||
  fail "Qubes volume list does not include the store volume"

grep -F "nixome.NixStoreRuntimeCheck * dom0 $DOMAIN allow" "$POLICY_LOG" >/dev/null ||
  fail "generated qrexec policy does not allow nixome.NixStoreRuntimeCheck"
grep -Fx "nixome.NixStoreRuntimeCheck * @anyvm @anyvm deny" "$POLICY_LOG" >/dev/null ||
  fail "generated qrexec policy does not deny nixome.NixStoreRuntimeCheck catch-all"

check_source_paths
qvm-prefs "$DOMAIN" kernelopts > "$KERNEL_OPTS_LOG" 2>&1
[ "$(extract_kernelopt init)" = "/sbin/init" ] ||
  fail "kernelopts do not route boot through /sbin/init"
[ "$(extract_kernelopt nixome.system)" = "$system_source" ] ||
  fail "kernelopts nixome.system does not match declared system"
[ "$(extract_kernelopt nixome.storedev)" = "$store_device" ] ||
  fail "kernelopts nixome.storedev does not match declared store device"
[ "$(extract_kernelopt nixome.storefmt)" = "$store_filesystem" ] ||
  fail "kernelopts nixome.storefmt does not match declared store filesystem"
if [ "$store_filesystem" = "erofs" ]; then
  [ "$(extract_kernelopt nixome.storeblobdev)" = "$store_blob_device" ] ||
    fail "kernelopts nixome.storeblobdev does not match declared blob device"
  qvm-volume info "$DOMAIN:store-data" >/dev/null 2>&1 ||
    fail "Qubes store-data volume was not created"
  qvm_volume_list_has_volume "$DOMAIN" store-data ||
    fail "Qubes volume list does not include the store-data volume"
fi

check_root_image_has_no_store
check_store_image_contains_system

write_guest_probe_script
start_vm "$DOMAIN"
wait_for_qrexec "$DOMAIN" "$DOMAIN" "$LOG_DIR/nix-closure-domain-qrexec.log"
run_runtime_service_check
run_guest_probe "$GUEST_PROBE_LOG"
run_persistence_probe
run_reboot_cycle
run_shutdown_start_cycle
run_optional_gui_probe

collect_diagnostics
write_result passed "nix-closure domain booted from a separate read-only Nix store volume"
cat "$RESULT_JSON" >&3
