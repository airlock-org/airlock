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

chmod -R u+w "$RESULT_DIR" 2>/dev/null || true
rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR" "$WORK_DIR/physical-store" "$WORK_DIR/closures" "$WORK_DIR/bin"

POOL="$WORK_DIR/pool"
PHYSICAL_STORE="$WORK_DIR/physical-store"
ENGINE=(python3 -m nixome_qubes_storage.cli)
PYTHONPATH="$ROOT/pkgs/nixome-qubes-storage/src"
export PYTHONPATH
export PYTHONDONTWRITEBYTECODE=1

if ! command -v mkfs.erofs >/dev/null 2>&1 || ! command -v fsck.erofs >/dev/null 2>&1; then
  EROFS_UTILS="$(
    while IFS= read -r output; do
      physical="$(nixome_physical_store_path "$output")"
      [ -x "$physical/bin/mkfs.erofs" ] && printf '%s\n' "$physical"
    done < <(nix build --no-link --print-out-paths nixpkgs#erofs-utils) |
      head -n 1
  )"
  PATH="$EROFS_UTILS/bin:$PATH"
fi

make_store_object() {
  entry="$1"
  content="$2"
  bytes="${3:-1024}"
  mkdir -p "$PHYSICAL_STORE/$entry/bin"
  python3 - "$PHYSICAL_STORE/$entry/bin/payload" "$content" "$bytes" <<'PY'
import sys
from pathlib import Path

target = Path(sys.argv[1])
content = (sys.argv[2] + "\n").encode()
size = int(sys.argv[3])
target.write_bytes((content * ((size // len(content)) + 1))[:size])
PY
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

admit() {
  name="$1"
  pool="$2"
  domain="$3"
  system="$4"
  closure="$5"
  target="${6:-32768}"
  shift 6
  run_engine_json "$name" admit \
    --pool-root "$pool" \
    --domain "$domain" \
    --system "/nix/store/$system" \
    --closure-source "$WORK_DIR/closures/$closure" \
    --physical-store-root "$PHYSICAL_STORE" \
    --root-source "root-$domain" \
    --kernel-name kernel-a \
    --verify-mode tree \
    --segment-target-bytes "$target" \
    "$@"
}

reconstruct_blob_view() {
  map="$1"
  output="$2"
  python3 - "$map" "$output" <<'PY'
import json
import sys
from pathlib import Path

blob_map = json.loads(Path(sys.argv[1]).read_text())
output = Path(sys.argv[2])
segments = {item["id"]: Path(item["blob"]) for item in blob_map["segments"]}
handles = {}
try:
    with output.open("wb") as target:
        for extent in blob_map["extents"]:
            segment_id = extent["segment_id"]
            if segment_id not in handles:
                handles[segment_id] = segments[segment_id].open("rb")
            source = handles[segment_id]
            source.seek(int(extent["segment_offset"]))
            data = source.read(int(extent["length"]))
            if len(data) != int(extent["length"]):
                raise SystemExit("segment extent is truncated")
            target.seek(int(extent["logical_offset"]))
            target.write(data)
        target.truncate(int(blob_map["logical_size"]))
finally:
    for handle in handles.values():
        handle.close()
PY
}

object_segment() {
  pool="$1"
  entry="$2"
  jq -r '.segment_id' "$pool/objects/blob-metadata/$entry.json"
}

COMMON="11111111111111111111111111111111-common"
WORK_SYSTEM_1="22222222222222222222222222222222-work-system-1"
WORK_UNIQUE_1="33333333333333333333333333333333-work-unique-1"
WORK_SYSTEM_2="44444444444444444444444444444444-work-system-2"
WORK_UNIQUE_2="55555555555555555555555555555555-work-unique-2"
DEV_SYSTEM_1="66666666666666666666666666666666-dev-system-1"
DEV_UNIQUE_1="77777777777777777777777777777777-dev-unique-1"
SYMLINK_TARGET="88888888888888888888888888888888-symlink-target"
SYMLINK_SYSTEM="99999999999999999999999999999999-symlink-system"
BIG_A="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-big-a"
BIG_B="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-big-b"
BIG_C="cccccccccccccccccccccccccccccccc-big-c"
SMALL_A="dddddddddddddddddddddddddddddddd-small-a"
SMALL_B="eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-small-b"
SMALL_C="ffffffffffffffffffffffffffffffff-small-c"
OVERSIZED="12121212121212121212121212121212-oversized"

make_store_object "$COMMON" common 1024
make_store_object "$WORK_SYSTEM_1" work-system-1 12000
make_store_object "$WORK_UNIQUE_1" work-unique-1 1024
make_store_object "$WORK_SYSTEM_2" work-system-2 12000
make_store_object "$WORK_UNIQUE_2" work-unique-2 1024
make_store_object "$DEV_SYSTEM_1" dev-system-1 12000
make_store_object "$DEV_UNIQUE_1" dev-unique-1 1024
make_store_object "$SYMLINK_TARGET" symlink-target 1024
make_store_object "$BIG_A" big-a 18000
make_store_object "$BIG_B" big-b 18000
make_store_object "$BIG_C" big-c 18000
make_store_object "$SMALL_A" small-a 1024
make_store_object "$SMALL_B" small-b 1024
make_store_object "$SMALL_C" small-c 1024
make_store_object "$OVERSIZED" oversized 64000
ln -s "$SYMLINK_TARGET" "$PHYSICAL_STORE/$SYMLINK_SYSTEM"

make_closure work-gen1 "$COMMON" "$WORK_SYSTEM_1" "$WORK_UNIQUE_1"
make_closure work-gen2 "$COMMON" "$WORK_SYSTEM_2" "$WORK_UNIQUE_2"
make_closure dev-gen1 "$COMMON" "$DEV_SYSTEM_1" "$DEV_UNIQUE_1"
make_closure symlink-gen "$SYMLINK_TARGET" "$SYMLINK_SYSTEM"
make_closure fit "$SMALL_A" "$SMALL_B" "$SMALL_C"
make_closure rollover "$BIG_A" "$BIG_B"
make_closure oversized "$OVERSIZED"
make_closure multi-segment "$COMMON" "$BIG_A" "$BIG_B" "$BIG_C"

cat > "$WORK_DIR/bin/nix" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$@" > "$NIXOME_FAKE_NIX_LOG"
EOF
chmod 0755 "$WORK_DIR/bin/nix"

PATH="$WORK_DIR/bin:$PATH" \
  NIXOME_FAKE_NIX_LOG="$RESULT_DIR/nix-verify.args" \
  run_engine_json verify-command admit \
  --pool-root "$WORK_DIR/verify-pool" \
  --domain verify-command \
  --system "/nix/store/$COMMON" \
  --closure-source "$WORK_DIR/closures/work-gen1" \
  --physical-store-root "$PHYSICAL_STORE" \
  --root-source root-verify \
  --kernel-name kernel-a \
  --verify-mode nix-store

jq -e '
  .schema == "nixome.closure-store.admission.v1" and
  .status == "passed" and
  .verification.mode == "nix-store" and
  (.verification.command | index("--extra-experimental-features")) != null and
  (.verification.command | index("nix-command")) != null
' "$RESULT_DIR/verify-command.json" >/dev/null
grep -Fx -- '--extra-experimental-features' "$RESULT_DIR/nix-verify.args" >/dev/null
grep -Fx -- 'nix-command' "$RESULT_DIR/nix-verify.args" >/dev/null

FIT_POOL="$WORK_DIR/fit-pool"
admit fit "$FIT_POOL" fit "$SMALL_A" fit 1048576
FIT_SEGMENT="$(object_segment "$FIT_POOL" "$SMALL_A")"
[ "$(object_segment "$FIT_POOL" "$SMALL_B")" = "$FIT_SEGMENT" ]
[ "$(object_segment "$FIT_POOL" "$SMALL_C")" = "$FIT_SEGMENT" ]
jq -e '.segment_count == 1 and .open_segment_count == 1 and .sealed_segment_count == 0' \
  < <("${ENGINE[@]}" status --pool-root "$FIT_POOL") >/dev/null

ROLLOVER_POOL="$WORK_DIR/rollover-pool"
admit rollover "$ROLLOVER_POOL" rollover "$BIG_A" rollover 32768
[ "$(object_segment "$ROLLOVER_POOL" "$BIG_A")" != "$(object_segment "$ROLLOVER_POOL" "$BIG_B")" ]
jq -e '
  .segment_count >= 2 and
  .sealed_segment_count >= 1 and
  all(.segments[]; .used_bytes <= .target_bytes or .oversized == true)
' < <("${ENGINE[@]}" status --pool-root "$ROLLOVER_POOL") >/dev/null

OVERSIZED_POOL="$WORK_DIR/oversized-pool"
admit oversized "$OVERSIZED_POOL" oversized "$OVERSIZED" oversized 4096
OVERSIZED_SEGMENT="$(object_segment "$OVERSIZED_POOL" "$OVERSIZED")"
jq -e --arg id "$OVERSIZED_SEGMENT" '
  any(.segments[]; .id == $id and .sealed == true and .oversized == true and .used_bytes > .target_bytes)
' < <("${ENGINE[@]}" status --pool-root "$OVERSIZED_POOL") >/dev/null

SEALED_HASH_BEFORE="$(sha256sum "$OVERSIZED_POOL/objects/segments/$OVERSIZED_SEGMENT.blob" | awk '{print $1}')"
admit oversized-later "$OVERSIZED_POOL" later "$SMALL_A" fit 4096
SEALED_HASH_AFTER="$(sha256sum "$OVERSIZED_POOL/objects/segments/$OVERSIZED_SEGMENT.blob" | awk '{print $1}')"
[ "$SEALED_HASH_BEFORE" = "$SEALED_HASH_AFTER" ]

admit work-gen1 "$POOL" work "$WORK_SYSTEM_1" work-gen1 32768
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
  .store.blob_device == "/dev/xvdf" and
  .store.mountpoint == "/nix/store" and
  .store.filesystem == "erofs" and
  .store.view_backend == "erofs-objectmap" and
  .store.image_contains_materialized_store_copy == false and
  .store.segment_target_bytes == 32768 and
  .filesystem == "erofs" and
  .view_backend == "erofs-objectmap" and
  .object_data_bytes > 0 and
  .object_extent_count == 4
' "$RESULT_DIR/work-gen1.json" >/dev/null

WORK_GEN1_VIEW="$(jq -r '.store.view_dir' "$RESULT_DIR/work-gen1.json")"
WORK_GEN1_IMAGE="$(jq -r '.canonical_metadata_image' "$RESULT_DIR/work-gen1.json")"
WORK_GEN1_BLOB_MAP="$(jq -r '.canonical_blob_map' "$RESULT_DIR/work-gen1.json")"
WORK_GEN1_LOGICAL_BLOB_SIZE="$(jq -r '.blob_logical_size' "$RESULT_DIR/work-gen1.json")"
[ -d "$WORK_GEN1_VIEW" ]
[ -f "$WORK_GEN1_IMAGE" ]
[ -f "$WORK_GEN1_BLOB_MAP" ]
[ ! -w "$WORK_GEN1_IMAGE" ]
[ ! -e "$POOL/objects/data.blob" ]
jq -e --arg object "$COMMON" '
  .view_backend == "erofs-objectmap" and
  .external_data_format == "posix-tar-index" and
  has("segments") and
  (has("central_blob") | not) and
  (.objects | index($object)) != null and
  any(.extents[]; .object == $object and .segment_id and .segment_offset >= 0 and .length > 0)
' "$WORK_GEN1_BLOB_MAP" >/dev/null

WORK_GEN1_LOGICAL_BLOB="$RESULT_DIR/work-gen1.blob"
WORK_GEN1_EXTRACT="$RESULT_DIR/work-gen1-extract"
mkdir -p "$WORK_GEN1_EXTRACT"
reconstruct_blob_view "$WORK_GEN1_BLOB_MAP" "$WORK_GEN1_LOGICAL_BLOB"
fsck.erofs --device="$WORK_GEN1_LOGICAL_BLOB" --extract="$WORK_GEN1_EXTRACT" "$WORK_GEN1_IMAGE" \
  > "$RESULT_DIR/work-gen1-fsck.log"
[ -f "$WORK_GEN1_EXTRACT/$COMMON/bin/payload" ]
[ -f "$WORK_GEN1_EXTRACT/$WORK_SYSTEM_1/bin/payload" ]
[ -f "$WORK_GEN1_EXTRACT/$WORK_UNIQUE_1/bin/payload" ]
[ ! -e "$WORK_GEN1_EXTRACT/$DEV_UNIQUE_1" ]

SEGMENT_BYTES_AFTER_WORK_GEN1="$(jq -r '.segment_bytes_total' < <("${ENGINE[@]}" status --pool-root "$POOL"))"

admit dev-gen1 "$POOL" dev "$DEV_SYSTEM_1" dev-gen1 32768
jq -e '
  .domain == "dev" and
  .generation == 1 and
  .objects_total == 3 and
  .objects_admitted == 2 and
  .objects_reused == 1 and
  .object_data_bytes > 0
' "$RESULT_DIR/dev-gen1.json" >/dev/null

DEV_GEN1_IMAGE="$(jq -r '.canonical_metadata_image' "$RESULT_DIR/dev-gen1.json")"
DEV_GEN1_BLOB_MAP="$(jq -r '.canonical_blob_map' "$RESULT_DIR/dev-gen1.json")"
DEV_GEN1_LOGICAL_BLOB="$RESULT_DIR/dev-gen1.blob"
DEV_GEN1_EXTRACT="$RESULT_DIR/dev-gen1-extract"
mkdir -p "$DEV_GEN1_EXTRACT"
reconstruct_blob_view "$DEV_GEN1_BLOB_MAP" "$DEV_GEN1_LOGICAL_BLOB"
fsck.erofs --device="$DEV_GEN1_LOGICAL_BLOB" --extract="$DEV_GEN1_EXTRACT" "$DEV_GEN1_IMAGE" \
  > "$RESULT_DIR/dev-gen1-fsck.log"
[ -f "$DEV_GEN1_EXTRACT/$COMMON/bin/payload" ]
[ -f "$DEV_GEN1_EXTRACT/$DEV_SYSTEM_1/bin/payload" ]
[ -f "$DEV_GEN1_EXTRACT/$DEV_UNIQUE_1/bin/payload" ]
[ ! -e "$DEV_GEN1_EXTRACT/$WORK_UNIQUE_1" ]
SEGMENT_BYTES_AFTER_DEV_GEN1="$(jq -r '.segment_bytes_total' < <("${ENGINE[@]}" status --pool-root "$POOL"))"
DEV_LOGICAL_BLOB_SIZE="$(jq -r '.blob_logical_size' "$RESULT_DIR/dev-gen1.json")"
DEV_PHYSICAL_BYTES_ADDED="$((SEGMENT_BYTES_AFTER_DEV_GEN1 - SEGMENT_BYTES_AFTER_WORK_GEN1))"
[ "$DEV_PHYSICAL_BYTES_ADDED" -lt "$DEV_LOGICAL_BLOB_SIZE" ]
COMMON_WORK_SEGMENT="$(jq -r --arg object "$COMMON" '.extents[] | select(.object == $object) | .segment_id' "$WORK_GEN1_BLOB_MAP")"
COMMON_DEV_SEGMENT="$(jq -r --arg object "$COMMON" '.extents[] | select(.object == $object) | .segment_id' "$DEV_GEN1_BLOB_MAP")"
COMMON_WORK_OFFSET="$(jq -r --arg object "$COMMON" '.extents[] | select(.object == $object) | .segment_offset' "$WORK_GEN1_BLOB_MAP")"
COMMON_DEV_OFFSET="$(jq -r --arg object "$COMMON" '.extents[] | select(.object == $object) | .segment_offset' "$DEV_GEN1_BLOB_MAP")"
COMMON_WORK_SHA="$(jq -r --arg object "$COMMON" '.extents[] | select(.object == $object) | .object_sha256' "$WORK_GEN1_BLOB_MAP")"
COMMON_DEV_SHA="$(jq -r --arg object "$COMMON" '.extents[] | select(.object == $object) | .object_sha256' "$DEV_GEN1_BLOB_MAP")"
[ "$COMMON_WORK_SEGMENT" = "$COMMON_DEV_SEGMENT" ]
[ "$COMMON_WORK_OFFSET" = "$COMMON_DEV_OFFSET" ]
[ "$COMMON_WORK_SHA" = "$COMMON_DEV_SHA" ]
jq -e --arg object "$WORK_UNIQUE_1" 'all(.extents[]; .object != $object)' "$DEV_GEN1_BLOB_MAP" >/dev/null
jq -e --arg object "$DEV_UNIQUE_1" 'all(.extents[]; .object != $object)' "$WORK_GEN1_BLOB_MAP" >/dev/null

admit symlink-gen "$POOL" symlink-domain "$SYMLINK_SYSTEM" symlink-gen 32768
jq -e '.domain == "symlink-domain" and .generation == 1 and .objects_total == 2 and .objects_admitted == 2' \
  "$RESULT_DIR/symlink-gen.json" >/dev/null
SYMLINK_GEN_IMAGE="$(jq -r '.canonical_metadata_image' "$RESULT_DIR/symlink-gen.json")"
SYMLINK_GEN_BLOB_MAP="$(jq -r '.canonical_blob_map' "$RESULT_DIR/symlink-gen.json")"
SYMLINK_GEN_LOGICAL_BLOB="$RESULT_DIR/symlink-gen.blob"
SYMLINK_GEN_EXTRACT="$RESULT_DIR/symlink-gen-extract"
mkdir -p "$SYMLINK_GEN_EXTRACT"
reconstruct_blob_view "$SYMLINK_GEN_BLOB_MAP" "$SYMLINK_GEN_LOGICAL_BLOB"
fsck.erofs --device="$SYMLINK_GEN_LOGICAL_BLOB" --extract="$SYMLINK_GEN_EXTRACT" "$SYMLINK_GEN_IMAGE" \
  > "$RESULT_DIR/symlink-gen-fsck.log"
[ -L "$SYMLINK_GEN_EXTRACT/$SYMLINK_SYSTEM" ]
[ "$(readlink "$SYMLINK_GEN_EXTRACT/$SYMLINK_SYSTEM")" = "$SYMLINK_TARGET" ]
SEGMENT_BYTES_AFTER_SYMLINK_GEN="$(jq -r '.segment_bytes_total' < <("${ENGINE[@]}" status --pool-root "$POOL"))"

admit work-gen2 "$POOL" work "$WORK_SYSTEM_2" work-gen2 32768 --retain false
SEGMENT_BYTES_AFTER_WORK_GEN2="$(jq -r '.segment_bytes_total' < <("${ENGINE[@]}" status --pool-root "$POOL"))"
WORK_GEN2_LOGICAL_BLOB_SIZE="$(jq -r '.blob_logical_size' "$RESULT_DIR/work-gen2.json")"
WORK_GEN2_PHYSICAL_BYTES_ADDED="$((SEGMENT_BYTES_AFTER_WORK_GEN2 - SEGMENT_BYTES_AFTER_SYMLINK_GEN))"
jq -e '
  .domain == "work" and
  .generation == 2 and
  .previous_generation == 1 and
  .current_generation == 2 and
  .objects_total == 3 and
  .objects_admitted == 2 and
  .objects_reused == 1 and
  .object_data_bytes > 0
' "$RESULT_DIR/work-gen2.json" >/dev/null
[ "$WORK_GEN2_PHYSICAL_BYTES_ADDED" -lt "$WORK_GEN2_LOGICAL_BLOB_SIZE" ]
WORK_GEN2_BLOB_MAP="$(jq -r '.canonical_blob_map' "$RESULT_DIR/work-gen2.json")"
COMMON_WORK_GEN2_SEGMENT="$(jq -r --arg object "$COMMON" '.extents[] | select(.object == $object) | .segment_id' "$WORK_GEN2_BLOB_MAP")"
COMMON_WORK_GEN2_OFFSET="$(jq -r --arg object "$COMMON" '.extents[] | select(.object == $object) | .segment_offset' "$WORK_GEN2_BLOB_MAP")"
[ "$COMMON_WORK_SEGMENT" = "$COMMON_WORK_GEN2_SEGMENT" ]
[ "$COMMON_WORK_OFFSET" = "$COMMON_WORK_GEN2_OFFSET" ]

MULTI_POOL="$WORK_DIR/multi-pool"
admit multi-segment "$MULTI_POOL" multi "$BIG_A" multi-segment 32768
MULTI_MAP="$(jq -r '.canonical_blob_map' "$RESULT_DIR/multi-segment.json")"
jq -e '.segment_count > 1 and ([.extents[].segment_id] | unique | length) > 1' "$MULTI_MAP" >/dev/null

run_engine_json mark-running-work-gen2-store mark-running \
  --pool-root "$POOL" \
  --domain work \
  --generation 2 \
  --volume store
run_engine_json mark-running-work-gen2-store-data mark-running \
  --pool-root "$POOL" \
  --domain work \
  --generation 2 \
  --volume store-data
jq -e '.status == "running" and .domain == "work" and .generation == 2 and .volume == "store"' \
  "$RESULT_DIR/mark-running-work-gen2-store.json" >/dev/null

run_engine_json rollback-work rollback \
  --pool-root "$POOL" \
  --domain work \
  --generation 1
jq -e '.schema == "nixome.closure-store.rollback.v1" and .domain == "work" and .previous_generation == 2 and .current_generation == 1' \
  "$RESULT_DIR/rollback-work.json" >/dev/null

run_engine_json gc-running gc \
  --pool-root "$POOL" \
  --keep-generations 0
jq -e '
  any(.running_generations[]; .domain == "work" and .generation == 2) and
  any(.retained_generations[]; .domain == "work" and .generation == 2) and
  all(.removed_generations[]; .domain != "work" or .generation != 2) and
  (.removed_object_blobs | index("'"$WORK_UNIQUE_2"'")) == null and
  .segment_store_append_only == true
' "$RESULT_DIR/gc-running.json" >/dev/null

run_engine_json clear-running-work-gen2-store clear-running \
  --pool-root "$POOL" \
  --domain work \
  --generation 2 \
  --volume store
run_engine_json clear-running-work-gen2-store-data clear-running \
  --pool-root "$POOL" \
  --domain work \
  --generation 2 \
  --volume store-data

run_engine_json retain-work-gen2 set-retention \
  --pool-root "$POOL" \
  --domain work \
  --generation 2 \
  --retain true
jq -e '.status == "passed" and .previous_retain == false and .retain == true' "$RESULT_DIR/retain-work-gen2.json" >/dev/null

run_engine_json unretain-work-gen2 set-retention \
  --pool-root "$POOL" \
  --domain work \
  --generation 2 \
  --retain false
jq -e '.status == "passed" and .previous_retain == true and .retain == false' "$RESULT_DIR/unretain-work-gen2.json" >/dev/null

run_engine_json gc gc \
  --pool-root "$POOL" \
  --keep-generations 0
jq -e '
  any(.removed_generations[]; .domain == "work" and .generation == 2) and
  (.removed_object_blobs | index("'"$WORK_UNIQUE_2"'")) != null and
  (.removed_object_blobs | index("'"$COMMON"'")) == null and
  (.removed_object_blobs | index("'"$WORK_UNIQUE_1"'")) == null and
  (.removed_object_blobs | index("'"$DEV_UNIQUE_1"'")) == null and
  (.removed_views | length) >= 1 and
  .segment_compaction == "not implemented"
' "$RESULT_DIR/gc.json" >/dev/null

DEAD_SEG_POOL="$WORK_DIR/dead-segment-pool"
make_closure dead-gen1 "$OVERSIZED"
make_closure dead-gen2 "$BIG_A"
admit dead-gen1 "$DEAD_SEG_POOL" dead "$OVERSIZED" dead-gen1 4096 --retain false
DEAD_SEGMENT="$(object_segment "$DEAD_SEG_POOL" "$OVERSIZED")"
admit dead-gen2 "$DEAD_SEG_POOL" dead "$BIG_A" dead-gen2 4096 --retain false
run_engine_json gc-dead-segment gc --pool-root "$DEAD_SEG_POOL" --keep-generations 0
jq -e --arg segment "$DEAD_SEGMENT" '(.removed_segments | index($segment)) != null' "$RESULT_DIR/gc-dead-segment.json" >/dev/null
[ ! -e "$DEAD_SEG_POOL/objects/segments/$DEAD_SEGMENT.blob" ]

PARTIAL_SEG_POOL="$WORK_DIR/partial-segment-pool"
make_closure partial-gen1 "$SMALL_A" "$SMALL_B"
make_closure partial-gen2 "$SMALL_B"
admit partial-gen1 "$PARTIAL_SEG_POOL" partial "$SMALL_A" partial-gen1 1048576 --retain false
PARTIAL_SEGMENT="$(object_segment "$PARTIAL_SEG_POOL" "$SMALL_A")"
admit partial-gen2 "$PARTIAL_SEG_POOL" partial "$SMALL_B" partial-gen2 1048576 --retain false
run_engine_json gc-partial-segment gc --pool-root "$PARTIAL_SEG_POOL" --keep-generations 0
jq -e --arg segment "$PARTIAL_SEGMENT" '
  (.removed_segments | index($segment)) == null and
  (.partially_live_segments | index($segment)) != null
' "$RESULT_DIR/gc-partial-segment.json" >/dev/null
[ -e "$PARTIAL_SEG_POOL/objects/segments/$PARTIAL_SEGMENT.blob" ]

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

MIGRATION_POOL="$WORK_DIR/migration-pool"
mkdir -p "$MIGRATION_POOL/objects/blob-metadata" "$MIGRATION_POOL/objects/metadata"
printf 'legacy-payload' > "$MIGRATION_POOL/objects/data.blob"
LEGACY_SHA="$(sha256sum "$MIGRATION_POOL/objects/data.blob" | awk '{print $1}')"
LEGACY_SIZE="$(stat -c %s "$MIGRATION_POOL/objects/data.blob")"
LEGACY_ENTRY="abababababababababababababababab-legacy"
jq -n \
  --arg path "/nix/store/$LEGACY_ENTRY" \
  --arg entry "$LEGACY_ENTRY" \
  --arg blob "$MIGRATION_POOL/objects/data.blob" \
  --arg sha "$LEGACY_SHA" \
  --argjson size "$LEGACY_SIZE" \
  '{schema:"nixome.closure-store.object-tar-fragment.v1", path:$path, entry:$entry, tree_hash:"legacy", format:"posix-tar-fragment-v1", central_blob:$blob, offset:0, size:$size, sha256:$sha}' \
  > "$MIGRATION_POOL/objects/blob-metadata/$LEGACY_ENTRY.json"
jq -n \
  --arg path "/nix/store/$LEGACY_ENTRY" \
  --arg entry "$LEGACY_ENTRY" \
  --arg blob "$MIGRATION_POOL/objects/data.blob" \
  --arg sha "$LEGACY_SHA" \
  --argjson size "$LEGACY_SIZE" \
  '{schema:"nixome.closure-store.object.v2", path:$path, entry:$entry, tree_hash:"legacy", tar_fragment:{schema:"nixome.closure-store.object-tar-fragment.v1", central_blob:$blob, offset:0, size:$size, sha256:$sha, format:"posix-tar-fragment-v1"}}' \
  > "$MIGRATION_POOL/objects/metadata/$LEGACY_ENTRY.json"
mkdir -p "$MIGRATION_POOL/views/by-closure/legacy"
jq -n \
  --arg blob "$MIGRATION_POOL/objects/data.blob" \
  --arg object "$LEGACY_ENTRY" \
  --arg sha "$LEGACY_SHA" \
  --argjson size "$LEGACY_SIZE" \
  '{schema:"nixome.closure-store.blob-map.v1", view_backend:"erofs-objectmap", external_data_format:"posix-tar-index", central_blob:$blob, logical_size:$size, objects:[$object], extents:[{logical_offset:0, central_offset:0, length:$size, object:$object, object_sha256:$sha}]}' \
  > "$MIGRATION_POOL/views/by-closure/legacy/store.blob-map.json"
run_engine_json migrate-status status --pool-root "$MIGRATION_POOL"
[ ! -e "$MIGRATION_POOL/objects/data.blob" ]
[ -e "$MIGRATION_POOL/objects/segments/000001.blob" ]
jq -e '.segment_id == "000001" and .segment_offset == 0 and (has("central_blob") | not)' \
  "$MIGRATION_POOL/objects/blob-metadata/$LEGACY_ENTRY.json" >/dev/null
jq -e '
  (has("central_blob") | not) and
  .segment_count == 1 and
  .segments[0].id == "000001" and
  .extents[0].segment_id == "000001" and
  .extents[0].segment_offset == 0 and
  (.extents[0] | has("central_offset") | not)
' "$MIGRATION_POOL/views/by-closure/legacy/store.blob-map.json" >/dev/null

run_engine_json status status --pool-root "$POOL"
jq -e '
  .schema == "nixome.closure-store.status.v1" and
  .domains.work.current == 1 and
  .domains.dev.current == 1 and
  .domains."symlink-domain".current == 1 and
  (.running | length) == 0 and
  .object_count == 7 and
  .object_blob_count == 7 and
  .object_segment_count == 7 and
  .segment_count > 0 and
  .segment_bytes_total > 0
' "$RESULT_DIR/status.json" >/dev/null

jq -n \
  --arg schema "nixome.closure-store.physical-sharing.v2" \
  --arg status "passed" \
  --arg common_object "$COMMON" \
  --arg common_segment "$COMMON_WORK_SEGMENT" \
  --argjson common_offset "$COMMON_WORK_OFFSET" \
  --arg common_sha "$COMMON_WORK_SHA" \
  --argjson segment_bytes_total "$(jq -r '.segment_bytes_total' "$RESULT_DIR/status.json")" \
  --argjson work_gen1_logical_blob_size "$WORK_GEN1_LOGICAL_BLOB_SIZE" \
  --argjson work_gen1_object_data_bytes "$(jq -r '.object_data_bytes' "$RESULT_DIR/work-gen1.json")" \
  --argjson dev_gen1_logical_blob_size "$DEV_LOGICAL_BLOB_SIZE" \
  --argjson dev_gen1_physical_bytes_added "$DEV_PHYSICAL_BYTES_ADDED" \
  --argjson dev_gen1_object_data_bytes "$(jq -r '.object_data_bytes' "$RESULT_DIR/dev-gen1.json")" \
  --argjson work_gen2_logical_blob_size "$WORK_GEN2_LOGICAL_BLOB_SIZE" \
  --argjson work_gen2_physical_bytes_added "$WORK_GEN2_PHYSICAL_BYTES_ADDED" \
  --argjson work_gen2_object_data_bytes "$(jq -r '.object_data_bytes' "$RESULT_DIR/work-gen2.json")" \
  '{
    schema: $schema,
    status: $status,
    segment_bytes_total: $segment_bytes_total,
    shared_object: {
      object: $common_object,
      segment_id: $common_segment,
      segment_offset: $common_offset,
      sha256: $common_sha,
      same_extent_in_work_dev_and_work_gen2: true
    },
    work_gen1: {
      logical_blob_size: $work_gen1_logical_blob_size,
      object_data_bytes: $work_gen1_object_data_bytes
    },
    dev_gen1: {
      logical_blob_size: $dev_gen1_logical_blob_size,
      physical_bytes_added: $dev_gen1_physical_bytes_added,
      object_data_bytes: $dev_gen1_object_data_bytes,
      sharing_verified: ($dev_gen1_physical_bytes_added < $dev_gen1_logical_blob_size)
    },
    work_gen2: {
      logical_blob_size: $work_gen2_logical_blob_size,
      physical_bytes_added: $work_gen2_physical_bytes_added,
      object_data_bytes: $work_gen2_object_data_bytes,
      sharing_verified: ($work_gen2_physical_bytes_added < $work_gen2_logical_blob_size)
    }
  }' > "$RESULT_DIR/physical-sharing.json"

printf 'closure-store engine test ok\n'
