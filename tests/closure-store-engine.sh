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
  mkdir -p "$PHYSICAL_STORE/$entry/bin"
  python3 - "$PHYSICAL_STORE/$entry/bin/payload" "$content" <<'PY'
import sys
from pathlib import Path

Path(sys.argv[1]).write_bytes((sys.argv[2] + "\n").encode() * 65536)
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

reconstruct_blob_view() {
  map="$1"
  output="$2"
  python3 - "$map" "$output" <<'PY'
import json
import sys
from pathlib import Path

blob_map = json.loads(Path(sys.argv[1]).read_text())
output = Path(sys.argv[2])
with Path(blob_map["central_blob"]).open("rb") as source, output.open("wb") as target:
    for extent in blob_map["extents"]:
        source.seek(int(extent["central_offset"]))
        data = source.read(int(extent["length"]))
        if len(data) != int(extent["length"]):
            raise SystemExit("central blob extent is truncated")
        target.seek(int(extent["logical_offset"]))
        target.write(data)
    target.truncate(int(blob_map["logical_size"]))
PY
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

make_store_object "$COMMON" common
make_store_object "$WORK_SYSTEM_1" work-system-1
make_store_object "$WORK_UNIQUE_1" work-unique-1
make_store_object "$WORK_SYSTEM_2" work-system-2
make_store_object "$WORK_UNIQUE_2" work-unique-2
make_store_object "$DEV_SYSTEM_1" dev-system-1
make_store_object "$DEV_UNIQUE_1" dev-unique-1
make_store_object "$SYMLINK_TARGET" symlink-target
ln -s "$SYMLINK_TARGET" "$PHYSICAL_STORE/$SYMLINK_SYSTEM"

make_closure work-gen1 "$COMMON" "$WORK_SYSTEM_1" "$WORK_UNIQUE_1"
make_closure work-gen2 "$COMMON" "$WORK_SYSTEM_2" "$WORK_UNIQUE_2"
make_closure dev-gen1 "$COMMON" "$DEV_SYSTEM_1" "$DEV_UNIQUE_1"
make_closure symlink-gen "$SYMLINK_TARGET" "$SYMLINK_SYSTEM"

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
  --verify-mode nix-store \
  --additional-space-mib 1

jq -e '
  .schema == "nixome.closure-store.admission.v1" and
  .status == "passed" and
  .verification.mode == "nix-store" and
  (.verification.command | index("--extra-experimental-features")) != null and
  (.verification.command | index("nix-command")) != null
' "$RESULT_DIR/verify-command.json" >/dev/null
grep -Fx -- '--extra-experimental-features' "$RESULT_DIR/nix-verify.args" >/dev/null
grep -Fx -- 'nix-command' "$RESULT_DIR/nix-verify.args" >/dev/null

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
  .store.blob_device == "/dev/xvdf" and
  .store.mountpoint == "/nix/store" and
  .store.filesystem == "erofs" and
  .store.view_backend == "erofs-blobmap" and
  .store.image_contains_materialized_store_copy == false and
  .filesystem == "erofs" and
  .view_backend == "erofs-blobmap" and
  .chunks_total > 0 and
  .chunks_admitted > 0
' "$RESULT_DIR/work-gen1.json" >/dev/null

WORK_GEN1_VIEW="$(jq -r '.store.view_dir' "$RESULT_DIR/work-gen1.json")"
WORK_GEN1_IMAGE="$(jq -r '.canonical_metadata_image' "$RESULT_DIR/work-gen1.json")"
WORK_GEN1_BLOB_MAP="$(jq -r '.canonical_blob_map' "$RESULT_DIR/work-gen1.json")"
WORK_GEN1_LOGICAL_BLOB_SIZE="$(jq -r '.blob_logical_size' "$RESULT_DIR/work-gen1.json")"
[ -d "$WORK_GEN1_VIEW" ]
[ -f "$WORK_GEN1_IMAGE" ]
[ -f "$WORK_GEN1_BLOB_MAP" ]
[ ! -w "$WORK_GEN1_IMAGE" ]
[ ! -e "$(dirname "$WORK_GEN1_IMAGE")/store.img" ]
[ "$(jq -r '.central_blob' "$WORK_GEN1_BLOB_MAP")" = "$POOL/chunks/data.blob" ]

pool_inode="$(stat -c '%d:%i' "$POOL/objects/store/$COMMON/bin/payload")"
view_inode="$(stat -c '%d:%i' "$WORK_GEN1_VIEW/$COMMON/bin/payload")"
[ "$pool_inode" = "$view_inode" ]

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

BLOB_AFTER_WORK_GEN1="$(stat -c %s "$POOL/chunks/data.blob")"

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
  .objects_reused == 1 and
  .chunks_total > 0 and
  .chunks_reused > 0
' "$RESULT_DIR/dev-gen1.json" >/dev/null

DEV_GEN1_VIEW="$(jq -r '.store.view_dir' "$RESULT_DIR/dev-gen1.json")"
[ -e "$DEV_GEN1_VIEW/$COMMON/bin/payload" ]
[ ! -e "$DEV_GEN1_VIEW/$WORK_UNIQUE_1" ]
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
BLOB_AFTER_DEV_GEN1="$(stat -c %s "$POOL/chunks/data.blob")"
DEV_LOGICAL_BLOB_SIZE="$(jq -r '.blob_logical_size' "$RESULT_DIR/dev-gen1.json")"
DEV_PHYSICAL_BYTES_ADDED="$((BLOB_AFTER_DEV_GEN1 - BLOB_AFTER_WORK_GEN1))"
[ "$DEV_PHYSICAL_BYTES_ADDED" -lt "$DEV_LOGICAL_BLOB_SIZE" ]

run_engine_json symlink-gen admit \
  --pool-root "$POOL" \
  --domain symlink-domain \
  --system "/nix/store/$SYMLINK_SYSTEM" \
  --closure-source "$WORK_DIR/closures/symlink-gen" \
  --physical-store-root "$PHYSICAL_STORE" \
  --root-source root-symlink \
  --kernel-name kernel-a \
  --verify-mode tree \
  --additional-space-mib 1

jq -e '
  .domain == "symlink-domain" and
  .generation == 1 and
  .objects_total == 2 and
  .objects_admitted == 2
' "$RESULT_DIR/symlink-gen.json" >/dev/null
[ -L "$POOL/objects/store/$SYMLINK_SYSTEM" ]
[ "$(readlink "$POOL/objects/store/$SYMLINK_SYSTEM")" = "$SYMLINK_TARGET" ]
[ -L "$(jq -r '.store.view_dir' "$RESULT_DIR/symlink-gen.json")/$SYMLINK_SYSTEM" ]
BLOB_AFTER_SYMLINK_GEN="$(stat -c %s "$POOL/chunks/data.blob")"

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

BLOB_AFTER_WORK_GEN2="$(stat -c %s "$POOL/chunks/data.blob")"
WORK_GEN2_LOGICAL_BLOB_SIZE="$(jq -r '.blob_logical_size' "$RESULT_DIR/work-gen2.json")"
WORK_GEN2_PHYSICAL_BYTES_ADDED="$((BLOB_AFTER_WORK_GEN2 - BLOB_AFTER_SYMLINK_GEN))"

jq -e '
  .domain == "work" and
  .generation == 2 and
  .previous_generation == 1 and
  .current_generation == 2 and
  .objects_total == 3 and
  .objects_admitted == 2 and
  .objects_reused == 1 and
  .chunks_reused > 0
' "$RESULT_DIR/work-gen2.json" >/dev/null
[ "$WORK_GEN2_PHYSICAL_BYTES_ADDED" -lt "$WORK_GEN2_LOGICAL_BLOB_SIZE" ]

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
  (.removed_objects | index("'"$DEV_UNIQUE_1"'")) == null and
  (.removed_views | length) >= 1 and
  .chunk_blob_append_only == true
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
  .domains."symlink-domain".current == 1 and
  .object_count == 7 and
  .chunk_count > 0 and
  .chunk_blob_size > 0
' "$RESULT_DIR/status.json" >/dev/null

jq -n \
  --arg schema "nixome.closure-store.physical-sharing.v1" \
  --arg status "passed" \
  --arg central_blob "$POOL/chunks/data.blob" \
  --argjson work_gen1_logical_blob_size "$WORK_GEN1_LOGICAL_BLOB_SIZE" \
  --argjson work_gen1_chunks_admitted "$(jq -r '.chunks_admitted' "$RESULT_DIR/work-gen1.json")" \
  --argjson dev_gen1_logical_blob_size "$DEV_LOGICAL_BLOB_SIZE" \
  --argjson dev_gen1_physical_bytes_added "$DEV_PHYSICAL_BYTES_ADDED" \
  --argjson dev_gen1_chunks_admitted "$(jq -r '.chunks_admitted' "$RESULT_DIR/dev-gen1.json")" \
  --argjson dev_gen1_chunks_reused "$(jq -r '.chunks_reused' "$RESULT_DIR/dev-gen1.json")" \
  --argjson work_gen2_logical_blob_size "$WORK_GEN2_LOGICAL_BLOB_SIZE" \
  --argjson work_gen2_physical_bytes_added "$WORK_GEN2_PHYSICAL_BYTES_ADDED" \
  --argjson work_gen2_chunks_admitted "$(jq -r '.chunks_admitted' "$RESULT_DIR/work-gen2.json")" \
  --argjson work_gen2_chunks_reused "$(jq -r '.chunks_reused' "$RESULT_DIR/work-gen2.json")" \
  '{
    schema: $schema,
    status: $status,
    central_blob: $central_blob,
    work_gen1: {
      logical_blob_size: $work_gen1_logical_blob_size,
      chunks_admitted: $work_gen1_chunks_admitted
    },
    dev_gen1: {
      logical_blob_size: $dev_gen1_logical_blob_size,
      physical_bytes_added: $dev_gen1_physical_bytes_added,
      chunks_admitted: $dev_gen1_chunks_admitted,
      chunks_reused: $dev_gen1_chunks_reused,
      sharing_verified: ($dev_gen1_physical_bytes_added < $dev_gen1_logical_blob_size)
    },
    work_gen2: {
      logical_blob_size: $work_gen2_logical_blob_size,
      physical_bytes_added: $work_gen2_physical_bytes_added,
      chunks_admitted: $work_gen2_chunks_admitted,
      chunks_reused: $work_gen2_chunks_reused,
      sharing_verified: ($work_gen2_physical_bytes_added < $work_gen2_logical_blob_size)
    }
  }' > "$RESULT_DIR/physical-sharing.json"

printf 'closure-store engine test ok\n'
