#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"

WORK_DIR="${NIXOME_CONFIG_CONTROL_TEST_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/config-control}"
BIN_DIR="$WORK_DIR/bin"
REPO="$WORK_DIR/repo"
CONTROL_LOG_DIR="$WORK_DIR/control-logs"
SYSTEM_STATE_DIR="$WORK_DIR/state"
HOST_PROFILE="$WORK_DIR/host-system-profile"
HOST_CURRENT="$WORK_DIR/run-current-system"
PAYLOAD_MARKER="$WORK_DIR/payload-executed"
ACTIVATION_MARKER="$WORK_DIR/activated"

rm -rf "$WORK_DIR"
mkdir -p "$BIN_DIR" "$REPO" "$SYSTEM_STATE_DIR" "$CONTROL_LOG_DIR"

cat > "$BIN_DIR/nixome-config-status" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
jq -c -n \
  --arg schema "nixome.config-status.v1" \
  --arg status "applied" \
  '{schema: $schema, status: $status, deployment: {current: {candidate_commit: "base"}}}'
EOF

cat > "$BIN_DIR/nixome-config-archive-candidate" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

candidate_tar=""
validate_only=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --candidate-tar)
      shift
      candidate_tar="$1"
      ;;
    --validate-only)
      validate_only=1
      ;;
  esac
  shift
done

[ -n "$candidate_tar" ] || exit 2
[ "$validate_only" = 1 ] || exit 3
printf 'archive %s validate-only=%s\n' "$candidate_tar" "$validate_only" >> "$FAKE_CALL_LOG"

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT
tar -C "$tmpdir" -xf "$candidate_tar"
commit="$(jq -r '.commit' "$tmpdir/manifest.json")"
scenario="$(jq -r '.scenario // "valid"' "$tmpdir/manifest.json")"

if [ "$scenario" = "invalid-eval" ]; then
  jq -c -n \
    --arg schema "nixome.config-archive.v1" \
    --arg status "failed" \
    --arg commit "$commit" \
    '{
      schema: $schema,
      status: $status,
      reason: "candidate validation failed",
      candidate_commit: $commit,
      validation: {
        schema: "nixome.config-validation.v1",
        status: "failed",
        commit: $commit
      }
    }' > "$NIXOME_LOG_DIR/config-archive.json"
  exit 1
fi

jq -c -n \
  --arg schema "nixome.config-archive.v1" \
  --arg status "passed" \
  --arg commit "$commit" \
  '{
    schema: $schema,
    status: $status,
    reason: "validated by fake validator",
    candidate_commit: $commit,
    validation_commit: $commit,
    validation: {
      schema: "nixome.config-validation.v1",
      status: "passed",
      commit: $commit
    }
  }' > "$NIXOME_LOG_DIR/config-archive.json"
printf 'fake archive ok\n'
EOF

cat > "$BIN_DIR/nixome-config-apply" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

candidate_tar=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --apply)
      ;;
    --candidate-tar)
      shift
      candidate_tar="$1"
      ;;
  esac
  shift
done

[ -n "$candidate_tar" ] || exit 2
printf 'apply %s\n' "$candidate_tar" >> "$FAKE_CALL_LOG"

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT
tar -C "$tmpdir" -xf "$candidate_tar"
commit="$(jq -r '.commit' "$tmpdir/manifest.json")"
scenario="$(jq -r '.scenario // "valid"' "$tmpdir/manifest.json")"

case "$scenario" in
  valid)
    printf '%s\n' "$commit" > "$ACTIVATION_MARKER"
    jq -c -n \
      --arg schema "nixome.config-apply.v1" \
      --arg status "passed" \
      --arg commit "$commit" \
      '{schema: $schema, status: $status, candidate_commit: $commit, system_apply_result: {status: "passed"}}' \
      > "$NIXOME_LOG_DIR/config-apply.json"
    ;;
  build-failed)
    jq -c -n \
      --arg schema "nixome.config-apply.v1" \
      --arg status "failed" \
      --arg commit "$commit" \
      '{schema: $schema, status: $status, reason: "system build failed", candidate_commit: $commit}' \
      > "$NIXOME_LOG_DIR/config-apply.json"
    exit 1
    ;;
  test-failed)
    jq -c -n \
      --arg schema "nixome.config-apply.v1" \
      --arg status "failed" \
      --arg commit "$commit" \
      '{schema: $schema, status: $status, reason: "candidate tests failed", candidate_commit: $commit}' \
      > "$NIXOME_LOG_DIR/config-apply.json"
    exit 1
    ;;
  invalid-eval)
    jq -c -n \
      --arg schema "nixome.config-apply.v1" \
      --arg status "failed" \
      --arg commit "$commit" \
      '{schema: $schema, status: $status, reason: "candidate validation failed", candidate_commit: $commit}' \
      > "$NIXOME_LOG_DIR/config-apply.json"
    exit 1
    ;;
esac

printf 'fake apply ok\n'
EOF

cat > "$BIN_DIR/nix-env" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf 'nix-env %s\n' "$*" >> "$FAKE_CALL_LOG"
exit 1
EOF

chmod 0755 \
  "$BIN_DIR/nixome-config-status" \
  "$BIN_DIR/nixome-config-archive-candidate" \
  "$BIN_DIR/nixome-config-apply" \
  "$BIN_DIR/nix-env"

make_candidate() {
  local scenario="$1"
  local output="$2"
  local tmpdir
  local commit

  tmpdir="$(mktemp -d "$WORK_DIR/candidate.XXXXXX")"
  mkdir -p "$tmpdir/repo"
  cat > "$tmpdir/repo/flake.nix" <<'EOF'
{
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
  (
    cd "$tmpdir/repo"
    git init -b main >/dev/null
    git add flake.nix
    git \
      -c user.name='Nixome Control Test' \
      -c user.email='nixome-control-test@localhost' \
      commit -m "$scenario candidate" >/dev/null
  )
  commit="$(git -C "$tmpdir/repo" rev-parse HEAD)"
  git -C "$tmpdir/repo" bundle create "$tmpdir/config.bundle" HEAD refs/heads/main >/dev/null
  jq -c -n \
    --arg schema "nixome.config-candidate.v1" \
    --arg commit "$commit" \
    --arg scenario "$scenario" \
    --arg payload "touch $PAYLOAD_MARKER" \
    '{
      schema: $schema,
      commit: $commit,
      branch: "main",
      source_role: "dev",
      scenario: $scenario,
      payload: $payload
    }' > "$tmpdir/manifest.json"
  tar -C "$tmpdir" -cf "$output" config.bundle manifest.json
  rm -rf "$tmpdir"
}

CONTROL_ENV=(
  PATH="$BIN_DIR:$PATH"
  NIXOME_CONFIG_CONTROL_LOG_DIR="$CONTROL_LOG_DIR"
  NIXOME_SYSTEM_STATE_DIR="$SYSTEM_STATE_DIR"
  NIXOME_HOST_SYSTEM_PROFILE="$HOST_PROFILE"
  NIXOME_HOST_CURRENT_SYSTEM_LINK="$HOST_CURRENT"
  FAKE_CALL_LOG="$WORK_DIR/fake-calls.log"
  ACTIVATION_MARKER="$ACTIVATION_MARKER"
)

env "${CONTROL_ENV[@]}" bash "$ROOT/scripts/config-control.sh" status > "$WORK_DIR/status.json"
jq -e '.schema == "nixome.config-status.v1" and .status == "applied"' "$WORK_DIR/status.json" >/dev/null

make_candidate valid "$WORK_DIR/valid.tar"
env "${CONTROL_ENV[@]}" bash "$ROOT/scripts/config-control.sh" check \
  < "$WORK_DIR/valid.tar" > "$WORK_DIR/check-valid.json"
jq -e '.schema == "nixome.config-archive.v1" and .status == "passed"' \
  "$WORK_DIR/check-valid.json" >/dev/null
grep -F 'validate-only=1' "$WORK_DIR/fake-calls.log" >/dev/null
[ ! -e "$PAYLOAD_MARKER" ]

rm -f "$ACTIVATION_MARKER"
env "${CONTROL_ENV[@]}" bash "$ROOT/scripts/config-control.sh" apply \
  < "$WORK_DIR/valid.tar" > "$WORK_DIR/apply-valid.json"
jq -e '.schema == "nixome.config-apply.v1" and .status == "passed"' \
  "$WORK_DIR/apply-valid.json" >/dev/null
[ -s "$ACTIVATION_MARKER" ]
[ ! -e "$PAYLOAD_MARKER" ]

for scenario in invalid-eval build-failed test-failed; do
  rm -f "$ACTIVATION_MARKER"
  make_candidate "$scenario" "$WORK_DIR/$scenario.tar"
  if env "${CONTROL_ENV[@]}" bash "$ROOT/scripts/config-control.sh" apply \
    < "$WORK_DIR/$scenario.tar" > "$WORK_DIR/apply-$scenario.json"; then
    printf 'expected %s apply to fail\n' "$scenario" >&2
    exit 1
  fi
  jq -e '.schema == "nixome.config-apply.v1" and .status == "failed"' \
    "$WORK_DIR/apply-$scenario.json" >/dev/null
  [ ! -e "$ACTIVATION_MARKER" ]
done

mkdir -p "$WORK_DIR/current-system-source/bin" "$WORK_DIR/previous-system/bin"
cat > "$WORK_DIR/previous-system/bin/switch-to-configuration" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$1" > "$NIXOME_SYSTEM_STATE_DIR/rollback-switch-action"
ln -sfn "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)" "$NIXOME_HOST_CURRENT_SYSTEM_LINK"
EOF
chmod 0755 "$WORK_DIR/previous-system/bin/switch-to-configuration"
ln -sfn "$WORK_DIR/current-system-source" "$HOST_PROFILE"
printf '%s\n' "$WORK_DIR/previous-system" > "$SYSTEM_STATE_DIR/previous-host-system"

env "${CONTROL_ENV[@]}" bash "$ROOT/scripts/config-control.sh" rollback > "$WORK_DIR/rollback.json"
jq -e '
  .schema == "nixome.config-rollback.v1" and
  .status == "passed" and
  .method == "state" and
  .target_system == "'"$WORK_DIR"'/previous-system"
' "$WORK_DIR/rollback.json" >/dev/null
[ "$(readlink "$HOST_PROFILE")" = "$WORK_DIR/previous-system" ]
[ "$(readlink "$HOST_CURRENT")" = "$WORK_DIR/previous-system" ]
grep -qx switch "$SYSTEM_STATE_DIR/rollback-switch-action"

tar -C "$WORK_DIR" -cf "$WORK_DIR/bad.tar" valid.tar
if env "${CONTROL_ENV[@]}" bash "$ROOT/scripts/config-control.sh" check \
  < "$WORK_DIR/bad.tar" > "$WORK_DIR/bad.json" 2> "$WORK_DIR/bad.stderr"; then
  printf 'expected unexpected tar member to fail\n' >&2
  exit 1
fi
jq -e '.schema == "nixome.config-control.v1" and .status == "failed"' "$WORK_DIR/bad.json" >/dev/null

printf 'config control test ok\n'
