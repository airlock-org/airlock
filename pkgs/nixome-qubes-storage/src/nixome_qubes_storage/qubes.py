import os
from pathlib import Path

import qubes.exc
import qubes.storage


class ClosurePool(qubes.storage.Pool):
    driver = "nixome-closure"

    def __init__(self, *, name, root_dir="/var/lib/nixome/closure-store", revisions_to_keep=0, ephemeral_volatile=False):
        super().__init__(
            name=name,
            revisions_to_keep=revisions_to_keep,
            ephemeral_volatile=ephemeral_volatile,
        )
        self.root_dir = os.path.abspath(root_dir)
        self._volumes = {}

    @property
    def config(self):
        return {
            "name": self.name,
            "driver": self.driver,
            "root_dir": self.root_dir,
            "revisions_to_keep": self.revisions_to_keep,
            "ephemeral_volatile": self.ephemeral_volatile,
        }

    async def setup(self):
        Path(self.root_dir).mkdir(parents=True, exist_ok=True)
        return self

    async def destroy(self):
        return self

    def init_volume(self, vm, volume_config):
        if volume_config.get("rw", False):
            raise qubes.exc.StoragePoolException("nixome-closure volumes are read-only")
        if volume_config.get("snap_on_start", False):
            raise qubes.exc.StoragePoolException("nixome-closure volumes do not use Qubes snapshots")
        if volume_config.get("source") is not None:
            raise qubes.exc.StoragePoolException("nixome-closure volumes do not use source volumes")
        if "vid" not in volume_config:
            generation = volume_config.get("generation")
            if generation is None:
                raise qubes.exc.StoragePoolException("nixome-closure volume requires generation")
            volume_config["vid"] = f"domains/{vm.name}/generations/{generation}/store"
        volume_config["pool"] = self
        volume = ClosureVolume(**volume_config)
        self._volumes[volume.vid] = volume
        return volume

    def list_volumes(self):
        return list(self._volumes.values())

    def get_volume(self, vid):
        return self._volumes[vid]

    @property
    def size(self):
        try:
            stat = os.statvfs(self.root_dir)
            return stat.f_frsize * stat.f_blocks
        except FileNotFoundError:
            return 0

    @property
    def usage(self):
        total = 0
        root = Path(self.root_dir)
        if not root.exists():
            return 0
        for path in root.rglob("*"):
            try:
                total += path.lstat().st_blocks * 512
            except OSError:
                pass
        return total


class ClosureVolume(qubes.storage.Volume):
    def __init__(
        self,
        *,
        domain_name=None,
        generation=None,
        closure_hash=None,
        view_path=None,
        manifest_path=None,
        **kwargs,
    ):
        kwargs["rw"] = False
        kwargs["snap_on_start"] = False
        kwargs["save_on_stop"] = True
        super().__init__(**kwargs)
        self.domain_name = domain_name
        self.generation = None if generation is None else str(generation)
        self.closure_hash = closure_hash
        self.path = os.path.abspath(view_path) if view_path else None
        self.manifest_path = os.path.abspath(manifest_path) if manifest_path else None

    @property
    def config(self):
        result = super().config
        if self.domain_name is not None:
            result["domain_name"] = self.domain_name
        if self.generation is not None:
            result["generation"] = self.generation
        if self.closure_hash is not None:
            result["closure_hash"] = self.closure_hash
        if self.path is not None:
            result["view_path"] = self.path
        if self.manifest_path is not None:
            result["manifest_path"] = self.manifest_path
        return result

    async def create(self):
        await self.verify()
        return self

    async def remove(self):
        with context_suppress(ValueError):
            self.pool._volumes.pop(self.vid, None)
        return self

    async def export(self):
        await self.verify()
        return self.path

    async def import_data(self, size):
        raise qubes.exc.StoragePoolException("nixome-closure volumes are immutable")

    async def import_volume(self, src_volume):
        raise qubes.exc.StoragePoolException("nixome-closure volumes are immutable")

    async def resize(self, size):
        raise qubes.exc.StoragePoolException("nixome-closure volumes are immutable")

    async def start(self):
        await self.verify()
        return self

    async def stop(self):
        return self

    async def verify(self):
        if not self.path:
            raise qubes.exc.StoragePoolException(f"missing view_path for {self.vid}")
        if not os.path.exists(self.path):
            raise qubes.exc.StoragePoolException(f"missing nixome closure view image: {self.path}")
        mode = os.stat(self.path).st_mode
        if mode & 0o222:
            raise qubes.exc.StoragePoolException(f"nixome closure view image is writable: {self.path}")
        if self.manifest_path and not os.path.exists(self.manifest_path):
            raise qubes.exc.StoragePoolException(f"missing nixome generation manifest: {self.manifest_path}")
        return True

    def is_dirty(self):
        return False

    def is_outdated(self):
        return False

    @property
    def revisions(self):
        return {}

    @property
    def size(self):
        if self.path and os.path.exists(self.path):
            self._size = os.path.getsize(self.path)
        return self._size

    @property
    def usage(self):
        if self.path and os.path.exists(self.path):
            return os.stat(self.path).st_blocks * 512
        return 0


class context_suppress:
    def __init__(self, *exceptions):
        self.exceptions = exceptions

    def __enter__(self):
        return None

    def __exit__(self, exc_type, exc, traceback):
        return exc_type is not None and issubclass(exc_type, self.exceptions)
