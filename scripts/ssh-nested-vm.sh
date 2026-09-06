#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"

SSH_PORT="${NIXOME_VM_SSH_PORT:-2222}"
SSH_USER="${NIXOME_VM_SSH_USER:-nixome}"
SSH_PASSWORD="${NIXOME_VM_SSH_PASSWORD:-nixome}"

export SSHPASS="$SSH_PASSWORD"

exec nix shell nixpkgs#sshpass -c sshpass -e \
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
