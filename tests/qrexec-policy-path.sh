#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${NIXOME_QREXEC_POLICY_PATH_TEST_DIR:-$ROOT/.nixome-logs/qrexec-policy-path}"
STATIC_DIR="$WORK_DIR/static-policy.d"
RUNTIME_DIR="$WORK_DIR/runtime-policy.d"
SCRIPT_PATH_FILE="$WORK_DIR/helper-path"

rm -rf "$WORK_DIR"
mkdir -p "$STATIC_DIR/include" "$RUNTIME_DIR"

printf 'nixome.Static * dom0 config-vault allow\n' > "$STATIC_DIR/10-static.policy"
printf 'nixome.Include * dom0 config-vault allow\n' > "$STATIC_DIR/include/example.policy"
printf 'this static managed-name file must be ignored\n' > "$STATIC_DIR/90-nixome.policy"
printf 'stale\n' > "$RUNTIME_DIR/old.policy"

nix build --no-link \
  "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.units.\"nixome-qubes-policy-path.service\".unit" \
  >/dev/null
nix eval --raw \
  "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.services.nixome-qubes-policy-path.serviceConfig.ExecStart" \
  > "$SCRIPT_PATH_FILE"
HELPER="$(cat "$SCRIPT_PATH_FILE")"

env \
  NIXOME_QUBES_STATIC_POLICY_DIR="$STATIC_DIR" \
  NIXOME_QUBES_POLICY_DIR="$RUNTIME_DIR" \
  "$HELPER"

[ -L "$RUNTIME_DIR/10-static.policy" ]
[ "$(readlink -f "$RUNTIME_DIR/10-static.policy")" = "$(readlink -f "$STATIC_DIR/10-static.policy")" ]
[ -L "$RUNTIME_DIR/include" ]
[ "$(readlink -f "$RUNTIME_DIR/include")" = "$(readlink -f "$STATIC_DIR/include")" ]
[ ! -e "$RUNTIME_DIR/old.policy" ]

cat > "$WORK_DIR/expected-empty-managed.policy" <<'EOF'
# Managed by Nixome setup-config.
# Source: nixome.qubes.topology.qrexec_policy.
# Bootstrap config transport policy remains in 30-nixome-config.policy.
EOF
cmp -s "$WORK_DIR/expected-empty-managed.policy" "$RUNTIME_DIR/90-nixome.policy"

cat > "$RUNTIME_DIR/90-nixome.policy" <<'EOF'
# Managed by Nixome setup-config.
# Source: nixome.qubes.topology.qrexec_policy.
# Bootstrap config transport policy remains in 30-nixome-config.policy.
nixome.PolicyProbe * dom0 config-vault allow
nixome.PolicyProbe * @anyvm @anyvm deny
EOF
printf 'nixome.StaticTwo * dom0 config-vault allow\n' > "$STATIC_DIR/20-static.policy"
printf 'stale\n' > "$RUNTIME_DIR/stale.policy"

env \
  NIXOME_QUBES_STATIC_POLICY_DIR="$STATIC_DIR" \
  NIXOME_QUBES_POLICY_DIR="$RUNTIME_DIR" \
  "$HELPER"

grep -Fx "nixome.PolicyProbe * dom0 config-vault allow" "$RUNTIME_DIR/90-nixome.policy" >/dev/null
[ -L "$RUNTIME_DIR/20-static.policy" ]
[ ! -e "$RUNTIME_DIR/stale.policy" ]

UNMANAGED_RUNTIME_DIR="$WORK_DIR/unmanaged-runtime-policy.d"
mkdir -p "$UNMANAGED_RUNTIME_DIR"
printf 'unmanaged\n' > "$UNMANAGED_RUNTIME_DIR/90-nixome.policy"
if env \
  NIXOME_QUBES_STATIC_POLICY_DIR="$STATIC_DIR" \
  NIXOME_QUBES_POLICY_DIR="$UNMANAGED_RUNTIME_DIR" \
  "$HELPER" > "$WORK_DIR/unmanaged-stdout.log" 2> "$WORK_DIR/unmanaged-stderr.log"; then
  printf 'policy helper accepted unmanaged 90-nixome.policy\n' >&2
  exit 1
fi
grep -F "refusing unmanaged Nixome qrexec policy file" "$WORK_DIR/unmanaged-stderr.log" >/dev/null

printf 'qrexec policy path test ok\n'
