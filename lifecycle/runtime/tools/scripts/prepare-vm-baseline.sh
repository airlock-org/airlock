#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
source "$ROOT/lifecycle/runtime/tools/scripts/lib/nixome-env.sh"

IMAGE_ATTR="${NIXOME_VM_IMAGE_ATTR:-nixome-vm-gui-config-image}"
BASELINE_DIR="${NIXOME_VM_BASELINE_DIR:-${NIXOME_DATA_ROOT:-$ROOT/.nixome}/vm-baselines/$IMAGE_ATTR-setup}"
BASELINE_LOG_DIR="${NIXOME_VM_BASELINE_LOG_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/vm-baseline}"
BASELINE_RESULT_LINK_DIR="${NIXOME_VM_BASELINE_RESULT_LINK_DIR:-$BASELINE_DIR/result-links}"
SHARED_IMAGE_LINK="${NIXOME_VM_SHARED_IMAGE_LINK:-$NIXOME_RESULT_LINK_DIR/${IMAGE_ATTR}-parallel-shared-result}"
BASELINE_DISK="${NIXOME_VM_BASELINE_DISK:-$BASELINE_DIR/$IMAGE_ATTR.qcow2}"
BASELINE_OVMF_VARS="${NIXOME_VM_BASELINE_OVMF_VARS:-$BASELINE_DIR/${IMAGE_ATTR}-OVMF_VARS.fd}"
BASELINE_META="$BASELINE_DIR/baseline.json"
BASELINE_JSON="$BASELINE_LOG_DIR/vm-baseline.json"
SETUP_CONFIG_JSON="$BASELINE_LOG_DIR/vm-gui-setup-config.json"
FORCE="${NIXOME_VM_BASELINE_FORCE:-0}"
SSH_PORT="${NIXOME_VM_BASELINE_SSH_PORT:-${NIXOME_VM_SSH_PORT:-2299}}"

mkdir -p "$BASELINE_DIR" "$BASELINE_LOG_DIR" "$BASELINE_RESULT_LINK_DIR"

case "$FORCE" in
  1|true|yes)
    force=1
    ;;
  0|false|no)
    force=0
    ;;
  *)
    printf 'error: NIXOME_VM_BASELINE_FORCE must be 0 or 1\n' >&2
    exit 1
    ;;
esac

if [ ! -e "$SHARED_IMAGE_LINK" ]; then
  printf 'error: shared VM image result link is missing: %s\n' "$SHARED_IMAGE_LINK" >&2
  printf 'run make vm-parallel-prebuild-image first\n' >&2
  exit 1
fi

source_image_output="$(readlink -f "$SHARED_IMAGE_LINK")"
if [ ! -d "$source_image_output" ]; then
  printf 'error: shared VM image output is missing: %s\n' "$source_image_output" >&2
  exit 1
fi

# The prepared baseline is an immutable qcow2 checkpoint. Parallel test VMs must
# create their own writable overlays backed by this disk; they must not boot it
# directly or mutate it in place.
ln -sfn "$source_image_output" "$BASELINE_DIR/${IMAGE_ATTR}-result"

if [ "$force" = "0" ] &&
  [ -f "$BASELINE_DISK" ] &&
  [ -f "$BASELINE_OVMF_VARS" ] &&
  [ -r "$BASELINE_META" ] &&
  jq -e \
    --arg image_attr "$IMAGE_ATTR" \
    --arg source_image_output "$source_image_output" \
    --arg disk "$BASELINE_DISK" \
    --arg ovmf_vars "$BASELINE_OVMF_VARS" \
    '
      .schema == "nixome.vm-baseline.v1" and
      .status == "passed" and
      .image_attr == $image_attr and
      .source_image_output == $source_image_output and
      .disk == $disk and
      .ovmf_vars == $ovmf_vars and
      .setup_config.schema == "nixome.setup-config.v1" and
      .setup_config.status == "passed"
    ' "$BASELINE_META" >/dev/null; then
  jq '. + {reused: true}' "$BASELINE_META" > "$BASELINE_JSON"
  printf 'VM baseline is current: %s\n' "$BASELINE_DISK"
  exit 0
fi

printf 'preparing VM baseline\n'
printf '  image attr: .#%s\n' "$IMAGE_ATTR"
printf '  source image: %s\n' "$source_image_output"
printf '  baseline dir: %s\n' "$BASELINE_DIR"
printf '  logs: %s\n' "$BASELINE_LOG_DIR"

start_epoch="$(date +%s)"
start_time="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

set +e
env \
  NIXOME_LOG_DIR="$BASELINE_LOG_DIR" \
  NIXOME_VM_DIR="$BASELINE_DIR" \
  NIXOME_RESULT_LINK_DIR="$BASELINE_RESULT_LINK_DIR" \
  NIXOME_VM_IMAGE_ATTR="$IMAGE_ATTR" \
  NIXOME_VM_DISPLAY="${NIXOME_VM_DISPLAY:-none}" \
  NIXOME_VM_SSH_PORT="$SSH_PORT" \
  NIXOME_VM_FRESH=1 \
  NIXOME_VM_SKIP_BASE_BUILD=1 \
  NIXOME_AUTO_GC_AFTER_RUN=0 \
  NIXOME_SHUTDOWN_VM_ON_SUCCESS=1 \
  NIXOME_REQUIRE_CLEAN_SHUTDOWN=1 \
  NIXOME_BOOTSTRAP_QUBES=0 \
  NIXOME_RUN_SETUP_CONFIG=1 \
  NIXOME_RUN_CONFIG_INPUTS_IMPORT=0 \
  NIXOME_RUN_CONFIG_VMS_SMOKE=0 \
  NIXOME_RUN_CONFIG_BOOT_VERIFY=0 \
  NIXOME_RUN_NIX_CLOSURE_SMOKE=0 \
  NIXOME_RUN_QUBES_SMOKE=0 \
  NIXOME_RUN_GUI_SMOKE=0 \
  "$ROOT/lifecycle/runtime/tools/scripts/run-nested-gui-smoke.sh"
run_rc="$?"
set -e

end_epoch="$(date +%s)"
end_time="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
duration_seconds=$((end_epoch - start_epoch))

if [ "$run_rc" -ne 0 ]; then
  jq -n \
    --arg image_attr "$IMAGE_ATTR" \
    --arg source_image_result_link "$SHARED_IMAGE_LINK" \
    --arg source_image_output "$source_image_output" \
    --arg disk "$BASELINE_DISK" \
    --arg ovmf_vars "$BASELINE_OVMF_VARS" \
    --arg started_at "$start_time" \
    --arg finished_at "$end_time" \
    --argjson duration_seconds "$duration_seconds" \
    --argjson exit_code "$run_rc" \
    '{
      schema: "nixome.vm-baseline.v1",
      status: "failed",
      reason: "baseline setup run failed",
      image_attr: $image_attr,
      source_image_result_link: $source_image_result_link,
      source_image_output: $source_image_output,
      disk: $disk,
      ovmf_vars: $ovmf_vars,
      started_at: $started_at,
      finished_at: $finished_at,
      duration_seconds: $duration_seconds,
      exit_code: $exit_code
    }' > "$BASELINE_JSON"
  printf 'error: VM baseline setup failed; see %s\n' "$BASELINE_LOG_DIR" >&2
  exit "$run_rc"
fi

if [ ! -f "$BASELINE_DISK" ]; then
  printf 'error: baseline disk was not created: %s\n' "$BASELINE_DISK" >&2
  exit 1
fi

if [ ! -f "$BASELINE_OVMF_VARS" ]; then
  printf 'error: baseline OVMF vars were not created: %s\n' "$BASELINE_OVMF_VARS" >&2
  exit 1
fi

if ! jq -e \
  '.schema == "nixome.setup-config.v1" and .status == "passed"' \
  "$SETUP_CONFIG_JSON" >/dev/null; then
  printf 'error: baseline setup-config did not pass; see %s\n' "$SETUP_CONFIG_JSON" >&2
  exit 1
fi

chmod a-w "$BASELINE_DISK" "$BASELINE_OVMF_VARS"

jq -n \
  --arg image_attr "$IMAGE_ATTR" \
  --arg source_image_result_link "$SHARED_IMAGE_LINK" \
  --arg source_image_output "$source_image_output" \
  --arg disk "$BASELINE_DISK" \
  --arg ovmf_vars "$BASELINE_OVMF_VARS" \
  --arg started_at "$start_time" \
  --arg finished_at "$end_time" \
  --argjson duration_seconds "$duration_seconds" \
  --slurpfile setup_config "$SETUP_CONFIG_JSON" \
  '{
    schema: "nixome.vm-baseline.v1",
    status: "passed",
    reason: "baseline setup completed",
    image_attr: $image_attr,
    source_image_result_link: $source_image_result_link,
    source_image_output: $source_image_output,
    disk: $disk,
    ovmf_vars: $ovmf_vars,
    started_at: $started_at,
    finished_at: $finished_at,
    duration_seconds: $duration_seconds,
    setup_config: $setup_config[0],
    reused: false
  }' > "$BASELINE_META"

cp "$BASELINE_META" "$BASELINE_JSON"
printf 'VM baseline ready: %s\n' "$BASELINE_DISK"
