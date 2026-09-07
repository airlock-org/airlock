import argparse
import asyncio
import json
import os
import shutil
import subprocess
import sys


def fail(message):
    print(f"nixome-nix-domain-storage: {message}", file=sys.stderr)
    raise SystemExit(1)


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


def systemctl_available():
    return shutil.which("systemctl") is not None


def qubesd_is_active():
    forced = os.environ.get("NIXOME_NIX_DOMAIN_STORAGE_QUBESD_ACTIVE")
    if forced is not None:
        return forced in ("1", "true", "yes")
    if not systemctl_available():
        return False
    return (
        subprocess.run(
            ["systemctl", "is-active", "--quiet", "qubesd.service"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        ).returncode
        == 0
    )


def run_systemctl(action):
    subprocess.run(
        ["systemctl", action, "qubesd.service"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=True,
        timeout=60,
    )


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


def legacy_volume_needs_mutation(vm, image_path, source):
    image_size = os.stat(image_path).st_size
    existing_source = vm.features.get("nixome-store-source", None)
    if "store" not in vm.volumes:
        return True
    store_volume = vm.volumes["store"]
    return (
        existing_source != source
        or bool(store_volume.rw)
        or int(store_volume.size) != int(image_size)
    )


def shared_volume_needs_mutation(vm, source, view_backend):
    existing_source = vm.features.get("nixome-store-source", None)
    existing_mode = vm.features.get("nixome-store-mode", None)
    existing_view_backend = vm.features.get("nixome-store-view-backend", None)
    if "store" not in vm.volumes:
        return True
    store_volume = vm.volumes["store"]
    if existing_view_backend != view_backend:
        return True
    base_needs_mutation = (
        existing_source != source
        or existing_mode != "shared-closure"
        or bool(store_volume.rw)
        or bool(store_volume.snap_on_start)
        or not bool(store_volume.save_on_stop)
        or getattr(store_volume, "source", None) is not None
    )
    if view_backend != "erofs-blobmap":
        return base_needs_mutation or "store-data" in vm.volumes
    if "store-data" not in vm.volumes:
        return True
    blob_volume = vm.volumes["store-data"]
    return (
        base_needs_mutation
        or bool(blob_volume.rw)
        or bool(blob_volume.snap_on_start)
        or not bool(blob_volume.save_on_stop)
        or getattr(blob_volume, "source", None) is not None
    )


def validate_vm(vm, domain):
    vm_class = getattr(vm, "klass", vm.__class__.__name__)
    if vm_class != "StandaloneVM":
        fail(f"domain is not a StandaloneVM: {domain}")
    if vm.storage is None:
        import qubes.storage

        vm.storage = qubes.storage.Storage(vm)


def validate_legacy_store_volume(vm, domain, image_path):
    image_size = os.stat(image_path).st_size
    if "store" not in vm.volumes:
        fail(f"store volume was not persisted for domain: {domain}")
    store_volume = vm.volumes["store"]
    if store_volume.snap_on_start:
        fail(f"store volume must not use snap_on_start: {domain}")
    if not store_volume.save_on_stop:
        fail(f"store volume must use save_on_stop: {domain}")
    if store_volume.source is not None:
        fail(f"store volume must not have a source volume: {domain}")
    if store_volume.rw:
        fail(f"store volume must be read-only after import: {domain}")
    if int(store_volume.size) != int(image_size):
        fail(
            "store volume size does not match imported image for "
            f"{domain}: {store_volume.size} != {image_size}"
        )


def validate_shared_store_volume(vm, domain, expected):
    if "store" not in vm.volumes:
        fail(f"store volume was not persisted for domain: {domain}")
    store_volume = vm.volumes["store"]
    if store_volume.snap_on_start:
        fail(f"shared store volume must not use snap_on_start: {domain}")
    if not store_volume.save_on_stop:
        fail(f"shared store volume must use save_on_stop: {domain}")
    if store_volume.source is not None:
        fail(f"shared store volume must not have a source volume: {domain}")
    if store_volume.rw:
        fail(f"shared store volume must be read-only: {domain}")
    if pool_name(store_volume.pool) != expected["pool_name"]:
        fail(
            "shared store volume pool mismatch for "
            f"{domain}: {pool_name(store_volume.pool)} != {expected['pool_name']}"
        )
    actual_path = getattr(store_volume, "path", None)
    if actual_path is not None and os.path.abspath(actual_path) != os.path.abspath(expected["view_image"]):
        fail(
            "shared store volume path mismatch for "
            f"{domain}: {actual_path} != {expected['view_image']}"
        )
    if int(store_volume.size) != int(os.stat(expected["view_image"]).st_size):
        fail(
            "shared store volume size mismatch for "
            f"{domain}: {store_volume.size} != {os.stat(expected['view_image']).st_size}"
        )
    if expected.get("view_backend") == "erofs-blobmap":
        if "store-data" not in vm.volumes:
            fail(f"shared store blob volume was not persisted for domain: {domain}")
        blob_volume = vm.volumes["store-data"]
        if blob_volume.snap_on_start:
            fail(f"shared store blob volume must not use snap_on_start: {domain}")
        if not blob_volume.save_on_stop:
            fail(f"shared store blob volume must use save_on_stop: {domain}")
        if blob_volume.source is not None:
            fail(f"shared store blob volume must not have a source volume: {domain}")
        if blob_volume.rw:
            fail(f"shared store blob volume must be read-only: {domain}")
        if pool_name(blob_volume.pool) != expected["pool_name"]:
            fail(
                "shared store blob volume pool mismatch for "
                f"{domain}: {pool_name(blob_volume.pool)} != {expected['pool_name']}"
            )
        blob_map_path = getattr(blob_volume, "blob_map_path", None)
        if blob_map_path is not None and os.path.abspath(blob_map_path) != os.path.abspath(expected["blob_map"]):
            fail(
                "shared store blob map mismatch for "
                f"{domain}: {blob_map_path} != {expected['blob_map']}"
            )
        if int(blob_volume.size) != int(expected["blob_logical_size"]):
            fail(
                "shared store blob volume size mismatch for "
                f"{domain}: {blob_volume.size} != {expected['blob_logical_size']}"
            )


def load_domain(qubes, domain):
    app = qubes.Qubes()
    if domain not in app.domains:
        fail(f"domain does not exist: {domain}")
    vm = app.domains[domain]
    validate_vm(vm, domain)
    return app, vm


async def ensure_legacy_store_loaded(qubes, domain, image_path, source):
    app, vm = load_domain(qubes, domain)

    image_size = os.stat(image_path).st_size
    existing_source = vm.features.get("nixome-store-source", None)
    volume_created = False
    metadata_changed = False
    image_imported = False
    size_changed = False

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
        size_changed = int(store_volume.size) != int(image_size)
        if store_volume.snap_on_start:
            fail(f"store volume must not use snap_on_start: {domain}")
        if not store_volume.save_on_stop:
            fail(f"store volume must use save_on_stop: {domain}")
        if store_volume.source is not None:
            fail(f"store volume must not have a source volume: {domain}")

    should_import = volume_created or existing_source != source or size_changed
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
        app.save(lock=False)

    verify_app, verify_vm = load_domain(qubes, domain)
    validate_legacy_store_volume(verify_vm, domain, image_path)
    verify_app.close()
    app.close()

    return {
        "schema": "nixome.nix-domain-storage.v1",
        "domain": domain,
        "volume": "store",
        "mode": "closure-image",
        "image": image_path,
        "source": source,
        "read_only": True,
        "volume_created": volume_created,
        "image_imported": image_imported,
        "metadata_changed": metadata_changed,
        "qubesd_restart_required": metadata_changed or image_imported,
        "qubesd_restarted": False,
    }


def run_engine(argv):
    try:
        from nixome_qubes_storage import engine
    except Exception as exc:
        fail(f"cannot import Nixome closure-store engine: {exc}")
    try:
        return engine.run(argv)
    except engine.EngineError as exc:
        fail(str(exc))


async def ensure_pool(app, pool_name, pool_root):
    if hasattr(app, "pools") and pool_name in app.pools:
        return app.pools[pool_name], False
    if not hasattr(app, "add_pool"):
        fail(f"Qubes API cannot create storage pool {pool_name}")
    pool = await maybe_await(
        app.add_pool(
            name=pool_name,
            driver="nixome-closure",
            root_dir=pool_root,
            revisions_to_keep=0,
        )
    )
    return pool, True


def shared_volume_configs(pool_name, domain, admission):
    generation = str(admission["generation"])
    view_image = admission.get("metadata_image") or admission["view_image"]
    store = {
        "name": "store",
        "pool": pool_name,
        "vid": f"domains/{domain}/generations/{generation}/store",
        "size": os.stat(view_image).st_size,
        "rw": False,
        "snap_on_start": False,
        "save_on_stop": True,
        "source": None,
        "revisions_to_keep": 0,
        "domain_name": domain,
        "generation": generation,
        "closure_hash": admission["closure_hash"],
        "view_kind": "metadata-image",
        "view_path": view_image,
        "manifest_path": admission["manifest"],
    }
    configs = {"store": store}
    if admission.get("view_backend") == "erofs-blobmap":
        configs["store-data"] = {
            "name": "store-data",
            "pool": pool_name,
            "vid": f"domains/{domain}/generations/{generation}/store-data",
            "size": int(admission["blob_logical_size"]),
            "rw": False,
            "snap_on_start": False,
            "save_on_stop": True,
            "source": None,
            "revisions_to_keep": 0,
            "domain_name": domain,
            "generation": generation,
            "closure_hash": admission["closure_hash"],
            "view_kind": "erofs-blob-map",
            "blob_map_path": admission["blob_map"],
            "blob_logical_size": int(admission["blob_logical_size"]),
            "manifest_path": admission["manifest"],
        }
    return configs


async def ensure_shared_store_loaded(qubes, args, admission):
    app, vm = load_domain(qubes, args.domain)
    _pool, pool_created = await ensure_pool(app, args.pool_name, args.pool_root)
    volume_created = False
    volumes_created = []
    metadata_changed = pool_created

    desired = shared_volume_configs(args.pool_name, args.domain, admission)
    existing = vm.volume_config.get("store", {})
    current_volume = vm.volumes.get("store")
    current_path = getattr(current_volume, "path", None) if current_volume is not None else None
    current_generation = getattr(current_volume, "generation", None) if current_volume is not None else None
    current_hash = getattr(current_volume, "closure_hash", None) if current_volume is not None else None
    current_blob_volume = vm.volumes.get("store-data")
    current_blob_generation = getattr(current_blob_volume, "generation", None) if current_blob_volume is not None else None
    current_blob_hash = getattr(current_blob_volume, "closure_hash", None) if current_blob_volume is not None else None

    needs_volume = (
        current_volume is None
        or existing.get("pool") != args.pool_name
        or str(existing.get("generation", "")) != str(desired["store"]["generation"])
        or existing.get("closure_hash") != desired["store"]["closure_hash"]
        or existing.get("view_path") != desired["store"]["view_path"]
        or str(current_generation or "") != str(desired["store"]["generation"])
        or current_hash != desired["store"]["closure_hash"]
        or current_path != desired["store"]["view_path"]
        or bool(getattr(current_volume, "rw", False))
    )
    if "store-data" in desired:
        existing_blob = vm.volume_config.get("store-data", {})
        needs_volume = (
            needs_volume
            or current_blob_volume is None
            or existing_blob.get("pool") != args.pool_name
            or str(existing_blob.get("generation", "")) != str(desired["store-data"]["generation"])
            or existing_blob.get("closure_hash") != desired["store-data"]["closure_hash"]
            or existing_blob.get("blob_map_path") != desired["store-data"]["blob_map_path"]
            or str(current_blob_generation or "") != str(desired["store-data"]["generation"])
            or current_blob_hash != desired["store-data"]["closure_hash"]
            or bool(getattr(current_blob_volume, "rw", False))
        )
    elif current_blob_volume is not None:
        needs_volume = True

    if needs_volume:
        if vm.is_running():
            fail(f"shared store generation changed but domain is running: {args.domain}")
        if "store-data" not in desired and "store-data" in vm.volume_config:
            del vm.volume_config["store-data"]
            vm.volumes.pop("store-data", None)
        vm.volume_config["store"] = desired["store"].copy()
        store_volume = vm.storage.init_volume("store", desired["store"].copy())
        await maybe_await(store_volume.create())
        if current_volume is None:
            volumes_created.append("store")
        if "store-data" in desired:
            vm.volume_config["store-data"] = desired["store-data"].copy()
            blob_volume = vm.storage.init_volume("store-data", desired["store-data"].copy())
            await maybe_await(blob_volume.create())
            if current_blob_volume is None:
                volumes_created.append("store-data")
        volume_created = bool(volumes_created)
        metadata_changed = True

    if metadata_changed:
        app.save(lock=False)

    switch_result = run_engine(
        [
            "switch",
            "--pool-root",
            args.pool_root,
            "--domain",
            args.domain,
            "--generation",
            str(admission["generation"]),
        ]
    )

    verify_app, verify_vm = load_domain(qubes, args.domain)
    validate_shared_store_volume(
        verify_vm,
        args.domain,
        {
            "pool_name": args.pool_name,
            "view_image": admission.get("metadata_image") or admission["view_image"],
            "view_backend": admission.get("view_backend"),
            "blob_map": admission.get("blob_map"),
            "blob_logical_size": admission.get("blob_logical_size"),
        },
    )
    verify_app.close()
    app.close()

    return {
        "schema": "nixome.nix-domain-storage.v1",
        "domain": args.domain,
        "volume": "store",
        "mode": "shared-closure",
        "source": args.source,
        "read_only": True,
        "volume_created": volume_created,
        "volumes_created": volumes_created,
        "image_imported": False,
        "view_created": admission["view_created"],
        "metadata_changed": metadata_changed,
        "qubesd_restart_required": metadata_changed,
        "qubesd_restarted": False,
        "pool": args.pool_name,
        "pool_created": pool_created,
        "pool_root": args.pool_root,
        "generation": admission["generation"],
        "previous_generation": admission["previous_generation"],
        "closure_hash": admission["closure_hash"],
        "manifest": admission["manifest"],
        "view_image": admission["view_image"],
        "canonical_view_image": admission["canonical_view_image"],
        "metadata_image": admission.get("metadata_image"),
        "canonical_metadata_image": admission.get("canonical_metadata_image"),
        "blob_map": admission.get("blob_map"),
        "canonical_blob_map": admission.get("canonical_blob_map"),
        "blob_device": admission.get("blob_device"),
        "blob_volume": admission.get("blob_volume"),
        "blob_logical_size": admission.get("blob_logical_size"),
        "filesystem": admission.get("filesystem"),
        "view_backend": admission.get("view_backend"),
        "view_size": admission["view_size"],
        "objects_total": admission["objects_total"],
        "objects_admitted": admission["objects_admitted"],
        "objects_reused": admission["objects_reused"],
        "chunks_total": admission.get("chunks_total", 0),
        "chunks_admitted": admission.get("chunks_admitted", 0),
        "chunks_reused": admission.get("chunks_reused", 0),
        "engine_current_generation": switch_result["current_generation"],
    }


async def with_qubesd_restart(needs_mutation, operation):
    qubesd_was_active = False
    qubesd_stopped = False
    if needs_mutation:
        qubesd_was_active = qubesd_is_active()
        if qubesd_was_active:
            run_systemctl("stop")
            qubesd_stopped = True

    try:
        result = await operation()
    finally:
        if qubesd_stopped:
            run_systemctl("start")

    if qubesd_stopped:
        result["qubesd_restart_required"] = False
        result["qubesd_restarted"] = True
    result["qubesd_was_active"] = qubesd_was_active
    return result


async def ensure_legacy_store(args):
    qubes = load_qubes()

    if not os.path.isfile(args.artifact):
        fail(f"store image does not exist: {args.artifact}")

    app, vm = load_domain(qubes, args.domain)
    needs_mutation = legacy_volume_needs_mutation(vm, args.artifact, args.source)
    app.close()

    return await with_qubesd_restart(
        needs_mutation,
        lambda: ensure_legacy_store_loaded(qubes, args.domain, args.artifact, args.source),
    )


async def ensure_shared_store(args):
    qubes = load_qubes()

    app, vm = load_domain(qubes, args.domain)
    needs_mutation = shared_volume_needs_mutation(vm, args.source, args.view_backend)
    vm_running = vm.is_running()
    generation = vm.features.get("nixome-store-generation", "")
    closure_hash = vm.features.get("nixome-store-closure-hash", "")
    view_backend = vm.features.get("nixome-store-view-backend", "")
    view_image = vm.features.get(
        "nixome-store-view-image",
        getattr(vm.volumes.get("store"), "path", "") if "store" in vm.volumes else "",
    )
    store_pool = pool_name(vm.volumes["store"].pool) if "store" in vm.volumes else ""
    app.close()

    if not needs_mutation:
        return {
            "schema": "nixome.nix-domain-storage.v1",
            "domain": args.domain,
            "volume": "store",
            "mode": "shared-closure",
            "source": args.source,
            "read_only": True,
            "volume_created": False,
            "volumes_created": [],
            "image_imported": False,
            "view_created": False,
            "metadata_changed": False,
            "qubesd_restart_required": False,
            "qubesd_restarted": False,
            "qubesd_was_active": False,
            "pool": store_pool,
            "generation": generation,
            "closure_hash": closure_hash,
            "view_image": view_image,
            "canonical_view_image": view_image,
            "view_backend": view_backend,
        }

    if vm_running:
        fail(f"shared store source changed but domain is running: {args.domain}")

    def operation():
        admission = run_engine(
            [
                "admit",
                "--pool-root",
                args.pool_root,
                "--domain",
                args.domain,
                "--system",
                args.system,
                "--closure-source",
                args.artifact,
                "--root-source",
                args.root,
                "--kernel-name",
                args.kernel,
                "--store-device",
                args.device,
                "--store-blob-device",
                args.blob_device,
                "--store-mountpoint",
                args.mountpoint,
                "--verify-mode",
                args.verify_mode,
                "--additional-space-mib",
                str(args.additional_space_mib),
                "--view-backend",
                args.view_backend,
                "--chunk-size",
                str(args.chunk_size),
                "--approval-source",
                args.source,
                "--activate",
                "false",
            ]
        )
        return ensure_shared_store_loaded(qubes, args, admission)

    return await with_qubesd_restart(True, operation)


async def ensure_store(args):
    if args.mode == "closure-image":
        return await ensure_legacy_store(args)
    if args.mode == "shared-closure":
        missing = [
            name
            for name in ["system", "root", "kernel", "device", "mountpoint"]
            if not getattr(args, name)
        ]
        if missing:
            fail(f"shared-closure mode requires: {', '.join(missing)}")
        return await ensure_shared_store(args)
    fail(f"unsupported store mode: {args.mode}")


def build_parser():
    parser = argparse.ArgumentParser(prog="nixome-nix-domain-storage")
    sub = parser.add_subparsers(dest="command", required=True)

    ensure = sub.add_parser("ensure-store")
    ensure.add_argument("domain")
    ensure.add_argument("artifact")
    ensure.add_argument("source")
    ensure.add_argument("--mode", choices=["closure-image", "shared-closure"], default="closure-image")
    ensure.add_argument("--system", default="")
    ensure.add_argument("--root", default="")
    ensure.add_argument("--kernel", default="")
    ensure.add_argument("--device", default="/dev/xvde")
    ensure.add_argument("--blob-device", default="/dev/xvdf")
    ensure.add_argument("--mountpoint", default="/nix/store")
    ensure.add_argument("--pool-name", default=os.environ.get("NIXOME_CLOSURE_STORE_POOL", "nixome-closure"))
    ensure.add_argument("--pool-root", default=os.environ.get("NIXOME_CLOSURE_STORE_ROOT", "/var/lib/nixome/closure-store"))
    ensure.add_argument("--verify-mode", default=os.environ.get("NIXOME_CLOSURE_STORE_VERIFY", "nix-store"))
    ensure.add_argument("--additional-space-mib", type=int, default=256)
    ensure.add_argument(
        "--view-backend",
        choices=["ext4-image", "erofs-blobmap"],
        default=os.environ.get("NIXOME_CLOSURE_STORE_VIEW_BACKEND", "erofs-blobmap"),
    )
    ensure.add_argument(
        "--chunk-size",
        type=int,
        default=int(os.environ.get("NIXOME_CLOSURE_STORE_CHUNK_SIZE", str(64 * 1024))),
    )
    ensure.set_defaults(func=ensure_store)

    return parser


def main(argv):
    args = build_parser().parse_args(argv[1:])
    result = asyncio.run(args.func(args))
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main(sys.argv)
