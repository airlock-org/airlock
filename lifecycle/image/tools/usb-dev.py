#!/usr/bin/env python3
"""Explicit, guarded USB provisioning. No command runs implicitly at boot."""

import argparse
import errno
import getpass
import hashlib
import json
import os
from pathlib import Path
import stat
import struct
import subprocess
import sys
import tempfile
import time

MARKER = "airlock-usb-dev-v1\n"
MAPPER = "airlock-codex"
UID, GID = 1000, 100
COPY_CHUNK = 8 * 1024 * 1024
PROGRESS_INTERVAL = 128 * 1024 * 1024


def run(*args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def output(*args):
    return run(*args, text=True, stdout=subprocess.PIPE).stdout.strip()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def root_only():
    require(os.geteuid() == 0, "Run this action with sudo.")


def flatten(nodes):
    for node in nodes:
        yield node
        yield from flatten(node.get("children", []))


def disks():
    return json.loads(
        output(
            "lsblk",
            "--json",
            "--bytes",
            "--paths",
            "--output",
            "NAME,TYPE,TRAN,SIZE,MODEL,SERIAL,RO,MOUNTPOINTS,LABEL",
        )
    )["blockdevices"]


def usb_disk(device):
    require(
        device.startswith("/dev/disk/by-id/usb-"),
        "Use a whole-disk /dev/disk/by-id/usb-… path, not /dev/sdX.",
    )
    resolved = Path(device).resolve(strict=True)
    require(stat.S_ISBLK(resolved.stat().st_mode), "Target is not a block device.")
    matches = [d for d in disks() if d["name"] == str(resolved)]
    require(len(matches) == 1, "Target is not a top-level disk.")
    disk = matches[0]
    require(
        disk["type"] == "disk" and disk["tran"] == "usb" and not disk["ro"],
        "Target must be a writable USB disk.",
    )
    for child in flatten([disk]):
        require(
            not any(child.get("mountpoints") or []),
            f"Mounted/in-use target: {child['name']}; unmount it first.",
        )
        holders = Path("/sys/class/block") / Path(child["name"]).name / "holders"
        require(not list(holders.iterdir()), f"Active device holders: {child['name']}")
    require(disk.get("serial"), "Refusing a USB device without a stable serial.")
    return resolved, disk


def usb_device_by_serial(serial):
    """Return the sole whole-disk USB by-id path with this hardware serial."""
    require(serial and serial.strip(), "Supply a non-empty USB serial.")
    matches = {}
    by_id = Path("/dev/disk/by-id")
    for candidate in sorted(by_id.glob("usb-*")):
        if "-part" in candidate.name:
            continue
        try:
            resolved = candidate.resolve(strict=True)
            if not stat.S_ISBLK(resolved.stat().st_mode):
                continue
        except OSError:
            continue
        disk = next(
            (
                item
                for item in disks()
                if item["name"] == str(resolved)
                and item["type"] == "disk"
                and item["tran"] == "usb"
                and item.get("serial") == serial
            ),
            None,
        )
        if disk:
            matches[str(resolved)] = candidate
    require(
        len(matches) == 1,
        f"Expected exactly one attached whole-disk USB with serial {serial!r}; found {len(matches)}.",
    )
    return str(next(iter(matches.values())))


def usb_disk_by_serial(serial):
    device = usb_device_by_serial(serial)
    target, disk = usb_disk(device)
    require(disk["serial"] == serial, "USB serial changed while selecting target.")
    return device


def unmount_usb(device):
    """Unmount every mounted partition of the already identified target USB."""
    root_only()
    resolved = Path(device).resolve(strict=True)
    disk = next((item for item in disks() if item["name"] == str(resolved)), None)
    require(
        disk and disk["type"] == "disk" and disk["tran"] == "usb",
        "Target must remain a USB whole disk while unmounting it.",
    )
    for child in flatten([disk]):
        if child["name"] == str(resolved) or not any(child.get("mountpoints") or []):
            continue
        print(f"Unmounting {child['name']}…", file=sys.stderr, flush=True)
        run("umount", "--all-targets", child["name"])


def result_image(result_link, nix_root=None):
    """Find the raw USB image in the current Nix build result link."""
    link = Path(result_link)
    try:
        result = link.resolve(strict=True)
    except FileNotFoundError:
        # A result link records Nix's logical /nix/store output path.  When
        # Airlock uses its isolated store, that path is physically rooted at
        # AIRLOCK_NIX_ROOT and therefore is not visible as /nix to sudo.
        require(
            nix_root,
            "Result link points outside the mounted /nix/store; supply --nix-root.",
        )
        target = Path(os.readlink(link))
        require(
            target.is_absolute() and str(target).startswith("/nix/store/"),
            "Dangling result link is not a Nix store output.",
        )
        root = Path(nix_root).resolve(strict=True)
        result = (root / target.relative_to("/")).resolve(strict=True)
        require(
            result.is_relative_to(root),
            "Resolved image escaped the configured Nix root.",
        )
    candidates = [result] if result.is_file() else list(result.glob("*.img"))
    candidates = [
        image for image in candidates if image.is_file() and image.suffix == ".img"
    ]
    require(
        len(candidates) == 1,
        f"Expected exactly one raw .img under the current result link {result}; found {len(candidates)}.",
    )
    return candidates[0]


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        while data := stream.read(8 * 1024 * 1024):
            result.update(data)
    return result.hexdigest()


def write_all(destination, data):
    view = memoryview(data)
    while view:
        written = destination.write(view)
        require(written, "USB write made no progress.")
        view = view[written:]


def progress_reporter(label, total):
    last_bytes = -PROGRESS_INTERVAL
    last_time = 0.0

    def report(current):
        nonlocal last_bytes, last_time
        now = time.monotonic()
        if (
            current != total
            and current - last_bytes < PROGRESS_INTERVAL
            and now - last_time < 2
        ):
            return
        percent = 100 if total == 0 else current * 100 // total
        print(
            f"{label}: {current / 1024**3:.1f} / {total / 1024**3:.1f} GiB ({percent}%)",
            file=sys.stderr,
            flush=True,
        )
        last_bytes, last_time = current, now

    return report


def copy_full_image(image, destination, progress=None):
    copied = 0
    with image.open("rb") as source:
        while data := source.read(COPY_CHUNK):
            write_all(destination, data)
            copied += len(data)
            if progress:
                progress(copied)
    return copied


def sparse_extent_bytes(image):
    source_fd = os.open(image, os.O_RDONLY)
    total = 0
    size = image.stat().st_size
    offset = 0
    try:
        while offset < size:
            try:
                data_offset = os.lseek(source_fd, offset, os.SEEK_DATA)
            except OSError as error:
                if error.errno == errno.ENXIO:
                    break
                if error.errno in (errno.EINVAL, errno.ENOTSUP):
                    raise ValueError(
                        "Source filesystem does not support sparse image extents; use --full-write."
                    )
                raise
            hole_offset = min(os.lseek(source_fd, data_offset, os.SEEK_HOLE), size)
            total += hole_offset - data_offset
            offset = hole_offset
    finally:
        os.close(source_fd)
    return total


def copy_sparse_image(image, destination, progress=None):
    """Copy allocated extents only; target holes remain untouched."""
    source_fd = os.open(image, os.O_RDONLY)
    copied = 0
    size = image.stat().st_size
    offset = 0
    try:
        while offset < size:
            try:
                data_offset = os.lseek(source_fd, offset, os.SEEK_DATA)
            except OSError as error:
                if error.errno == errno.ENXIO:
                    break
                if error.errno in (errno.EINVAL, errno.ENOTSUP):
                    raise ValueError(
                        "Source filesystem does not support sparse image extents; use --full-write."
                    )
                raise
            hole_offset = min(os.lseek(source_fd, data_offset, os.SEEK_HOLE), size)
            os.lseek(source_fd, data_offset, os.SEEK_SET)
            destination.seek(data_offset)
            remaining = hole_offset - data_offset
            while remaining:
                data = os.read(source_fd, min(COPY_CHUNK, remaining))
                require(data, "Unexpected end of sparse image extent.")
                write_all(destination, data)
                copied += len(data)
                if progress:
                    progress(copied)
                remaining -= len(data)
            offset = hole_offset
    finally:
        os.close(source_fd)
    return copied


def write_image(args):
    image = Path(args.image).resolve(strict=True)
    require(
        image.is_file() and image.suffix == ".img",
        "Use the uncompressed raw .img file.",
    )
    target, info = usb_disk(args.device)
    require(image.stat().st_size <= int(info["size"]), "USB is smaller than the image.")
    require(image.stat().st_size > 1024 * 1024, "Image is implausibly small.")
    with image.open("rb") as stream:
        stream.seek(512)
        require(stream.read(8) == b"EFI PART", "Image does not have a GPT header.")
    print(
        json.dumps(
            {
                "device": str(target),
                "identity": args.device,
                "model": info["model"],
                "serial": info["serial"],
                "bytes": info["size"],
                "image": str(image),
                "image_bytes": image.stat().st_size,
                "write": args.write,
                "write_mode": "full" if args.full_write or args.verify else "sparse",
                "full_verify": args.verify,
            },
            indent=2,
        )
    )
    if not args.write:
        print(
            "Inspection only. Writing requires --write --confirm-serial SERIAL --sha256 HASH."
        )
        return
    root_only()
    require(
        args.confirm_serial == info["serial"], "Serial confirmation does not match."
    )
    actual_sha256 = getattr(args, "calculated_sha256", None) or digest(image)
    require(
        args.sha256 and actual_sha256 == args.sha256.lower(),
        "Image SHA-256 does not match.",
    )
    # Recheck immediately before opening. O_EXCL on a Linux block device also
    # rejects mounted/claimed devices; the fd pins the checked disk identity.
    again, latest = usb_disk(args.device)
    require(
        again == target and latest["serial"] == info["serial"], "USB identity changed."
    )
    fd = os.open(target, os.O_WRONLY | os.O_EXCL)
    with os.fdopen(fd, "wb", buffering=0) as destination:
        if args.full_write or args.verify:
            total = image.stat().st_size
            written = copy_full_image(
                image, destination, progress_reporter("Writing image", total)
            )
        else:
            total = sparse_extent_bytes(image)
            written = copy_sparse_image(
                image, destination, progress_reporter("Writing sparse extents", total)
            )
        print("Flushing USB device…", file=sys.stderr, flush=True)
        os.fsync(destination.fileno())
    print("Refreshing USB partition table…", file=sys.stderr, flush=True)
    run("blockdev", "--rereadpt", str(target))
    if args.verify:
        # Verify only the image prefix; a larger USB may have unused trailing space.
        check = hashlib.sha256()
        remaining = image.stat().st_size
        verified = 0
        progress = progress_reporter("Verifying image", remaining)
        with target.open("rb", buffering=0) as stream:
            while remaining:
                data = stream.read(min(8 * 1024 * 1024, remaining))
                require(data, "Unexpected end of USB during verification.")
                check.update(data)
                remaining -= len(data)
                verified += len(data)
                progress(verified)
        require(
            check.hexdigest() == args.sha256.lower(),
            "USB read-back verification failed.",
        )
        result = f"Image fully written ({written} bytes), flushed and verified."
    else:
        result = f"Image {('fully' if args.full_write else 'sparsely')} written ({written} bytes) and flushed; full read-back verification was skipped."
    print(
        f"{result} Previous USB contents were overwritten and are not recoverable by this tool."
    )


def prepare_latest_write(args):
    """Resolve the result image and selected USB without exposing store paths."""
    image = result_image(args.result_link, args.nix_root)
    args.image = str(image)
    if args.write:
        args.device = usb_device_by_serial(args.serial)
        unmount_usb(args.device)
    else:
        args.device = usb_disk_by_serial(args.serial)
    # The SHA is calculated locally from the result link, then used by the
    # normal writer's integrity check.  This avoids users copying a changing
    # store path or a separate hash into the destructive command.
    args.calculated_sha256 = digest(image)
    args.sha256 = args.calculated_sha256
    args.confirm_serial = args.serial


def write_latest_image(args):
    """Write the image addressed by the latest explicit build result link."""
    prepare_latest_write(args)
    write_image(args)


def provision_latest_image(args):
    """Write, personalize, sync, and unmount a USB in one explicit action."""
    write_latest_image(args)
    run("udevadm", "settle")
    root = Path(tempfile.mkdtemp(prefix="airlock-usb-", dir="/run"))
    partition = Path(f"{args.device}-part2")
    try:
        require(
            partition.exists(),
            f"Expected USB root partition did not appear: {partition}",
        )
        run("mount", "-o", "nosuid,nodev", str(partition), str(root))
        personalize(
            argparse.Namespace(
                root=str(root),
                repo=args.repo,
                public_key=args.public_key,
                auth_file=args.auth_file,
                amd_vfct=args.amd_vfct,
                insecure_dev_password=args.insecure_dev_password,
                insecure_plain_codex_auth=args.insecure_plain_codex_auth,
            )
        )
        print("Flushing personalized USB filesystem…", file=sys.stderr, flush=True)
        run("sync")
    finally:
        if os.path.ismount(root):
            print("Unmounting personalized USB…", file=sys.stderr, flush=True)
            run("umount", str(root))
        root.rmdir()


def checked_root(value):
    root_only()
    root = Path(value).resolve(strict=True)
    mount = json.loads(
        output(
            "findmnt", "--json", "--mountpoint", str(root), "--output", "SOURCE,FSTYPE"
        )
    )["filesystems"][0]
    require(mount["fstype"] == "ext4", "Expected the USB ext4 root mount.")
    source = str(Path(mount["source"]).resolve(strict=True))
    candidates = [
        d
        for d in disks()
        if d["tran"] == "usb"
        and any(
            c["name"] == source and c.get("label") == "airlock-usb"
            for c in flatten([d])
        )
    ]
    require(
        len(candidates) == 1, "Root must be the airlock-usb filesystem on a USB disk."
    )
    require(
        (root / "airlock-usb-image").read_text() == MARKER, "Missing USB image marker."
    )
    return root


def beneath(root, relative):
    path = root / relative
    # Refuse symlinks in mutable provisioning paths, including absent leaves.
    current = root
    for part in Path(relative).parts:
        current /= part
        require(not current.is_symlink(), f"Unexpected symlink: {current}")
    return path


def private_file(path, data, uid=0, gid=0):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as stream:
        stream.write(data)
        os.fchown(stream.fileno(), uid, gid)
        stream.flush()
        os.fsync(stream.fileno())


def read_auth_cache(value):
    source = Path(value).resolve(strict=True)
    require(source.is_file(), "Auth source must be a regular file.")
    data = source.read_bytes()
    require(isinstance(json.loads(data), dict), "Invalid auth JSON.")
    return data


def read_amd_vfct(value):
    """Read a machine-local, structurally valid ACPI VFCT table."""
    source = Path(value).resolve(strict=True)
    require(
        stat.S_ISREG(source.stat().st_mode), "AMD VFCT source must be a regular file."
    )
    data = source.read_bytes()
    require(len(data) >= 36, "AMD VFCT source is too short.")
    require(data[:4] == b"VFCT", "AMD VFCT source has the wrong ACPI signature.")
    require(
        struct.unpack_from("<I", data, 4)[0] == len(data),
        "AMD VFCT source length does not match its ACPI header.",
    )
    require(sum(data) % 256 == 0, "AMD VFCT source has an invalid ACPI checksum.")
    return data


def readonly_root_file(path, data):
    """Create a root-owned firmware input without following symlinks."""
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o400)
    with os.fdopen(fd, "wb") as stream:
        stream.write(data)
        os.fchown(stream.fileno(), 0, 0)
        os.fchmod(stream.fileno(), 0o444)
        stream.flush()
        os.fsync(stream.fileno())


def personalize(args):
    root = checked_root(args.root)
    require(root != Path("/"), "Personalize offline, before the first USB boot.")
    password_file = beneath(root, "var/lib/airlock/usb/local-password.hash")
    repo_target = beneath(root, "home/airlock/dev/nixome")
    key_target = beneath(root, "home/airlock/.ssh/authorized_keys")
    vfct_target = beneath(root, "var/lib/airlock/amd-lab/sys-gui.vfct")
    require(
        not any(p.exists() for p in [password_file, repo_target, key_target, vfct_target]),
        "Already personalized (or interrupted); refusing to overwrite data.",
    )
    auth_data = read_auth_cache(args.auth_file) if args.auth_file else None
    vfct_data = read_amd_vfct(args.amd_vfct) if args.amd_vfct else None
    require(
        not args.insecure_plain_codex_auth or auth_data is not None,
        "--insecure-plain-codex-auth requires --auth-file.",
    )
    key_source = Path(args.public_key).resolve(strict=True)
    run("ssh-keygen", "-l", "-f", str(key_source), stdout=subprocess.DEVNULL)
    key = key_source.read_bytes()
    require(
        b"PRIVATE KEY" not in key and len(key.splitlines()) == 1,
        "Supply exactly one SSH public key, never a private key.",
    )
    repo = Path(args.repo).resolve(strict=True)
    git = ("git", "-c", f"safe.directory={repo}", "-C", str(repo))
    require(
        not output(*git, "status", "--porcelain"), "Commit the repo before copying it."
    )
    commit = output(*git, "rev-parse", "HEAD")
    if args.insecure_dev_password:
        # Explicitly opt-in development convenience. Never make this a default
        # or put it in the reusable image.
        password = "password"
        print(
            "WARNING: using the insecure literal development password for dom0 and sys-gui.",
            file=sys.stderr,
        )
    else:
        password = getpass.getpass("New local dom0 password (sys-gui uses password): ")
        require(len(password) >= 12, "Use at least 12 characters.")
        require(password == getpass.getpass("Confirm password: "), "Passwords differ.")
    password_hash = run(
        "openssl",
        "passwd",
        "-6",
        "-stdin",
        input=password + "\n",
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.encode()
    del password
    password_file.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    repo_target.parent.mkdir(parents=True, exist_ok=True)
    key_target.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    # --no-local copies through Git's transport, not hardlinks, alternates,
    # local hooks, untracked files, build artifacts or local configuration.
    run(
        "git",
        "-c",
        f"safe.directory={repo}",
        "clone",
        "--no-local",
        str(repo),
        str(repo_target),
    )
    run("git", "-C", str(repo_target), "checkout", "-B", "usb-dev", commit)
    run("git", "-C", str(repo_target), "remote", "remove", "origin")
    private_file(key_target, key if key.endswith(b"\n") else key + b"\n", UID, GID)
    private_file(password_file, password_hash)
    if vfct_data is not None:
        vfct_target.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
        readonly_root_file(vfct_target, vfct_data)
    for current, directories, files in os.walk(
        root / "home/airlock", followlinks=False
    ):
        os.chown(current, UID, GID)
        for name in directories + files:
            os.chown(Path(current) / name, UID, GID, follow_symlinks=False)
    os.chmod(key_target.parent, 0o700)
    if auth_data is not None:
        if args.insecure_plain_codex_auth:
            codex_home = beneath(root, "home/airlock/.codex")
            marker = beneath(root, "var/lib/airlock/usb/insecure-codex-auth")
            require(
                not codex_home.exists() and not marker.exists(),
                "Plain Codex auth is already configured.",
            )
            codex_home.mkdir(mode=0o700)
            os.chown(codex_home, UID, GID)
            private_file(codex_home / "auth.json", auth_data, UID, GID)
            private_file(marker, b"plaintext-codex-auth-v1\n")
            print(
                f"WARNING: Personalized USB; writable repo at /home/airlock/dev/nixome ({commit}); Codex auth is stored unencrypted.",
                file=sys.stderr,
            )
        else:
            # Keep the cache outside the raw filesystem. vault-create opens a
            # new LUKS2 vault and asks for its independent passphrase; always
            # lock it again before returning from offline personalization.
            vault(argparse.Namespace(action="vault-create", root=str(root)))
            _, mount = vault_paths(root)
            try:
                private_file(mount / "auth.json", auth_data, UID, GID)
            finally:
                vault(argparse.Namespace(action="vault-lock", root=str(root)))
            print(
                f"Personalized USB; writable repo at /home/airlock/dev/nixome ({commit}); Codex auth is in the locked encrypted vault."
            )
    else:
        print(
            f"Personalized USB; writable repo at /home/airlock/dev/nixome ({commit}). No credentials copied."
        )
    if vfct_data is not None:
        print(
            "Installed the machine-local AMD VFCT at /var/lib/airlock/amd-lab/sys-gui.vfct.",
            file=sys.stderr,
        )


def vault_paths(root):
    return (
        beneath(root, "var/lib/airlock/usb/codex.luks"),
        beneath(root, "home/airlock/.codex"),
    )


def require_vault(mount):
    require(os.path.ismount(mount), "Codex vault is locked; run vault-unlock first.")
    source = output(
        "findmnt", "--noheadings", "--mountpoint", str(mount), "--output", "SOURCE"
    )
    require(
        Path(source).resolve() == Path(f"/dev/mapper/{MAPPER}").resolve(),
        "Unexpected vault mount.",
    )
    run("cryptsetup", "status", MAPPER, stdout=subprocess.DEVNULL)


def vault(args):
    root = checked_root(args.root)
    image, mount = vault_paths(root)
    mapper = Path(f"/dev/mapper/{MAPPER}")
    if args.action == "vault-create":
        require(
            not mapper.exists() and not image.exists(),
            "Vault already exists or is open.",
        )
        require(
            not mount.exists() or not list(mount.iterdir()),
            "Codex directory is not empty.",
        )
        image.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        with os.fdopen(
            os.open(image, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600), "wb"
        ) as stream:
            stream.truncate(512 * 1024 * 1024)
        run("cryptsetup", "luksFormat", "--type", "luks2", str(image))
        run("cryptsetup", "open", str(image), MAPPER)
        try:
            run("mkfs.ext4", "-L", "airlock-codex", str(mapper))
        finally:
            run("cryptsetup", "close", MAPPER)
    if args.action in ["vault-create", "vault-unlock"]:
        require(
            not mapper.exists() and not os.path.ismount(mount), "Vault is already open."
        )
        require(
            not mount.exists() or not list(mount.iterdir()),
            "Codex mountpoint is not empty.",
        )
        mount.mkdir(parents=True, exist_ok=True, mode=0o700)
        os.chown(mount, 0, 0)
        os.chmod(mount, 0o555)
        run("cryptsetup", "open", str(image), MAPPER)
        try:
            run("mount", "-o", "nosuid,nodev,noexec", str(mapper), str(mount))
        except BaseException:
            run("cryptsetup", "close", MAPPER)
            raise
        os.chown(mount, UID, GID)
        os.chmod(mount, 0o700)
        print("Codex vault unlocked. It must be unlocked again after reboot.")
    elif args.action == "vault-lock":
        require_vault(mount)
        run("umount", str(mount))
        run("cryptsetup", "close", MAPPER)
    elif args.action == "auth-import":
        require_vault(mount)
        target = beneath(root, "home/airlock/.codex/auth.json")
        data = read_auth_cache(args.auth_file)
        private_file(target, data, UID, GID)
        print("Auth imported into the encrypted vault; contents were not logged.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    writer = commands.add_parser(
        "write", help="Inspect by default; explicitly opt into erasing a USB"
    )
    writer.add_argument("--image", required=True)
    writer.add_argument("--device", required=True)
    writer.add_argument("--write", action="store_true")
    writer.add_argument("--confirm-serial")
    writer.add_argument("--sha256")
    writer.add_argument(
        "--full-write",
        action="store_true",
        help="Materialize all zero-filled image holes on the USB.",
    )
    writer.add_argument(
        "--verify",
        action="store_true",
        help="Read the full image back and verify its SHA-256.",
    )
    writer.set_defaults(handler=write_image)
    latest_writer = commands.add_parser(
        "write-latest",
        help="Write the image in a build result link to the USB selected by serial",
    )
    latest_writer.add_argument(
        "--result-link", required=True, help="Result link created by make usb-dev-image"
    )
    latest_writer.add_argument(
        "--nix-root", help="Physical root of an isolated Nix store, when used"
    )
    latest_writer.add_argument(
        "--serial", required=True, help="Exact stable serial of the target USB"
    )
    latest_writer.add_argument("--write", action="store_true")
    latest_writer.add_argument(
        "--full-write",
        action="store_true",
        help="Materialize all zero-filled image holes on the USB.",
    )
    latest_writer.add_argument(
        "--verify",
        action="store_true",
        help="Read the full image back and verify its SHA-256.",
    )
    latest_writer.set_defaults(handler=write_latest_image)
    provisioner = commands.add_parser(
        "provision-latest",
        help="Write and personalize the image in a build result link",
    )
    provisioner.add_argument(
        "--result-link", required=True, help="Result link created by make usb-dev-image"
    )
    provisioner.add_argument(
        "--nix-root", help="Physical root of an isolated Nix store, when used"
    )
    provisioner.add_argument(
        "--serial", required=True, help="Exact stable serial of the target USB"
    )
    provisioner.add_argument(
        "--repo", required=True, help="Clean committed repository to copy to the USB"
    )
    provisioner.add_argument(
        "--public-key", required=True, help="One SSH public key to authorize"
    )
    provisioner.add_argument("--auth-file", help="Optional existing Codex auth.json")
    provisioner.add_argument(
        "--amd-vfct",
        help="Optional validated, machine-local AMD VFCT to install before first boot.",
    )
    provisioner.add_argument(
        "--insecure-dev-password",
        action="store_true",
        help="Set the literal development password 'password' without prompting.",
    )
    provisioner.add_argument(
        "--insecure-plain-codex-auth",
        action="store_true",
        help="Store auth.json unencrypted; requires --auth-file.",
    )
    provisioner.set_defaults(
        write=True,
        full_write=False,
        verify=False,
        handler=provision_latest_image,
    )
    setup = commands.add_parser("personalize")
    setup.add_argument("--root", required=True)
    setup.add_argument("--repo", required=True)
    setup.add_argument("--public-key", required=True)
    setup.add_argument(
        "--auth-file",
        help="Optional existing Codex auth.json; imports it into a new encrypted vault.",
    )
    setup.add_argument(
        "--amd-vfct",
        help="Optional validated, machine-local AMD VFCT to install before first boot.",
    )
    setup.add_argument(
        "--insecure-dev-password",
        action="store_true",
        help="Set the literal development password 'password' without prompting.",
    )
    setup.add_argument(
        "--insecure-plain-codex-auth",
        action="store_true",
        help="Store auth.json unencrypted; requires --auth-file.",
    )
    setup.set_defaults(handler=personalize)
    for name in ["vault-create", "vault-unlock", "vault-lock", "auth-import"]:
        command = commands.add_parser(name)
        command.add_argument("--root", default="/")
        if name == "auth-import":
            command.add_argument("--auth-file", required=True)
        command.set_defaults(handler=vault)
    args = parser.parse_args()
    try:
        args.handler(args)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"airlock-usb: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
