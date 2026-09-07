#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/closure-store-engine"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nixome-closure-store-engine.XXXXXX")"

cleanup() {
  chmod -R u+w "$WORK_DIR" 2>/dev/null || true
  rm -rf "$WORK_DIR"
}

trap cleanup EXIT

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR" "$WORK_DIR/physical-store" "$WORK_DIR/closures" "$WORK_DIR/bin"

POOL="$WORK_DIR/pool"
PHYSICAL_STORE="$WORK_DIR/physical-store"
ENGINE=(python3 -m nixome_qubes_storage.cli)
PYTHONPATH="$ROOT/pkgs/nixome-qubes-storage/src"
export PYTHONPATH
export PYTHONDONTWRITEBYTECODE=1

make_store_object() {
  entry="$1"
  content="$2"
  mkdir -p "$PHYSICAL_STORE/$entry/bin"
  printf '%s\n' "$content" > "$PHYSICAL_STORE/$entry/bin/payload"
  chmod 0555 "$PHYSICAL_STORE/$entry" "$PHYSICAL_STORE/$entry/bin"
  chmod 0444 "$PHYSICAL_STORE/$entry/bin/payload"
}

make_closure() {
  name="$1"
  shift
  mkdir -p "$WORK_DIR/closures/$name"
  {
    for entry in "$@"; do
      printf '/nix/store/%s\n' "$entry"
    done
  } > "$WORK_DIR/closures/$name/store-paths"
  printf '{"schema":"nixome.store-closure-manifest.v1","name":"%s","store_paths_file":"store-paths","read_only":true}\n' \
    "$name" > "$WORK_DIR/closures/$name/manifest.json"
}

run_engine_json() {
  name="$1"
  shift
  "${ENGINE[@]}" "$@" > "$RESULT_DIR/$name.json"
}

COMMON="11111111111111111111111111111111-common"
WORK_SYSTEM_1="22222222222222222222222222222222-work-system-1"
WORK_UNIQUE_1="33333333333333333333333333333333-work-unique-1"
WORK_SYSTEM_2="44444444444444444444444444444444-work-system-2"
WORK_UNIQUE_2="55555555555555555555555555555555-work-unique-2"
DEV_SYSTEM_1="66666666666666666666666666666666-dev-system-1"
DEV_UNIQUE_1="77777777777777777777777777777777-dev-unique-1"

make_store_object "$COMMON" common
make_store_object "$WORK_SYSTEM_1" work-system-1
make_store_object "$WORK_UNIQUE_1" work-unique-1
make_store_object "$WORK_SYSTEM_2" work-system-2
make_store_object "$WORK_UNIQUE_2" work-unique-2
make_store_object "$DEV_SYSTEM_1" dev-system-1
make_store_object "$DEV_UNIQUE_1" dev-unique-1

make_closure work-gen1 "$COMMON" "$WORK_SYSTEM_1" "$WORK_UNIQUE_1"
make_closure work-gen2 "$COMMON" "$WORK_SYSTEM_2" "$WORK_UNIQUE_2"
make_closure dev-gen1 "$COMMON" "$DEV_SYSTEM_1" "$DEV_UNIQUE_1"

run_engine_json work-gen1 admit \
  --pool-root "$POOL" \
  --domain work \
  --system "/nix/store/$WORK_SYSTEM_1" \
  --closure-source "$WORK_DIR/closures/work-gen1" \
  --physical-store-root "$PHYSICAL_STORE" \
  --root-source root-work-gen1 \
  --kernel-name kernel-a \
  --verify-mode tree \
  --additional-space-mib 1

jq -e '
  .schema == "nixome.closure-store.admission.v1" and
  .status == "passed" and
  .domain == "work" and
  .generation == 1 and
  .current_generation == 1 and
  .objects_total == 3 and
  .objects_admitted == 3 and
  .objects_reused == 0 and
  .read_only == true and
  .store.mode == "shared-closure" and
  .store.device == "/dev/xvde" and
  .store.mountpoint == "/nix/store" and
  .store.image_contains_materialized_store_copy == true
' "$RESULT_DIR/work-gen1.json" >/dev/null

WORK_GEN1_VIEW="$(jq -r '.store.view_dir' "$RESULT_DIR/work-gen1.json")"
WORK_GEN1_IMAGE="$(jq -r '.canonical_view_image' "$RESULT_DIR/work-gen1.json")"
[ -d "$WORK_GEN1_VIEW" ]
[ -f "$WORK_GEN1_IMAGE" ]
[ ! -w "$WORK_GEN1_IMAGE" ]

pool_inode="$(stat -c '%d:%i' "$POOL/objects/store/$COMMON/bin/payload")"
view_inode="$(stat -c '%d:%i' "$WORK_GEN1_VIEW/$COMMON/bin/payload")"
[ "$pool_inode" = "$view_inode" ]

run_engine_json dev-gen1 admit \
  --pool-root "$POOL" \
  --domain dev \
  --system "/nix/store/$DEV_SYSTEM_1" \
  --closure-source "$WORK_DIR/closures/dev-gen1" \
  --physical-store-root "$PHYSICAL_STORE" \
  --root-source root-dev-gen1 \
  --kernel-name kernel-a \
  --verify-mode tree \
  --additional-space-mib 1

jq -e '
  .domain == "dev" and
  .generation == 1 and
  .objects_total == 3 and
  .objects_admitted == 2 and
  .objects_reused == 1
' "$RESULT_DIR/dev-gen1.json" >/dev/null

DEV_GEN1_VIEW="$(jq -r '.store.view_dir' "$RESULT_DIR/dev-gen1.json")"
[ -e "$DEV_GEN1_VIEW/$COMMON/bin/payload" ]
[ ! -e "$DEV_GEN1_VIEW/$WORK_UNIQUE_1" ]

run_engine_json work-gen2 admit \
  --pool-root "$POOL" \
  --domain work \
  --system "/nix/store/$WORK_SYSTEM_2" \
  --closure-source "$WORK_DIR/closures/work-gen2" \
  --physical-store-root "$PHYSICAL_STORE" \
  --root-source root-work-gen2 \
  --kernel-name kernel-a \
  --verify-mode tree \
  --additional-space-mib 1 \
  --retain false

jq -e '
  .domain == "work" and
  .generation == 2 and
  .previous_generation == 1 and
  .current_generation == 2 and
  .objects_total == 3 and
  .objects_admitted == 2 and
  .objects_reused == 1
' "$RESULT_DIR/work-gen2.json" >/dev/null

run_engine_json rollback-work rollback \
  --pool-root "$POOL" \
  --domain work \
  --generation 1

jq -e '
  .schema == "nixome.closure-store.rollback.v1" and
  .domain == "work" and
  .previous_generation == 2 and
  .current_generation == 1
' "$RESULT_DIR/rollback-work.json" >/dev/null

run_engine_json gc gc \
  --pool-root "$POOL" \
  --keep-generations 0

jq -e '
  any(.removed_generations[]; .domain == "work" and .generation == 2) and
  (.removed_objects | index("'"$WORK_UNIQUE_2"'")) != null and
  (.removed_objects | index("'"$COMMON"'")) == null and
  (.removed_objects | index("'"$WORK_UNIQUE_1"'")) == null and
  (.removed_objects | index("'"$DEV_UNIQUE_1"'")) == null
' "$RESULT_DIR/gc.json" >/dev/null

ALT_STORE="$WORK_DIR/alternate-store"
mkdir -p "$ALT_STORE/$COMMON/bin"
printf 'corrupt\n' > "$ALT_STORE/$COMMON/bin/payload"
chmod -R a-w "$ALT_STORE/$COMMON"

set +e
"${ENGINE[@]}" admit \
  --pool-root "$POOL" \
  --domain corrupt \
  --system "/nix/store/$COMMON" \
  --closure-source "$WORK_DIR/closures/work-gen1" \
  --physical-store-root "$ALT_STORE" \
  --verify-mode tree \
  > "$RESULT_DIR/corrupt.json" 2> "$RESULT_DIR/corrupt.err"
rc="$?"
set -e
[ "$rc" != "0" ]
grep -F 'existing object hash mismatch' "$RESULT_DIR/corrupt.err" >/dev/null

mkdir -p "$POOL/tmp/stale"
printf '{"schema":"nixome.closure-store.transaction.v1","id":"stale","status":"preparing"}\n' \
  > "$POOL/transactions/stale.json"
ln -s generations/99 "$POOL/domains/work/.current.tmp-stale"

run_engine_json recover recover --pool-root "$POOL"

jq -e '
  .schema == "nixome.closure-store.recovery.v1" and
  (.aborted_transactions | length) >= 2 and
  any(.aborted_transactions[]; endswith("/transactions/stale.json"))
' "$RESULT_DIR/recover.json" >/dev/null
jq -e '.status == "aborted"' "$POOL/transactions/stale.json" >/dev/null
[ ! -e "$POOL/tmp/stale" ]
[ ! -e "$POOL/domains/work/.current.tmp-stale" ]

run_engine_json status status --pool-root "$POOL"
jq -e '
  .schema == "nixome.closure-store.status.v1" and
  .domains.work.current == 1 and
  .domains.dev.current == 1 and
  .object_count == 5
' "$RESULT_DIR/status.json" >/dev/null

printf 'closure-store engine test ok\n'
