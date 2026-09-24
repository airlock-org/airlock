#!/usr/bin/env python3
"""Bundle the pinned machine source and import it into a sys-dev guest."""

import argparse
import hashlib
import json
import re
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[4]
MACHINE = ROOT / "airlock-machine"
SCHEMA = "airlock.sys-dev-machine-source.v1"
SEED_SCHEMA = "airlock.sys-dev-machine-seed.v1"
OFFLINE_SCHEMA = "airlock.sys-dev-offline-inputs.v1"


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


def shallow_seed_archive(destination, repository=MACHINE):
    repository = Path(repository).resolve()
    if not (repository / ".git").exists():
        raise RuntimeError(f"machine source is not a Git checkout: {repository}")
    clean_repository(repository)
    revision = output("git", "rev-parse", "HEAD", cwd=repository)
    lock_hash = sha256(repository / "flake.lock")
    destination = Path(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="airlock-machine-seed-") as name:
        temporary = Path(name)
        checkout = temporary / "repo"
        output("git", "clone", "--depth", "1", f"file://{repository}", str(checkout))
        if output("git", "rev-parse", "HEAD", cwd=checkout) != revision:
            raise RuntimeError("shallow seed revision differs from source")
        if (
            output("git", "rev-parse", "--is-shallow-repository", cwd=checkout)
            != "true"
        ):
            raise RuntimeError("seed checkout is not shallow")
        manifest = temporary / "manifest.json"
        manifest.write_text(
            json.dumps(
                {"schema": SEED_SCHEMA, "revision": revision, "lock_hash": lock_hash},
                sort_keys=True,
            )
            + "\n"
        )
        with tarfile.open(destination, "w:gz", format=tarfile.PAX_FORMAT) as archive:
            archive.add(checkout, arcname="repo")
            archive.add(manifest, arcname="manifest.json")
    return {"revision": revision, "lock_hash": lock_hash, "archive": str(destination)}


def offline_inputs(destination, repository=MACHINE, archive=None):
    """Record every archived flake input against the committed machine lock."""
    repository = Path(repository).resolve()
    clean_repository(repository)
    archive = json.load(sys.stdin) if archive is None else archive
    if not isinstance(archive, dict) or not isinstance(archive.get("inputs"), dict):
        raise RuntimeError("invalid nix flake archive JSON")
    inputs = {}

    def collect(nodes, prefix=""):
        for name, node in nodes.items():
            if not isinstance(node, dict):
                raise RuntimeError(f"invalid archived input: {name}")
            input_name = f"{prefix}/{name}" if prefix else name
            path = node.get("path")
            if not isinstance(path, str) or not re.fullmatch(
                r"/nix/store/[0-9abcdfghijklmnpqrsvwxyz]{32}-[^/]+", path
            ):
                raise RuntimeError(f"archived input lacks a store path: {input_name}")
            inputs[input_name] = path
            collect(node.get("inputs", {}), input_name)

    collect(archive["inputs"])
    if not inputs:
        raise RuntimeError("machine flake archive has no inputs")
    data = {
        "schema": OFFLINE_SCHEMA,
        "lock_hash": sha256(repository / "flake.lock"),
        "inputs": inputs,
    }
    destination = Path(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(data, sort_keys=True) + "\n")
    return {"manifest": str(destination), "inputs": len(inputs)}


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
    seed = commands.add_parser("seed")
    seed.add_argument("--output", required=True, type=Path)
    seed.add_argument("--repository", type=Path, default=MACHINE)
    offline = commands.add_parser("offline-inputs")
    offline.add_argument("--output", required=True, type=Path)
    offline.add_argument("--repository", type=Path, default=MACHINE)
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
        if args.command == "seed":
            print(
                json.dumps(
                    shallow_seed_archive(args.output, args.repository), sort_keys=True
                )
            )
            return 0
        if args.command == "offline-inputs":
            print(
                json.dumps(offline_inputs(args.output, args.repository), sort_keys=True)
            )
            return 0
        with tempfile.TemporaryDirectory(prefix="airlock-machine-source-push-") as name:
            archive = args.output or Path(name) / "source.tar.gz"
            result = source_archive(archive, args.repository)
            push_archive(args.target, archive)
            print(json.dumps(result, sort_keys=True))
            return 0
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as exc:
        print(f"sys-dev-machine-source: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
