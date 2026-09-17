from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "lifecycle/misc/tools/scripts/sys-dev-machine-source.py"


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


def test_source_archive_preserves_machine_revision_and_lock(tmp_path: Path):
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
            "schema": "airlock.sys-dev-machine-source.v1",
            "revision": revision,
            "lock_hash": report["lock_hash"],
            "submodules": [],
        }
        bundle = source.extractfile("repo.bundle").read()
    bundle_path = tmp_path / "repo.bundle"
    bundle_path.write_bytes(bundle)
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
