#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/closure-store-failure-recovery"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nixome-closure-store-failure.XXXXXX")"

cleanup() {
  chmod -R u+w "$WORK_DIR" 2>/dev/null || true
  rm -rf "$WORK_DIR"
}

trap cleanup EXIT

chmod -R u+w "$RESULT_DIR" 2>/dev/null || true
rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR" "$WORK_DIR/physical-store" "$WORK_DIR/closures" "$WORK_DIR/bin"

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
  mkdir -p "$WORK_DIR/physical-store/$entry/bin"
  printf '%s\n' "$content" > "$WORK_DIR/physical-store/$entry/bin/payload"
  chmod 0555 "$WORK_DIR/physical-store/$entry" "$WORK_DIR/physical-store/$entry/bin"
  chmod 0444 "$WORK_DIR/physical-store/$entry/bin/payload"
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

run_engine_failpoint() {
  name="$1"
  failpoint="$2"
  shift 2
  set +e
  NIXOME_CLOSURE_STORE_FAILPOINT="$failpoint" "${ENGINE[@]}" "$@" \
    > "$RESULT_DIR/$name.json" 2> "$RESULT_DIR/$name.err"
  rc="$?"
  set -e
  [ "$rc" != "0" ]
  grep -F "simulated failure at $failpoint" "$RESULT_DIR/$name.err" >/dev/null
}

assert_no_committed_domain() {
  pool="$1"
  domain="$2"
  name="$3"

  run_engine_json "$name" status --pool-root "$pool"
  jq -e --arg domain "$domain" '
    (.domains[$domain] // null) == null
  ' "$RESULT_DIR/$name.json" >/dev/null
}

PHYSICAL_STORE="$WORK_DIR/physical-store"
COMMON="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-common"
ONE="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-one"
TWO="cccccccccccccccccccccccccccccccc-two"
THREE="dddddddddddddddddddddddddddddddd-three"
FOUR="eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-four"
FIVE="ffffffffffffffffffffffffffffffff-five"
SIX="11111111111111111111111111111111-six"
SEVEN="22222222222222222222222222222222-seven"

for entry in "$COMMON" "$ONE" "$TWO" "$THREE" "$FOUR" "$FIVE" "$SIX" "$SEVEN"; do
  make_store_object "$entry" "$entry"
done

make_closure one "$ONE"
make_closure two "$TWO"
make_closure three "$THREE"
make_closure four "$FOUR"
make_closure five "$FIVE"
make_closure six "$COMMON" "$SIX"
make_closure seven "$COMMON" "$SEVEN"

cat > "$WORK_DIR/bin/nix" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$@" > "$NIXOME_FAKE_NIX_LOG"
EOF
chmod 0755 "$WORK_DIR/bin/nix"

VERIFY_POOL="$WORK_DIR/verify-pool"
PATH="$WORK_DIR/bin:$PATH" \
  NIXOME_FAKE_NIX_LOG="$RESULT_DIR/nix-verify.args" \
  run_engine_failpoint verification-before-command verification-before-command admit \
    --pool-root "$VERIFY_POOL" \
    --domain verify \
    --system "/nix/store/$ONE" \
    --closure-source "$WORK_DIR/closures/one" \
    --physical-store-root "$PHYSICAL_STORE" \
    --verify-mode nix-store
run_engine_json recover-verify recover --pool-root "$VERIFY_POOL"
assert_no_committed_domain "$VERIFY_POOL" verify verify-status

APPEND_POOL="$WORK_DIR/append-pool"
run_engine_failpoint object-after-append object-after-append admit \
  --pool-root "$APPEND_POOL" \
  --domain append \
  --system "/nix/store/$ONE" \
  --closure-source "$WORK_DIR/closures/one" \
  --physical-store-root "$PHYSICAL_STORE" \
  --verify-mode tree
[ ! -e "$APPEND_POOL/objects/blob-metadata/$ONE.json" ]
find "$APPEND_POOL/objects/segments" -name '*.blob' -size +0c -print -quit | grep -q .
run_engine_json recover-append recover --pool-root "$APPEND_POOL"
assert_no_committed_domain "$APPEND_POOL" append append-status

SEGMENT_CREATE_POOL="$WORK_DIR/segment-create-pool"
run_engine_failpoint segment-after-create segment-after-create admit \
  --pool-root "$SEGMENT_CREATE_POOL" \
  --domain segment-create \
  --system "/nix/store/$ONE" \
  --closure-source "$WORK_DIR/closures/one" \
  --physical-store-root "$PHYSICAL_STORE" \
  --verify-mode tree \
  --segment-target-bytes 4096
[ ! -e "$SEGMENT_CREATE_POOL/objects/blob-metadata/$ONE.json" ]
run_engine_json recover-segment-create recover --pool-root "$SEGMENT_CREATE_POOL"
assert_no_committed_domain "$SEGMENT_CREATE_POOL" segment-create segment-create-status

SEGMENT_SEAL_POOL="$WORK_DIR/segment-seal-pool"
run_engine_failpoint segment-before-seal segment-before-seal admit \
  --pool-root "$SEGMENT_SEAL_POOL" \
  --domain segment-seal \
  --system "/nix/store/$ONE" \
  --closure-source "$WORK_DIR/closures/one" \
  --physical-store-root "$PHYSICAL_STORE" \
  --verify-mode tree \
  --segment-target-bytes 4096
[ ! -e "$SEGMENT_SEAL_POOL/objects/blob-metadata/$ONE.json" ]
run_engine_json recover-segment-seal recover --pool-root "$SEGMENT_SEAL_POOL"
assert_no_committed_domain "$SEGMENT_SEAL_POOL" segment-seal segment-seal-status

METADATA_POOL="$WORK_DIR/metadata-pool"
run_engine_failpoint object-after-metadata object-after-metadata admit \
  --pool-root "$METADATA_POOL" \
  --domain metadata \
  --system "/nix/store/$TWO" \
  --closure-source "$WORK_DIR/closures/two" \
  --physical-store-root "$PHYSICAL_STORE" \
  --verify-mode tree
[ -f "$METADATA_POOL/objects/blob-metadata/$TWO.json" ]
run_engine_json gc-active-metadata gc --pool-root "$METADATA_POOL" --keep-generations 0
jq -e --arg object "$TWO" '
  (.active_transaction_objects | index($object)) != null and
  (.removed_object_blobs | index($object)) == null
' "$RESULT_DIR/gc-active-metadata.json" >/dev/null
run_engine_json recover-metadata recover --pool-root "$METADATA_POOL"
assert_no_committed_domain "$METADATA_POOL" metadata metadata-status
run_engine_json gc-aborted-metadata gc --pool-root "$METADATA_POOL" --keep-generations 0
jq -e --arg object "$TWO" '
  (.removed_object_blobs | index($object)) != null
' "$RESULT_DIR/gc-aborted-metadata.json" >/dev/null

VIEW_POOL="$WORK_DIR/view-pool"
run_engine_failpoint view-after-metadata-build view-after-metadata-build admit \
  --pool-root "$VIEW_POOL" \
  --domain view \
  --system "/nix/store/$THREE" \
  --closure-source "$WORK_DIR/closures/three" \
  --physical-store-root "$PHYSICAL_STORE" \
  --verify-mode tree
run_engine_json recover-view recover --pool-root "$VIEW_POOL"
assert_no_committed_domain "$VIEW_POOL" view view-status
[ ! -d "$VIEW_POOL/tmp" ] || [ -z "$(find "$VIEW_POOL/tmp" -mindepth 1 -print -quit)" ]

MANIFEST_POOL="$WORK_DIR/manifest-pool"
run_engine_failpoint generation-after-manifest generation-after-manifest admit \
  --pool-root "$MANIFEST_POOL" \
  --domain manifest \
  --system "/nix/store/$FOUR" \
  --closure-source "$WORK_DIR/closures/four" \
  --physical-store-root "$PHYSICAL_STORE" \
  --verify-mode tree
jq -e '.status == "prepared"' "$MANIFEST_POOL/domains/manifest/generations/1/manifest.json" >/dev/null
run_engine_json recover-manifest recover --pool-root "$MANIFEST_POOL"
assert_no_committed_domain "$MANIFEST_POOL" manifest manifest-status
[ ! -d "$MANIFEST_POOL/domains/manifest/generations/1" ]

FINALIZE_POOL="$WORK_DIR/finalize-pool"
run_engine_failpoint generation-after-tx-commit generation-after-tx-commit admit \
  --pool-root "$FINALIZE_POOL" \
  --domain finalize \
  --system "/nix/store/$FIVE" \
  --closure-source "$WORK_DIR/closures/five" \
  --physical-store-root "$PHYSICAL_STORE" \
  --verify-mode tree \
  --activate false
jq -e '.status == "prepared"' "$FINALIZE_POOL/domains/finalize/generations/1/manifest.json" >/dev/null
run_engine_json status-finalize-before status --pool-root "$FINALIZE_POOL"
jq -e '(.domains.finalize // null) == null' "$RESULT_DIR/status-finalize-before.json" >/dev/null
run_engine_json gc-finalize-before-recover gc --pool-root "$FINALIZE_POOL" --keep-generations 0
jq -e --arg object "$FIVE" '
  any(.in_progress_generations[]; .domain == "finalize" and .generation == 1 and .transaction_status == "committed") and
  (.active_transaction_objects | index($object)) != null and
  (.removed_object_blobs | index($object)) == null and
  (.removed_views | length) == 0
' "$RESULT_DIR/gc-finalize-before-recover.json" >/dev/null
run_engine_json recover-finalize recover --pool-root "$FINALIZE_POOL"
jq -e '(.finalized_generations | length) == 1' "$RESULT_DIR/recover-finalize.json" >/dev/null
run_engine_json status-finalize-after status --pool-root "$FINALIZE_POOL"
jq -e '
  .domains.finalize.current == null and
  (.domains.finalize.generations | length) == 1 and
  .domains.finalize.generations[0].generation == 1
' "$RESULT_DIR/status-finalize-after.json" >/dev/null

SWITCH_POOL="$WORK_DIR/switch-pool"
run_engine_json switch-gen1 admit \
  --pool-root "$SWITCH_POOL" \
  --domain switch \
  --system "/nix/store/$SIX" \
  --closure-source "$WORK_DIR/closures/six" \
  --physical-store-root "$PHYSICAL_STORE" \
  --verify-mode tree
run_engine_json switch-gen2 admit \
  --pool-root "$SWITCH_POOL" \
  --domain switch \
  --system "/nix/store/$SEVEN" \
  --closure-source "$WORK_DIR/closures/seven" \
  --physical-store-root "$PHYSICAL_STORE" \
  --verify-mode tree \
  --activate false
run_engine_failpoint switch-before-current-replace switch-before-current-replace switch \
  --pool-root "$SWITCH_POOL" \
  --domain switch \
  --generation 2
run_engine_json recover-switch-before recover --pool-root "$SWITCH_POOL"
run_engine_json status-switch-before status --pool-root "$SWITCH_POOL"
jq -e '.domains.switch.current == 1' "$RESULT_DIR/status-switch-before.json" >/dev/null
run_engine_failpoint switch-after-current-replace switch-after-current-replace switch \
  --pool-root "$SWITCH_POOL" \
  --domain switch \
  --generation 2
run_engine_json recover-switch-after recover --pool-root "$SWITCH_POOL"
run_engine_json status-switch-after status --pool-root "$SWITCH_POOL"
jq -e '.domains.switch.current == 2' "$RESULT_DIR/status-switch-after.json" >/dev/null

GC_POOL="$WORK_DIR/gc-pool"
run_engine_json gc-gen1 admit \
  --pool-root "$GC_POOL" \
  --domain gc-domain \
  --system "/nix/store/$COMMON" \
  --closure-source "$WORK_DIR/closures/six" \
  --physical-store-root "$PHYSICAL_STORE" \
  --verify-mode tree
run_engine_json gc-gen2 admit \
  --pool-root "$GC_POOL" \
  --domain gc-domain \
  --system "/nix/store/$SEVEN" \
  --closure-source "$WORK_DIR/closures/seven" \
  --physical-store-root "$PHYSICAL_STORE" \
  --verify-mode tree \
  --retain false
run_engine_json gc-rollback rollback \
  --pool-root "$GC_POOL" \
  --domain gc-domain \
  --generation 1
run_engine_failpoint gc-after-object-metadata-remove gc-after-object-metadata-remove gc \
  --pool-root "$GC_POOL" \
  --keep-generations 0
run_engine_json recover-gc recover --pool-root "$GC_POOL"
run_engine_json status-gc status --pool-root "$GC_POOL"
jq -e '
  .domains."gc-domain".current == 1 and
  (.domains."gc-domain".generations | length) == 1
' "$RESULT_DIR/status-gc.json" >/dev/null

printf 'closure-store failure recovery test ok\n'
