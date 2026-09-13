"""Prepared guests must boot without their source packages or re-admission."""

import asyncio
import importlib.util
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[1]
ENGINE_SOURCE = ROOT / "lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src"
sys.path.insert(0, str(ENGINE_SOURCE))

from airlock_qubes_storage._engine import core, prepared, views  # noqa: E402


@pytest.fixture
def seed(tmp_path, monkeypatch):
    # Exercise the real tar/EROFS format, using the guarded Airlock Nix store
    # already configured by conftest; no block devices or mounts are needed.
    original_require = views.require_tool
    monkeypatch.setattr(
        views,
        "require_tool",
        lambda name, envvar=None: (
            name if name == "mkfs.erofs" else original_require(name, envvar)
        ),
    )
    monkeypatch.setattr(
        views,
        "run_checked",
        lambda command, **kwargs: core.run_checked(
            ["nix", "shell", "nixpkgs#erofs-utils", "-c"] + command, **kwargs
        ),
    )
    monkeypatch.setenv("AIRLOCK_CLOSURE_STORE_INDEX_SOURCE", "file")
    source = tmp_path / "sources"
    source.mkdir()
    common = "/nix/store/" + "a" * 32 + "-shared-package"
    systems = ["/nix/store/" + char * 32 + "-system" for char in ("b", "c")]
    for path in [common] + systems:
        tree = source / Path(path).name
        tree.mkdir()
        (tree / "payload").write_text(path * 20)
    specs = []
    for i, system in enumerate(systems):
        manifest = tmp_path / f"closure-{i}"
        manifest.mkdir()
        (manifest / "manifest.json").write_text("{}")
        (manifest / "store-paths").write_text(common + "\n" + system + "\n")
        specs.append(
            {
                "name": f"variant-{i}",
                "domain": "sys-gui",
                "system": system,
                "root_source": "/nix/store/" + "d" * 32 + "-root/root.img",
                "kernel_name": "test-kernel",
                "closure_source": str(manifest),
                "physical_store_root": str(source),
            }
        )
    build_root = tmp_path / "build-pool"
    installed_root = tmp_path / "installed-pool"
    descriptors = tmp_path / "descriptors"
    prepared.prepare_pool(build_root, descriptors, specs, str(installed_root))
    build_root.rename(installed_root)
    shutil.rmtree(source)
    args = SimpleNamespace(
        artifact=str(descriptors / "variant-0"),
        source=str(descriptors / "variant-0"),
        pool_root=str(installed_root),
        domain="sys-gui",
        system=systems[0],
        root=specs[0]["root_source"],
        kernel="test-kernel",
        device="/dev/xvde",
        blob_device="/dev/xvdf",
        mountpoint="/nix/store",
        view_backend="erofs-objectmap",
    )
    return args, descriptors, systems


def test_prepared_adoption_without_sources_or_repacking(seed, monkeypatch):
    args, descriptors, systems = seed
    pool = Path(args.pool_root)
    before = {
        str(p): (p.stat().st_mtime_ns, core.hash_file(p)) for p in pool.rglob("*.blob")
    }
    assert prepared.verify_prepared(args) == 1
    # Both variants share the common object; only three objects were stored.
    assert len(list((pool / "objects/metadata").glob("*.json"))) == 3
    for path in pool.rglob("*.json"):
        assert "build-pool" not in path.read_text()
    helper_spec = importlib.util.spec_from_file_location(
        "prepared_storage_helper",
        ROOT / "lifecycle/runtime/tools/scripts/nix-domain-storage.py",
    )
    helper = importlib.util.module_from_spec(helper_spec)
    helper_spec.loader.exec_module(helper)
    vm = SimpleNamespace(features={}, volumes={}, is_running=lambda: False)
    monkeypatch.setattr(helper, "load_qubes", lambda: None)
    monkeypatch.setattr(
        helper, "load_domain", lambda *a: (SimpleNamespace(close=lambda: None), vm)
    )
    monkeypatch.setattr(helper, "shared_volume_needs_mutation", lambda *a: True)
    monkeypatch.setattr(
        helper, "run_engine", lambda *a: pytest.fail("must not re-admit")
    )

    async def loaded(_qubes, _args, admission):
        return admission

    async def restart(_needed, operation):
        return await operation()

    monkeypatch.setattr(helper, "ensure_shared_store_loaded", loaded)
    monkeypatch.setattr(helper, "with_qubesd_restart", restart)
    admission = asyncio.run(helper.ensure_shared_store(args))
    assert admission["objects_admitted"] == 0
    assert admission["objects_reused"] == 2
    args.artifact = args.source = str(descriptors / "variant-1")
    args.system = systems[1]
    assert prepared.verify_prepared(args) == 2
    assert before == {
        str(p): (p.stat().st_mtime_ns, core.hash_file(p)) for p in pool.rglob("*.blob")
    }


@pytest.mark.parametrize(
    "damage", ["payload", "metadata", "missing", "identity", "escape"]
)
def test_prepared_adoption_rejects_corrupt_or_wrong_seed(seed, damage):
    args, _, _ = seed
    pool = Path(args.pool_root)
    if damage == "payload":
        path = next(pool.rglob("*.blob"))
        path.chmod(0o600)
        data = path.read_bytes()
        path.write_bytes(bytes([data[0] ^ 1]) + data[1:])
    elif damage == "metadata":
        path = next((pool / "views").rglob("store.blob-map.json"))
        path.write_text("{}")
    elif damage == "missing":
        next(pool.rglob("*.blob")).unlink()
    elif damage == "identity":
        args.system = "/nix/store/" + "e" * 32 + "-wrong"
    else:
        path = Path(args.artifact) / "prepared.json"
        descriptor = json.loads(path.read_text())
        descriptor["files"]["../outside"] = {"size": 0, "sha256": ""}
        path.write_text(json.dumps(descriptor))
    with pytest.raises((core.EngineError, OSError)):
        prepared.verify_prepared(args)


def test_ordinary_store_has_no_prepared_admission(tmp_path):
    assert prepared.verify_prepared(SimpleNamespace(artifact=str(tmp_path))) is None


@pytest.mark.nix
def test_usb_prepared_store_preserves_sealed_view_mode():
    expression = f"""
      let config = (builtins.getFlake \"{ROOT}\").nixosConfigurations.usb-dev.config;
      in {{
        mode = config.airlock.image.preparedStoreMode;
        image = config.system.build.image.drvPath;
      }}
    """
    result = subprocess.run(
        ["nix", "eval", "--impure", "--json", "--expr", expression],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    prepared = json.loads(result.stdout)
    assert prepared["mode"] == "go-rwx"
    assert prepared["image"].endswith(".drv")


@pytest.mark.nix
@pytest.mark.skipif(
    os.environ.get("AIRLOCK_RUN_PREPARED_IMAGE_CHECK") != "1",
    reason="set AIRLOCK_RUN_PREPARED_IMAGE_CHECK=1 for real image closure accounting",
)
def test_prepared_usb_runtime_closure():
    def nix(*args):
        return subprocess.check_output(["nix", *args], text=True, cwd=ROOT)

    attr = ".#nixosConfigurations.usb-dev.config"
    nix(
        "build",
        attr + ".system.build.toplevel",
        attr + ".system.build.usbPreparedGuestStore",
        "--no-link",
    )
    system = nix("eval", attr + ".system.build.toplevel", "--raw")
    payload = nix("eval", attr + ".system.build.usbPreparedGuestStore", "--raw")
    descriptors = nix(
        "eval", attr + ".system.build.usbPreparedGuestStore.descriptors", "--raw"
    )
    topology = json.loads(nix("eval", attr + ".airlock.qubes.topology", "--json"))
    runtime = json.loads(nix("path-info", "--recursive", "--json", system))
    # The project's pinned Nix emits a dictionary keyed by store path.
    if isinstance(runtime, list):
        runtime = {item["path"]: item for item in runtime}
    assert payload not in runtime, "the pool must not be copied into dom0's store too"
    assert descriptors in runtime
    physical_root = Path(os.environ.get("AIRLOCK_NIX_ROOT", "/"))
    descriptor_root = physical_root / descriptors.lstrip("/")
    guest_paths = set()
    for descriptor_path in descriptor_root.glob("*/prepared.json"):
        descriptor = json.loads(descriptor_path.read_text())
        assert descriptor["system"] not in runtime
        guest_paths.update(
            (descriptor_path.parent / "store-paths").read_text().splitlines()
        )
    assert guest_paths
    domain = topology["domains"]["sys-gui"]
    root_artifact = domain["backend"]["root"]["source"].removesuffix("/root.img")
    assert root_artifact in runtime
    assert domain["backend"]["system"]["source"] not in runtime
    for kernel in topology["kernels"].values():
        assert kernel["source"] in runtime
    for path in (payload, descriptors):
        info = json.loads(nix("path-info", "--json", path))
        item = info[0] if isinstance(info, list) else info[path]
        assert (
            item["references"] == []
        ), "seed must not retain its original source closure"
    report = {
        "system": system,
        "payload": payload,
        "descriptors": descriptors,
        "guest_objects": len(guest_paths),
        "guest_only_objects_omitted": len(guest_paths - runtime.keys()),
        "objects_also_needed_by_dom0": len(guest_paths & runtime.keys()),
        "payload_in_dom0_store": False,
    }
    assert report["guest_only_objects_omitted"] > 0
    (ROOT / ".airlock-logs/prepared-usb-closure.json").write_text(
        json.dumps(report, indent=2) + "\n"
    )
