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


def shared_volume_needs_mutation(vm, source, view_backend):
    existing_source = vm.features.get("nixome-store-source", None)
    existing_mode = vm.features.get("nixome-store-mode", None)
    existing_view_backend = vm.features.get("nixome-store-view-backend", None)
    existing_generation = vm.features.get("nixome-store-generation", "")
    existing_closure_hash = vm.features.get("nixome-store-closure-hash", "")
    existing_view_image = vm.features.get("nixome-store-view-image", "")
    existing_blob_map = vm.features.get("nixome-store-blob-map", "")

    if (
        existing_source != source
        or existing_mode != "shared-closure"
        or existing_view_backend != view_backend
    ):
        return True
    if not existing_generation:
        return True
    if "store" not in vm.volumes:
        return True
    store_volume = vm.volumes["store"]
    base_needs_mutation = (
        bool(store_volume.rw)
        or bool(store_volume.snap_on_start)
        or not bool(store_volume.save_on_stop)
        or getattr(store_volume, "source", None) is not None
    )
    if "store-data" not in vm.volumes:
        return True
    blob_volume = vm.volumes["store-data"]

    store_config = vm.volume_config.get("store", {})
    blob_config = vm.volume_config.get("store-data", {})
    store_generation = getattr(store_volume, "generation", None) or store_config.get(
        "generation", ""
    )
    blob_generation = getattr(blob_volume, "generation", None) or blob_config.get(
        "generation", ""
    )
    store_hash = getattr(store_volume, "closure_hash", None) or store_config.get(
        "closure_hash", ""
    )
    blob_hash = getattr(blob_volume, "closure_hash", None) or blob_config.get(
        "closure_hash", ""
    )
    store_path = getattr(store_volume, "path", None) or store_config.get(
        "view_path", ""
    )
    blob_map = getattr(blob_volume, "blob_map_path", None) or blob_config.get(
        "blob_map_path", ""
    )

    def same_path(left, right):
        if not left or not right:
            return str(left or "") == str(right or "")
        return os.path.abspath(str(left)) == os.path.abspath(str(right))

    return (
        base_needs_mutation
        or bool(blob_volume.rw)
        or bool(blob_volume.snap_on_start)
        or not bool(blob_volume.save_on_stop)
        or getattr(blob_volume, "source", None) is not None
        or str(store_generation) != str(existing_generation)
        or str(blob_generation) != str(existing_generation)
        or (existing_closure_hash and store_hash != existing_closure_hash)
        or (existing_closure_hash and blob_hash != existing_closure_hash)
        or (existing_view_image and not same_path(store_path, existing_view_image))
        or (existing_blob_map and not same_path(blob_map, existing_blob_map))
    )


def feature_admission_for_current_store(args, vm):
    generation = vm.features.get("nixome-store-generation", "")
    if not generation:
        return None

    admission = load_generation_admission(args.pool_root, args.domain, generation)
    expected_source = vm.features.get("nixome-store-source", "")
    expected_hash = vm.features.get("nixome-store-closure-hash", "")
    expected_view = vm.features.get("nixome-store-view-image", "")
    expected_blob_map = vm.features.get("nixome-store-blob-map", "")

    admission_sources = {
        source
        for source in [
            admission.get("source", ""),
            admission.get("approval_source", ""),
        ]
        if source
    }
    if expected_source and expected_source not in admission_sources:
        fail(
            "Nix-native store feature/source mismatch for "
            f"{args.domain}: generation {generation} has {sorted(admission_sources)} != {expected_source}"
        )
    if expected_hash and admission.get("closure_hash") != expected_hash:
        fail(
            "Nix-native store feature/hash mismatch for "
            f"{args.domain}: generation {generation} has {admission.get('closure_hash')} != {expected_hash}"
        )
    if expected_view and os.path.abspath(
        admission.get("metadata_image") or admission["view_image"]
    ) != os.path.abspath(expected_view):
        fail(
            "Nix-native store feature/view mismatch for "
            f"{args.domain}: generation {generation} has {admission.get('metadata_image') or admission['view_image']} != {expected_view}"
        )
    if (
        expected_blob_map
        and admission.get("blob_map")
        and os.path.abspath(admission["blob_map"]) != os.path.abspath(expected_blob_map)
    ):
        fail(
            "Nix-native store feature/blob-map mismatch for "
            f"{args.domain}: generation {generation} has {admission['blob_map']} != {expected_blob_map}"
        )
    return admission


def validate_vm(vm, domain):
    vm_class = getattr(vm, "klass", vm.__class__.__name__)
    if vm_class != "StandaloneVM":
        fail(f"domain is not a StandaloneVM: {domain}")
    if vm.storage is None:
        import qubes.storage

        vm.storage = qubes.storage.Storage(vm)


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
    if actual_path is not None and os.path.abspath(actual_path) != os.path.abspath(
        expected["view_image"]
    ):
        fail(
            "shared store volume path mismatch for "
            f"{domain}: {actual_path} != {expected['view_image']}"
        )
    if int(store_volume.size) != int(os.stat(expected["view_image"]).st_size):
        fail(
            "shared store volume size mismatch for "
            f"{domain}: {store_volume.size} != {os.stat(expected['view_image']).st_size}"
        )
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
    if blob_map_path is not None and os.path.abspath(blob_map_path) != os.path.abspath(
        expected["blob_map"]
    ):
        fail(
            "shared store blob map mismatch for "
            f"{domain}: {blob_map_path} != {expected['blob_map']}"
        )
    if int(blob_volume.size) != int(expected["blob_logical_size"]):
        fail(
            "shared store blob volume size mismatch for "
            f"{domain}: {blob_volume.size} != {expected['blob_logical_size']}"
        )


def frontend_name(device_path):
    if device_path.startswith("/dev/"):
        return device_path[len("/dev/") :]
    return device_path


def qubes_boot_frontends(volume_names):
    fixed = {
        "root": "xvda",
        "private": "xvdb",
        "volatile": "xvdc",
        "kernel": "xvdd",
    }
    extra_letters = list("efghijklmnopqrstuvwxy")
    extra_index = 0
    result = {}
    for name in volume_names:
        if name in fixed:
            result[name] = fixed[name]
            continue
        if extra_index >= len(extra_letters):
            fail(
                "too many non-standard boot volumes for deterministic frontend assignment"
            )
        result[name] = f"xvd{extra_letters[extra_index]}"
        extra_index += 1
    return result


def validate_shared_store_frontends(
    vm, domain, store_device, store_blob_device, view_backend
):
    volume_names = list(vm.volumes.keys())
    frontends = qubes_boot_frontends(volume_names)
    if view_backend != "erofs-objectmap":
        fail(f"unsupported Nixome store view backend for {domain}: {view_backend}")
    expected = {
        "store": frontend_name(store_device),
        "store-data": frontend_name(store_blob_device),
    }

    for volume_name, target in expected.items():
        actual = frontends.get(volume_name)
        if actual != target:
            fail(
                "Qubes boot frontend assignment mismatch for "
                f"{domain}:{volume_name}: expected {target}, got {actual or 'absent'}; "
                f"volume order is {', '.join(volume_names)}"
            )

    for volume_name, target in expected.items():
        for other_name, other_target in frontends.items():
            if other_name != volume_name and other_target == target:
                fail(
                    "Qubes boot frontend target is occupied for "
                    f"{domain}:{volume_name}: {target} is also assigned to {other_name}"
                )


def load_domain(qubes, domain):
    app = qubes.Qubes()
    if domain not in app.domains:
        fail(f"domain does not exist: {domain}")
    vm = app.domains[domain]
    validate_vm(vm, domain)
    return app, vm


def run_engine(argv):
    try:
        from nixome_qubes_storage import engine
    except Exception as exc:
        fail(f"cannot import Nixome closure-store engine: {exc}")
    try:
        return engine.run(argv)
    except engine.EngineError as exc:
        fail(str(exc))


def load_generation_admission(pool_root, domain, generation):
    try:
        from nixome_qubes_storage import engine
    except Exception as exc:
        fail(f"cannot import Nixome closure-store engine: {exc}")

    root = engine.ensure_pool(pool_root)
    domain = engine.safe_domain_name(domain)
    manifest_path = engine.generation_manifest_path(root, domain, generation)
    if not os.path.exists(manifest_path):
        fail(f"generation does not exist for {domain}: {generation}")
    manifest = engine.read_json(manifest_path)
    if not engine.generation_is_committed(root, manifest):
        fail(f"generation is not committed for {domain}: {generation}")
    store = manifest.get("store", {})
    view_image = store.get("metadata_image") or store.get("image")
    canonical_view_image = (
        store.get("canonical_metadata_image")
        or store.get("canonical_image")
        or view_image
    )
    if not view_image:
        fail(f"generation is missing store metadata image for {domain}: {generation}")

    return {
        "generation": manifest["generation"],
        "previous_generation": manifest.get("previous_generation"),
        "closure_hash": store["closure_hash"],
        "manifest": str(manifest_path),
        "system": manifest["system"],
        "root_source": manifest.get("root", {}).get("source", ""),
        "kernel_name": manifest.get("kernel", {}).get("name", ""),
        "source": store.get("source", ""),
        "approval_source": manifest.get("approval", {}).get("source", ""),
        "view_created": False,
        "view_image": view_image,
        "canonical_view_image": canonical_view_image,
        "metadata_image": view_image,
        "canonical_metadata_image": canonical_view_image,
        "blob_map": store.get("blob_map"),
        "canonical_blob_map": store.get("canonical_blob_map"),
        "blob_device": store.get("blob_device"),
        "blob_volume": store.get("blob_volume"),
        "blob_logical_size": store.get("blob_logical_size"),
        "filesystem": store.get("filesystem", "erofs"),
        "view_backend": store.get("view_backend", "erofs-objectmap"),
        "view_size": os.path.getsize(view_image),
        "objects_total": store.get("object_count", len(store.get("paths", []))),
        "objects_admitted": 0,
        "objects_reused": store.get("object_count", len(store.get("paths", []))),
    }


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
    configs = {
        "store": store,
        "store-data": {
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
        },
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
    current_path = (
        getattr(current_volume, "path", None) if current_volume is not None else None
    )
    current_generation = (
        getattr(current_volume, "generation", None)
        if current_volume is not None
        else None
    )
    current_hash = (
        getattr(current_volume, "closure_hash", None)
        if current_volume is not None
        else None
    )
    current_blob_volume = vm.volumes.get("store-data")
    current_blob_generation = (
        getattr(current_blob_volume, "generation", None)
        if current_blob_volume is not None
        else None
    )
    current_blob_hash = (
        getattr(current_blob_volume, "closure_hash", None)
        if current_blob_volume is not None
        else None
    )

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
    existing_blob = vm.volume_config.get("store-data", {})
    needs_volume = (
        needs_volume
        or current_blob_volume is None
        or existing_blob.get("pool") != args.pool_name
        or str(existing_blob.get("generation", ""))
        != str(desired["store-data"]["generation"])
        or existing_blob.get("closure_hash") != desired["store-data"]["closure_hash"]
        or existing_blob.get("blob_map_path") != desired["store-data"]["blob_map_path"]
        or str(current_blob_generation or "")
        != str(desired["store-data"]["generation"])
        or current_blob_hash != desired["store-data"]["closure_hash"]
        or bool(getattr(current_blob_volume, "rw", False))
    )

    if needs_volume:
        if vm.is_running():
            fail(
                f"shared store generation changed but domain is running: {args.domain}"
            )
        vm.volume_config["store"] = desired["store"].copy()
        store_volume = vm.storage.init_volume("store", desired["store"].copy())
        await maybe_await(store_volume.create())
        if current_volume is None:
            volumes_created.append("store")
        vm.volume_config["store-data"] = desired["store-data"].copy()
        blob_volume = vm.storage.init_volume("store-data", desired["store-data"].copy())
        await maybe_await(blob_volume.create())
        if current_blob_volume is None:
            volumes_created.append("store-data")
        volume_created = bool(volumes_created)
        metadata_changed = True

    validate_shared_store_frontends(
        vm,
        args.domain,
        args.device,
        args.blob_device,
        admission.get("view_backend", args.view_backend),
    )

    if metadata_changed:
        app.save(lock=False)

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
    validate_shared_store_frontends(
        verify_vm,
        args.domain,
        args.device,
        args.blob_device,
        admission.get("view_backend", args.view_backend),
    )
    verify_app.close()
    app.close()

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

    return {
        "schema": "nixome.nix-domain-storage.v1",
        "domain": args.domain,
        "volume": "store",
        "mode": "shared-closure",
        "source": getattr(args, "source", admission.get("source", "")),
        "read_only": True,
        "volume_created": volume_created,
        "volumes_created": volumes_created,
        "view_created": admission["view_created"],
        "metadata_changed": metadata_changed,
        "qubesd_restart_required": metadata_changed,
        "qubesd_restarted": False,
        "pool": args.pool_name,
        "pool_created": pool_created,
        "pool_root": args.pool_root,
        "generation": admission["generation"],
        "previous_generation": admission["previous_generation"],
        "system": admission.get("system"),
        "root_source": admission.get("root_source"),
        "kernel_name": admission.get("kernel_name"),
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


async def ensure_shared_store(args):
    qubes = load_qubes()

    app, vm = load_domain(qubes, args.domain)
    needs_mutation = shared_volume_needs_mutation(vm, args.source, args.view_backend)
    source_current = (
        vm.features.get("nixome-store-source", None) == args.source
        and vm.features.get("nixome-store-mode", None) == "shared-closure"
        and vm.features.get("nixome-store-view-backend", None) == args.view_backend
    )
    current_admission = (
        feature_admission_for_current_store(args, vm) if source_current else None
    )
    vm_running = vm.is_running()
    generation = vm.features.get("nixome-store-generation", "")
    closure_hash = vm.features.get("nixome-store-closure-hash", "")
    view_backend = vm.features.get("nixome-store-view-backend", "")
    view_image = vm.features.get(
        "nixome-store-view-image",
        getattr(vm.volumes.get("store"), "path", "") if "store" in vm.volumes else "",
    )
    store_pool = pool_name(vm.volumes["store"].pool) if "store" in vm.volumes else ""

    if not needs_mutation:
        validate_shared_store_frontends(
            vm, args.domain, args.device, args.blob_device, view_backend
        )
        app.close()
        return {
            "schema": "nixome.nix-domain-storage.v1",
            "domain": args.domain,
            "volume": "store",
            "mode": "shared-closure",
            "source": args.source,
            "read_only": True,
            "volume_created": False,
            "volumes_created": [],
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

    app.close()

    if vm_running:
        fail(f"shared store source changed but domain is running: {args.domain}")

    def operation():
        if current_admission is not None:
            return ensure_shared_store_loaded(qubes, args, current_admission)

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
                "--view-backend",
                args.view_backend,
                "--approval-source",
                args.source,
                "--activate",
                "false",
            ]
        )
        return ensure_shared_store_loaded(qubes, args, admission)

    return await with_qubesd_restart(True, operation)


async def switch_shared_store(args):
    qubes = load_qubes()
    admission = load_generation_admission(args.pool_root, args.domain, args.generation)
    args.view_backend = admission.get("view_backend", "erofs-objectmap")

    app, vm = load_domain(qubes, args.domain)
    if vm.is_running():
        app.close()
        fail(
            f"shared store generation switch requested but domain is running: {args.domain}"
        )
    app.close()

    return await with_qubesd_restart(
        True,
        lambda: ensure_shared_store_loaded(qubes, args, admission),
    )


async def ensure_store(args):
    if args.mode != "shared-closure":
        fail(f"unsupported store mode: {args.mode}")
    if args.view_backend != "erofs-objectmap":
        fail(f"unsupported store view backend: {args.view_backend}")
    missing = [
        name
        for name in ["system", "root", "kernel", "device", "mountpoint"]
        if not getattr(args, name)
    ]
    if missing:
        fail(f"shared-closure mode requires: {', '.join(missing)}")
    return await ensure_shared_store(args)


def build_parser():
    parser = argparse.ArgumentParser(prog="nixome-nix-domain-storage")
    sub = parser.add_subparsers(dest="command", required=True)

    ensure = sub.add_parser("ensure-store")
    ensure.add_argument("domain")
    ensure.add_argument("artifact")
    ensure.add_argument("source")
    ensure.add_argument("--mode", choices=["shared-closure"], default="shared-closure")
    ensure.add_argument("--system", default="")
    ensure.add_argument("--root", default="")
    ensure.add_argument("--kernel", default="")
    ensure.add_argument("--device", default="/dev/xvde")
    ensure.add_argument("--blob-device", default="/dev/xvdf")
    ensure.add_argument("--mountpoint", default="/nix/store")
    ensure.add_argument(
        "--pool-name",
        default=os.environ.get("NIXOME_CLOSURE_STORE_POOL", "nixome-closure"),
    )
    ensure.add_argument(
        "--pool-root",
        default=os.environ.get(
            "NIXOME_CLOSURE_STORE_ROOT", "/var/lib/nixome/closure-store"
        ),
    )
    ensure.add_argument(
        "--verify-mode",
        default=os.environ.get("NIXOME_CLOSURE_STORE_VERIFY", "nix-store"),
    )
    ensure.add_argument(
        "--view-backend",
        choices=["erofs-objectmap"],
        default=os.environ.get("NIXOME_CLOSURE_STORE_VIEW_BACKEND", "erofs-objectmap"),
    )
    ensure.set_defaults(func=ensure_store)

    switch = sub.add_parser("switch-store")
    switch.add_argument("domain")
    switch.add_argument("generation")
    switch.add_argument("--device", default="/dev/xvde")
    switch.add_argument("--blob-device", default="/dev/xvdf")
    switch.add_argument("--mountpoint", default="/nix/store")
    switch.add_argument(
        "--pool-name",
        default=os.environ.get("NIXOME_CLOSURE_STORE_POOL", "nixome-closure"),
    )
    switch.add_argument(
        "--pool-root",
        default=os.environ.get(
            "NIXOME_CLOSURE_STORE_ROOT", "/var/lib/nixome/closure-store"
        ),
    )
    switch.set_defaults(func=switch_shared_store)

    return parser


def main(argv):
    args = build_parser().parse_args(argv[1:])
    result = asyncio.run(args.func(args))
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main(sys.argv)
