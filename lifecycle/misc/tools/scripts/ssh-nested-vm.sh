#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
source "$ROOT/lifecycle/misc/tools/scripts/lib/airlock-env.sh"

SSH_PORT="${AIRLOCK_VM_SSH_PORT:-2222}"
SSH_USER="${AIRLOCK_VM_SSH_USER:-airlock}"
SSH_PASSWORD="${AIRLOCK_VM_SSH_PASSWORD:-airlock}"

export SSHPASS="$SSH_PASSWORD"

exec "$ROOT/lifecycle/misc/tools/scripts/airlock-nix.sh" shell nixpkgs#sshpass -c sshpass -e \
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
