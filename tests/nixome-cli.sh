#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"

WORK_DIR="${NIXOME_CLI_TEST_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/nixome-cli}"
BIN_DIR="$WORK_DIR/bin"
REPO="$WORK_DIR/repo"
STATUS_JSON="$WORK_DIR/status.json"
APPLY_JSON="$WORK_DIR/apply.json"
CHECK_JSON="$WORK_DIR/check.json"
ROLLBACK_JSON="$WORK_DIR/rollback.json"
CAPTURED_TAR="$WORK_DIR/captured-candidate.tar"
QREXEC_LOG="$WORK_DIR/qrexec.log"

rm -rf "$WORK_DIR"
mkdir -p "$BIN_DIR" "$REPO/modules"

cat > "$STATUS_JSON" <<'EOF'
{
  "schema": "nixome.config-status.v1",
  "status": "applied",
  "deployment": {
    "current": {
      "schema": "nixome.config-deployment.v1",
      "candidate_commit": "base000000000000000000000000000000000000"
    }
  },
  "host_generation": {
    "active": true,
    "current_source": "/nix/store/base-system"
  }
}
EOF

cat > "$BIN_DIR/qrexec-client-vm" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

target="$1"
service="$2"
printf '%s %s\n' "$target" "$service" >> "$QREXEC_LOG"

case "$service" in
  nixome.Config.Status)
    cat "$STATUS_JSON"
    ;;
  nixome.Config.Check)
    cat > "$CAPTURED_TAR"
    jq -c -n --arg schema "nixome.config-archive.v1" --arg status "passed" \
      '{schema: $schema, status: $status}'
    ;;
  nixome.Config.Apply)
    cat > "$CAPTURED_TAR"
    jq -c -n --arg schema "nixome.config-apply.v1" --arg status "passed" \
      '{schema: $schema, status: $status}'
    ;;
  nixome.Config.Rollback)
    jq -c -n --arg schema "nixome.config-rollback.v1" --arg status "passed" \
      '{schema: $schema, status: $status}'
    ;;
  *)
    printf 'unexpected service: %s\n' "$service" >&2
    exit 2
    ;;
esac
EOF
chmod 0755 "$BIN_DIR/qrexec-client-vm"

cat > "$REPO/flake.nix" <<'EOF'
{
  description = "Nixome CLI test repo";
  outputs = { self }: {
    nixome.topology = {
      schema = "nixome.qubes-topology.v1";
      templates = {};
      kernels = {};
      domains = {};
      prefs = {};
      qrexec_policy.rules = [];
    };
  };
}
EOF
printf 'initial controller\n' > "$REPO/modules/controller.nix"

(
  cd "$REPO"
  git init -b main >/dev/null
  git add flake.nix modules/controller.nix
  git \
    -c user.name='Nixome CLI Test' \
    -c user.email='nixome-cli-test@localhost' \
    commit -m 'initial candidate' >/dev/null
  printf 'self-hosted controller change\n' > modules/controller.nix
  git add modules/controller.nix
  git \
    -c user.name='Nixome CLI Test' \
    -c user.email='nixome-cli-test@localhost' \
    commit -m 'change controller source' >/dev/null
)

expected_commit="$(git -C "$REPO" rev-parse HEAD)"
expected_tree="$(git -C "$REPO" rev-parse 'HEAD^{tree}')"
nixome_out="$(nix build "$ROOT#packages.x86_64-linux.nixome-cli" --no-link --print-out-paths)"
nixome_bin="$(nixome_physical_store_path "$nixome_out")/bin/nixome"

(
  cd "$REPO"
  env \
    PATH="$BIN_DIR:$PATH" \
    NIXOME_QREXEC_CLIENT="$BIN_DIR/qrexec-client-vm" \
    QREXEC_LOG="$QREXEC_LOG" \
    STATUS_JSON="$STATUS_JSON" \
    CAPTURED_TAR="$CAPTURED_TAR" \
    "$nixome_bin" status > "$WORK_DIR/status-output.json"
)
jq -e '.schema == "nixome.config-status.v1" and .status == "applied"' \
  "$WORK_DIR/status-output.json" >/dev/null

(
  cd "$REPO"
  env \
    PATH="$BIN_DIR:$PATH" \
    NIXOME_QREXEC_CLIENT="$BIN_DIR/qrexec-client-vm" \
    QREXEC_LOG="$QREXEC_LOG" \
    STATUS_JSON="$STATUS_JSON" \
    CAPTURED_TAR="$CAPTURED_TAR" \
    "$nixome_bin" check > "$CHECK_JSON"
)
jq -e '.schema == "nixome.config-archive.v1" and .status == "passed"' "$CHECK_JSON" >/dev/null

tar -tf "$CAPTURED_TAR" | sort > "$WORK_DIR/candidate-members.txt"
grep -Fx config.bundle "$WORK_DIR/candidate-members.txt" >/dev/null
grep -Fx manifest.json "$WORK_DIR/candidate-members.txt" >/dev/null
mkdir -p "$WORK_DIR/candidate"
tar -C "$WORK_DIR/candidate" -xf "$CAPTURED_TAR"
jq -e \
  --arg commit "$expected_commit" \
  --arg tree "$expected_tree" \
  '
    .schema == "nixome.config-candidate.v1" and
    .commit == $commit and
    .tree == $tree and
    .branch == "main" and
    .source_role == "dev" and
    .transport == "qrexec" and
    .base_generation.schema == "nixome.config-status.v1" and
    .base_generation.deployment.current.candidate_commit == "base000000000000000000000000000000000000" and
    .requested_targets == ["nixome"]
  ' "$WORK_DIR/candidate/manifest.json" >/dev/null

git clone "$WORK_DIR/candidate/config.bundle" "$WORK_DIR/cloned" >/dev/null 2>&1
grep -qx 'self-hosted controller change' "$WORK_DIR/cloned/modules/controller.nix"

(
  cd "$REPO"
  env \
    PATH="$BIN_DIR:$PATH" \
    NIXOME_QREXEC_CLIENT="$BIN_DIR/qrexec-client-vm" \
    QREXEC_LOG="$QREXEC_LOG" \
    STATUS_JSON="$STATUS_JSON" \
    CAPTURED_TAR="$CAPTURED_TAR" \
    "$nixome_bin" apply > "$APPLY_JSON"
)
jq -e '.schema == "nixome.config-apply.v1" and .status == "passed"' "$APPLY_JSON" >/dev/null

(
  cd "$REPO"
  env \
    PATH="$BIN_DIR:$PATH" \
    NIXOME_QREXEC_CLIENT="$BIN_DIR/qrexec-client-vm" \
    QREXEC_LOG="$QREXEC_LOG" \
    STATUS_JSON="$STATUS_JSON" \
    CAPTURED_TAR="$CAPTURED_TAR" \
    "$nixome_bin" rollback > "$ROLLBACK_JSON"
)
jq -e '.schema == "nixome.config-rollback.v1" and .status == "passed"' "$ROLLBACK_JSON" >/dev/null

printf 'dirty\n' >> "$REPO/modules/controller.nix"
if (
  cd "$REPO"
  env \
    PATH="$BIN_DIR:$PATH" \
    NIXOME_QREXEC_CLIENT="$BIN_DIR/qrexec-client-vm" \
    QREXEC_LOG="$QREXEC_LOG" \
    STATUS_JSON="$STATUS_JSON" \
    CAPTURED_TAR="$CAPTURED_TAR" \
    "$nixome_bin" check > "$WORK_DIR/dirty.json" 2> "$WORK_DIR/dirty.stderr"
); then
  printf 'expected dirty worktree submission to fail\n' >&2
  exit 1
fi
grep -F 'worktree is dirty' "$WORK_DIR/dirty.stderr" >/dev/null

grep -Fx 'dom0 nixome.Config.Status' "$QREXEC_LOG" >/dev/null
grep -Fx 'dom0 nixome.Config.Check' "$QREXEC_LOG" >/dev/null
grep -Fx 'dom0 nixome.Config.Apply' "$QREXEC_LOG" >/dev/null
grep -Fx 'dom0 nixome.Config.Rollback' "$QREXEC_LOG" >/dev/null

printf 'nixome cli test ok\n'
