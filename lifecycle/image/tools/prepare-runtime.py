"""Build Qubes' initial runtime state offline, never against a live qubesd."""

import argparse
import asyncio
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess
import uuid

RUNTIME_QUBES = Path("/var/lib/qubes")
RUNTIME_POOL = Path("/var/lib/airlock/closure-store")


def read(path):
    return json.loads(Path(path).read_text())


def feature(value):
    if isinstance(value, bool):
        return "1" if value else ""
    return str(value)


def register_custom_labels(app, domains, label_factory, max_default_label):
    """Match setup-config's managed labels using Qubes' offline label API."""
    for _, domain in sorted(domains.items()):
        color = domain.get("features", {}).get("airlock-label-color")
        if not color:
            continue
        name = domain.get("label", "green")
        if (
            name not in {"khaki", "light-gray"}
            or not isinstance(color, str)
            or not re.fullmatch(r"0x[0-9a-fA-F]{6}", color)
        ):
            raise ValueError("invalid managed custom label")
        try:
            existing = app.get_label(name)
        except KeyError:
            index = max(max_default_label, *app.labels.keys()) + 1
            app.labels[index] = label_factory(index, color, name)
        else:
            if existing.color.lower().replace("#", "0x") != color.lower():
                raise ValueError("existing custom label colour differs; migration required")


def kernelopts(domain):
    backend = domain["backend"]
    words = (
        domain.get("prefs", {}).get("kernelopts") or "console=hvc0 init=/sbin/init"
    ).split()
    values = dict(word.split("=", 1) for word in words if "=" in word)
    expected = {
        "init": "/sbin/init",
        "airlock.system": backend["system"]["source"],
        "airlock.storedev": "/dev/xvde",
        "airlock.storefmt": "erofs",
        "airlock.storeblobdev": "/dev/xvdf",
    }
    for key, value in expected.items():
        if key in values and values[key] != value:
            raise ValueError(f"conflicting kernel option: {key}")
        if key not in values:
            words.append(f"{key}={value}")
    return " ".join(words)


def admission_for(name, domain, physical_pool):
    from airlock_qubes_storage._engine import state
    from airlock_qubes_storage._engine.root_images import verify_generation_root

    backend = domain["backend"]
    descriptor = read(Path(backend["store"]["source"]) / "prepared.json")
    expected = {
        "domain": name,
        "pool_root": str(RUNTIME_POOL),
        "system": backend["system"]["source"],
        "root_source": backend["root"]["source"],
        "kernel_name": backend["kernel"]["name"],
    }
    if any(descriptor.get(key) != value for key, value in expected.items()):
        raise ValueError(f"prepared store identity mismatch for {name}")
    generation = descriptor["generation"]
    if type(generation) is not int or generation < 1:
        raise ValueError("invalid prepared generation")
    relative = (
        Path("domains") / name / "generations" / str(generation) / "manifest.json"
    )
    manifest = read(physical_pool / relative)
    if not state.generation_is_committed(physical_pool, manifest):
        raise ValueError(f"uncommitted generation for {name}")
    if manifest["system"] != expected["system"] or manifest["domain"] != name:
        raise ValueError(f"manifest identity mismatch for {name}")
    if manifest.get("root", {}).get("source") != expected["root_source"]:
        raise ValueError(f"root source mismatch for {name}")
    root_image = verify_generation_root(physical_pool, manifest)
    store = manifest["store"]
    if store["closure_hash"] != descriptor["closure_hash"]:
        raise ValueError(f"closure mismatch for {name}")
    admission = {
        "root_image": str(root_image),
        "root_artifact": manifest["root"]["artifact"],
        "generation": generation,
        "manifest": str(RUNTIME_POOL / relative),
        "closure_hash": store["closure_hash"],
        "view_image": store["metadata_image"],
        "blob_map": store["blob_map"],
        "blob_logical_size": store["blob_logical_size"],
    }
    metadata = physical_pool / Path(admission["view_image"]).relative_to(RUNTIME_POOL)
    return admission, metadata.stat().st_size


async def prepare(topology, physical_pool, output, storage_helper):
    import qubes
    import qubes.config
    import qubes.ext
    import qubes.vm.appvm  # initialize upstream VM mixins before StandaloneVM
    from airlock_qubes_storage.qubes import ClosurePool
    from airlock_qubes_storage._engine.builder import builder_volume_configs

    # This process only writes image state. Optional extensions include USB
    # discovery callbacks that contact dom0 QubesDB even in offline mode.
    # Keep upstream VM/storage loading, without registering those callbacks.
    qubes.ext.get_extensions = set

    if output.exists():
        raise ValueError("runtime output must not already exist")
    for name, domain in topology["domains"].items():
        if (
            domain["class"] != "StandaloneVM"
            or domain["backend"]["type"] != "nix-closure"
        ):
            raise ValueError(f"unsupported prepared domain: {name}")
        if Path(name).name != name or name in (".", ".."):
            raise ValueError("invalid domain name")
    output.mkdir(parents=True)
    qubes_root = output / "qubes"
    qubes_root.mkdir()
    qubes.config.qubes_base_dir = str(qubes_root)
    qubes.config.defaults["pool_configs"] = {
        "varlibqubes": {
            "name": "varlibqubes",
            "driver": "file",
            "dir_path": str(qubes_root),
        },
        "linux-kernel": {
            "name": "linux-kernel",
            "driver": "linux-kernel",
            "dir_path": str(qubes_root / "vm-kernels"),
        },
    }
    spec = importlib.util.spec_from_file_location("storage_helper", storage_helper)
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    for name, kernel in topology["kernels"].items():
        target = qubes_root / "vm-kernels" / name
        shutil.copytree(kernel["source"], target, symlinks=True)
        target.chmod(0o755)
        (target / "MANAGED_BY_AIRLOCK").write_text(
            f"source={kernel['source']}\nschema=airlock.qubes-kernel.v1\n"
        )

    app = qubes.Qubes.create_empty_store(
        str(qubes_root / "qubes.xml"), offline_mode=True
    )
    app.events_enabled = False
    app.default_kernel = ""
    app.default_template = None
    register_custom_labels(
        app, topology["domains"], qubes.Label, qubes.config.max_default_label
    )
    app.pools["airlock-closure"] = ClosurePool(
        name="airlock-closure", root_dir=str(RUNTIME_POOL)
    )
    for name, domain in topology["domains"].items():
        vm = app.add_new_vm(
            "StandaloneVM",
            name=name,
            label=domain.get("label", "green"),
            uuid=uuid.uuid5(uuid.NAMESPACE_URL, "airlock-image:" + name),
        )
        # Offline property assignment must not create native autostart links or
        # contact device backends. Runtime reconciliation owns hardware effects.
        vm.events_enabled = False
    for name, domain in topology["domains"].items():
        vm = app.domains[name]
        backend = domain["backend"]
        vm.netvm = None
        for key, value in domain.get("prefs", {}).items():
            if key in ("netvm", "guivm", "kernelopts", "kernel"):
                continue
            setattr(vm, key, value)
        vm.kernel = backend["kernel"]["name"]
        vm.kernelopts = kernelopts(domain)
        for key, value in domain.get("features", {}).items():
            vm.features[key] = feature(value)
        for key, value in domain.get("services", {}).items():
            vm.features["service." + key] = feature(value)
        admission, metadata_size = admission_for(name, domain, physical_pool)
        root_source = Path(admission.pop("root_image"))
        vm_dir = Path(vm.dir_path)
        vm_dir.mkdir(parents=True)
        # Use Qubes' file-volume implementation to create correctly sized sparse
        # disks. Software-store volumes are registered only after this step.
        private_size = (
            backend["state"].get("persistent_size")
            or qubes.config.defaults["private_img_size"]
        )
        for volume_name, size in (
            ("root", root_source.stat().st_size),
            ("private", private_size),
        ):
            volume_config = vm.volume_config[volume_name].copy()
            volume_config["size"] = size
            vm.volume_config[volume_name] = volume_config
            volume = vm.storage.init_volume(volume_name, volume_config.copy())
            await helper.maybe_await(volume.create())
            if volume_name == "root":
                subprocess.run(
                    [
                        "cp",
                        "--sparse=always",
                        "--reflink=auto",
                        str(root_source),
                        volume.path,
                    ],
                    check=True,
                )
            else:
                subprocess.run(["mkfs.ext4", "-q", "-F", volume.path], check=True)
        configs = helper.shared_volume_configs(
            "airlock-closure", name, admission, metadata_size=metadata_size
        )
        if name in ("usr-assistant", "sys-ctrl"):
            configs.update(
                builder_volume_configs(
                    RUNTIME_POOL, "airlock-closure", domain=name, physical_root=physical_pool
                )
            )
        for volume_name, volume_config in configs.items():
            vm.volume_config[volume_name] = volume_config.copy()
            vm.storage.init_volume(volume_name, volume_config.copy())
        helper.validate_shared_store_frontends(
            vm, name, "/dev/xvde", "/dev/xvdf", "erofs-objectmap"
        )
        vm.features.update(
            {
                "airlock-backend": "nix-closure",
                "airlock-root-source": backend["root"]["source"],
                "airlock-root-artifact": admission["root_artifact"]["sha256"],
                "airlock-system-source": backend["system"]["source"],
                "airlock-store-source": backend["store"]["source"],
                "airlock-store-mode": "shared-closure",
                "airlock-store-view-backend": "erofs-objectmap",
                "airlock-store-filesystem": "erofs",
                "airlock-store-generation": str(admission["generation"]),
                "airlock-store-closure-hash": admission["closure_hash"],
                "airlock-store-view-image": admission["view_image"],
                "airlock-store-blob-map": admission["blob_map"],
                "airlock-store-blob-volume": "store-data",
                "airlock-store-mountpoint": "/nix/store",
                "airlock-store-device": "/dev/xvde",
                "airlock-store-blob-device": "/dev/xvdf",
                "airlock-store-read-only": "True",
            }
        )
        selection = output / "pool-selections/domains" / name
        selection.mkdir(parents=True)
        (selection / "current").symlink_to(f"generations/{admission['generation']}")
    # References are assigned after every VM and provider flag exists.
    for name, domain in topology["domains"].items():
        vm = app.domains[name]
        vm.netvm = domain.get("netvm")
        vm.guivm = domain.get("prefs", {}).get("guivm")
    for key, value in topology.get("prefs", {}).items():
        setattr(app, key, value)
    app.save()
    # Reload through upstream Qubes before publication, exercising its parser
    # and the storage drivers rather than merely trusting our serialized XML.
    app.close()
    checked = qubes.Qubes(str(qubes_root / "qubes.xml"), offline_mode=True)
    for name, domain in topology["domains"].items():
        vm = checked.domains[name]
        if vm.label.name != domain.get("label", "green"):
            raise ValueError(f"label did not survive reload: {name}")
        color = domain.get("features", {}).get("airlock-label-color")
        if color and vm.label.color.lower().replace("#", "0x") != color.lower():
            raise ValueError(f"custom label colour did not survive reload: {name}")
        helper.validate_native_volume_isolation(checked, vm)
        if vm.features["airlock-root-source"] != domain["backend"]["root"]["source"]:
            raise ValueError(f"root identity did not survive reload: {name}")
        helper.validate_shared_store_frontends(
            vm, name, "/dev/xvde", "/dev/xvdf", "erofs-objectmap"
        )
        if helper.shared_volume_needs_mutation(
            vm, domain["backend"]["store"]["source"], "erofs-objectmap"
        ):
            raise ValueError(f"prepared store would require boot-time mutation: {name}")
    checked.close()
    # The file pool was created under the build output; serialize installed
    # paths without inventing an alternative Qubes XML schema.
    xml = qubes_root / "qubes.xml"
    xml.write_text(xml.read_text().replace(str(qubes_root), str(RUNTIME_QUBES)))
    (output / "receipt.json").write_text(
        json.dumps(
            {
                "schema": "airlock.prepared-runtime.v1",
                "domains": sorted(topology["domains"]),
                "hardware_assignments": "deferred-to-boot",
            },
            indent=2,
        )
        + "\n"
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("topology", type=Path)
    parser.add_argument("pool", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("storage_helper", type=Path)
    args = parser.parse_args()
    asyncio.run(
        prepare(read(args.topology), args.pool, args.output, args.storage_helper)
    )


if __name__ == "__main__":
    main()
