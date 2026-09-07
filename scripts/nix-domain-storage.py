import asyncio
import json
import os
import shutil
import sys


def fail(message):
    print(f"nixome-nix-domain-storage: {message}", file=sys.stderr)
    raise SystemExit(1)


def usage():
    fail("usage: nixome-nix-domain-storage ensure-store DOMAIN IMAGE SOURCE")


async def maybe_await(value):
    if asyncio.iscoroutine(value):
        return await value
    return value


def pool_name(pool):
    return getattr(pool, "name", str(pool))


def load_qubes():
    try:
        import qubes
        import qubes.storage
    except Exception as exc:
        fail(f"cannot import Qubes core-admin Python modules: {exc}")
    return qubes


async def import_image(vm, volume_name, image_path):
    size = os.stat(image_path).st_size
    import_path = await maybe_await(vm.storage.import_data(volume_name, size))
    success = False
    try:
        with open(image_path, "rb") as src, open(import_path, "wb") as dst:
            shutil.copyfileobj(src, dst, length=16 * 1024 * 1024)
        success = True
    finally:
        await maybe_await(vm.storage.import_data_end(volume_name, success))


async def ensure_store(domain, image_path, source):
    qubes = load_qubes()

    if not os.path.isfile(image_path):
        fail(f"store image does not exist: {image_path}")

    app = qubes.Qubes()
    if domain not in app.domains:
        fail(f"domain does not exist: {domain}")

    vm = app.domains[domain]
    vm_class = getattr(vm, "klass", vm.__class__.__name__)
    if vm_class != "StandaloneVM":
        fail(f"domain is not a StandaloneVM: {domain}")

    if vm.storage is None:
        vm.storage = qubes.storage.Storage(vm)

    image_size = os.stat(image_path).st_size
    existing_source = vm.features.get("nixome-store-source", None)
    volume_created = False
    metadata_changed = False
    image_imported = False

    if "store" not in vm.volumes:
        if vm.is_running():
            fail(f"store volume is missing but domain is running: {domain}")

        root_pool = pool_name(vm.volumes["root"].pool)
        volume_config = {
            "name": "store",
            "pool": root_pool,
            "size": image_size,
            "rw": True,
            "snap_on_start": False,
            "save_on_stop": True,
            "source": None,
            "revisions_to_keep": 0,
        }
        vm.volume_config["store"] = volume_config.copy()
        store_volume = vm.storage.init_volume("store", volume_config.copy())
        await maybe_await(store_volume.create())
        volume_created = True
        metadata_changed = True
    else:
        store_volume = vm.volumes["store"]
        if store_volume.snap_on_start:
            fail(f"store volume must not use snap_on_start: {domain}")
        if not store_volume.save_on_stop:
            fail(f"store volume must use save_on_stop: {domain}")
        if store_volume.source is not None:
            fail(f"store volume must not have a source volume: {domain}")

    should_import = volume_created or existing_source != source
    if should_import:
        if vm.is_running():
            fail(f"store image changed but domain is running: {domain}")

        original_rw = bool(store_volume.rw)
        if not store_volume.rw:
            store_volume.rw = True
            metadata_changed = True

        await import_image(vm, "store", image_path)
        image_imported = True

        if not original_rw:
            store_volume.rw = False
            metadata_changed = True

    if store_volume.rw:
        if vm.is_running():
            fail(f"store volume is writable but domain is running: {domain}")
        store_volume.rw = False
        metadata_changed = True

    if metadata_changed or image_imported:
        app.save()

    result = {
        "schema": "nixome.nix-domain-storage.v1",
        "domain": domain,
        "volume": "store",
        "image": image_path,
        "source": source,
        "read_only": True,
        "volume_created": volume_created,
        "image_imported": image_imported,
        "metadata_changed": metadata_changed,
        "qubesd_restart_required": metadata_changed or image_imported,
    }
    print(json.dumps(result, sort_keys=True))


def main(argv):
    if len(argv) != 5 or argv[1] != "ensure-store":
        usage()
    asyncio.run(ensure_store(argv[2], argv[3], argv[4]))


if __name__ == "__main__":
    main(sys.argv)
