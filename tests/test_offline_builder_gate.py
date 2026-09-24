"""Focused checks for the explicit offline build acceptance command."""

import importlib.util
from pathlib import Path
import subprocess


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
