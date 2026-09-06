#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
WORK_DIR="${NIXOME_QUBE_GUEST_POLICY_PROBE_TEST_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/qube-guest-policy-probe}"
SERVICE_PATH_FILE="$WORK_DIR/policy-probe-path"
FAKE_QDB="$WORK_DIR/qubesdb-read"

rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"

nix build --no-link \
  "$ROOT#nixosConfigurations.qube-template-config-system.config.environment.etc.\"qubes-rpc/nixome.PolicyProbe\".source" \
  >/dev/null
nix eval --raw \
  "$ROOT#nixosConfigurations.qube-template-config-system.config.environment.etc.\"qubes-rpc/nixome.PolicyProbe\".source" \
  > "$SERVICE_PATH_FILE"
SERVICE="$(cat "$SERVICE_PATH_FILE")"

cat > "$FAKE_QDB" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  /features/nixome-role)
    exit 1
    ;;
  /name)
    printf 'config-vault\n'
    ;;
esac
EOF
chmod 0755 "$FAKE_QDB"

NIXOME_QUBESDB_READ="$FAKE_QDB" nixome_run_store_path "$SERVICE" > "$WORK_DIR/from-name.log"
grep -E '^nixome-policy-probe role=config-vault user=.+$' "$WORK_DIR/from-name.log" >/dev/null

cat > "$FAKE_QDB" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  /features/nixome-role)
    printf 'config-validator\n'
    ;;
  /name)
    printf 'other-name\n'
    ;;
esac
EOF

NIXOME_QUBESDB_READ="$FAKE_QDB" nixome_run_store_path "$SERVICE" > "$WORK_DIR/from-feature.log"
grep -E '^nixome-policy-probe role=config-validator user=.+$' "$WORK_DIR/from-feature.log" >/dev/null

printf 'qube guest policy probe test ok\n'
