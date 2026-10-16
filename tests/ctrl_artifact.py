"""Explicit GUI build preparation and verification; never builds during pytest."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
RECEIPT = ROOT / ".airlock-logs/ctrl-build.json"


def file_digest(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def source_digest(root=ROOT):
    files = [
        root / "apps/Cargo.toml",
        root / "apps/Cargo.lock",
        root / "apps/airlock-ctrl-gui/Cargo.toml",
    ]
    for name in ("src", "assets", "examples"):
        files.extend(
            p for p in (root / "apps/airlock-ctrl-gui" / name).rglob("*") if p.is_file()
        )
    digest = hashlib.sha256()
    for path in sorted(files):
        digest.update(str(path.relative_to(root)).encode() + b"\0")
        digest.update(file_digest(path).encode() + b"\0")
    return digest.hexdigest()


def verify(receipt=RECEIPT, configured=None, root=ROOT):
    record = json.loads(receipt.read_text())
    binary = Path(record["binary"])
    if configured and Path(configured).resolve() != binary.resolve():
        raise ValueError(
            "AIRLOCK_CTRL_BIN differs from the verified build; run make ctrl-build"
        )
    if record["source_sha256"] != source_digest(root):
        raise ValueError("GUI sources changed after the build; run make ctrl-build")
    if file_digest(binary) != record["binary_sha256"]:
        raise ValueError("GUI binary changed after the build; run make ctrl-build")
    return binary.resolve()


def build(builder):
    before = source_digest()
    if builder == "cargo":
        subprocess.run(
            [
                "cargo",
                "build",
                "--locked",
                "--manifest-path",
                "apps/Cargo.toml",
                "-p",
                "airlock-ctrl-gui",
            ],
            cwd=ROOT,
            check=True,
        )
        target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "apps/target"))
        binary = target / "debug/airlock-ctrl-gui"
    else:
        result = subprocess.check_output(
            [
                str(ROOT / "lifecycle/misc/tools/scripts/airlock-nix.sh"),
                "build",
                "./apps#airlock-ctrl-gui",
                "--no-write-lock-file",
                "--no-link",
                "--json",
            ],
            cwd=ROOT,
            text=True,
        )
        output = json.loads(result)[0]["outputs"]["out"]
        store_root = (
            ""
            if os.environ.get("AIRLOCK_USE_HOST_NIX_STORE") == "1"
            else os.environ.get("AIRLOCK_NIX_ROOT", "")
        )
        binary = Path(store_root + output) / "bin/airlock-ctrl-gui"
    if source_digest() != before:
        raise RuntimeError(
            "GUI sources changed during the build; rebuild before testing"
        )
    RECEIPT.parent.mkdir(parents=True, exist_ok=True)
    RECEIPT.write_text(
        json.dumps(
            {
                "binary": str(binary.resolve()),
                "source_sha256": before,
                "binary_sha256": file_digest(binary),
            },
            indent=2,
        )
        + "\n"
    )
    print(f"GUI test artifact recorded: {binary}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--builder", choices=("cargo", "nix"), default="cargo")
    build(parser.parse_args().builder)
