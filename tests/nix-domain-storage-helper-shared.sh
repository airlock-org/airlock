#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/nix-domain-storage-helper-shared"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nixome-nix-domain-storage-shared.XXXXXX")"

cleanup() {
  chmod -R u+w "$WORK_DIR" 2>/dev/null || true
  rm -rf "$WORK_DIR"
}

trap cleanup EXIT

rm -rf "$RESULT_DIR"
mkdir -p \
  "$RESULT_DIR" \
  "$WORK_DIR/fake/qubes" \
  "$WORK_DIR/bin" \
  "$WORK_DIR/nix-root/nix/store" \
  "$WORK_DIR/closure"

POOL="$WORK_DIR/pool"
SAVE_JSON="$RESULT_DIR/fake-qubes-save.json"
STATE_JSON="$WORK_DIR/fake-qubes-state.json"
SYSTEMCTL_LOG="$RESULT_DIR/systemctl.log"
OUTPUT_JSON="$RESULT_DIR/helper-output.json"
OUTPUT_2_JSON="$RESULT_DIR/helper-output-2.json"
export PYTHONDONTWRITEBYTECODE=1

COMMON="11111111111111111111111111111111-common"
SYSTEM="22222222222222222222222222222222-system"
UNIQUE="33333333333333333333333333333333-unique"

for entry in "$COMMON" "$SYSTEM" "$UNIQUE"; do
  mkdir -p "$WORK_DIR/nix-root/nix/store/$entry/bin"
  printf '%s\n' "$entry" > "$WORK_DIR/nix-root/nix/store/$entry/bin/payload"
  chmod -R a-w "$WORK_DIR/nix-root/nix/store/$entry"
done

{
  printf '/nix/store/%s\n' "$COMMON"
  printf '/nix/store/%s\n' "$SYSTEM"
  printf '/nix/store/%s\n' "$UNIQUE"
} > "$WORK_DIR/closure/store-paths"
printf '{"schema":"nixome.store-closure-manifest.v1","name":"helper-shared","read_only":true}\n' \
  > "$WORK_DIR/closure/manifest.json"

cat > "$WORK_DIR/fake/qubes/exc.py" <<'PY'
class StoragePoolException(Exception):
    pass
PY

cat > "$WORK_DIR/fake/qubes/storage.py" <<'PY'
class Pool:
    driver = "fake"

    def __init__(self, *, name, revisions_to_keep=1, ephemeral_volatile=False):
        self.name = name
        self.revisions_to_keep = revisions_to_keep
        self.ephemeral_volatile = ephemeral_volatile

    @property
    def config(self):
        return {
            "name": self.name,
            "driver": self.driver,
            "revisions_to_keep": self.revisions_to_keep,
            "ephemeral_volatile": self.ephemeral_volatile,
        }

    def init_volume(self, vm, volume_config):
        volume_config["pool"] = self
        volume = Volume(**volume_config)
        vm.volumes[volume.name] = volume
        return volume


class BasicPool(Pool):
    driver = "file"


class Volume:
    devtype = "disk"
    domain = None
    path = None

    def __init__(
        self,
        name,
        pool,
        vid=None,
        *,
        revisions_to_keep=0,
        rw=False,
        save_on_stop=False,
        size=0,
        snap_on_start=False,
        source=None,
        **_kwargs,
    ):
        self.name = name
        self.pool = pool
        self.vid = vid or name
        self.revisions_to_keep = revisions_to_keep
        self.rw = rw
        self.save_on_stop = save_on_stop
        self._size = int(size)
        self.snap_on_start = snap_on_start
        self.source = source

    @property
    def size(self):
        return self._size

    @property
    def config(self):
        return {
            "name": self.name,
            "pool": self.pool.name,
            "vid": self.vid,
            "size": self.size,
            "rw": self.rw,
            "snap_on_start": self.snap_on_start,
            "save_on_stop": self.save_on_stop,
            "source": self.source,
            "revisions_to_keep": self.revisions_to_keep,
        }

    async def create(self):
        return self


class Storage:
    def __init__(self, vm):
        self.vm = vm

    def init_volume(self, name, volume_config):
        pool = self.vm.app.get_pool(volume_config["pool"])
        volume = pool.init_volume(self.vm, volume_config.copy())
        self.vm.volumes[name] = volume
        return volume
PY

cat > "$WORK_DIR/fake/qubes/__init__.py" <<'PY'
import json
import os

from . import storage


class VM:
    klass = "StandaloneVM"

    def __init__(self, app, name, data=None):
        self.app = app
        self.name = name
        self.features = (data or {}).get("features", {})
        self.volume_config = {}
        self.volumes = {}
        self.storage = storage.Storage(self)
        self.events = []
        if data is None:
            root_pool = app.get_pool("default")
            root = storage.Volume(
                name="root",
                pool=root_pool,
                vid="root",
                size=1024,
                rw=True,
                snap_on_start=False,
                save_on_stop=True,
                source=None,
                revisions_to_keep=1,
            )
            self.volumes["root"] = root
            self.volume_config["root"] = root.config
        else:
            self.volume_config = data.get("volume_config", {})
            for name, config in self.volume_config.items():
                self.storage.init_volume(name, config.copy())

    def is_running(self):
        return False


class Qubes:
    def __init__(self):
        self.pools = {"default": storage.BasicPool(name="default")}
        self.domains = {}
        state = os.environ["NIXOME_FAKE_QUBES_STATE_JSON"]
        domain = os.environ["NIXOME_FAKE_QUBES_DOMAIN"]
        if os.path.exists(state):
            with open(state, "r", encoding="utf-8") as handle:
                data = json.load(handle)
            for name, config in data.get("pools", {}).items():
                if name == "default":
                    continue
                if config.get("driver") == "nixome-closure":
                    from nixome_qubes_storage.qubes import ClosurePool

                    self.pools[name] = ClosurePool(
                        name=name,
                        root_dir=config["root_dir"],
                        revisions_to_keep=config.get("revisions_to_keep", 0),
                        ephemeral_volatile=config.get("ephemeral_volatile", False),
                    )
            self.domains = {
                domain: VM(self, domain, data.get("domains", {}).get(domain, {}))
            }
        else:
            self.domains = {domain: VM(self, domain)}

    def get_pool(self, name):
        return self.pools[name]

    async def add_pool(self, name, **kwargs):
        if kwargs.get("driver") != "nixome-closure":
            raise RuntimeError(f"unsupported pool driver {kwargs.get('driver')}")
        from nixome_qubes_storage.qubes import ClosurePool

        pool = ClosurePool(
            name=name,
            root_dir=kwargs["root_dir"],
            revisions_to_keep=kwargs.get("revisions_to_keep", 0),
            ephemeral_volatile=kwargs.get("ephemeral_volatile", False),
        )
        await pool.setup()
        self.pools[name] = pool
        return pool

    def save(self, lock=True):
        payload = {
            "pools": {
                name: pool.config
                for name, pool in sorted(self.pools.items())
            },
            "domains": {},
        }
        for name, vm in sorted(self.domains.items()):
            payload["domains"][name] = {
                "features": vm.features,
                "volume_config": {
                    volume_name: volume.config
                    for volume_name, volume in sorted(vm.volumes.items())
                },
            }
        with open(os.environ["NIXOME_FAKE_QUBES_SAVE_JSON"], "w", encoding="utf-8") as handle:
            json.dump(payload, handle, sort_keys=True)
        with open(os.environ["NIXOME_FAKE_QUBES_STATE_JSON"], "w", encoding="utf-8") as handle:
            json.dump(payload, handle, sort_keys=True)

    def close(self):
        return None
PY

cat > "$WORK_DIR/bin/systemctl" <<'EOF'
#!/usr/bin/env bash
printf 'systemctl %s\n' "$*" >> "$NIXOME_FAKE_SYSTEMCTL_LOG"
case "$1" in
  is-active|stop|start)
    exit 0
    ;;
esac
exit 1
EOF
chmod 0755 "$WORK_DIR/bin/systemctl"

PATH="$WORK_DIR/bin:$PATH" \
  PYTHONPATH="$WORK_DIR/fake:$ROOT/pkgs/nixome-qubes-storage/src" \
  NIXOME_NIX_ROOT="$WORK_DIR/nix-root" \
  NIXOME_CLOSURE_STORE_VERIFY=tree \
  NIXOME_FAKE_QUBES_DOMAIN=nix-native-work \
  NIXOME_FAKE_QUBES_SAVE_JSON="$SAVE_JSON" \
  NIXOME_FAKE_QUBES_STATE_JSON="$STATE_JSON" \
  NIXOME_FAKE_SYSTEMCTL_LOG="$SYSTEMCTL_LOG" \
  python3 "$ROOT/scripts/nix-domain-storage.py" \
    ensure-store nix-native-work "$WORK_DIR/closure" /nix/store/example-source \
    --mode shared-closure \
    --system "/nix/store/$SYSTEM" \
    --root /nix/store/example-root \
    --kernel kernel-a \
    --pool-root "$POOL" \
    --additional-space-mib 1 \
    > "$OUTPUT_JSON"

jq -e '
  .schema == "nixome.nix-domain-storage.v1" and
  .domain == "nix-native-work" and
  .mode == "shared-closure" and
  .pool == "nixome-closure" and
  .pool_created == true and
  .volume_created == true and
  .view_created == true and
  .image_imported == false and
  .metadata_changed == true and
  .qubesd_restart_required == false and
  .qubesd_restarted == true and
  .generation == 1 and
  .objects_total == 3 and
  .objects_admitted == 3 and
  .objects_reused == 0
' "$OUTPUT_JSON" >/dev/null

VIEW_IMAGE="$(jq -r '.view_image' "$OUTPUT_JSON")"
[ -f "$VIEW_IMAGE" ]
[ ! -w "$VIEW_IMAGE" ]

jq -e '
  .pools."nixome-closure".driver == "nixome-closure" and
  .domains."nix-native-work".volume_config.store.pool == "nixome-closure" and
  .domains."nix-native-work".volume_config.store.rw == false and
  .domains."nix-native-work".volume_config.store.snap_on_start == false and
  .domains."nix-native-work".volume_config.store.save_on_stop == true and
  .domains."nix-native-work".volume_config.store.source == null and
  .domains."nix-native-work".volume_config.store.generation == "1" and
  (.domains."nix-native-work".volume_config.store.view_path | length) > 0 and
  (.domains."nix-native-work".volume_config.store.manifest_path | length) > 0
' "$SAVE_JSON" >/dev/null

tmp_state="$WORK_DIR/state-with-features.json"
jq \
  --arg source /nix/store/example-source \
  --arg generation "$(jq -r '.generation' "$OUTPUT_JSON")" \
  --arg closure_hash "$(jq -r '.closure_hash' "$OUTPUT_JSON")" \
  --arg view_image "$(jq -r '.view_image' "$OUTPUT_JSON")" \
  '.domains."nix-native-work".features = {
    "nixome-store-source": $source,
    "nixome-store-mode": "shared-closure",
    "nixome-store-generation": $generation,
    "nixome-store-closure-hash": $closure_hash,
    "nixome-store-view-image": $view_image
  }' "$STATE_JSON" > "$tmp_state"
mv "$tmp_state" "$STATE_JSON"

PATH="$WORK_DIR/bin:$PATH" \
  PYTHONPATH="$WORK_DIR/fake:$ROOT/pkgs/nixome-qubes-storage/src" \
  NIXOME_NIX_ROOT="$WORK_DIR/nix-root" \
  NIXOME_CLOSURE_STORE_VERIFY=tree \
  NIXOME_FAKE_QUBES_DOMAIN=nix-native-work \
  NIXOME_FAKE_QUBES_SAVE_JSON="$SAVE_JSON" \
  NIXOME_FAKE_QUBES_STATE_JSON="$STATE_JSON" \
  NIXOME_FAKE_SYSTEMCTL_LOG="$SYSTEMCTL_LOG" \
  python3 "$ROOT/scripts/nix-domain-storage.py" \
    ensure-store nix-native-work "$WORK_DIR/closure" /nix/store/example-source \
    --mode shared-closure \
    --system "/nix/store/$SYSTEM" \
    --root /nix/store/example-root \
    --kernel kernel-a \
    --pool-root "$POOL" \
    --additional-space-mib 1 \
    > "$OUTPUT_2_JSON"

jq -e '
  .mode == "shared-closure" and
  .volume_created == false and
  .view_created == false and
  .metadata_changed == false and
  .qubesd_restarted == false and
  .generation == "1"
' "$OUTPUT_2_JSON" >/dev/null

cat > "$RESULT_DIR/expected-systemctl.log" <<'EOF'
systemctl is-active --quiet qubesd.service
systemctl stop qubesd.service
systemctl start qubesd.service
EOF
cmp -s "$RESULT_DIR/expected-systemctl.log" "$SYSTEMCTL_LOG"

printf 'nix-domain shared storage helper test ok\n'
