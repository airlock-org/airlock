#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
VM_DIR="${NIXOME_VM_DIR:-$ROOT/.nixome-vm}"
IMAGE_ATTR="${NIXOME_VM_IMAGE_ATTR:-nixome-vm-image}"
MEMORY="${NIXOME_VM_MEMORY:-8192}"
SMP="${NIXOME_VM_SMP:-4}"
SSH_PORT="${NIXOME_VM_SSH_PORT:-2222}"
DISPLAY_MODE="${NIXOME_VM_DISPLAY:-none}"
VGA="${NIXOME_VM_VGA:-std}"
BACKUP_RETENTION="${NIXOME_VM_BACKUP_RETENTION:-1}"
FRESH="${NIXOME_VM_FRESH:-0}"
OUT_LINK="$VM_DIR/${IMAGE_ATTR}-result"
SERIAL_LOG="$VM_DIR/serial.log"
MONITOR_SOCKET="$VM_DIR/${IMAGE_ATTR}.monitor"

mkdir -p "$VM_DIR"

case "$BACKUP_RETENTION" in
  '' | *[!0-9]*)
    printf 'error: NIXOME_VM_BACKUP_RETENTION must be a non-negative integer\n' >&2
    exit 1
    ;;
esac

case "$FRESH" in
  0 | 1)
    ;;
  *)
    printf 'error: NIXOME_VM_FRESH must be 0 or 1\n' >&2
    exit 1
    ;;
esac

if [ ! -e /dev/kvm ]; then
  printf 'error: /dev/kvm is missing; nested Xen needs KVM on the host\n' >&2
  exit 1
fi

if ! awk -F: '/flags|Features/ { print $2; exit }' /proc/cpuinfo | grep -Eqw 'vmx|svm'; then
  printf 'error: host CPU flags do not expose vmx or svm\n' >&2
  exit 1
fi

printf 'building/resolving Nixome VM base image: .#%s\n' "$IMAGE_ATTR"
base_image_out="$(nix build "$ROOT#$IMAGE_ATTR" --no-link --print-out-paths --print-build-logs --show-trace)"
base_image_out="$(awk 'NF { last = $0 } END { print last }' <<< "$base_image_out")"
base_image_out_physical="$(nixome_physical_store_path "$base_image_out")"
ln -sfn "$base_image_out_physical" "$OUT_LINK"

mapfile -t base_images < <(find -L "$base_image_out_physical" -maxdepth 1 -type f \( -name '*.qcow2' -o -name '*.img' \) | sort)
if [ "${#base_images[@]}" -ne 1 ]; then
  printf 'error: expected one qcow2 or raw img under %s, found %s\n' "$base_image_out" "${#base_images[@]}" >&2
  exit 1
fi

base_image_relative="${base_images[0]#"$base_image_out_physical"/}"
BASE_IMAGE="$base_image_out/$base_image_relative"
RUN_DISK="$VM_DIR/${IMAGE_ATTR}.qcow2"
OVMF_VARS="$VM_DIR/${IMAGE_ATTR}-OVMF_VARS.fd"
RUN_DISK_BACKUP_TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"

delete_file_if_present() {
  local path="$1"

  [ -e "$path" ] || return 0
  if [ -f "$path" ] || [ -L "$path" ]; then
    printf 'fresh run: removing generated VM state: %s\n' "$path"
    unlink "$path"
  else
    printf 'error: refusing to remove non-file generated VM state: %s\n' "$path" >&2
    exit 1
  fi
}

prune_run_disk_backups() {
  local disk_name
  local index
  local backup
  disk_name="$(basename "$RUN_DISK")"
  index=0

  while IFS= read -r backup; do
    if [ "$index" -ge "$BACKUP_RETENTION" ]; then
      printf 'removing old writable overlay backup: %s\n' "$backup"
      unlink "$backup"
    fi
    index=$((index + 1))
  done < <(find "$VM_DIR" -maxdepth 1 -type f -name "$disk_name.*.bak" | sort -r)
}

qemu_out="$(nix build --no-link --print-out-paths "$ROOT#qemu-kvm")"
qemu_utils_out="$(nix build --no-link --print-out-paths "$ROOT#qemu-utils")"
ovmf_out="$(nix build --no-link --print-out-paths "$ROOT#ovmf-fd")"

BASE_FORMAT="$(
  nixome_run_store_path "$qemu_utils_out/bin/qemu-img" info --output=json "$BASE_IMAGE" |
    sed -n 's/^[[:space:]]*"format": "\(raw\|qcow2\)",[[:space:]]*$/\1/p' |
    head -n 1
)"

case "$BASE_FORMAT" in
  raw | qcow2)
    ;;
  *)
    printf 'error: unsupported base image format for %s: %s\n' "$BASE_IMAGE" "${BASE_FORMAT:-unknown}" >&2
    exit 1
    ;;
esac

if [ -e "$RUN_DISK" ]; then
  if [ "$FRESH" = "1" ]; then
    delete_file_if_present "$RUN_DISK"
  fi
fi

if [ -e "$OVMF_VARS" ]; then
  if [ "$FRESH" = "1" ]; then
    delete_file_if_present "$OVMF_VARS"
  fi
fi

if [ -e "$RUN_DISK" ]; then
  current_backing="$(
    nixome_run_store_path "$qemu_utils_out/bin/qemu-img" info --output=json "$RUN_DISK" |
      sed -n 's/^[[:space:]]*"backing-filename": "\(.*\)",[[:space:]]*$/\1/p' |
      head -n 1
  )"

  if [ "$current_backing" != "$BASE_IMAGE" ]; then
    backup_disk="$RUN_DISK.$RUN_DISK_BACKUP_TIMESTAMP.bak"
    printf 'base image changed; moving stale writable overlay to: %s\n' "$backup_disk"
    mv "$RUN_DISK" "$backup_disk"
    prune_run_disk_backups
  fi
fi

if [ ! -e "$RUN_DISK" ]; then
  printf 'creating writable overlay: %s\n' "$RUN_DISK"
  nixome_run_store_path "$qemu_utils_out/bin/qemu-img" create \
    -f qcow2 \
    -F "$BASE_FORMAT" \
    -b "$BASE_IMAGE" \
    "$RUN_DISK"
fi

if [ ! -e "$OVMF_VARS" ]; then
  cp "$(nixome_physical_store_path "$ovmf_out")/FV/OVMF_VARS.fd" "$OVMF_VARS"
  chmod u+w "$OVMF_VARS"
fi

display_args=()
case "$DISPLAY_MODE" in
  none)
    display_args=(-display none -serial mon:stdio)
    ;;
  gtk)
    : > "$SERIAL_LOG"
    display_args=(-display gtk,gl=off -serial "file:$SERIAL_LOG")
    ;;
  sdl)
    : > "$SERIAL_LOG"
    display_args=(-display sdl -serial "file:$SERIAL_LOG")
    ;;
  *)
    printf 'error: unsupported NIXOME_VM_DISPLAY=%s; use none, gtk, or sdl\n' "$DISPLAY_MODE" >&2
    exit 1
    ;;
esac

cat <<EOF
starting VM
  image attr: .#$IMAGE_ATTR
  base: $BASE_IMAGE ($BASE_FORMAT)
  disk: $RUN_DISK
  display: $DISPLAY_MODE
  vga: $VGA
  memory: ${MEMORY} MiB
  ssh: ssh -p $SSH_PORT nixome@127.0.0.1
  password: nixome
  monitor: $MONITOR_SOCKET

Inside the VM, first check:
  nixome-observe-dom0 | jq
  cat /proc/xen/capabilities
  systemctl status qubes-db-dom0 qubesd qubes-qrexec-policy-daemon
EOF

rm -f "$MONITOR_SOCKET"

nixome_exec_store_path "$qemu_out/bin/qemu-system-x86_64" \
  -machine q35,accel=kvm \
  -cpu host,migratable=off \
  -smp "$SMP" \
  -m "$MEMORY" \
  -drive if=pflash,format=raw,readonly=on,file="$ovmf_out/FV/OVMF_CODE.fd" \
  -drive if=pflash,format=raw,file="$OVMF_VARS" \
  -drive if=virtio,format=qcow2,file="$RUN_DISK",discard=unmap \
  -vga "$VGA" \
  -device virtio-net-pci,netdev=net0 \
  -netdev "user,id=net0,hostfwd=tcp:127.0.0.1:$SSH_PORT-:22" \
  -device virtio-rng-pci \
  -monitor "unix:$MONITOR_SOCKET,server=on,wait=off" \
  -no-reboot \
  "${display_args[@]}"
