"""Focused checks for the offline build acceptance command and receipt cache."""

import importlib.util
import json
from pathlib import Path
import subprocess
import sys

import pytest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "lifecycle/image/tools/offline-builder-gate.py"
SPEC = importlib.util.spec_from_file_location("offline_builder_gate", SCRIPT)
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


def test_isolated_store_registers_only_selected_paths(tmp_path):
    source = tmp_path / "source-store"
    first = tmp_path / "first"
    second = tmp_path / "second"
    first.mkdir()
    second.mkdir()
    (first / "flake.nix").write_text("{ outputs = { self }: {}; }\n")
    (second / "value").write_text("ordinary input\n")
    nix = [
        "nix",
        "--extra-experimental-features",
        "nix-command flakes",
        "--store",
        str(source),
    ]
    paths = [
        subprocess.run(
            [*nix, "store", "add-path", str(path)],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        for path in (first, second)
    ]
    registration = subprocess.run(
        ["nix-store", "--store", str(source), "--dump-db"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    trial = tmp_path / "trial-store"
    gate.prepare_store(trial, source, set(paths), registration, {paths[0]})
    assert (trial / paths[0].lstrip("/")).is_dir()
    assert not (trial / paths[0].lstrip("/")).is_symlink()
    assert (trial / paths[1].lstrip("/")).is_dir()
    assert not (trial / paths[1].lstrip("/")).is_symlink()
    assert not any((trial / paths[1].lstrip("/")).iterdir())
    for path in paths:
        result = subprocess.run(
            [
                "nix",
                "--extra-experimental-features",
                "nix-command",
                "--store",
                str(trial),
                "path-info",
                path,
            ],
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0, result.stderr


def test_examples_change_only_a_temporary_machine_checkout(tmp_path):
    machine = ROOT / "airlock-machine"
    original = (machine / gate.GUEST_MODULE).read_text()
    for name, change in gate.SCENARIOS.items():
        checkout = tmp_path / name
        gate.checkout_with_change(machine, checkout, change)
        updated = (checkout / gate.GUEST_MODULE).read_text()
        assert updated.count(change) == 1
        assert updated != original
        assert (checkout / "flake.lock").read_bytes() == (
            machine / "flake.lock"
        ).read_bytes()
    assert (machine / gate.GUEST_MODULE).read_text() == original


@pytest.fixture
def cached_gate(tmp_path, monkeypatch):
    root = tmp_path / "store"
    closure = "/nix/store/" + "a" * 32 + "-closure"
    flake = "/nix/store/" + "b" * 32 + "-flake"
    nix_root = "/nix/store/" + "c" * 32 + "-nix"
    nix_bin = nix_root + "/bin/nix"
    closure_dir = gate.physical(root, closure)
    closure_dir.mkdir(parents=True)
    (closure_dir / "store-paths").write_text(f"{flake}\n{nix_root}\n")
    (closure_dir / "registration").write_text("registered closure\n")
    gate.physical(root, flake).mkdir(parents=True)
    gate.physical(root, nix_bin).parent.mkdir(parents=True)
    gate.physical(root, nix_bin).write_text("nix executable")
    lock_hash = gate.digest((ROOT / "airlock-machine/flake.lock").read_bytes())
    manifest = tmp_path / "inputs.json"
    manifest.write_text(json.dumps({
        "schema": "airlock.sys-agent-offline-inputs.v1",
        "lock_hash": lock_hash,
        "inputs": {"nixpkgs": flake},
    }))
    seed = tmp_path / "seed.tar.gz"
    seed.write_bytes(b"committed machine source")
    log_dir = tmp_path / "logs"
    cache_dir = tmp_path / "cache"
    image_link = tmp_path / "image"
    args = [
        "gate", "--closure", closure, "--machine", str(ROOT / "airlock-machine"),
        "--manifest", str(manifest), "--seed-archive", str(seed),
        "--source-store-root", str(root), "--nix-bin", nix_bin,
        "--log-dir", str(log_dir), "--cache-dir", str(cache_dir),
        "--temporary-root", str(tmp_path),
    ]
    trials = []
    builds = []

    def fake_checked(command, **_kwargs):
        if "status" in command:
            return ""
        if "rev-parse" in command:
            return "committed-revision"
        if "--version" in command:
            return "nix 2.test"
        if "--realise" in command:
            return ""
        raise AssertionError(command)

    def fake_trial(_args, name, _change, _paths, _registration, _inputs, logs, _temp):
        trials.append(name)
        for suffix in ("stdout", "stderr"):
            (logs / f"{name}.{suffix}").write_text(name)
        return {"system": "/nix/store/" + "d" * 32 + "-system", "elapsed_seconds": 1.0}

    def fake_build(command, env):
        builds.append((command, env))
        Path(command[command.index("--out-link") + 1]).symlink_to(closure_dir)
        return subprocess.CompletedProcess(command, 0)

    monkeypatch.setattr(gate, "checked", fake_checked)
    monkeypatch.setattr(gate, "run_trial", fake_trial)
    monkeypatch.setattr(gate.subprocess, "run", fake_build)

    def invoke(*extra, image=False):
        command = args + list(extra)
        if image:
            command += ["--image-command", "nix", "build", "--out-link", str(image_link)]
        monkeypatch.setattr(sys, "argv", command)
        result = gate.main()
        return result, json.loads((log_dir / "report.json").read_text())

    return invoke, trials, builds, manifest, seed, cache_dir, image_link


def test_image_cache_miss_then_hit_and_exact_prepared_inputs(cached_gate):
    invoke, trials, builds, _manifest, _seed, _cache, image_link = cached_gate
    code, first = invoke(image=True)
    assert code == 0 and first["cache"] == "miss"
    assert trials == list(gate.SCENARIOS)
    assert len(builds) == 1 and image_link.is_symlink()
    assert builds[0][1]["AIRLOCK_MACHINE_SEED_ARCHIVE"].endswith("/seed.tar.gz")
    assert builds[0][1]["AIRLOCK_MACHINE_OFFLINE_INPUTS"].endswith("/inputs.json")
    code, second = invoke(image=True)
    assert code == 0 and second["cache"] == "hit"
    assert trials == list(gate.SCENARIOS)
    assert len(builds) == 2


def test_changed_inputs_and_invalid_receipt_rerun(cached_gate):
    invoke, trials, _builds, manifest, seed, cache, _image_link = cached_gate
    assert invoke()[0] == 0
    first = invoke()[1]
    assert first["cache"] == "hit"
    seed.write_bytes(b"changed prepared seed")
    changed = invoke()[1]
    assert changed["cache"] == "miss"
    assert len(trials) == 4
    receipt = Path(changed["receipt"])
    receipt.write_text("{}")
    assert invoke()[1]["cache"] == "miss"
    assert len(trials) == 6
    manifest.write_text(manifest.read_text() + " ")
    assert invoke()[1]["cache"] == "miss"
    assert len(trials) == 8


def test_forced_failure_invalidates_receipt_and_stops_image(cached_gate, monkeypatch):
    invoke, trials, builds, _manifest, _seed, cache, _image_link = cached_gate
    assert invoke()[0] == 0
    receipt = next(cache.glob("*.json"))

    def failing_trial(*_args):
        raise RuntimeError("trial failed")

    monkeypatch.setattr(gate, "run_trial", failing_trial)
    code, report = invoke("--force", image=True)
    assert code == 1 and report["status"] == "failed"
    assert not receipt.exists()
    assert builds == []
    assert trials == list(gate.SCENARIOS)


def test_changed_image_snapshot_cannot_publish_result(cached_gate, monkeypatch):
    invoke, _trials, _builds, _manifest, _seed, _cache, image_link = cached_gate

    def tampering_build(command, env):
        Path(command[command.index("--out-link") + 1]).symlink_to("/nix/store/image")
        Path(env["AIRLOCK_MACHINE_OFFLINE_INPUTS"]).write_text("changed during build")
        return subprocess.CompletedProcess(command, 0)

    monkeypatch.setattr(gate.subprocess, "run", tampering_build)
    code, report = invoke(image=True)
    assert code == 1
    assert "snapshot changed" in report["error"]
    assert not image_link.is_symlink()
