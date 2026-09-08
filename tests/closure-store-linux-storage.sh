#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/closure-store-linux-storage"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nixome-closure-store-linux.XXXXXX")"

cleanup() {
  set +e
  if [ -d "$WORK_DIR" ]; then
    if [ -x "$WORK_DIR/cleanup.sh" ]; then
      "$WORK_DIR/cleanup.sh"
    fi
    chmod -R u+w "$WORK_DIR" 2>/dev/null || true
    rm -rf "$WORK_DIR"
  fi
}

trap cleanup EXIT

chmod -R u+w "$RESULT_DIR" 2>/dev/null || true
rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR"

if [ "$(id -u)" != "0" ]; then
  if command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then
    exec sudo -n \
      NIXOME_LOG_DIR="$LOG_DIR" \
      NIXOME_NIX_ROOT="${NIXOME_NIX_ROOT:-}" \
      NIXOME_USE_HOST_NIX_STORE="${NIXOME_USE_HOST_NIX_STORE:-}" \
      TMPDIR="${TMPDIR:-/tmp}" \
      "$0"
  fi
  cat > "$RESULT_DIR/result.json" <<'EOF'
{"schema":"nixome.closure-store-linux-storage.v1","status":"skipped","reason":"root or passwordless sudo is required for loop, device-mapper, and mount namespace coverage"}
EOF
  printf 'closure-store linux storage test skipped: root or passwordless sudo required\n'
  exit 0
fi

mkdir -p "$WORK_DIR/physical-store" "$WORK_DIR/closures" "$WORK_DIR/mounts/a" "$WORK_DIR/mounts/b"
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

require_root_command() {
  command -v "$1" >/dev/null 2>&1 || {
    printf 'missing required command: %s\n' "$1" >&2
    exit 1
  }
}

for command in dmsetup losetup mount unshare findmnt mkfs.erofs fsck.erofs; do
  require_root_command "$command"
done

make_store_object() {
  entry="$1"
  content="$2"
  mkdir -p "$PHYSICAL_STORE/$entry/bin"
  python3 - "$PHYSICAL_STORE/$entry/bin/payload" "$content" <<'PY'
import sys
from pathlib import Path

Path(sys.argv[1]).write_bytes((sys.argv[2] + "\n").encode() * 4096)
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

COMMON="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-common"
A_UNIQUE="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-a-unique"
B_UNIQUE="cccccccccccccccccccccccccccccccc-b-unique"
make_store_object "$COMMON" common
make_store_object "$A_UNIQUE" a-unique
make_store_object "$B_UNIQUE" b-unique
make_closure a "$COMMON" "$A_UNIQUE"
make_closure b "$COMMON" "$B_UNIQUE"

run_engine_json admit-a admit \
  --pool-root "$POOL" \
  --domain a \
  --system "/nix/store/$A_UNIQUE" \
  --closure-source "$WORK_DIR/closures/a" \
  --physical-store-root "$PHYSICAL_STORE" \
  --verify-mode tree \
  --segment-target-bytes 16384

run_engine_json admit-b admit \
  --pool-root "$POOL" \
  --domain b \
  --system "/nix/store/$B_UNIQUE" \
  --closure-source "$WORK_DIR/closures/b" \
  --physical-store-root "$PHYSICAL_STORE" \
  --verify-mode tree \
  --segment-target-bytes 16384

run_engine_json mapper-before-create-failed attach-blob-view \
  --pool-root "$POOL" \
  --domain a \
  --generation 1

run_engine_json detach-before-create detach-blob-view \
  --pool-root "$POOL" \
  --domain a \
  --generation 1

set +e
NIXOME_CLOSURE_STORE_FAILPOINT=mapper-before-create "${ENGINE[@]}" attach-blob-view \
  --pool-root "$POOL" \
  --domain a \
  --generation 1 \
  > "$RESULT_DIR/mapper-before-create.json" 2> "$RESULT_DIR/mapper-before-create.err"
before_rc="$?"
set -e
[ "$before_rc" != "0" ]
grep -F 'simulated failure at mapper-before-create' "$RESULT_DIR/mapper-before-create.err" >/dev/null

set +e
NIXOME_CLOSURE_STORE_FAILPOINT=mapper-after-create "${ENGINE[@]}" attach-blob-view \
  --pool-root "$POOL" \
  --domain a \
  --generation 1 \
  > "$RESULT_DIR/mapper-after-create.json" 2> "$RESULT_DIR/mapper-after-create.err"
after_rc="$?"
set -e
[ "$after_rc" != "0" ]
grep -F 'simulated failure at mapper-after-create' "$RESULT_DIR/mapper-after-create.err" >/dev/null

run_engine_json attach-a attach-blob-view \
  --pool-root "$POOL" \
  --domain a \
  --generation 1
run_engine_json attach-b attach-blob-view \
  --pool-root "$POOL" \
  --domain b \
  --generation 1

A_META="$(jq -r '.metadata_image' "$RESULT_DIR/admit-a.json")"
B_META="$(jq -r '.metadata_image' "$RESULT_DIR/admit-b.json")"
A_DEVICE="$(jq -r '.device' "$RESULT_DIR/attach-a.json")"
B_DEVICE="$(jq -r '.device' "$RESULT_DIR/attach-b.json")"
mapfile -t BACKING_PATHS < <(jq -r '.backing_paths[]' "$RESULT_DIR/attach-a.json" "$RESULT_DIR/attach-b.json" | sort -u)

cat > "$WORK_DIR/cleanup.sh" <<EOF
#!/usr/bin/env bash
set +e
PYTHONPATH='$PYTHONPATH' python3 -m nixome_qubes_storage.cli detach-blob-view --pool-root '$POOL' --domain a --generation 1 >/dev/null 2>&1
PYTHONPATH='$PYTHONPATH' python3 -m nixome_qubes_storage.cli detach-blob-view --pool-root '$POOL' --domain b --generation 1 >/dev/null 2>&1
EOF
chmod 0755 "$WORK_DIR/cleanup.sh"

case "$A_DEVICE" in /dev/mapper/nixome-*) ;; *) printf 'unexpected A mapper: %s\n' "$A_DEVICE" >&2; exit 1 ;; esac
case "$B_DEVICE" in /dev/mapper/nixome-*) ;; *) printf 'unexpected B mapper: %s\n' "$B_DEVICE" >&2; exit 1 ;; esac
for backing in "${BACKING_PATHS[@]}"; do
  [ "$(readlink -f "$A_DEVICE")" != "$(readlink -f "$backing")" ]
  [ "$(readlink -f "$B_DEVICE")" != "$(readlink -f "$backing")" ]
done

A_MAP="$(jq -r '.blob_map' "$RESULT_DIR/attach-a.json")"
B_MAP="$(jq -r '.blob_map' "$RESULT_DIR/attach-b.json")"
COMMON_A_SEGMENT="$(jq -r --arg object "$COMMON" '.extents[] | select(.object == $object) | .segment_id' "$A_MAP")"
COMMON_B_SEGMENT="$(jq -r --arg object "$COMMON" '.extents[] | select(.object == $object) | .segment_id' "$B_MAP")"
COMMON_A_OFFSET="$(jq -r --arg object "$COMMON" '.extents[] | select(.object == $object) | .segment_offset' "$A_MAP")"
COMMON_B_OFFSET="$(jq -r --arg object "$COMMON" '.extents[] | select(.object == $object) | .segment_offset' "$B_MAP")"
[ "$COMMON_A_SEGMENT" = "$COMMON_B_SEGMENT" ]
[ "$COMMON_A_OFFSET" = "$COMMON_B_OFFSET" ]
jq -e --arg object "$B_UNIQUE" 'all(.objects[]; . != $object)' "$A_MAP" >/dev/null
jq -e --arg object "$A_UNIQUE" 'all(.objects[]; . != $object)' "$B_MAP" >/dev/null

unshare --mount --propagation private bash -eu <<EOF
mount -t erofs -o ro,loop,device=$A_DEVICE '$A_META' '$WORK_DIR/mounts/a'
mount -t erofs -o ro,loop,device=$B_DEVICE '$B_META' '$WORK_DIR/mounts/b'
findmnt -rn -T '$WORK_DIR/mounts/a' -o OPTIONS | grep -E '(^|,)ro(,|$)' >/dev/null
findmnt -rn -T '$WORK_DIR/mounts/b' -o OPTIONS | grep -E '(^|,)ro(,|$)' >/dev/null
test -f '$WORK_DIR/mounts/a/$COMMON/bin/payload'
test -f '$WORK_DIR/mounts/a/$A_UNIQUE/bin/payload'
test ! -e '$WORK_DIR/mounts/a/$B_UNIQUE'
test -f '$WORK_DIR/mounts/b/$COMMON/bin/payload'
test -f '$WORK_DIR/mounts/b/$B_UNIQUE/bin/payload'
test ! -e '$WORK_DIR/mounts/b/$A_UNIQUE'
if touch '$WORK_DIR/mounts/a/.write-probe' 2>/dev/null; then
  exit 42
fi
if touch '$WORK_DIR/mounts/b/.write-probe' 2>/dev/null; then
  exit 43
fi
umount '$WORK_DIR/mounts/a'
umount '$WORK_DIR/mounts/b'
EOF

run_engine_json mark-running-a-store mark-running \
  --pool-root "$POOL" \
  --domain a \
  --generation 1 \
  --volume store
run_engine_json gc-running gc --pool-root "$POOL" --keep-generations 0
jq -e '
  any(.running_generations[]; .domain == "a" and .generation == 1) and
  all(.removed_generations[]; .domain != "a" or .generation != 1)
' "$RESULT_DIR/gc-running.json" >/dev/null
run_engine_json clear-running-a-store clear-running \
  --pool-root "$POOL" \
  --domain a \
  --generation 1 \
  --volume store

jq -n \
  --arg schema "nixome.closure-store-linux-storage.v1" \
  --arg status passed \
  --arg a_device "$A_DEVICE" \
  --arg b_device "$B_DEVICE" \
  --arg common_segment "$COMMON_A_SEGMENT" \
  --argjson common_offset "$COMMON_A_OFFSET" \
  --arg common_object "$COMMON" \
  '{
    schema: $schema,
    status: $status,
    a_device: $a_device,
    b_device: $b_device,
    backing_storage_exposed: false,
    read_only_mounts: true,
    closure_filtering: true,
    shared_object: {
      object: $common_object,
      segment_id: $common_segment,
      segment_offset: $common_offset,
      same_offset_in_both_views: true
    }
  }' > "$RESULT_DIR/result.json"

printf 'closure-store linux storage test ok\n'
