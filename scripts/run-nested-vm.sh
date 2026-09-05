#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VM_DIR="${NIXOME_VM_DIR:-$ROOT/.nixome-vm}"
IMAGE_ATTR="${NIXOME_VM_IMAGE_ATTR:-nixome-vm-image}"
MEMORY="${NIXOME_VM_MEMORY:-8192}"
SMP="${NIXOME_VM_SMP:-4}"
SSH_PORT="${NIXOME_VM_SSH_PORT:-2222}"
DISPLAY_MODE="${NIXOME_VM_DISPLAY:-none}"
OUT_LINK="$VM_DIR/${IMAGE_ATTR}-result"

mkdir -p "$VM_DIR"

if [ ! -e /dev/kvm ]; then
  printf 'error: /dev/kvm is missing; nested Xen needs KVM on the host\n' >&2
  exit 1
fi

if ! awk -F: '/flags|Features/ { print $2; exit }' /proc/cpuinfo | grep -Eqw 'vmx|svm'; then
  printf 'error: host CPU flags do not expose vmx or svm\n' >&2
  exit 1
fi

printf 'building/resolving Nixome VM base image: .#%s\n' "$IMAGE_ATTR"
nix build "$ROOT#$IMAGE_ATTR" --out-link "$OUT_LINK" --print-build-logs --show-trace

mapfile -t base_images < <(find -L "$OUT_LINK" -maxdepth 1 -type f -name '*.qcow2' | sort)
if [ "${#base_images[@]}" -ne 1 ]; then
  printf 'error: expected one qcow2 under %s, found %s\n' "$OUT_LINK" "${#base_images[@]}" >&2
  exit 1
fi

BASE_IMAGE="$(readlink -f "${base_images[0]}")"
RUN_DISK="$VM_DIR/${IMAGE_ATTR}.qcow2"

qemu_out="$(nix build --no-link --print-out-paths "$ROOT#qemu-kvm")"
qemu_utils_out="$(nix build --no-link --print-out-paths "$ROOT#qemu-utils")"
ovmf_out="$(nix build --no-link --print-out-paths "$ROOT#ovmf-fd")"

if [ ! -e "$RUN_DISK" ]; then
  printf 'creating writable overlay: %s\n' "$RUN_DISK"
  "$qemu_utils_out/bin/qemu-img" create \
    -f qcow2 \
    -F qcow2 \
    -b "$BASE_IMAGE" \
    "$RUN_DISK"
fi

OVMF_VARS="$VM_DIR/${IMAGE_ATTR}-OVMF_VARS.fd"
if [ ! -e "$OVMF_VARS" ]; then
  cp "$ovmf_out/FV/OVMF_VARS.fd" "$OVMF_VARS"
  chmod u+w "$OVMF_VARS"
fi

display_args=()
case "$DISPLAY_MODE" in
  none)
    display_args=(-display none -serial mon:stdio)
    ;;
  gtk)
    display_args=(-display gtk,gl=off -serial "file:$VM_DIR/serial.log")
    ;;
  sdl)
    display_args=(-display sdl -serial "file:$VM_DIR/serial.log")
    ;;
  *)
    printf 'error: unsupported NIXOME_VM_DISPLAY=%s; use none, gtk, or sdl\n' "$DISPLAY_MODE" >&2
    exit 1
    ;;
esac

cat <<EOF
starting VM
  image attr: .#$IMAGE_ATTR
  disk: $RUN_DISK
  display: $DISPLAY_MODE
  memory: ${MEMORY} MiB
  ssh: ssh -p $SSH_PORT nixome@127.0.0.1
  password: nixome

Inside the VM, first check:
  nixome-observe-dom0 | jq
  cat /proc/xen/capabilities
  systemctl status qubes-db-dom0 qubesd qubes-qrexec-policy-daemon
EOF

exec "$qemu_out/bin/qemu-system-x86_64" \
  -machine q35,accel=kvm \
  -cpu host,migratable=off \
  -smp "$SMP" \
  -m "$MEMORY" \
  -drive if=pflash,format=raw,readonly=on,file="$ovmf_out/FV/OVMF_CODE.fd" \
  -drive if=pflash,format=raw,file="$OVMF_VARS" \
  -drive if=virtio,format=qcow2,file="$RUN_DISK",discard=unmap \
  -vga virtio \
  -device virtio-net-pci,netdev=net0 \
  -netdev "user,id=net0,hostfwd=tcp:127.0.0.1:$SSH_PORT-:22" \
  -device virtio-rng-pci \
  -no-reboot \
  "${display_args[@]}"
