#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/nix-domain-storage-helper"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nixome-nix-domain-storage.XXXXXX")"

trap 'rm -rf "$WORK_DIR"' EXIT

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR" "$WORK_DIR/fake/qubes"

IMAGE="$WORK_DIR/store.img"
IMPORT_PATH="$WORK_DIR/import-target.img"
SAVE_JSON="$RESULT_DIR/fake-qubes-save.json"
OUTPUT_JSON="$RESULT_DIR/helper-output.json"
SYSTEMCTL_LOG="$RESULT_DIR/systemctl.log"
STATE_JSON="$WORK_DIR/fake-qubes-state.json"

printf 'nixome-store-image\n' > "$IMAGE"

cat > "$WORK_DIR/fake/qubes/storage.py" <<'PY'
import os


class Pool:
    def __init__(self, name):
        self.name = name


class Volume:
    def __init__(self, config):
        self.name = config["name"]
        self.pool = config["pool"] if isinstance(config["pool"], Pool) else Pool(config["pool"])
        self.size = config.get("size", 0)
        self.rw = config.get("rw", True)
        self.snap_on_start = config.get("snap_on_start", False)
        self.save_on_stop = config.get("save_on_stop", False)
        self.source = config.get("source")
        self.revisions_to_keep = config.get("revisions_to_keep", 0)

    async def create(self):
        return None

    def as_json(self):
        return {
            "name": self.name,
            "pool": self.pool.name,
            "size": self.size,
            "rw": self.rw,
            "snap_on_start": self.snap_on_start,
            "save_on_stop": self.save_on_stop,
            "source": self.source,
            "revisions_to_keep": self.revisions_to_keep,
        }


class Storage:
    def __init__(self, vm):
        self.vm = vm

    def init_volume(self, name, volume_config):
        volume = Volume(volume_config)
        self.vm.volumes[name] = volume
        return volume

    async def import_data(self, volume_name, size):
        self.vm.events.append(["import_data", volume_name, size])
        return os.environ["NIXOME_FAKE_QUBES_IMPORT_PATH"]

    async def import_data_end(self, volume_name, success):
        self.vm.events.append(["import_data_end", volume_name, success])
PY

cat > "$WORK_DIR/fake/qubes/__init__.py" <<'PY'
import json
import os

from . import storage


class VM:
    klass = "StandaloneVM"

    def __init__(self, name):
        self.name = name
        self.features = {}
        self.volume_config = {}
        self.volumes = {
            "root": storage.Volume(
                {
                    "name": "root",
                    "pool": storage.Pool("default"),
                    "size": 1024,
                    "rw": True,
                    "snap_on_start": False,
                    "save_on_stop": True,
                    "source": None,
                    "revisions_to_keep": 1,
                }
            )
        }
        self.storage = None
        self.events = []

    def is_running(self):
        return False


class Qubes:
    def __init__(self):
        domain = os.environ["NIXOME_FAKE_QUBES_DOMAIN"]
        state = os.environ["NIXOME_FAKE_QUBES_STATE_JSON"]
        vm = VM(domain)
        if os.path.exists(state):
            with open(state, "r", encoding="utf-8") as handle:
                data = json.load(handle)
            vm.volumes = {
                name: storage.Volume(config)
                for name, config in data["volumes"].items()
            }
            vm.volume_config = {
                name: volume.as_json()
                for name, volume in vm.volumes.items()
            }
        self.domains = {domain: vm}

    def save(self, lock=True):
        domain, vm = next(iter(self.domains.items()))
        payload = {
            "domain": domain,
            "volumes": {
                name: volume.as_json()
                for name, volume in sorted(vm.volumes.items())
            },
            "events": vm.events,
        }
        with open(os.environ["NIXOME_FAKE_QUBES_SAVE_JSON"], "w", encoding="utf-8") as handle:
            json.dump(payload, handle, sort_keys=True)
        with open(os.environ["NIXOME_FAKE_QUBES_STATE_JSON"], "w", encoding="utf-8") as handle:
            json.dump(payload, handle, sort_keys=True)

    def close(self):
        return None
PY

mkdir -p "$WORK_DIR/bin"
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
  PYTHONPATH="$WORK_DIR/fake" \
  NIXOME_FAKE_QUBES_DOMAIN=nix-native-work \
  NIXOME_FAKE_QUBES_IMPORT_PATH="$IMPORT_PATH" \
  NIXOME_FAKE_QUBES_SAVE_JSON="$SAVE_JSON" \
  NIXOME_FAKE_QUBES_STATE_JSON="$STATE_JSON" \
  NIXOME_FAKE_SYSTEMCTL_LOG="$SYSTEMCTL_LOG" \
  python3 "$ROOT/scripts/nix-domain-storage.py" \
    ensure-store nix-native-work "$IMAGE" /nix/store/example-source \
    > "$OUTPUT_JSON"

jq -e '
  .schema == "nixome.nix-domain-storage.v1" and
  .domain == "nix-native-work" and
  .volume == "store" and
  .read_only == true and
  .volume_created == true and
  .image_imported == true and
  .metadata_changed == true and
  .qubesd_restart_required == false and
  .qubesd_restarted == true and
  .qubesd_was_active == true
' "$OUTPUT_JSON" >/dev/null

cmp -s "$IMAGE" "$IMPORT_PATH"

jq -e '
  .domain == "nix-native-work" and
  .volumes.store.pool == "default" and
  .volumes.store.rw == false and
  .volumes.store.snap_on_start == false and
  .volumes.store.save_on_stop == true and
  .volumes.store.source == null and
  .events == [
    ["import_data", "store", 19],
    ["import_data_end", "store", true]
  ]
' "$SAVE_JSON" >/dev/null

cat > "$RESULT_DIR/expected-systemctl.log" <<'EOF'
systemctl is-active --quiet qubesd.service
systemctl stop qubesd.service
systemctl start qubesd.service
EOF
cmp -s "$RESULT_DIR/expected-systemctl.log" "$SYSTEMCTL_LOG"

printf 'nix-domain storage helper test ok\n'
