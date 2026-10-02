from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile

import pytest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "lifecycle/misc/tools/scripts/sys-agent-machine-source.py"


def git(path: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=path,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


def repository(path: Path) -> str:
    path.mkdir()
    git(path, "init")
    git(path, "config", "user.email", "test@airlock.invalid")
    git(path, "config", "user.name", "Airlock Test")
    (path / "flake.lock").write_text('{"version": 7}\n')
    (path / "flake.nix").write_text("{ outputs = _: {}; }\n")
    git(path, "add", ".")
    git(path, "commit", "-m", "seed")
    return git(path, "rev-parse", "HEAD")


def test_offline_input_manifest_records_nested_sources_and_lock(tmp_path: Path):
    repo = tmp_path / "machine"
    repository(repo)
    (repo / "flake.lock").write_text(
        json.dumps(
            {
                "root": "root",
                "nodes": {
                    "root": {"inputs": {"nixos-qubes": "nixos-qubes"}},
                    "nixos-qubes": {"inputs": {"shelly": "shelly"}},
                    "shelly": {},
                },
            }
        )
        + "\n"
    )
    git(repo, "add", "flake.lock")
    git(repo, "commit", "-m", "lock inputs")
    source_a = "/nix/store/" + "a" * 32 + "-source"
    source_b = "/nix/store/" + "b" * 32 + "-source"
    archive = {
        "path": "/nix/store/" + "c" * 32 + "-machine",
        "inputs": {
            "nixos-qubes": {
                "path": source_a,
                "inputs": {"shelly": {"path": source_b, "inputs": {}}},
            }
        },
    }
    manifest = tmp_path / "offline-inputs.json"
    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "offline-inputs",
            "--repository",
            str(repo),
            "--output",
            str(manifest),
        ],
        input=json.dumps(archive),
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    data = json.loads(manifest.read_text())
    assert data["schema"] == "airlock.sys-agent-offline-inputs.v1"
    assert (
        data["lock_hash"]
        == hashlib.sha256((repo / "flake.lock").read_bytes()).hexdigest()
    )
    assert data["inputs"] == {"nixos-qubes": source_a, "nixos-qubes/shelly": source_b}


def test_offline_input_manifest_resolves_local_input_in_machine_archive(tmp_path: Path):
    repo = tmp_path / "machine"
    repository(repo)
    (repo / "flake.lock").write_text(
        json.dumps(
            {
                "root": "root",
                "nodes": {
                    "root": {"inputs": {"nixos-qubes": "nixos-qubes"}},
                    "nixos-qubes": {
                        "locked": {"path": "./vendor/nixos-qubes"},
                        "inputs": {"shelly": "shelly"},
                    },
                    "shelly": {},
                },
            }
        )
        + "\n"
    )
    git(repo, "add", "flake.lock")
    git(repo, "commit", "-m", "lock local input")
    machine_path = "/nix/store/" + "c" * 32 + "-machine"
    shelly_path = "/nix/store/" + "b" * 32 + "-source"
    manifest = tmp_path / "offline-inputs.json"
    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "offline-inputs",
            "--repository",
            str(repo),
            "--output",
            str(manifest),
        ],
        input=json.dumps(
            {
                "path": machine_path,
                "inputs": {
                    "nixos-qubes": {
                        "inputs": {"shelly": {"path": shelly_path, "inputs": {}}}
                    }
                },
            }
        ),
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    assert json.loads(manifest.read_text())["inputs"] == {
        "nixos-qubes": machine_path + "/vendor/nixos-qubes",
        "nixos-qubes/shelly": shelly_path,
    }


@pytest.mark.parametrize("schema", ["airlock.sys-agent-machine-source.v1", "airlock.sys-dev-machine-source.v1"])
def test_source_archive_preserves_machine_revision_and_lock(tmp_path: Path, monkeypatch, schema):
    monkeypatch.setenv("AIRLOCK_DEPLOY_SOURCE_SCHEMA", schema)
    repo = tmp_path / "machine"
    revision = repository(repo)
    archive = tmp_path / "machine-source.tar.gz"
    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "create",
            "--repository",
            str(repo),
            "--output",
            str(archive),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    report = json.loads(result.stdout)
    assert report["revision"] == revision
    assert (
        report["lock_hash"]
        == hashlib.sha256((repo / "flake.lock").read_bytes()).hexdigest()
    )

    with tarfile.open(archive, "r:gz") as source:
        assert sorted(source.getnames()) == ["manifest.json", "repo.bundle"]
        manifest = json.load(source.extractfile("manifest.json"))
        assert manifest == {
            "schema": schema,
            "revision": revision,
            "lock_hash": report["lock_hash"],
            "submodules": [],
        }
        bundle = source.extractfile("repo.bundle").read()
    bundle_path = tmp_path / "repo.bundle"
    bundle_path.write_bytes(bundle)
    verifier = tmp_path / "verifier"
    verifier.mkdir()
    git(verifier, "init")
    git(verifier, "bundle", "verify", str(bundle_path))
    clone = tmp_path / "clone"
    git(tmp_path, "clone", "--no-checkout", str(bundle_path), str(clone))
    git(clone, "checkout", "--detach", revision)
    assert (clone / "flake.lock").read_bytes() == (repo / "flake.lock").read_bytes()

    existing = tmp_path / "existing"
    existing.mkdir()
    git(existing, "init")
    git(existing, "fetch", str(bundle_path), revision)
    git(existing, "checkout", "--detach", revision)
    assert (existing / "flake.lock").read_bytes() == (repo / "flake.lock").read_bytes()


def test_source_archive_refuses_uncommitted_machine_changes(tmp_path: Path):
    repo = tmp_path / "machine"
    repository(repo)
    (repo / "flake.nix").write_text("changed\n")
    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "create",
            "--repository",
            str(repo),
            "--output",
            str(tmp_path / "machine-source.tar.gz"),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 1
    assert "commit or stash changes" in result.stderr


@pytest.mark.nix
def test_shallow_seed_archive_contains_only_the_pinned_checkout(tmp_path: Path):
    repo = tmp_path / "machine"
    revision = repository(repo)
    archive = tmp_path / "machine-seed.tar.gz"
    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "seed",
            "--repository",
            str(repo),
            "--output",
            str(archive),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    extracted = tmp_path / "extracted"
    extracted.mkdir()
    with tarfile.open(archive, "r:gz") as source:
        source.extractall(extracted)
        manifest = json.load(source.extractfile("manifest.json"))
    checkout = extracted / "repo"
    assert manifest["schema"] == "airlock.sys-agent-machine-seed.v1"
    assert manifest["revision"] == revision
    assert git(checkout, "rev-parse", "HEAD") == revision
    assert git(checkout, "rev-parse", "--is-shallow-repository") == "true"
    assert git(checkout, "rev-list", "--count", "HEAD") == "1"

    original = archive.read_bytes()
    reused = subprocess.run(
        [sys.executable, str(SCRIPT), "seed", "--reuse", "--repository", str(repo), "--output", str(archive)],
        capture_output=True,
        text=True,
    )
    assert reused.returncode == 0, reused.stderr
    assert archive.read_bytes() == original

    # Exercise the rendered first-boot script in an isolated home. Ownership
    # uses the test runner's UID/GID so this check needs no root privileges.
    expression = f"""
      let flake = builtins.getFlake "{ROOT}/airlock-machine";
          entries = flake.lib.mkMachineGuestEntries "x86_64-linux" {{
            airlockMachineSeedArchive = "/seed.tar.gz";
          }};
      in entries.sys-agent.configuration.config.systemd.services.airlock-machine-source-seed.script
    """
    evaluated = subprocess.run(
        ["nix", "eval", "--impure", "--raw", "--expr", expression],
        capture_output=True,
        text=True,
        check=True,
    )
    guest_home = tmp_path / "guest-home"
    guest_home.mkdir()
    script = (
        evaluated.stdout.replace("/home/user", str(guest_home))
        .replace("/seed.tar.gz", str(archive))
        .replace("-o user -g users", f"-o {os.getuid()} -g {os.getgid()}")
        .replace("chown -R user:users", f"chown -R {os.getuid()}:{os.getgid()}")
    )
    subprocess.run(["bash", "-c", script], check=True, capture_output=True, text=True)
    seeded = guest_home / "airlock-machine"
    assert git(seeded, "config", "airlock.seedRevision") == revision
    assert git(seeded, "config", "airlock.seedLockHash") == manifest["lock_hash"]
    assert git(seeded, "rev-parse", "--is-shallow-repository") == "true"
    assert git(seeded, "status", "--porcelain") == ""
    assert (guest_home / ".cache").stat().st_mode & 0o777 == 0o700
    (seeded / "local-work").write_text("preserve me")
    subprocess.run(["bash", "-c", script], check=True, capture_output=True, text=True)
    assert (seeded / "local-work").read_text() == "preserve me"
