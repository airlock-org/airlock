#!/usr/bin/env python3
"""Bundle the pinned machine source and import it into a sys-dev guest."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[4]
MACHINE = ROOT / "airlock-machine"
SCHEMA = "airlock.sys-dev-machine-source.v1"


def run(*args, cwd=None, input=None):
    return subprocess.run(
        args,
        cwd=cwd,
        input=input,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def output(*args, cwd=None):
    result = run(*args, cwd=cwd)
    if result.returncode:
        detail = result.stderr.decode().strip() or result.stdout.decode().strip()
        raise RuntimeError(detail or "command failed: " + " ".join(args))
    return result.stdout.decode().strip()


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def clean_repository(path):
    if output("git", "status", "--porcelain", cwd=path):
        raise RuntimeError(f"commit or stash changes in {path} before bundling")


def bundle_repository(path, destination):
    output("git", "bundle", "create", str(destination), "HEAD", "--all", cwd=path)


def source_archive(destination, repository=MACHINE):
    repository = Path(repository).resolve()
    if not (repository / ".git").exists():
        raise RuntimeError(f"machine source is not a Git checkout: {repository}")
    clean_repository(repository)
    revision = output("git", "rev-parse", "HEAD", cwd=repository)
    lock_hash = sha256(repository / "flake.lock")
    status = output("git", "submodule", "status", "--recursive", cwd=repository)
    if status:
        raise RuntimeError(
            "machine source has Git submodules; sys-dev source bundles do not yet "
            "support importing them"
        )

    destination = Path(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="airlock-machine-source-") as name:
        temporary = Path(name)
        bundle = temporary / "repo.bundle"
        manifest = temporary / "manifest.json"
        bundle_repository(repository, bundle)
        manifest.write_text(
            json.dumps(
                {
                    "schema": SCHEMA,
                    "revision": revision,
                    "lock_hash": lock_hash,
                    "submodules": [],
                },
                sort_keys=True,
            )
            + "\n"
        )
        with tarfile.open(destination, "w:gz", format=tarfile.PAX_FORMAT) as archive:
            archive.add(bundle, arcname="repo.bundle")
            archive.add(manifest, arcname="manifest.json")
    return {"revision": revision, "lock_hash": lock_hash, "archive": str(destination)}


def push_archive(target, archive):
    archive = Path(archive)
    with tarfile.open(archive, "r:gz") as source:
        manifest = json.load(source.extractfile("manifest.json"))
    command = [
        "ssh",
        target,
        "airlock-sys-dev-machine-source",
        "import",
        "--revision",
        manifest["revision"],
        "--lock-hash",
        manifest["lock_hash"],
    ]
    with archive.open("rb") as stream:
        result = subprocess.run(command, stdin=stream, check=False)
    if result.returncode:
        raise RuntimeError(f"sys-dev import failed with exit code {result.returncode}")


def main(argv=None):
    parser = argparse.ArgumentParser(prog="sys-dev-machine-source")
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("create")
    create.add_argument("--output", required=True, type=Path)
    create.add_argument("--repository", type=Path, default=MACHINE)
    push = commands.add_parser("push")
    push.add_argument("--target", required=True)
    push.add_argument("--output", type=Path)
    push.add_argument("--repository", type=Path, default=MACHINE)
    args = parser.parse_args(argv)
    try:
        if args.command == "create":
            print(
                json.dumps(source_archive(args.output, args.repository), sort_keys=True)
            )
            return 0
        with tempfile.TemporaryDirectory(prefix="airlock-machine-source-push-") as name:
            archive = args.output or Path(name) / "source.tar.gz"
            result = source_archive(archive, args.repository)
            push_archive(args.target, archive)
            print(json.dumps(result, sort_keys=True))
            return 0
    except (OSError, RuntimeError, subprocess.SubprocessError) as exc:
        print(f"sys-dev-machine-source: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
