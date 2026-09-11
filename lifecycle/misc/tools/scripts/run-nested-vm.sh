#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
source "$ROOT/lifecycle/misc/tools/scripts/lib/nixome-env.sh"
VM_DIR="${NIXOME_VM_DIR:-$ROOT/.nixome-vm}"
RESULT_LINK_DIR="${NIXOME_RESULT_LINK_DIR:-${NIXOME_DATA_ROOT:-$ROOT/.nixome}/result-links}"
IMAGE_ATTR="${NIXOME_VM_IMAGE_ATTR:-nixome-vm-dev-image}"
MEMORY="${NIXOME_VM_MEMORY:-8192}"
SMP="${NIXOME_VM_SMP:-4}"
SSH_PORT="${NIXOME_VM_SSH_PORT:-2222}"
SSH_USER="${NIXOME_VM_SSH_USER:-nixome}"
SSH_PASSWORD="${NIXOME_VM_SSH_PASSWORD:-nixome}"
DISPLAY_MODE="${NIXOME_VM_DISPLAY:-none}"
VGA="${NIXOME_VM_VGA:-std}"
IOMMU="${NIXOME_VM_IOMMU:-1}"
BACKUP_RETENTION="${NIXOME_VM_BACKUP_RETENTION:-1}"
FRESH="${NIXOME_VM_FRESH:-0}"
SKIP_BASE_BUILD="${NIXOME_VM_SKIP_BASE_BUILD:-0}"
BACKING_IMAGE="${NIXOME_VM_BACKING_IMAGE:-}"
OVMF_VARS_TEMPLATE="${NIXOME_VM_OVMF_VARS_TEMPLATE:-}"
OUT_LINK="$VM_DIR/${IMAGE_ATTR}-result"
SERIAL_LOG="$VM_DIR/serial.log"
MONITOR_SOCKET="$VM_DIR/${IMAGE_ATTR}.monitor"

mkdir -p "$VM_DIR" "$RESULT_LINK_DIR"

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

case "$SKIP_BASE_BUILD" in
  0 | 1)
    ;;
  *)
    printf 'error: NIXOME_VM_SKIP_BASE_BUILD must be 0 or 1\n' >&2
    exit 1
    ;;
esac

case "$IOMMU" in
  0 | 1)
    ;;
  *)
    printf 'error: NIXOME_VM_IOMMU must be 0 or 1\n' >&2
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

if [ "$SKIP_BASE_BUILD" = "1" ]; then
  [ -e "$OUT_LINK" ] ||
    {
      printf 'error: NIXOME_VM_SKIP_BASE_BUILD=1 but base image result link is missing: %s\n' "$OUT_LINK" >&2
      exit 1
    }
  printf 'using existing Nixome VM base image result link: %s\n' "$OUT_LINK"
  base_image_out_physical="$(readlink -f "$OUT_LINK")"
  base_image_out="$base_image_out_physical"
else
  printf 'building/resolving Nixome VM base image: .#%s\n' "$IMAGE_ATTR"
  base_image_out="$(nix build "$ROOT#$IMAGE_ATTR" --no-link --print-out-paths --print-build-logs --show-trace)"
  base_image_out="$(awk 'NF { last = $0 } END { print last }' <<< "$base_image_out")"
  base_image_out_physical="$(nixome_physical_store_path "$base_image_out")"
  ln -sfn "$base_image_out_physical" "$OUT_LINK"
fi

build_attr_with_result_link() {
  local attr="$1"
  local name="$2"
  local out

  out="$(nix build "$ROOT#$attr" \
    --out-link "$RESULT_LINK_DIR/$name" \
    --print-out-paths \
    --print-build-logs \
    --show-trace)"
  awk 'NF { last = $0 } END { print last }' <<< "$out"
}

mapfile -t base_images < <(find -L "$base_image_out_physical" -maxdepth 1 -type f \( -name '*.qcow2' -o -name '*.img' \) | sort)
if [ "${#base_images[@]}" -ne 1 ]; then
  printf 'error: expected one qcow2 or raw img under %s, found %s\n' "$base_image_out" "${#base_images[@]}" >&2
  exit 1
fi

base_image_relative="${base_images[0]#"$base_image_out_physical"/}"
BASE_IMAGE="$base_image_out/$base_image_relative"
NIX_BASE_IMAGE="$BASE_IMAGE"

if [ -n "$BACKING_IMAGE" ]; then
  if [ ! -f "$BACKING_IMAGE" ]; then
    printf 'error: NIXOME_VM_BACKING_IMAGE is not a file: %s\n' "$BACKING_IMAGE" >&2
    exit 1
  fi
  BASE_IMAGE="$BACKING_IMAGE"
fi

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

qemu_out="$(build_attr_with_result_link qemu-kvm qemu-kvm-result)"
qemu_utils_out="$(build_attr_with_result_link qemu-utils qemu-utils-result)"
ovmf_out="$(build_attr_with_result_link ovmf-fd ovmf-fd-result)"

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
  if [ -n "$OVMF_VARS_TEMPLATE" ]; then
    if [ ! -f "$OVMF_VARS_TEMPLATE" ]; then
      printf 'error: NIXOME_VM_OVMF_VARS_TEMPLATE is not a file: %s\n' "$OVMF_VARS_TEMPLATE" >&2
      exit 1
    fi
    cp "$OVMF_VARS_TEMPLATE" "$OVMF_VARS"
  else
    cp "$(nixome_physical_store_path "$ovmf_out")/FV/OVMF_VARS.fd" "$OVMF_VARS"
  fi
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

MACHINE="q35,accel=kvm"
iommu_args=()
if [ "$IOMMU" = "1" ]; then
  MACHINE="q35,accel=kvm,kernel-irqchip=split"
  iommu_args=(-device intel-iommu,intremap=on,caching-mode=on)
fi

cat <<EOF
starting VM
  image attr: .#$IMAGE_ATTR
  machine: $MACHINE
  nix base: $NIX_BASE_IMAGE
  backing: $BASE_IMAGE ($BASE_FORMAT)
  disk: $RUN_DISK
  display: $DISPLAY_MODE
  vga: $VGA
  iommu: $IOMMU
  memory: ${MEMORY} MiB
  ssh: NIXOME_VM_SSH_PORT=$SSH_PORT $ROOT/lifecycle/misc/tools/scripts/ssh-nested-vm.sh
  dom0 NIC: QEMU user network with SSH forwarding
  sys-net NIC: separate QEMU user network, auto-assigned by setup-config
  login: $SSH_USER / $SSH_PASSWORD
  monitor: $MONITOR_SOCKET

Inside the VM, first check:
  nixome-observe-dom0 | jq
  cat /proc/xen/capabilities
  systemctl status qubes-db-dom0 qubesd qubes-qrexec-policy-daemon
EOF

rm -f "$MONITOR_SOCKET"

nixome_exec_store_path "$qemu_out/bin/qemu-system-x86_64" \
  -machine "$MACHINE" \
  -cpu host,migratable=off \
  -smp "$SMP" \
  -m "$MEMORY" \
  -drive if=pflash,format=raw,readonly=on,file="$ovmf_out/FV/OVMF_CODE.fd" \
  -drive if=pflash,format=raw,file="$OVMF_VARS" \
  "${iommu_args[@]}" \
  -drive if=virtio,format=qcow2,file="$RUN_DISK",discard=unmap \
  -vga "$VGA" \
  -device virtio-net-pci,netdev=net0 \
  -netdev "user,id=net0,hostfwd=tcp:127.0.0.1:$SSH_PORT-:22" \
  -device virtio-net-pci,disable-legacy=on,iommu_platform=on,vectors=0,netdev=sysnet0 \
  -netdev "user,id=sysnet0" \
  -device virtio-rng-pci \
  -monitor "unix:$MONITOR_SOCKET,server=on,wait=off" \
  -no-reboot \
  "${display_args[@]}"
