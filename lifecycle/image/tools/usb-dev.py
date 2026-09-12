#!/usr/bin/env python3
"""Explicit, guarded USB provisioning. No command runs implicitly at boot."""

import argparse
import getpass
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys


MARKER = "airlock-usb-dev-v1\n"
MAPPER = "airlock-codex"
UID, GID = 1000, 100


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
    return json.loads(output("lsblk", "--json", "--bytes", "--paths", "--output",
                             "NAME,TYPE,TRAN,SIZE,MODEL,SERIAL,RO,MOUNTPOINTS,LABEL"))["blockdevices"]


def usb_disk(device):
    require(device.startswith("/dev/disk/by-id/usb-"),
            "Use a whole-disk /dev/disk/by-id/usb-… path, not /dev/sdX.")
    resolved = Path(device).resolve(strict=True)
    require(stat.S_ISBLK(resolved.stat().st_mode), "Target is not a block device.")
    matches = [d for d in disks() if d["name"] == str(resolved)]
    require(len(matches) == 1, "Target is not a top-level disk.")
    disk = matches[0]
    require(disk["type"] == "disk" and disk["tran"] == "usb" and not disk["ro"],
            "Target must be a writable USB disk.")
    for child in flatten([disk]):
        require(not any(child.get("mountpoints") or []),
                f"Mounted/in-use target: {child['name']}; unmount it first.")
        holders = Path("/sys/class/block") / Path(child["name"]).name / "holders"
        require(not list(holders.iterdir()), f"Active device holders: {child['name']}")
    require(disk.get("serial"), "Refusing a USB device without a stable serial.")
    return resolved, disk


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        while data := stream.read(8 * 1024 * 1024):
            result.update(data)
    return result.hexdigest()


def write_image(args):
    image = Path(args.image).resolve(strict=True)
    require(image.is_file() and image.suffix == ".img", "Use the uncompressed raw .img file.")
    target, info = usb_disk(args.device)
    require(image.stat().st_size <= int(info["size"]), "USB is smaller than the image.")
    require(image.stat().st_size > 1024 * 1024, "Image is implausibly small.")
    with image.open("rb") as stream:
        stream.seek(512)
        require(stream.read(8) == b"EFI PART", "Image does not have a GPT header.")
    print(json.dumps({"device": str(target), "identity": args.device,
                      "model": info["model"], "serial": info["serial"],
                      "bytes": info["size"], "image": str(image),
                      "image_bytes": image.stat().st_size, "write": args.write,
                      "full_verify": args.verify}, indent=2))
    if not args.write:
        print("Inspection only. Writing requires --write --confirm-serial SERIAL --sha256 HASH.")
        return
    root_only()
    require(args.confirm_serial == info["serial"], "Serial confirmation does not match.")
    require(args.sha256 and digest(image) == args.sha256.lower(), "Image SHA-256 does not match.")
    # Recheck immediately before opening. O_EXCL on a Linux block device also
    # rejects mounted/claimed devices; the fd pins the checked disk identity.
    again, latest = usb_disk(args.device)
    require(again == target and latest["serial"] == info["serial"], "USB identity changed.")
    with image.open("rb") as source:
        fd = os.open(target, os.O_WRONLY | os.O_EXCL)
        with os.fdopen(fd, "wb", buffering=0) as destination:
            shutil.copyfileobj(source, destination, 8 * 1024 * 1024)
            os.fsync(destination.fileno())
    run("blockdev", "--rereadpt", str(target))
    if args.verify:
        # Verify only the image prefix; a larger USB may have unused trailing space.
        check = hashlib.sha256()
        remaining = image.stat().st_size
        with target.open("rb", buffering=0) as stream:
            while remaining:
                data = stream.read(min(8 * 1024 * 1024, remaining))
                require(data, "Unexpected end of USB during verification.")
                check.update(data)
                remaining -= len(data)
        require(check.hexdigest() == args.sha256.lower(), "USB read-back verification failed.")
        result = "Image written, flushed and fully verified."
    else:
        result = "Image written and flushed; full read-back verification was skipped."
    print(f"{result} Previous USB contents were overwritten and are not recoverable by this tool.")


def checked_root(value):
    root_only()
    root = Path(value).resolve(strict=True)
    mount = json.loads(output("findmnt", "--json", "--mountpoint", str(root),
                              "--output", "SOURCE,FSTYPE"))["filesystems"][0]
    require(mount["fstype"] == "ext4", "Expected the USB ext4 root mount.")
    source = str(Path(mount["source"]).resolve(strict=True))
    candidates = [d for d in disks() if d["tran"] == "usb" and
                  any(c["name"] == source and c.get("label") == "airlock-usb"
                      for c in flatten([d]))]
    require(len(candidates) == 1, "Root must be the airlock-usb filesystem on a USB disk.")
    require((root / "airlock-usb-image").read_text() == MARKER, "Missing USB image marker.")
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


def personalize(args):
    root = checked_root(args.root)
    require(root != Path("/"), "Personalize offline, before the first USB boot.")
    password_file = beneath(root, "var/lib/airlock/usb/local-password.hash")
    repo_target = beneath(root, "home/airlock/dev/nixome")
    key_target = beneath(root, "home/airlock/.ssh/authorized_keys")
    require(not any(p.exists() for p in [password_file, repo_target, key_target]),
            "Already personalized (or interrupted); refusing to overwrite data.")
    auth_data = read_auth_cache(args.auth_file) if args.auth_file else None
    key_source = Path(args.public_key).resolve(strict=True)
    run("ssh-keygen", "-l", "-f", str(key_source), stdout=subprocess.DEVNULL)
    key = key_source.read_bytes()
    require(b"PRIVATE KEY" not in key and len(key.splitlines()) == 1,
            "Supply exactly one SSH public key, never a private key.")
    repo = Path(args.repo).resolve(strict=True)
    git = ("git", "-c", f"safe.directory={repo}", "-C", str(repo))
    require(not output(*git, "status", "--porcelain"), "Commit the repo before copying it.")
    commit = output(*git, "rev-parse", "HEAD")
    password = getpass.getpass("New local dom0 + sys-gui password: ")
    require(len(password) >= 12, "Use at least 12 characters.")
    require(password == getpass.getpass("Confirm password: "), "Passwords differ.")
    password_hash = run("openssl", "passwd", "-6", "-stdin", input=password + "\n",
                        text=True, stdout=subprocess.PIPE).stdout.encode()
    del password
    password_file.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    repo_target.parent.mkdir(parents=True, exist_ok=True)
    key_target.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    # --no-local copies through Git's transport, not hardlinks, alternates,
    # local hooks, untracked files, build artifacts or local configuration.
    run("git", "-c", f"safe.directory={repo}", "clone", "--no-local", str(repo), str(repo_target))
    run("git", "-C", str(repo_target), "checkout", "-B", "usb-dev", commit)
    run("git", "-C", str(repo_target), "remote", "remove", "origin")
    private_file(key_target, key if key.endswith(b"\n") else key + b"\n", UID, GID)
    private_file(password_file, password_hash)
    for current, directories, files in os.walk(root / "home/airlock", followlinks=False):
        os.chown(current, UID, GID)
        for name in directories + files:
            os.chown(Path(current) / name, UID, GID, follow_symlinks=False)
    os.chmod(key_target.parent, 0o700)
    if auth_data is not None:
        # Keep the cache outside the raw filesystem. vault-create opens a new
        # LUKS2 vault and asks for its independent passphrase; always lock it
        # again before returning from offline personalization.
        vault(argparse.Namespace(action="vault-create", root=str(root)))
        _, mount = vault_paths(root)
        try:
            private_file(mount / "auth.json", auth_data, UID, GID)
        finally:
            vault(argparse.Namespace(action="vault-lock", root=str(root)))
        print(f"Personalized USB; writable repo at /home/airlock/dev/nixome ({commit}); Codex auth is in the locked encrypted vault.")
    else:
        print(f"Personalized USB; writable repo at /home/airlock/dev/nixome ({commit}). No credentials copied.")


def vault_paths(root):
    return (beneath(root, "var/lib/airlock/usb/codex.luks"),
            beneath(root, "home/airlock/.codex"))


def require_vault(mount):
    require(os.path.ismount(mount), "Codex vault is locked; run vault-unlock first.")
    source = output("findmnt", "--noheadings", "--mountpoint", str(mount), "--output", "SOURCE")
    require(Path(source).resolve() == Path(f"/dev/mapper/{MAPPER}").resolve(), "Unexpected vault mount.")
    run("cryptsetup", "status", MAPPER, stdout=subprocess.DEVNULL)


def vault(args):
    root = checked_root(args.root)
    image, mount = vault_paths(root)
    mapper = Path(f"/dev/mapper/{MAPPER}")
    if args.action == "vault-create":
        require(not mapper.exists() and not image.exists(), "Vault already exists or is open.")
        require(not mount.exists() or not list(mount.iterdir()), "Codex directory is not empty.")
        image.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        with os.fdopen(os.open(image, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600), "wb") as stream:
            stream.truncate(512 * 1024 * 1024)
        run("cryptsetup", "luksFormat", "--type", "luks2", str(image))
        run("cryptsetup", "open", str(image), MAPPER)
        try:
            run("mkfs.ext4", "-L", "airlock-codex", str(mapper))
        finally:
            run("cryptsetup", "close", MAPPER)
    if args.action in ["vault-create", "vault-unlock"]:
        require(not mapper.exists() and not os.path.ismount(mount), "Vault is already open.")
        require(not mount.exists() or not list(mount.iterdir()), "Codex mountpoint is not empty.")
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
    writer = commands.add_parser("write", help="Inspect by default; explicitly opt into erasing a USB")
    writer.add_argument("--image", required=True)
    writer.add_argument("--device", required=True)
    writer.add_argument("--write", action="store_true")
    writer.add_argument("--confirm-serial")
    writer.add_argument("--sha256")
    writer.add_argument("--verify", action="store_true", help="Read the full image back and verify its SHA-256.")
    writer.set_defaults(handler=write_image)
    setup = commands.add_parser("personalize")
    setup.add_argument("--root", required=True)
    setup.add_argument("--repo", required=True)
    setup.add_argument("--public-key", required=True)
    setup.add_argument("--auth-file", help="Optional existing Codex auth.json; imports it into a new encrypted vault.")
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
