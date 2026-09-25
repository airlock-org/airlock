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
ENGINE_SOURCE = (
    ROOT / "airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src"
)
sys.path.insert(0, str(ENGINE_SOURCE))

from airlock_qubes_storage._engine import (  # noqa: E402
    artifacts,
    core,
    objects,
    prepared,
    views,
)


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
            ["nix", "shell", "path:./airlock-machine#airlock-erofs-utils", "-c"]
            + command,
            **kwargs,
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
    root_image = source / ("d" * 32 + "-root") / "root.img"
    root_image.parent.mkdir()
    with root_image.open("wb") as stream:
        stream.truncate(16 * 1024 * 1024)
        stream.seek(1080)
        stream.write(b"\x53\xef")
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


@pytest.fixture
def builder_seed(seed_inputs, tmp_path, request):
    specs, systems, _ = seed_inputs
    parameter = getattr(request, "param", 0)
    builder_domain = parameter.get("domain", "sys-dev-builder") if isinstance(parameter, dict) else "sys-dev-builder"
    closure = tmp_path / "builder-closure"
    closure.mkdir()
    paths = sorted(
        {
            p
            for spec in specs
            for p in (Path(spec["closure_source"]) / "store-paths")
            .read_text()
            .splitlines()
        }
    )
    (closure / "store-paths").write_text("\n".join(paths) + "\n")
    (closure / "manifest.json").write_text("{}")
    # Real closureInfo framing; these synthetic payloads use tree verification.
    references = paths * (parameter if isinstance(parameter, int) else 0)
    reference_text = "".join(path + "\n" for path in references)
    (closure / "registration").write_text(
        "".join(
            f"{path}\nsha256:{'0' * 52}\n1\n\n{len(references)}\n{reference_text}"
            for path in paths
        )
    )
    builder = dict(
        specs[0],
        name=builder_domain,
        domain=builder_domain,
        builder=True,
        closure_source=str(closure),
    )
    build_root = tmp_path / "builder-pool-build"
    root = tmp_path / "builder-pool-installed"
    descriptors = tmp_path / "builder-descriptors"
    prepared.prepare_pool(build_root, descriptors, [*specs, builder], str(root))
    build_root.rename(root)
    return root, descriptors, paths


@pytest.mark.parametrize("builder_seed", [{"domain": "sys-ctrl-builder"}], indirect=True)
def test_sys_ctrl_builder_has_its_own_pinned_view(builder_seed):
    from airlock_qubes_storage._engine import builder

    root, descriptors, _ = builder_seed
    assert prepared.verify_outputs(root, descriptors)["status"] == "passed"
    assert not (root / "builders/sys-dev.json").exists()
    assert (root / "builders/sys-ctrl.json").exists()
    configs = builder.builder_volume_configs(root, "airlock-closure", domain="sys-ctrl")
    assert set(configs) == {"builder-store", "builder-data"}
    assert all(config["domain_name"] == "sys-ctrl-builder" for config in configs.values())


def test_builder_seed_shares_payload_and_retains_frozen_view_offline(builder_seed):
    from airlock_qubes_storage._engine import builder, generations, maintenance, state

    root, descriptors, paths = builder_seed
    assert prepared.verify_outputs(root, descriptors)["status"] == "passed"
    assert len(list((root / "objects/metadata").glob("*.json"))) == len(paths)
    configs = builder.builder_volume_configs(root, "airlock-closure")
    assert list(configs) == ["builder-store", "builder-data"]
    assert all(not cfg["rw"] and cfg["save_on_stop"] for cfg in configs.values())
    # There is no running VM, no current generation, and no retention fallback.
    for domain, generation in (("sys-gui", 1), ("sys-gui", 2), ("sys-dev-builder", 1)):
        generations.set_retention(
            SimpleNamespace(
                pool_root=str(root),
                domain=domain,
                generation=generation,
                retain="false",
            )
        )
    fingerprints = {
        p: core.hash_file(p) for p in (root / "objects/segments").glob("*.blob")
    }
    cleanup = SimpleNamespace(pool_root=str(root), keep_generations=0)
    for _ in range(2):
        result = maintenance.gc(cleanup)
        assert result["retained_generations"] == [
            {"domain": "sys-dev-builder", "generation": 1}
        ]
        assert builder.builder_volume_configs(root, "airlock-closure") == configs
        assert fingerprints == {p: core.hash_file(p) for p in fingerprints}
    state.clear_running_ref(root, "sys-dev-builder", 1, "builder-persistent")
    with pytest.raises(FileNotFoundError):
        builder.builder_volume_configs(root, "airlock-closure")
    maintenance.gc(cleanup)
    assert all(not p.exists() for p in fingerprints)


def test_builder_registration_accepts_build_time_pool_location(builder_seed):
    from airlock_qubes_storage._engine import builder

    root, _, _ = builder_seed
    runtime = "/var/lib/airlock/closure-store"
    expected = prepared.relocate(
        builder.builder_volume_configs(root, "airlock-closure"), str(root), runtime
    )
    for path in root.rglob("*.json"):
        if not path.is_symlink():
            path.write_text(
                json.dumps(
                    prepared.relocate(json.loads(path.read_text()), str(root), runtime)
                )
            )
    assert (
        builder.builder_volume_configs(runtime, "airlock-closure", physical_root=root)
        == expected
    )


def test_builder_seed_validates_metadata_and_physical_pin(builder_seed):
    from airlock_qubes_storage._engine import builder, state

    root, descriptors, _ = builder_seed
    metadata = Path(
        builder.builder_volume_configs(root, "airlock-closure")["builder-store"][
            "view_path"
        ]
    )
    metadata.chmod(0o644)
    with pytest.raises(core.EngineError, match="read-only"):
        builder.builder_volume_configs(root, "airlock-closure")
    metadata.chmod(0o444)
    pin = state.running_ref_path(root, "sys-dev-builder", 1, "builder-persistent")
    data = core.read_json(pin)
    data["physical_references"] = []
    core.atomic_json(pin, data)
    with pytest.raises(core.EngineError, match="pin"):
        builder.builder_volume_configs(root, "airlock-closure")
    with pytest.raises(core.EngineError, match="integrity"):
        prepared.verify_outputs(root, descriptors)


@pytest.mark.parametrize("source_suffix", ["", "/vendor/nixos-qubes"])
def test_builder_offline_input_manifest_is_bound_to_registered_paths(
    builder_seed, tmp_path, source_suffix
):
    from airlock_qubes_storage._engine import builder

    root, _, paths = builder_seed
    config = builder.builder_volume_configs(root, "airlock-closure")["builder-store"]
    manifest = tmp_path / "flake-inputs.json"
    manifest.write_text(
        json.dumps(
            {
                "schema": "airlock.sys-dev-offline-inputs.v1",
                "lock_hash": "a" * 64,
                "inputs": {"nixpkgs": paths[0] + source_suffix},
            }
        )
    )
    registration = tmp_path / "registration"
    # Use the registration embedded in the original combined index.
    image = Path(config["view_path"])
    blob_map = core.read_json(
        builder.builder_volume_configs(root, "airlock-closure")["builder-data"][
            "blob_map_path"
        ]
    )
    blob = tmp_path / "payload"
    views.reconstruct_blob_view(blob_map, blob)
    original = tmp_path / "original"
    views.run_checked(
        ["fsck.erofs", f"--device={blob}", f"--extract={original}", str(image)]
    )
    registration.write_bytes((original / ".airlock-builder/registration").read_bytes())
    image.rename(image.with_suffix(".previous"))
    builder.prepare_builder(
        root, {"manifest": config["manifest_path"]}, registration, manifest
    )
    updated = tmp_path / "updated"
    views.run_checked(
        ["fsck.erofs", f"--device={blob}", f"--extract={updated}", str(image)]
    )
    metadata = updated / ".airlock-builder"
    identity, _ = load_builder_guest().read_identity(metadata)
    assert identity["flake_inputs_sha256"] == core.hash_file(manifest)
    (metadata / "flake-inputs.json").write_text("{}")
    with pytest.raises(RuntimeError, match="flake inputs"):
        load_builder_guest().read_identity(metadata)

    manifest.write_text(
        json.dumps(
            {
                "schema": "airlock.sys-dev-offline-inputs.v1",
                "lock_hash": "a" * 64,
                "inputs": {"nixpkgs": "/nix/store/" + "z" * 32 + "-missing"},
            }
        )
    )
    with pytest.raises(core.EngineError, match="do not match"):
        builder.prepare_builder(
            root, {"manifest": config["manifest_path"]}, registration, manifest
        )


@pytest.mark.parametrize(
    "source",
    [
        None,
        [],
        "/tmp/source",
        "/nix/store/" + "a" * 32 + "-source/../other",
        "/nix/store/" + "a" * 32 + "-source/vendor/./qubes",
        "/nix/store/" + "a" * 32 + "-source//vendor",
        "/nix/store/" + "a" * 32 + "-source/vendor/",
    ],
)
def test_builder_offline_source_rejects_noncanonical_paths(source):
    from airlock_qubes_storage._engine import builder

    with pytest.raises(core.EngineError, match="invalid offline flake source"):
        builder.offline_source_root(source)


@pytest.mark.parametrize("builder_seed", [0, 80], indirect=True)
def test_builder_combined_index_reads_metadata_and_external_packages(
    builder_seed, tmp_path
):
    from airlock_qubes_storage._engine import builder

    root, _, paths = builder_seed
    configs = builder.builder_volume_configs(root, "airlock-closure")
    image = Path(configs["builder-store"]["view_path"])
    blob_map = core.read_json(configs["builder-data"]["blob_map_path"])
    manifest = core.read_json(configs["builder-store"]["manifest_path"])
    canonical = Path(manifest["store"]["canonical_metadata_image"])
    # The canonical guest index remains separate and unmodified. Only the
    # builder-specific index grows; payload extents still come from one device.
    assert image != canonical
    assert image.stat().st_size < canonical.stat().st_size + 65536
    canonical_hash = core.hash_file(canonical)
    blob = tmp_path / "external-payload"
    views.reconstruct_blob_view(blob_map, blob)
    extracted = tmp_path / "extracted"
    views.run_checked(
        ["fsck.erofs", f"--device={blob}", f"--extract={extracted}", str(image)]
    )
    identity, registration = load_builder_guest().read_identity(
        extracted / ".airlock-builder"
    )
    assert identity["closure_hash"] == configs["builder-store"]["closure_hash"]
    assert builder.registration_paths(registration.decode()) == set(paths)
    assert (extracted / ".links").is_dir()
    for path in paths:
        assert (extracted / Path(path).name / "payload").read_text() == path * 20
    # No embedded fallback copy: the combined index cannot supply packages
    # without the external pool data device.
    with pytest.raises(core.EngineError):
        views.run_checked(["fsck.erofs", "--extract", str(image)])
    # Preparing the same frozen view again must be deterministic and must not
    # mutate the canonical index shared with normal guest views.
    image_hash = core.hash_file(image)
    image.rename(image.with_suffix(".previous"))
    builder.prepare_builder(
        root,
        {"manifest": configs["builder-store"]["manifest_path"]},
        extracted / ".airlock-builder/registration",
    )
    assert core.hash_file(image) == image_hash
    assert core.hash_file(canonical) == canonical_hash


def load_builder_guest():
    spec = importlib.util.spec_from_file_location(
        "builder_guest",
        ROOT / "airlock-machine/lifecycle/runtime/tools/scripts/sys-dev-build-store.py",
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_builder_qubes_attachment_is_stable_after_serialization(
    builder_seed, monkeypatch, capsys
):
    root, _, _ = builder_seed
    spec = importlib.util.spec_from_file_location(
        "builder_storage_helper",
        ROOT / "airlock-machine/lifecycle/runtime/tools/scripts/nix-domain-storage.py",
    )
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    vm = SimpleNamespace(
        volume_config={},
        volumes={
            name: object()
            for name in ("root", "private", "volatile", "kernel", "store", "store-data")
        },
        is_running=lambda: False,
    )
    saves = []

    def init_volume(name, config):
        vm.volumes[name] = SimpleNamespace(create=lambda: None)
        return vm.volumes[name]

    vm.storage = SimpleNamespace(init_volume=init_volume)
    app = SimpleNamespace(close=lambda: None, save=lambda **_: saves.append(True))
    monkeypatch.setattr(helper, "load_qubes", lambda: None)
    monkeypatch.setattr(helper, "load_domain", lambda *_: (app, vm))

    async def restart(_needed, operation):
        return await operation()

    monkeypatch.setattr(helper, "with_qubesd_restart", restart)
    args = SimpleNamespace(
        pool_root=str(root), pool_name="airlock-closure", domain="sys-dev"
    )
    asyncio.run(helper.ensure_builder_store(args))
    assert len(saves) == 1
    assert len(vm.volumes) == 8
    assert helper.qubes_boot_frontends(list(vm.volumes))["builder-data"] == "xvdh"
    assert "builder-metadata" not in vm.volumes
    # Match upstream Qubes XML serialization: omit False, stringify numbers,
    # omit source=None, read True back as a boolean.
    vm.volume_config = {
        name: {
            key: True if value is True else str(value)
            for key, value in config.items()
            if value is not False and value is not None
        }
        for name, config in vm.volume_config.items()
    }
    vm.is_running = lambda: True
    asyncio.run(helper.ensure_builder_store(args))
    assert len(saves) == 1  # Restart/update does not replace the base.
    vm.volumes["builder-metadata"] = object()
    with pytest.raises(SystemExit):
        asyncio.run(helper.ensure_builder_store(args))
    assert "offline migration" in capsys.readouterr().err
    del vm.volumes["builder-metadata"]
    vm.volume_config["builder-store"]["generation"] = "2"
    with pytest.raises(SystemExit):
        asyncio.run(helper.ensure_builder_store(args))


def test_builder_guest_restarts_without_rebasing_or_losing_upper(tmp_path):
    guest = load_builder_guest()
    identity = {"closure_hash": "original"}
    guest.record_base(tmp_path, identity)
    (tmp_path / "upper").mkdir()
    output = tmp_path / "upper/new-package"
    output.write_text("persistent build output")
    guest.record_base(tmp_path, identity)  # VM reboot or guest OS update
    with pytest.raises(RuntimeError, match="base changed"):
        guest.record_base(tmp_path, {"closure_hash": "replacement"})
    assert output.read_text() == "persistent build output"
    (tmp_path / "base.json").unlink()  # Lost identity must not adopt unknown state.
    with pytest.raises(RuntimeError, match="without its base identity"):
        guest.record_base(tmp_path, identity)


@pytest.mark.parametrize("fail_last_mount", [False, True])
def test_builder_guest_mount_lifecycle_preserves_upper(
    tmp_path, monkeypatch, fail_last_mount
):
    guest = load_builder_guest()
    args = SimpleNamespace(
        state=str(tmp_path / "state"),
        runtime=str(tmp_path / "run"),
        user=guest.pwd.getpwuid(os.getuid()).pw_name,
        store_device="store-device",
        blob_device="blob-device",
        nix_store="nix-store",
    )
    mounted = set()
    calls = []
    original_stat = Path.stat

    def root_stat(path, **kwargs):
        value = list(original_stat(path, **kwargs))
        value[4] = 0
        return os.stat_result(value)

    monkeypatch.setattr(Path, "stat", root_stat)
    monkeypatch.setattr(guest.os, "chown", lambda *_: None)
    monkeypatch.setattr(guest.os.path, "ismount", lambda path: str(path) in mounted)
    monkeypatch.setattr(
        guest, "read_identity", lambda _, domain="sys-dev": ({"base": "fixed"}, b"registration")
    )

    def command(*argv, **kwargs):
        calls.append(argv)
        if argv[0] == "nix-store":
            lower = Path(argv[2])
            (lower / "nix/store/.links").mkdir(parents=True, exist_ok=True)
            (lower / "nix/var/nix/db").mkdir(parents=True, exist_ok=True)
            assert kwargs["input"] == "registration"
        elif argv[0] == "mount":
            if fail_last_mount and argv[-1] == args.state + "/merged/nix/store":
                raise RuntimeError("injected mount failure")
            mounted.add(argv[-1])
        elif argv[0] == "umount":
            mounted.remove(argv[-1])

    monkeypatch.setattr(guest, "checked", command)
    if fail_last_mount:
        with pytest.raises(RuntimeError, match="injected"):
            guest.start(args)
        assert not mounted
        assert not (Path(args.runtime) / "ready.json").exists()
        return
    guest.start(args)
    assert sum(call[:3] == ("mount", "-t", "erofs") for call in calls) == 1
    assert any("ro,nodev,nosuid,device=blob-device" in call for call in calls)
    ready = json.loads((Path(args.runtime) / "ready.json").read_text())
    assert ready["uri"].startswith("local-overlay://")
    assert "read-only%3Dtrue" in ready["uri"]
    output = Path(args.state) / "upper/built-output"
    output.write_text("keep")
    guest.stop(args)
    assert not mounted
    guest.start(args)
    assert output.read_text() == "keep"
    assert json.loads((Path(args.runtime) / "ready.json").read_text()) == ready
    guest.stop(args)
    assert not mounted
    assert any("remount,bind,ro" in call for call in calls)


def test_builder_guest_rejects_bad_metadata_and_stop_preserves_private_state(
    tmp_path, monkeypatch
):
    guest = load_builder_guest()
    metadata = tmp_path / "metadata"
    metadata.mkdir()
    (metadata / "identity.json").write_text('{"schema":"wrong"}')
    (metadata / "registration").write_text("corrupt")
    with pytest.raises(RuntimeError, match="invalid pinned"):
        guest.read_identity(metadata)
    runtime = tmp_path / "run"
    runtime.mkdir()
    (runtime / "ready.json").write_text(json.dumps({"mounts": ["lower", "merged"]}))
    output = tmp_path / "upper-output"
    output.write_text("keep")
    calls = []
    monkeypatch.setattr(guest.os.path, "ismount", lambda _: True)
    monkeypatch.setattr(guest, "checked", lambda *args: calls.append(args))
    guest.stop(SimpleNamespace(runtime=str(runtime)))
    assert calls == [("umount", "merged"), ("umount", "lower")]
    assert output.read_text() == "keep"
    assert not (runtime / "ready.json").exists()


def test_builder_pin_protects_pool_view_until_released(seed):
    """A temporary builder lease must outlive ordinary generation retention."""
    from airlock_qubes_storage._engine import generations, maintenance, segments, state

    args, _, systems = seed
    root = Path(args.pool_root)
    assert state.current_generation(root, args.domain) is None
    for generation in (1, 2):
        generations.set_retention(
            SimpleNamespace(**vars(args), generation=generation, retain="false")
        )
    pin = SimpleNamespace(**vars(args), generation=1, volume="builder-proof")
    generations.mark_running(pin)
    reference = core.read_json(state.running_ref_path(root, args.domain, 1, pin.volume))
    assert reference["physical_references"]
    backing = {
        segments.segment_blob_path(root, ref["segment_id"])
        for ref in reference["physical_references"]
    }
    fingerprints = {path: core.hash_file(path) for path in backing}
    cleanup = SimpleNamespace(pool_root=str(root), keep_generations=0)
    result = maintenance.gc(cleanup)
    assert result["retained_generations"] == [{"domain": args.domain, "generation": 1}]
    assert not state.generation_manifest_path(root, args.domain, 2).exists()
    assert objects.reusable_object(root, systems[0])
    # Exercise actual compaction as well: the mounted view's original physical
    # extents must survive even if canonical object locations change.
    maintenance.compact(
        SimpleNamespace(
            pool_root=str(root),
            keep_generations=0,
            threshold_percent=100,
            max_segments=20,
            segment_target_bytes="",
            dry_run="false",
        )
    )
    maintenance.gc(cleanup)
    assert fingerprints == {path: core.hash_file(path) for path in backing}
    assert state.generation_manifest_path(root, args.domain, 1).exists()
    generations.clear_running(pin)
    result = maintenance.gc(cleanup)
    assert result["retained_generations"] == []
    assert not state.generation_manifest_path(root, args.domain, 1).exists()
    assert all(not path.exists() for path in backing)


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


def test_transferred_artifact_builds_a_real_pool_view(seed_inputs, tmp_path):
    specs, systems, source = seed_inputs
    common = "/nix/store/" + "a" * 32 + "-shared-package"
    output = io.BytesIO()
    artifacts.write_artifact(
        output,
        domain="usr-home",
        system=systems[0],
        closure_source=specs[0]["closure_source"],
        root_source=specs[0]["root_source"],
        kernel_name=specs[0]["kernel_name"],
        source_revision="1" * 40,
        lock_hash="2" * 64,
        physical_store_root=source,
        path_info={
            common: {"references": [], "narHash": "sha256-common", "narSize": 1},
            systems[0]: {
                "references": [common],
                "narHash": "sha256-system",
                "narSize": 2,
            },
        },
    )
    result = artifacts.receive_artifact(
        io.BytesIO(output.getvalue()),
        pool_root=tmp_path / "pool",
        expected_domain="usr-home",
        approved_by="sys-dev",
    )
    manifest = core.read_json(result["manifest"])
    assert result["activated"] is False
    assert result["objects_admitted"] == 2
    assert Path(manifest["store"]["canonical_image"]).is_file()
    blob_map = core.read_json(manifest["store"]["canonical_blob_map"])
    assert blob_map["view_backend"] == "erofs-objectmap"
    assert {extent["object"] for extent in blob_map["extents"]} == {
        Path(common).name,
        Path(systems[0]).name,
        ".tar-eof",
    }


def test_negotiated_admission_reuses_usb_seed_without_payload(
    seed_inputs, tmp_path, monkeypatch
):
    specs, systems, source = seed_inputs
    pool = tmp_path / "pool"
    prepared.prepare_pool(pool, tmp_path / "descriptors", specs, str(pool))
    common = "/nix/store/" + "a" * 32 + "-shared-package"
    inventory_stream = io.BytesIO()
    payload = io.BytesIO()

    def negotiate(inventory):
        artifacts.write_message(inventory_stream, inventory)
        return {"schema": artifacts.SCHEMA_REQUEST, "missing": []}

    monkeypatch.setattr(
        artifacts,
        "create_tar_fragment",
        lambda *args: pytest.fail("USB-seeded objects must not be packaged"),
    )
    artifacts.write_artifact(
        payload,
        domain="usr-home",
        system=systems[0],
        closure_source=specs[0]["closure_source"],
        root_source=specs[0]["root_source"],
        kernel_name=specs[0]["kernel_name"],
        source_revision="1" * 40,
        lock_hash="2" * 64,
        physical_store_root=source,
        negotiate=negotiate,
        path_info={
            common: {"references": [], "narHash": "sha256-common", "narSize": 1},
            systems[0]: {
                "references": [common],
                "narHash": "sha256-system",
                "narSize": 2,
            },
        },
    )
    response = io.BytesIO()
    result = artifacts.receive_incremental(
        io.BytesIO(inventory_stream.getvalue() + payload.getvalue()),
        response,
        pool_root=pool,
        expected_domain="usr-home",
        approved_by="sys-dev",
    )
    assert json.loads(response.getvalue())["missing"] == []
    assert result["objects_reused"] == 2
    assert result["objects_transferred"] == result["objects_admitted"] == 0
    assert result["payload_bytes_transferred"] == 0
    assert result["activated"] is False
    assert result["retained"] is True
    manifest = core.read_json(result["manifest"])
    assert Path(manifest["store"]["canonical_image"]).is_file()


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
        ROOT / "airlock-machine/lifecycle/runtime/tools/scripts/nix-domain-storage.py",
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
        imageCommand = config.system.build.image.buildCommand;
        runtimeSeed = config.system.build.usbPreparedRuntime.outPath;
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
    assert prepared["runtimeSeed"] not in prepared["dependencies"]
    assert (
        "cp -a --sparse=always " + prepared["runtimeSeed"] + "/qubes/."
        in prepared["imageCommand"]
    )
    command = prepared["imageCommand"]
    # Activation runs upstream qubes-create when qubes.xml is absent. Seed
    # before it, but resolve root:qubes ownership only after users exist.
    assert (
        command.index("mount $rootDisk $mountPoint")
        < command.index("Installing prepared Qubes runtime state")
        < command.index("switch-to-configuration boot")
        < command.index("chown -R root:qubes /var/lib/qubes")
        < command.index("umount -R /mnt")
    )
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
    assert root_artifact not in runtime, (
        "root disks are installed directly into Qubes storage"
    )
    assert domain["backend"]["system"]["source"] not in runtime
    for kernel in topology["kernels"].values():
        assert kernel["source"] in runtime
    for path in (payload, descriptors):
        info = json.loads(nix("path-info", "--json", path))
        item = info[0] if isinstance(info, list) else info[path]
        assert item["references"] == [], (
            "seed must not retain its original source closure"
        )
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


@pytest.mark.nix
def test_prepared_runtime_uses_real_qubes_offline_api(tmp_path, seed_inputs):
    from xml.etree import ElementTree as ET

    specs, _, _ = seed_inputs
    root_image = tmp_path / "root.img"
    with root_image.open("wb") as stream:
        stream.truncate(16 * 1024 * 1024)
    subprocess.run(["mkfs.ext4", "-q", "-F", str(root_image)], check=True)
    spec = {**specs[0], "root_source": str(root_image)}
    pool = tmp_path / "pool"
    descriptors = tmp_path / "descriptors"
    prepared.prepare_pool(pool, descriptors, [spec], "/var/lib/airlock/closure-store")
    kernel = tmp_path / "kernel"
    kernel.mkdir()
    (kernel / "vmlinuz").write_bytes(b"fixture")
    (kernel / "modules.img").write_bytes(b"fixture")
    topology = {
        "kernels": {"test-kernel": {"source": str(kernel)}},
        "prefs": {"default_netvm": None},
        "domains": {
            "sys-gui": {
                "class": "StandaloneVM",
                "label": "red",
                "netvm": None,
                "prefs": {"autostart": False, "guivm": None, "virt_mode": "hvm"},
                "features": {"qrexec": True},
                "services": {"test-service": False},
                "devices": {"pci": [{"backend": "dom0", "id": "00_08.1"}]},
                "backend": {
                    "type": "nix-closure",
                    "system": {"source": spec["system"]},
                    "root": {"source": str(root_image)},
                    "kernel": {"name": "test-kernel"},
                    "state": {"persistentSize": 32 * 1024 * 1024},
                    "store": {"source": str(descriptors / spec["name"])},
                },
            }
        },
    }
    config = tmp_path / "topology.json"
    config.write_text(json.dumps(topology))
    output = tmp_path / "runtime"
    command = [
        "nix",
        "shell",
        "path:.#airlock-prepare-runtime",
        "-c",
        "airlock-prepare-runtime",
        str(config),
        str(pool),
        str(output),
    ]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    assert result.returncode == 0, result.stdout + result.stderr
    xml = (output / "qubes/qubes.xml").read_text()
    tree = ET.fromstring(xml)
    assert str(output) not in xml
    assert "/var/lib/qubes" in xml
    guest = next(
        vm
        for vm in tree.findall("./domains/domain")
        if vm.findtext("./properties/property[@name='name']") == "sys-gui"
    )
    assert guest.findtext("./features/feature[@name='airlock-root-source']") == str(
        root_image
    )
    volumes = {
        v.attrib["name"]: v.attrib for v in guest.findall("./volume-config/volume")
    }
    assert volumes["store"]["pool"] == "airlock-closure"
    assert volumes["store-data"]["view_kind"] == "erofs-blob-map"
    assert not guest.findall("./devices/device")  # hardware remains boot-time work
    installed_root = output / "qubes/appvms/sys-gui/root.img"
    private = output / "qubes/appvms/sys-gui/private.img"
    from airlock_qubes_storage.root_format import encode

    assert guest.findtext("./features/feature[@name='airlock-root-artifact']") == encode(root_image)["sha256"]
    assert core.hash_file(installed_root) == core.hash_file(root_image)
    assert installed_root.stat().st_blocks * 512 < installed_root.stat().st_size
    assert private.stat().st_size == 32 * 1024 * 1024
    assert (
        os.readlink(output / "pool-selections/domains/sys-gui/current")
        == "generations/1"
    )
    # Never overwrite a pre-existing VM installation.
    again = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    assert again.returncode != 0
    assert "must not already exist" in again.stderr
