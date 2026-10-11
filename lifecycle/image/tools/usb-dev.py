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

MARKER = "airlock-usb-dev-v2\n"
UID, GID = 1000, 100
COPY_CHUNK = 8 * 1024 * 1024
PROGRESS_INTERVAL = 128 * 1024 * 1024


def run(*args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def report_command(*args, ok=(0,), **kwargs):
    """Show successful command diagnostics normally; reserve stderr for failure."""
    kwargs.setdefault("text", True)
    result = subprocess.run(
        args,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        **kwargs,
    )
    destination = sys.stdout if result.returncode in ok else sys.stderr
    for content in (result.stdout, result.stderr):
        if content:
            print(content, end="" if content.endswith("\n") else "\n", file=destination)
    if result.returncode not in ok:
        raise subprocess.CalledProcessError(
            result.returncode, args, output=result.stdout, stderr=result.stderr
        )
    return result


def output(*args):
    result = subprocess.run(
        args,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode:
        for content in (result.stdout, result.stderr):
            if not content:
                continue
            print(
                content,
                end="" if content.endswith("\n") else "\n",
                file=sys.stderr,
            )
        raise subprocess.CalledProcessError(
            result.returncode, args, output=result.stdout, stderr=result.stderr
        )
    if result.stderr:
        print(result.stderr, end="" if result.stderr.endswith("\n") else "\n")
    return result.stdout.strip()


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
        print(f"Unmounting {child['name']}…", flush=True)
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
        print("Flushing USB device…", flush=True)
        os.fsync(destination.fileno())
    print("Refreshing USB partition table…", flush=True)
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


def checked_growth_layout(table, target, partition, disk_bytes):
    """Only the seed image's two-partition GPT layout may be expanded."""
    require(table.get("label") == "gpt", "Root expansion requires GPT.")
    require(table.get("sectorsize") == 512, "Expected 512-byte image sectors.")
    require(table.get("device") == str(target), "Partition table device mismatch.")
    parts = table.get("partitions", [])
    require(len(parts) == 2, "Expected exactly ESP and root; refusing a custom layout.")
    esp, root = parts
    require(
        esp["node"] == str(target) + "1" and root["node"] == str(partition),
        "Unexpected partition numbering/order.",
    )
    require(
        esp["type"].upper() == "C12A7328-F81F-11D2-BA4B-00A0C93EC93B"
        and root["type"].upper() == "0FC63DAF-8483-4772-8E79-3D69D8477DE4",
        "Expected EFI system partition followed by Linux root.",
    )
    require(
        34 <= esp["start"] < esp["start"] + esp["size"] <= root["start"]
        and root["size"] > 0
        and root["start"] + root["size"] <= disk_bytes // 512 - 33,
        "Invalid or overlapping partition boundaries.",
    )
    require(root.get("uuid") and esp.get("uuid"), "Missing partition identities.")
    return root


def checked_growth_result(before, after):
    require(before["id"] == after["id"], "Disk UUID changed during expansion.")
    require(
        before["partitions"][0] == after["partitions"][0],
        "ESP changed during expansion.",
    )
    old, new = before["partitions"][1], after["partitions"][1]
    require(
        {key: value for key, value in old.items() if key != "size"}
        == {key: value for key, value in new.items() if key != "size"},
        "Root start/identity changed; refusing filesystem resize.",
    )
    require(new["size"] >= old["size"], "Refusing to shrink the root partition.")
    require(
        0 <= after["lastlba"] + 1 - new["start"] - new["size"] < 2048,
        "Root partition did not expand to the end of usable space.",
    )


def check_ext4(partition):
    try:
        report_command("e2fsck", "-f", "-p", str(partition), ok=(0, 1))
    except subprocess.CalledProcessError as error:
        raise ValueError(
            f"Filesystem check failed ({error.returncode}); stopping."
        ) from error


def root_fills_disk(table, disk_bytes):
    root = table["partitions"][1]
    return (
        0 <= disk_bytes // 512 - 1 - table["lastlba"] < 2048
        and 0 <= table["lastlba"] + 1 - root["start"] - root["size"] < 2048
    )


def verify_kernel_partition_size(partition, expected_bytes):
    # sfdisk already notified the kernel. Repeating BLKRRPART races udev's
    # partition probes and can return EBUSY even when the new size is visible.
    run("udevadm", "settle")
    require(
        int(output("blockdev", "--getsize64", str(partition))) == expected_bytes,
        "Kernel partition size is stale. Unplug/replug this USB, then rerun "
        "usb-dev-provision from scratch; no filesystem resize was attempted.",
    )


def expand_provisioned_root(device, serial, backup_base):
    """Grow the newly written seed, offline, before any personalization."""
    root_only()
    target, info = usb_disk(device)
    require(info["serial"] == serial, "USB serial changed before root expansion.")
    partition = Path(device + "-part2").resolve(strict=True)
    before = json.loads(output("sfdisk", "--json", str(target)))["partitiontable"]
    checked_growth_layout(before, target, partition, int(info["size"]))
    require(
        output("blkid", "-s", "TYPE", "-o", "value", str(partition)) == "ext4",
        "Expected ext4 root.",
    )
    require(
        output("blkid", "-s", "LABEL", "-o", "value", str(partition)) == "airlock-usb",
        "Expected airlock-usb root label.",
    )
    filesystem_uuid = output("blkid", "-s", "UUID", "-o", "value", str(partition))
    # noload prevents this validation mount from replaying the journal.
    with tempfile.TemporaryDirectory(
        prefix="airlock-grow-check-", dir="/run"
    ) as directory:
        run("mount", "-o", "ro,noload,nosuid,nodev,noexec", str(partition), directory)
        try:
            require(
                (Path(directory) / "airlock-usb-image").read_text() == MARKER,
                "Missing Airlock image marker.",
            )
        finally:
            run("umount", directory)

    backup_base = Path(backup_base).resolve()
    existing = backup_base
    while not existing.exists():
        existing = existing.parent
    target_devices = {Path(child["name"]).stat().st_rdev for child in flatten([info])}
    require(
        existing.stat().st_dev not in target_devices,
        "Save expansion backups off the target USB.",
    )
    backup_base.mkdir(parents=True, exist_ok=True)
    backup = Path(tempfile.mkdtemp(prefix="grow-", dir=backup_base))
    (backup / "before.json").write_text(json.dumps(before, indent=2) + "\n")
    (backup / "partition-table.sfdisk").write_text(
        output("sfdisk", "--dump", str(target)) + "\n"
    )
    report_command(
        "sfdisk",
        "--backup-file",
        str(backup / "partition-table"),
        "--backup-pt-sectors",
        str(target),
    )
    run("sync", "-f", str(backup))
    print(f"Root expansion recovery metadata: {backup}", flush=True)

    def recheck():
        again, latest = usb_disk(device)
        require(
            again == target and latest["serial"] == serial,
            "USB identity changed during expansion.",
        )

    recheck()
    check_ext4(partition)
    recheck()
    if not root_fills_disk(before, int(info["size"])):
        # The seed GPT's backup header is still at the image boundary.
        report_command("sfdisk", "--lock=yes", "--relocate", "gpt-bak-std", str(target))
        recheck()
        report_command(
            "sfdisk",
            "--lock=yes",
            "--wipe=never",
            "--wipe-partitions=never",
            "-N",
            "2",
            str(target),
            input=", +\n",
            text=True,
        )
    after = json.loads(output("sfdisk", "--json", str(target)))["partitiontable"]
    checked_growth_layout(after, target, partition, int(info["size"]))
    checked_growth_result(before, after)
    verify_kernel_partition_size(partition, after["partitions"][1]["size"] * 512)
    recheck()
    report_command("resize2fs", str(partition))
    recheck()
    check_ext4(partition)
    require(
        output("blkid", "-s", "UUID", "-o", "value", str(partition)) == filesystem_uuid,
        "Filesystem UUID changed.",
    )
    run("sync")
    result = {
        "schema": "airlock.usb-root-expansion.v1",
        "serial": serial,
        "device": str(target),
        "filesystem_uuid": filesystem_uuid,
        "root_bytes_before": before["partitions"][1]["size"] * 512,
        "root_bytes_after": after["partitions"][1]["size"] * 512,
        "partition_table": after,
        "status": "complete",
    }
    (backup / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    run("sync", "-f", str(backup))
    print(
        f"USB root expanded to {result['root_bytes_after'] / 1024**3:.1f} GiB; filesystem checked.",
        flush=True,
    )


def provision_latest_image(args):
    """Write, expand root, personalize, sync, and unmount in one explicit action."""
    write_latest_image(args)
    run("udevadm", "settle")
    expand_provisioned_root(args.device, args.serial, args.resize_backup_dir)
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
                amd_vfct=args.amd_vfct,
                insecure_dev_password=args.insecure_dev_password,
            )
        )
        print("Flushing personalized USB filesystem…", flush=True)
        run("sync")
    finally:
        if os.path.ismount(root):
            print("Unmounting personalized USB…", flush=True)
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
    repo_target = beneath(root, "home/user/dev/airlock")
    key_target = beneath(root, "home/user/.ssh/authorized_keys")
    vfct_target = beneath(root, "var/lib/airlock/amd-lab/sys-gui.vfct")
    require(
        not any(
            p.exists() for p in [password_file, repo_target, key_target, vfct_target]
        ),
        "Already personalized (or interrupted); refusing to overwrite data.",
    )
    vfct_data = read_amd_vfct(args.amd_vfct) if args.amd_vfct else None
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
            "DEVELOPMENT MODE: using the insecure literal password for dom0 and sys-gui."
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
    report_command(
        "git",
        "-c",
        f"safe.directory={repo}",
        "clone",
        "--no-local",
        str(repo),
        str(repo_target),
    )
    report_command("git", "-C", str(repo_target), "checkout", "-B", "usb-dev", commit)
    run("git", "-C", str(repo_target), "remote", "remove", "origin")
    private_file(key_target, key if key.endswith(b"\n") else key + b"\n", UID, GID)
    private_file(password_file, password_hash)
    if vfct_data is not None:
        vfct_target.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
        readonly_root_file(vfct_target, vfct_data)
    for current, directories, files in os.walk(root / "home/user", followlinks=False):
        os.chown(current, UID, GID)
        for name in directories + files:
            os.chown(Path(current) / name, UID, GID, follow_symlinks=False)
    os.chmod(key_target.parent, 0o700)
    print(f"Personalized USB; writable repo at /home/user/dev/airlock ({commit}).")
    if vfct_data is not None:
        print(
            "Installed the machine-local AMD VFCT at /var/lib/airlock/amd-lab/sys-gui.vfct."
        )


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
        help="Write the image, expand root to the USB capacity, and personalize",
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
    provisioner.add_argument(
        "--resize-backup-dir",
        default="/var/log/airlock/usb-root-expansion",
        help="Off-USB directory for partition-table backups and expansion receipts.",
    )
    provisioner.add_argument(
        "--amd-vfct",
        help="Optional validated, machine-local AMD VFCT to install before first boot.",
    )
    provisioner.add_argument(
        "--insecure-dev-password",
        action="store_true",
        help="Set the literal development password 'password' without prompting.",
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
        "--amd-vfct",
        help="Optional validated, machine-local AMD VFCT to install before first boot.",
    )
    setup.add_argument(
        "--insecure-dev-password",
        action="store_true",
        help="Set the literal development password 'password' without prompting.",
    )
    setup.set_defaults(handler=personalize)
    args = parser.parse_args()
    try:
        args.handler(args)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"airlock-usb: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
