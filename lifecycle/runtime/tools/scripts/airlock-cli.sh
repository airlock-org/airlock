#!/usr/bin/env bash
set -euo pipefail

TARGET="${AIRLOCK_QREXEC_TARGET:-dom0}"
STATUS_SERVICE="${AIRLOCK_STATUS_SERVICE:-airlock.Config.Status}"
CHECK_SERVICE="${AIRLOCK_CHECK_SERVICE:-airlock.Config.Check}"
APPLY_SERVICE="${AIRLOCK_APPLY_SERVICE:-airlock.Config.Apply}"
ROLLBACK_SERVICE="${AIRLOCK_ROLLBACK_SERVICE:-airlock.Config.Rollback}"
QREXEC_CLIENT="${AIRLOCK_QREXEC_CLIENT:-qrexec-client-vm}"

usage() {
  cat >&2 <<'EOF'
Usage: airlock (status|check|apply|rollback)

Submit committed Airlock candidates to dom0 over narrow qrexec services.
EOF
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'missing command: %s\n' "$1" >&2
    exit 1
  fi
}

run_qrexec() {
  local service="$1"
  shift

  "$QREXEC_CLIENT" "$TARGET" "$service" "$@"
}

status_json_or_null() {
  local tmp

  tmp="$(mktemp)"
  if run_qrexec "$STATUS_SERVICE" > "$tmp" 2>/dev/null && jq -e . "$tmp" >/dev/null 2>&1; then
    cat "$tmp"
  else
    printf 'null'
  fi
  rm -f "$tmp"
}

candidate_tar() {
  local repo
  local tmpdir
  local dirty
  local commit
  local tree
  local branch
  local base_generation

  repo="$(git rev-parse --show-toplevel)"
  cd "$repo"

  dirty="$(git status --porcelain)"
  if [ -n "$dirty" ]; then
    printf 'Airlock candidate worktree is dirty; commit changes before submitting:\n%s\n' "$dirty" >&2
    exit 1
  fi

  commit="$(git rev-parse HEAD)"
  tree="$(git rev-parse 'HEAD^{tree}')"
  branch="$(git symbolic-ref --quiet --short HEAD || true)"
  base_generation="$(status_json_or_null)"
  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' RETURN

  if [ -n "$branch" ]; then
    git bundle create "$tmpdir/config.bundle" HEAD "refs/heads/$branch" >/dev/null
  else
    git bundle create "$tmpdir/config.bundle" HEAD >/dev/null
  fi

  jq -c -n \
    --arg schema "airlock.config-candidate.v1" \
    --arg commit "$commit" \
    --arg tree "$tree" \
    --arg branch "$branch" \
    --arg source_role "dev" \
    --arg transport "qrexec" \
    --argjson base_generation "$base_generation" \
    '{
      schema: $schema,
      commit: $commit,
      tree: $tree,
      branch: $branch,
      source_role: $source_role,
      transport: $transport,
      base_generation: $base_generation,
      requested_targets: ["airlock"]
    }' > "$tmpdir/manifest.json"

  tar -C "$tmpdir" -cf - config.bundle manifest.json
}

main() {
  local command="${1:-}"

  case "$command" in
    status)
      require_command "$QREXEC_CLIENT"
      run_qrexec "$STATUS_SERVICE"
      ;;
    check)
      require_command git
      require_command jq
      require_command tar
      require_command "$QREXEC_CLIENT"
      candidate_tar | run_qrexec "$CHECK_SERVICE"
      ;;
    apply)
      require_command git
      require_command jq
      require_command tar
      require_command "$QREXEC_CLIENT"
      candidate_tar | run_qrexec "$APPLY_SERVICE"
      ;;
    rollback)
      require_command "$QREXEC_CLIENT"
      run_qrexec "$ROLLBACK_SERVICE"
      ;;
    --help|-h|"")
      usage
      ;;
    *)
      usage
      ;;
  esac
}

main "$@"
