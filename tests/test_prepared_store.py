"""Prepared guests must boot without their source packages or re-admission."""

import asyncio
import importlib.util
import io
import json
import os
import shlex
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[1]
ENGINE_SOURCE = ROOT / "os/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src"
sys.path.insert(0, str(ENGINE_SOURCE))

from airlock_qubes_storage._engine import core, objects, prepared, views  # noqa: E402


def test_incremental_reuse_needs_no_source_or_payload_read(seed, monkeypatch):
    args, _, systems = seed
    monkeypatch.setattr(objects, "tree_hash", lambda *a: pytest.fail("rehashed source"))
    from airlock_qubes_storage._engine import segments

    monkeypatch.setattr(
        segments, "verify_segment_region", lambda *a: pytest.fail("rehashed payload")
    )
    reused = objects.reusable_object(args.pool_root, systems[0])
    assert reused and not reused["admitted"]
    # A partial publication cannot qualify for reuse after a crash.
    meta = segments.object_meta_path(Path(args.pool_root), Path(systems[0]).name)
    meta.unlink()
    assert objects.reusable_object(args.pool_root, systems[0]) is None


def test_incremental_reuse_rejects_truncated_backing(seed):
    args, _, systems = seed
    from airlock_qubes_storage._engine import segments

    meta = core.read_json(
        segments.object_blob_meta_path(Path(args.pool_root), Path(systems[0]).name)
    )
    blob = segments.segment_blob_path(Path(args.pool_root), meta["segment_id"])
    blob.chmod(0o600)
    with blob.open("r+b") as stream:
        stream.truncate(1)
    with pytest.raises(Exception, match="outside backing segment"):
        objects.reusable_object(args.pool_root, systems[0])


def test_incremental_nix_verification_is_nonrecursive_and_rejects_corruption(
    monkeypatch,
):
    calls = []

    def command(argv, **kwargs):
        calls.append(argv)
        return SimpleNamespace(returncode=1, stderr="corrupt new object", stdout="")

    monkeypatch.setattr(core.subprocess, "run", command)
    with pytest.raises(Exception, match="corrupt new object"):
        core.run_verify(["/nix/store/new-object"], "nix-store", recursive=False)
    assert "--recursive" not in calls[0]
    assert calls[0][-1] == "/nix/store/new-object"
    calls.clear()
    assert core.run_verify([], "nix-store", recursive=False)["status"] == "passed"
    assert not calls


@pytest.fixture
def seed_inputs(tmp_path, monkeypatch):
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
    return specs, systems, source


@pytest.fixture
def seed(tmp_path, seed_inputs):
    specs, systems, source = seed_inputs
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


def tree_fingerprints(root):
    return {
        str(path.relative_to(root)): (
            ("symlink", os.readlink(path))
            if path.is_symlink()
            else ("file", core.hash_file(path))
        )
        for path in root.rglob("*")
        if path.is_symlink() or path.is_file()
    }


def test_incremental_generation_verifies_only_additions(
    seed_inputs, tmp_path, monkeypatch
):
    from airlock_qubes_storage._engine import generations

    specs, systems, source = seed_inputs
    calls = []
    monkeypatch.setattr(
        generations,
        "run_verify",
        lambda paths, mode, **kw: calls.append((paths, kw)) or {"status": "passed"},
    )
    args = SimpleNamespace(
        pool_root=str(tmp_path / "pool"),
        domain="usr-home",
        system=systems[0],
        closure_source=specs[0]["closure_source"],
        physical_store_root=str(source),
        segment_target_bytes="",
        verify_mode="incremental",
        root_source="",
        kernel_name="test",
        view_backend="erofs-objectmap",
        approved_by="dom0",
        approval_source="test",
        store_mountpoint="/nix/store",
        store_device="/dev/xvde",
        store_blob_device="/dev/xvdf",
        retain="true",
        activate="false",
    )
    generations.admit_generation(args)
    assert len(calls[0][0]) == 2
    args.system = systems[1]
    args.closure_source = specs[1]["closure_source"]
    generations.admit_generation(args)
    assert calls[1] == ([systems[1]], {"recursive": False})
    # Rollback to an admitted closure needs no source verification.
    args.system = systems[0]
    args.closure_source = specs[0]["closure_source"]
    generations.admit_generation(args)
    assert calls[2] == ([], {"recursive": False})


def test_tar_fragments_ignore_inode_timestamps_and_hardlink_layout(tmp_path):
    source = tmp_path / ("a" * 32 + "-package")
    source.mkdir()
    payload = source / ("long-name-" * 15)
    payload.write_text("identical package contents")
    duplicate = source / "duplicate"
    duplicate.write_bytes(payload.read_bytes())
    (source / "symlink").symlink_to(payload.name)
    first = tmp_path / "first.fragment"
    second = tmp_path / "second.fragment"
    os.utime(payload, ns=(1_000_000_000, 1_000_000_000))
    objects.create_tar_fragment(source, source.name, first)
    # Nix can restore or deduplicate the same content into different inodes.
    # No package bytes, modes, paths or symlink targets have changed.
    before = core.tree_hash(source)
    duplicate.unlink()
    duplicate.hardlink_to(payload)
    os.utime(payload, ns=(2_000_000_000, 3_000_000_000))
    objects.create_tar_fragment(source, source.name, second)
    assert core.tree_hash(source) == before
    assert first.read_bytes() == second.read_bytes()
    with tarfile.open(fileobj=io.BytesIO(first.read_bytes() + b"\0" * 1024)) as tar:
        members = tar.getmembers()
        assert any(member.pax_headers.get("path") for member in members)
        for member in members:
            assert "atime" not in member.pax_headers
            assert "ctime" not in member.pax_headers
            assert member.mtime == 1
        assert (
            tar.extractfile(source.name + "/" + payload.name).read()
            == payload.read_bytes()
        )
        assert tar.getmember(source.name + "/symlink").linkname == payload.name


def test_seed_rebuild_can_reuse_either_cached_output(tmp_path, seed_inputs):
    specs, _, source = seed_inputs
    pool = tmp_path / "pool"
    descriptors = tmp_path / "descriptors"
    runtime = "/var/lib/airlock/closure-store"
    prepared.prepare_pool(pool, descriptors, specs, runtime)
    old_pool, old_descriptors = (
        tmp_path / "cached-pool",
        tmp_path / "cached-descriptors",
    )
    pool.rename(old_pool)
    descriptors.rename(old_descriptors)
    # Rebuild with unchanged contents but different inode timestamps, as after
    # restoring source packages from cache. The previous test missed this.
    for path in source.rglob("*"):
        os.utime(path, ns=(2_000_000_000, 3_000_000_000))
    prepared.prepare_pool(pool, descriptors, specs, runtime)
    assert tree_fingerprints(old_pool) == tree_fingerprints(pool)
    assert tree_fingerprints(old_descriptors) == tree_fingerprints(descriptors)
    # Model Nix retaining either one of a multi-output derivation's outputs.
    assert prepared.verify_outputs(pool, old_descriptors)["status"] == "passed"
    assert prepared.verify_outputs(old_pool, descriptors)["status"] == "passed"


def test_output_validation_rejects_mismatched_manifest(seed):
    args, descriptors, _ = seed
    path = Path(args.pool_root) / "domains/sys-gui/generations/1/manifest.json"
    data = json.loads(path.read_text())
    data["transaction"] = "different-build-transaction"
    path.write_text(json.dumps(data))
    with pytest.raises(core.EngineError, match="integrity check failed.*manifest.json"):
        prepared.verify_outputs(args.pool_root, descriptors)


def test_seed_context_does_not_change_runtime_transactions_or_clock(monkeypatch):
    monkeypatch.setattr(core.time, "strftime", lambda *args: "runtime-clock")
    assert core.utc_now() == "runtime-clock"
    assert core.transaction_id("same") != core.transaction_id("same")
    with pytest.raises(ValueError), core.reproducible_image_build():
        assert core.utc_now() == "1970-01-01T00:00:01Z"
        assert core.transaction_id("same") == core.transaction_id("same")
        assert core.transaction_id("same") != core.transaction_id("different")
        raise ValueError("restore context on failure")
    assert core.utc_now() == "runtime-clock"
    assert core.transaction_id("same") != core.transaction_id("same")


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
        ROOT / "os/lifecycle/runtime/tools/scripts/nix-domain-storage.py",
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
    "damage", ["truncated", "metadata", "missing", "identity", "escape"]
)
def test_prepared_adoption_rejects_corrupt_or_wrong_seed(seed, damage):
    args, _, _ = seed
    pool = Path(args.pool_root)
    if damage == "truncated":
        path = next(pool.rglob("*.blob"))
        path.chmod(0o600)
        data = path.read_bytes()
        path.write_bytes(data[:-1])
    elif damage == "metadata":
        descriptor = json.loads((Path(args.artifact) / "prepared.json").read_text())
        relative = next(
            p for p in descriptor["files"] if p.endswith("store.blob-map.json")
        )
        path = pool / relative
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


def test_bootstrap_trusts_payload_but_full_validation_detects_corruption(
    seed, monkeypatch
):
    args, descriptors, _ = seed
    path = next((Path(args.pool_root) / "objects/segments").glob("*.blob"))
    path.chmod(0o600)
    data = path.read_bytes()
    path.write_bytes(bytes([data[0] ^ 1]) + data[1:])
    original_open = Path.open

    def metadata_only_open(self, *a, **kw):
        if self.suffix == ".blob":
            pytest.fail("bootstrap must not open payload segments")
        return original_open(self, *a, **kw)

    with monkeypatch.context() as patch:
        patch.setattr(Path, "open", metadata_only_open)
        assert prepared.verify_prepared(args) == 1
    with pytest.raises(core.EngineError, match="integrity check failed.*blob"):
        prepared.verify_outputs(args.pool_root, descriptors)


def test_ordinary_store_has_no_prepared_admission(tmp_path):
    assert prepared.verify_prepared(SimpleNamespace(artifact=str(tmp_path))) is None


@pytest.mark.nix
def test_usb_prepared_store_preserves_sealed_view_mode(seed):
    expression = f"""
      let config = (builtins.getFlake \"{ROOT}\").nixosConfigurations.usb-dev.config;
      in {{
        mode = config.airlock.image.preparedStoreMode;
        image = config.system.build.image.drvPath;
        validation = config.system.build.usbPreparedGuestStore.validation.outPath;
        checkCommand = config.system.build.usbPreparedGuestStore.checkCommand;
        preVM = config.system.build.image.preVM;
        dependencies = map toString config.system.extraDependencies;
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
    assert prepared["validation"] in prepared["dependencies"]
    # The command must execute in the disk-image build, not only in a separately
    # cacheable validation derivation. Fail before copying or partitioning.
    assert (
        prepared["preVM"]
        .lstrip()
        .startswith(
            'echo "Validating prepared guest store before image assembly"\n'
            + prepared["checkCommand"]
            + "\n"
        )
    )
    # Execute the real hook prefix against a small pool. A stale successful
    # receipt must not allow assembly to proceed after metadata corruption.
    args, descriptors, _ = seed
    gate, assembly = prepared["preVM"].split(prepared["checkCommand"], 1)
    assert assembly.strip()  # Preserve upstream's image assembly after the gate.
    checker = shlex.join(
        [
            sys.executable,
            "-m",
            "airlock_qubes_storage._engine.prepared",
            "--check",
            args.pool_root,
            str(descriptors),
        ]
    )
    script = gate + checker + '\necho "ASSEMBLY_REACHED"\n'
    env = dict(os.environ, PYTHONPATH=str(ENGINE_SOURCE))
    valid = subprocess.run(
        ["bash", "-e", "-c", script], capture_output=True, text=True, env=env
    )
    assert valid.returncode == 0, valid.stderr
    assert "ASSEMBLY_REACHED" in valid.stdout
    (descriptors / "cached-validation.json").write_text('{"status":"passed"}')
    metadata = next((Path(args.pool_root) / "objects/blob-metadata").glob("*.json"))
    metadata.write_text("{}")
    invalid = subprocess.run(
        ["bash", "-e", "-c", script], capture_output=True, text=True, env=env
    )
    assert invalid.returncode != 0
    assert "integrity check failed" in invalid.stderr
    assert "ASSEMBLY_REACHED" not in invalid.stdout


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
