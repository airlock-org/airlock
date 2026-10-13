#!/usr/bin/env python3
"""Reclaim upstream EFI cache only when every boot entry is independently sealed.

Files move to the root filesystem, with a receipt, rather than deleting history.
No retained Airlock entry, payload, profile or Nix-store object is changed.
"""

import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import time


def fingerprint(path):
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    return {"size": path.stat().st_size, "sha256": digest}


def embedded_payloads(binary):
    data = binary.read_bytes()
    if len(data) < 64 or data[:2] != b"MZ":
        raise ValueError("unrecognized sealed EFI binary")
    offset = struct.unpack_from("<I", data, 60)[0]
    if data[offset:offset + 4] != b"PE\0\0":
        raise ValueError("invalid sealed EFI header")
    count = struct.unpack_from("<H", data, offset + 6)[0]
    table = offset + 24 + struct.unpack_from("<H", data, offset + 20)[0]
    configs = []
    for index in range(count):
        section = table + index * 40
        if data[section:section + 8].rstrip(b"\0") == b".config":
            size, start = struct.unpack_from("<II", data, section + 16)
            configs.append(data[start:start + size].rstrip(b"\0").decode())
    if len(configs) != 1:
        raise ValueError("sealed Xen binary requires one configuration")
    fields = dict(line.split("=", 1) for line in configs[0].splitlines() if "=" in line)
    names = [fields["kernel"].split()[0], fields["ramdisk"]]
    if any(Path(name).name != name for name in names):
        raise ValueError("embedded Xen payload escapes sealed directory")
    return [str((binary.parent / name).relative_to(binary.parents[2])) for name in names]


def sync_directory(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def write_receipt(path, value):
    temporary = path.with_suffix(".tmp")
    with temporary.open("w") as stream:
        stream.write(json.dumps(value, indent=2) + "\n")
        stream.flush()
        os.fsync(stream.fileno())
    temporary.replace(path)
    sync_directory(path.parent)


def inspect(esp, descriptors):
    esp, descriptors = Path(esp), Path(descriptors)
    authenticated = {}
    for descriptor in sorted(descriptors.glob("*.json")):
        record = json.loads(descriptor.read_text())
        if record.get("schema") != "airlock.host-boot-artifacts.v1":
            raise ValueError("unknown boot artifact descriptor")
        for entry in record["entries"]:
            for name, expected in entry["files"].items():
                relative = Path(name)
                if (relative.is_absolute() or ".." in relative.parts
                        or not name.startswith(("EFI/airlock/", "loader/entries/airlock-"))):
                    raise ValueError("descriptor contains an unsealed path")
                if name in authenticated and authenticated[name] != expected:
                    raise ValueError("conflicting boot artifact fingerprints")
                authenticated[name] = expected
    if not authenticated:
        raise ValueError("no sealed boot artifacts")
    for name, expected in authenticated.items():
        path = esp / name
        if path.is_symlink() or fingerprint(path) != expected:
            raise ValueError(f"retained boot artifact failed verification: {name}")
    entries = sorted((esp / "loader/entries").glob("*.conf"))
    if not entries:
        raise ValueError("no boot entries")
    for entry in entries:
        if str(entry.relative_to(esp)) not in authenticated:
            raise ValueError(f"unsealed boot entry: {entry.name}")
        fields = [line.split(None, 1) for line in entry.read_text().splitlines()
                  if line.strip() and not line.startswith("#")
                  and len(line.split(None, 1)) == 2]
        efi = [value for key, value in fields if key == "efi"]
        if len(efi) != 1 or not efi[0].startswith("/EFI/airlock/"):
            raise ValueError("boot entry does not use a sealed EFI binary")
        if efi[0].lstrip("/") not in authenticated:
            raise ValueError("EFI binary is not authenticated")
        for payload in embedded_payloads(esp / efi[0].lstrip("/")):
            if payload not in authenticated:
                raise ValueError("embedded Xen payload is not authenticated")
        if any(key in {"linux", "initrd", "devicetree"} for key, _ in fields):
            raise ValueError("unsupported direct boot payload reference")
        if "/EFI/nixos/" in entry.read_text():
            raise ValueError("boot entry references upstream cache")
    cache = esp / "EFI/nixos"
    files = []
    if cache.is_symlink():
        raise ValueError("EFI cache is a symlink")
    for path in sorted(cache.iterdir()) if cache.exists() else []:
        if path.name == ".extra-files" and path.is_dir() and not path.is_symlink() and not list(path.iterdir()):
            continue
        if path.is_symlink() or not path.is_file():
            raise ValueError("unsupported EFI cache object")
        if not path.name.endswith((".efi", ".tmp")):
            raise ValueError("unknown upstream EFI cache filename")
        files.append({"name": path.name, **fingerprint(path)})
    return {"schema": "airlock.boot-cache-quarantine.v1", "esp": str(esp),
            "sealed_entries": len(entries), "bytes": sum(f["size"] for f in files),
            "files": files}


def apply(esp, descriptors, destination):
    esp, destination = Path(esp), Path(destination)
    plan = inspect(esp, descriptors)
    if os.stat(esp).st_dev == os.stat(destination.parent).st_dev:
        raise ValueError("quarantine must be on another filesystem")
    if shutil.disk_usage(destination.parent).free < plan["bytes"] + 256 * 1024**2:
        raise ValueError("insufficient space for reversible quarantine")
    destination.mkdir(mode=0o700)
    sync_directory(destination.parent)
    receipt = destination / "receipt.json"
    write_receipt(receipt, {**plan, "status": "copying"})
    copied = []
    for entry in plan["files"]:
        source, target = esp / "EFI/nixos" / entry["name"], destination / entry["name"]
        shutil.copyfile(source, target)
        with target.open("rb") as stream:
            os.fsync(stream.fileno())
        expected = {key: entry[key] for key in ("size", "sha256")}
        if fingerprint(target) != expected or fingerprint(source) != expected:
            raise ValueError("EFI cache changed while copying")
        copied.append((source, target))
    # Recheck all sealed boot entries before reclaiming any cache file.
    if inspect(esp, descriptors) != plan:
        raise ValueError("boot state changed while copying")
    sync_directory(destination)
    for source, _ in copied:
        source.unlink()
    for directory in (destination, esp / "EFI/nixos"):
        sync_directory(directory)
    write_receipt(receipt, {**plan, "status": "complete"})
    return {**plan, "destination": str(destination), "status": "complete"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--esp", default="/boot")
    parser.add_argument("--descriptors", default="/var/lib/airlock/closure-store/host/artifacts")
    parser.add_argument("--quarantine-root", default="/var/lib/airlock/boot-cache-quarantine")
    parser.add_argument("--lock", default="/var/lib/airlock/machine/lock")
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    with Path(args.lock).open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        if args.apply:
            root = Path(args.quarantine_root)
            root.mkdir(parents=True, exist_ok=True, mode=0o700)
            result = apply(args.esp, args.descriptors, root / str(time.time_ns()))
        else:
            result = inspect(args.esp, args.descriptors)
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
