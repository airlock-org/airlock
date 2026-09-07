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
            volume_name = volume_config.get("name", "store")
            volume_config["vid"] = f"domains/{vm.name}/generations/{generation}/{volume_name}"
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
        view_kind="metadata-image",
        blob_map_path=None,
        blob_logical_size=None,
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
        self.view_kind = view_kind
        self.path = os.path.abspath(view_path) if view_path else None
        self.blob_map_path = os.path.abspath(blob_map_path) if blob_map_path else None
        self.blob_logical_size = None if blob_logical_size is None else int(blob_logical_size)
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
        result["view_kind"] = self.view_kind
        if self.path is not None:
            result["view_path"] = self.path
        if self.blob_map_path is not None:
            result["blob_map_path"] = self.blob_map_path
        if self.blob_logical_size is not None:
            result["blob_logical_size"] = self.blob_logical_size
        if self.manifest_path is not None:
            result["manifest_path"] = self.manifest_path
        return result

    async def create(self):
        await self.verify_definition()
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
        if self.view_kind == "erofs-blob-map":
            self.path = self.attach_blob_view()
        await self.verify()
        return self

    async def stop(self):
        if self.view_kind == "erofs-blob-map":
            self.detach_blob_view()
        return self

    async def verify_definition(self):
        if self.view_kind == "erofs-blob-map":
            if not self.blob_map_path:
                raise qubes.exc.StoragePoolException(f"missing blob_map_path for {self.vid}")
            if not os.path.exists(self.blob_map_path):
                raise qubes.exc.StoragePoolException(f"missing nixome closure blob map: {self.blob_map_path}")
            if self.blob_logical_size is None:
                raise qubes.exc.StoragePoolException(f"missing blob_logical_size for {self.vid}")
        else:
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

    async def verify(self):
        await self.verify_definition()
        if self.view_kind == "erofs-blob-map":
            if not self.path:
                return True
            if not os.path.exists(self.path):
                raise qubes.exc.StoragePoolException(f"missing nixome blob-map block device: {self.path}")
        return True

    def attach_blob_view(self):
        if not self.domain_name or self.generation is None:
            raise qubes.exc.StoragePoolException(f"missing domain/generation for {self.vid}")
        try:
            from nixome_qubes_storage import engine

            result = engine.attach_blob_view(self.pool.root_dir, self.domain_name, self.generation)
        except engine.EngineError as exc:
            raise qubes.exc.StoragePoolException(str(exc)) from exc
        return result["device"]

    def detach_blob_view(self):
        if not self.domain_name or self.generation is None:
            return
        try:
            from nixome_qubes_storage import engine

            engine.detach_blob_view(self.pool.root_dir, self.domain_name, self.generation)
        except engine.EngineError as exc:
            raise qubes.exc.StoragePoolException(str(exc)) from exc

    def is_dirty(self):
        return False

    def is_outdated(self):
        return False

    def block_device(self):
        if self.view_kind == "erofs-blob-map" and not self.path:
            self.path = self.attach_blob_view()
        return super().block_device()

    @property
    def revisions(self):
        return {}

    @property
    def size(self):
        if self.view_kind == "erofs-blob-map" and self.blob_logical_size is not None:
            self._size = self.blob_logical_size
            return self._size
        if self.path and os.path.exists(self.path):
            self._size = os.path.getsize(self.path)
        return self._size

    @property
    def usage(self):
        if self.view_kind == "erofs-blob-map":
            return 0
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
