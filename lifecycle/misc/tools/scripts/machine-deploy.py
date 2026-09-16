#!/usr/bin/env python3
"""Build a committed whole-machine release and deploy via the sys-dev bridge."""

import argparse
import base64
import getpass
import hashlib
import json
import os
from pathlib import Path
import select
import shlex
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[4]
NIX = ROOT / "lifecycle/misc/tools/scripts/airlock-nix.sh"
REMOTE_INVENTORY = "nix --extra-experimental-features nix-command path-info --all"


def command(*args, **kwargs):
    return subprocess.check_output(args, text=True, **kwargs).strip()


class Dom0:
    """Text-only control over the existing password-protected development PTY.

    Payloads use a separate binary qrexec stream, never this PTY. Nothing here
    installs SSH in dom0 or extends production Qubes policy.
    """

    def __init__(self, target):
        self.process = subprocess.Popen(
            ["ssh", "-tt", target, "airlock-dom0-shell"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        self.buffer = b""
        self.until([b"DEVELOPMENT: dom0 shell."], timeout=60)
        token = uuid.uuid4().hex
        self.send(f"stty -echo; printf '%s%s\\n' '{token[:16]}' '{token[16:]}'")
        self.until([token.encode()], timeout=30)
        prompt, ready = uuid.uuid4().hex, uuid.uuid4().hex
        script = f"printf '%s\\n' {ready}; exec bash --noprofile --norc"
        self.send(f"sudo -S -p {prompt} bash -c {shlex.quote(script)}")
        for attempt in range(4):
            match, _ = self.until([prompt.encode(), ready.encode()], timeout=60)
            if match == ready.encode():
                break
            if attempt == 3:
                raise RuntimeError("dom0 sudo authentication failed")
            password = getpass.getpass("dom0 sudo password: ")
            self.send(password)
            del password

    def send(self, line):
        self.process.stdin.write(line.encode() + b"\n")
        self.process.stdin.flush()

    def until(self, tokens, timeout):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            for token in tokens:
                index = self.buffer.find(token)
                if index >= 0:
                    result = self.buffer[:index]
                    self.buffer = self.buffer[index + len(token) :]
                    return token, result
            ready, _, _ = select.select(
                [self.process.stdout],
                [],
                [],
                min(1, max(0, deadline - time.monotonic())),
            )
            if ready:
                data = os.read(self.process.stdout.fileno(), 65536)
                if not data:
                    raise RuntimeError(
                        "dom0 connection closed; reconnect and run status"
                    )
                self.buffer += data
        raise RuntimeError("dom0 control command timed out; reconnect and run status")

    def run(self, script, timeout=60):
        token = uuid.uuid4().hex
        payload = base64.b64encode(("set -euo pipefail; " + script).encode()).decode()
        wrapper = (
            "import subprocess,base64,json; "
            f"p=subprocess.run(base64.b64decode('{payload}'),shell=True,executable='/run/current-system/sw/bin/bash',capture_output=True); "
            f"print('{token}'+base64.b64encode(json.dumps([p.returncode,p.stdout.decode(),p.stderr.decode()]).encode()).decode()+'{token}')"
        )
        line = "python3 -c " + shlex.quote(wrapper)
        if len(line) > 3500:
            raise RuntimeError(
                "control message too large for PTY; use artifact transfer"
            )
        self.send(line)
        self.until([token.encode()], timeout)
        _, response = self.until([token.encode()], timeout)
        status, output, error = json.loads(base64.b64decode(response))
        if status:
            raise RuntimeError(f"dom0 command failed ({status}): {error or output}")
        return output.strip()

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
        self.process.wait(timeout=10)


def build(profile):
    if command("git", "status", "--porcelain", cwd=ROOT):
        raise RuntimeError(
            "commit repository changes before deployment (including new files)"
        )
    revision = command("git", "rev-parse", "HEAD", cwd=ROOT)
    attr = f"nixosConfigurations.usb-dev.config.specialisation.{profile}.configuration.system.build.airlockMachineRelease"
    result = json.loads(
        command(str(NIX), "build", ".#" + attr, "--json", "--no-link", cwd=ROOT)
    )
    if command("git", "rev-parse", "HEAD", cwd=ROOT) != revision or command(
        "git", "status", "--porcelain", cwd=ROOT
    ):
        raise RuntimeError("repository changed during build; retry from a clean commit")
    release = result[0]["outputs"]["out"]
    # Pin the output in the guarded Airlock store, not the host store.
    log = ROOT / ".airlock-logs" / "machine-deploy" / revision
    log.mkdir(parents=True, exist_ok=True)
    command(str(NIX), "build", release, "--out-link", str(log / "release"), cwd=ROOT)
    (log / "build.json").write_text(
        json.dumps(
            {"revision": revision, "profile": profile, "release": release}, indent=2
        )
        + "\n"
    )
    return release, revision


def transfer(target, bridge_vm, release, session):
    paths = json.loads(command(str(NIX), "path-info", "--recursive", "--json", release))
    local = (
        [entry["path"] for entry in paths] if isinstance(paths, list) else list(paths)
    )
    valid = set(session.run(REMOTE_INVENTORY, timeout=120).splitlines())
    missing = [path for path in local if path not in valid]
    if not missing:
        return
    metadata = (
        {entry["path"]: entry for entry in paths} if isinstance(paths, list) else paths
    )
    needed = sum(metadata[path]["narSize"] for path in missing)
    free = int(
        session.run(
            "python3 -c 'import os; s=os.statvfs(\"/nix/store\"); print(s.f_bavail*s.f_frsize)'"
        )
    )
    # Reserve both incoming NAR/unpacked paths and immutable guest staging.
    required = 2 * needed + 2 * 1024**3
    if free < required:
        raise RuntimeError(
            f"dom0 needs approximately {required / 1024**3:.1f} GiB free to safely stage this release; "
            f"only {free / 1024**3:.1f} GiB is available. No VMs or partitions changed. "
            "Provision development storage first; do not delete active/rollback store generations."
        )
    # sys-dev deliberately has a small package set and need not include Python.
    # GNU stat is already available; use available blocks, not total/free blocks.
    bridge_blocks, bridge_block_size = map(
        int,
        command(
            "ssh",
            target,
            "stat -f -c '%a %S' /home/user",
        ).split(),
    )
    if bridge_blocks < 0 or bridge_block_size <= 0:
        raise RuntimeError("invalid sys-dev filesystem space report")
    bridge_free = bridge_blocks * bridge_block_size
    if bridge_free < needed + 512 * 1024**2:
        raise RuntimeError(
            "sys-dev needs more private-volume space for the compressed closure transfer"
        )
    nonce = uuid.uuid4().hex
    remote = f"/home/user/.cache/airlock-deploy/{nonce}.nar.gz"
    incoming = f"/var/lib/airlock/machine/incoming-{nonce}.nar.gz"
    print(
        f"Transferring {len(missing)} missing Nix paths through {target}…", flush=True
    )
    # airlock-env.sh selects the existing guarded custom store for nix-store.
    export = subprocess.Popen(
        [
            "bash",
            "-c",
            'source "$1"; shift; exec nix-store --export "$@"',
            "airlock-export",
            str(ROOT / "lifecycle/misc/tools/scripts/lib/airlock-env.sh"),
            *missing,
        ],
        stdout=subprocess.PIPE,
    )
    gzip = subprocess.Popen(
        ["gzip", "-1c"], stdin=export.stdout, stdout=subprocess.PIPE
    )
    export.stdout.close()
    upload = subprocess.Popen(
        [
            "ssh",
            target,
            f"umask 077; mkdir -p /home/user/.cache/airlock-deploy && cat > {shlex.quote(remote)}",
        ],
        stdin=subprocess.PIPE,
    )
    digest = hashlib.sha256()
    try:
        while data := gzip.stdout.read(1024 * 1024):
            digest.update(data)
            upload.stdin.write(data)
        upload.stdin.close()
        if any([export.wait(), gzip.wait(), upload.wait()]):
            raise RuntimeError("Nix closure upload failed")
    finally:
        for process in (export, gzip, upload):
            if process.poll() is None:
                process.terminate()
    fetch = " ".join(
        shlex.quote(arg)
        for arg in ("qrexec-client", "-d", bridge_vm, "user:cat " + remote)
    )
    session.run(
        f"umask 077; mkdir -p /var/lib/airlock/machine; {fetch} > {incoming} && "
        f"echo '{digest.hexdigest()}  {incoming}' | sha256sum -c - && "
        f"bash -o pipefail -c 'gzip -dc {incoming} | nix-store --import > /var/lib/airlock/machine/import.log' && "
        f"rm -- {incoming}",
        timeout=3600,
    )
    subprocess.run(["ssh", target, "rm -- " + shlex.quote(remote)], check=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "action",
        choices=["build", "plan", "apply", "status", "verify", "resume", "rollback"],
    )
    parser.add_argument("--target", help="sys-dev SSH/Tailscale name, e.g. sys-dev-2")
    parser.add_argument(
        "--bridge-vm", default="sys-dev", help="Qubes VM name (not its Tailscale name)"
    )
    parser.add_argument(
        "--profile", default="usb-lab", choices=["usb-lab", "usb-gui-lab"]
    )
    args = parser.parse_args(argv)
    if args.action != "build" and not args.target:
        parser.error("--target is required")
    session = None
    try:
        if args.action in ("build", "plan", "apply"):
            release, revision = build(args.profile)
            print(json.dumps({"release": release, "revision": revision}), flush=True)
            if args.action == "build":
                return 0
        session = Dom0(args.target)
        if args.action in ("plan", "apply"):
            transfer(args.target, args.bridge_vm, release, session)
            manifest = json.loads(
                session.run("cat " + shlex.quote(release + "/release.json"))
            )
            if manifest["revision"] != revision or manifest["profile"] != args.profile:
                raise RuntimeError(
                    "built release provenance does not match the requested commit/profile"
                )
            engine = manifest["coordinator"]
            preview = session.run(shlex.join([engine, "plan", release]))
            print(preview, flush=True)
            if args.action == "plan":
                return 0
            if (
                input(
                    "Apply this whole-machine release (may disconnect desktop and SSH)? [y/N] "
                ).lower()
                != "y"
            ):
                return 0
            action = [engine, "apply", release]
        else:
            action = ["/var/lib/airlock/machine/controller", args.action]
        if args.action in ("status", "verify"):
            print(session.run(shlex.join(action), timeout=900))
        else:
            unit = "airlock-machine-deploy-" + uuid.uuid4().hex[:12]
            script = shlex.join(
                [
                    "systemd-run",
                    "--unit=" + unit,
                    "--collect",
                    "--property=Type=exec",
                    *action,
                ]
            )
            print(session.run(script))
            print(
                f"Started {unit} in dom0; it survives SSH loss. Reconnect with the status command."
            )
        return 0
    except (RuntimeError, OSError, subprocess.SubprocessError, ValueError) as exc:
        print(f"machine-deploy: {exc}", file=sys.stderr)
        return 1
    finally:
        if session:
            session.close()


if __name__ == "__main__":
    raise SystemExit(main())
