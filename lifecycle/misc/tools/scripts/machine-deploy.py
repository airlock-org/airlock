#!/usr/bin/env python3
"""Build a committed whole-machine release and deploy via the usr-assistant bridge."""

import argparse
import base64
import getpass
import hashlib
import json
import os
import re
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


def build(profile, *, assistant_rename=False):
    if command("git", "status", "--porcelain", cwd=ROOT):
        raise RuntimeError(
            "commit repository changes before deployment (including new files)"
        )
    dev_revision = command("git", "rev-parse", "HEAD", cwd=ROOT)
    os_revision = command("git", "-C", "airlock-machine", "rev-parse", "HEAD", cwd=ROOT)
    attr = f"airlock-machine-release-{profile}"
    if assistant_rename:
        if profile != "usb-lab":
            raise RuntimeError("assistant rename requires the usb-lab profile")
        attr += "-assistant-rename"
    result = json.loads(
        command(
            str(NIX),
            "build",
            "./airlock-machine#" + attr,
            "--json",
            "--no-link",
            cwd=ROOT,
        )
    )
    if (
        command("git", "rev-parse", "HEAD", cwd=ROOT) != dev_revision
        or command("git", "status", "--porcelain", cwd=ROOT)
        or command("git", "-C", "airlock-machine", "rev-parse", "HEAD", cwd=ROOT)
        != os_revision
    ):
        raise RuntimeError("repository changed during build; retry from a clean commit")
    release = result[0]["outputs"]["out"]
    # Pin the output in the guarded Airlock store, not the host store.
    log = ROOT / ".airlock-logs" / "machine-deploy" / dev_revision
    log.mkdir(parents=True, exist_ok=True)
    command(str(NIX), "build", release, "--out-link", str(log / "release"), cwd=ROOT)
    (log / "build.json").write_text(
        json.dumps(
            {
                "devRevision": dev_revision,
                "osRevision": os_revision,
                "profile": profile,
                "release": release,
            },
            indent=2,
        )
        + "\n"
    )
    return release, os_revision, dev_revision


def host_repair_snapshot(session, profile):
    script = '''from pathlib import Path
import json
selection=Path('/var/lib/airlock/machine/selection.json')
chosen=json.loads(selection.read_text()) if selection.exists() else None
manifest=json.loads((Path(chosen['release'])/'release.json').read_text()) if chosen else None
print(json.dumps({'host':str(Path('/run/current-system').resolve()),
 'profile':Path('/etc/airlock/machine-profile').read_text().strip(),
 'devMode':Path('/etc/airlock/dev-mode').exists(),
 'boot':json.loads(Path('/etc/airlock/boot-source.json').read_text()),
 'manifest':manifest,'topology':json.loads(Path('/var/lib/airlock/system/current-topology.json').read_text())}))'''
    snapshot = json.loads(session.run("python3 -c " + shlex.quote(script)))
    if not snapshot.pop("devMode") or snapshot["profile"] != profile:
        raise RuntimeError("host repair requires the matching development boot profile")
    manifest = snapshot.pop("manifest")
    if manifest:
        revision = manifest.get("revision")
        expected_lock = manifest.get("lockHash")
    else:
        # The immutable boot label identifies the source used for the image.
        # Resolve its committed gitlink, never the current guest checkout HEAD.
        boot_revision = snapshot["boot"].get("revision")
        if not re.fullmatch(r"[0-9a-f]{40}", str(boot_revision)):
            raise RuntimeError("image has no verified boot source revision")
        entry = command("git", "ls-tree", boot_revision, "airlock-machine", cwd=ROOT).split()
        if len(entry) != 4 or entry[0] != "160000":
            raise RuntimeError("image source does not identify a committed machine gitlink")
        revision = entry[2]
        expected_lock = None
    if not re.fullmatch(r"[0-9a-f]{40}", str(revision)):
        raise RuntimeError("host repair has no verified machine source baseline")
    lock = subprocess.check_output(["git", "show", revision + ":flake.lock"], cwd=ROOT / "airlock-machine")
    lock_hash = hashlib.sha256(lock).hexdigest()
    if manifest and expected_lock != lock_hash:
        raise RuntimeError("selected machine lock differs from its committed source")
    snapshot.update(revision=revision, lockHash=lock_hash,
                    topologyHash=hashlib.sha256(json.dumps(snapshot["topology"], sort_keys=True).encode()).hexdigest(),
                    baselineFlake="git+" + (ROOT / "airlock-machine").as_uri() + "?rev=" + revision,
                    repairRevision=command("git", "-C", "airlock-machine", "rev-parse", "HEAD", cwd=ROOT))
    return snapshot


def build_host_repair(profile, session):
    if command("git", "status", "--porcelain", cwd=ROOT):
        raise RuntimeError("commit repository changes before building a host repair")
    revision = command("git", "rev-parse", "HEAD", cwd=ROOT)
    snapshot = host_repair_snapshot(session, profile)
    log = ROOT / ".airlock-logs/machine-host-repair" / revision
    log.mkdir(parents=True, exist_ok=True)
    path = log / "snapshot.json"
    path.write_text(json.dumps(snapshot, sort_keys=True) + "\n")
    result = json.loads(command(str(NIX), "build", "--impure", ".#airlock-machine-host-repair",
                               "--json", "--out-link", str(log / "release"), cwd=ROOT,
                               env=dict(os.environ, AIRLOCK_HOST_REPAIR_SNAPSHOT=str(path))))
    if (command("git", "rev-parse", "HEAD", cwd=ROOT) != revision
            or command("git", "status", "--porcelain", cwd=ROOT)):
        raise RuntimeError("repository changed during the host repair build")
    release = result[0]["outputs"]["out"]
    (log / "build.json").write_text(json.dumps({"release": release, "snapshot": snapshot}, indent=2) + "\n")
    return release, snapshot["revision"], revision


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
    # usr-assistant deliberately has a small package set and need not include Python.
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
        raise RuntimeError("invalid usr-assistant filesystem space report")
    bridge_free = bridge_blocks * bridge_block_size
    if bridge_free < needed + 512 * 1024**2:
        raise RuntimeError(
            "usr-assistant needs more private-volume space for the compressed closure transfer"
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


def remote_receipt(target, path):
    payload = command("ssh", target, "cat -- " + shlex.quote(path))
    receipt = json.loads(payload)
    if not isinstance(receipt, dict):
        raise RuntimeError("usr-assistant admission receipt is not a JSON object")
    return receipt


def install_receipt(session, receipt):
    payload = (json.dumps(receipt, sort_keys=True, indent=2) + "\n").encode()
    digest = hashlib.sha256(payload).hexdigest()
    encoded = base64.b64encode(payload).decode()
    destination = f"/var/lib/airlock/machine/receipts/{digest}.json"
    temporary = destination + ".new"
    if len(encoded) > 1024:
        # Admission receipts include transfer timings and can exceed the PTY's
        # control-line limit. Publish only after every metadata chunk is intact.
        temporary += "-" + uuid.uuid4().hex
        session.run(
            "umask 077; mkdir -p /var/lib/airlock/machine/receipts; "
            f": > {shlex.quote(temporary)}"
        )
        for offset in range(0, len(encoded), 1024):
            session.run(
                f"printf %s {shlex.quote(encoded[offset:offset + 1024])} | "
                f"base64 -d >> {shlex.quote(temporary)}"
            )
        session.run(
            f"echo {shlex.quote(digest + '  ' + temporary)} | sha256sum -c - >/dev/null; "
            f"mv {shlex.quote(temporary)} {shlex.quote(destination)}"
        )
        return destination
    session.run(
        "umask 077; mkdir -p /var/lib/airlock/machine/receipts; "
        f"printf %s {shlex.quote(encoded)} | base64 -d > {shlex.quote(temporary)}; "
        f"mv {shlex.quote(temporary)} {shlex.quote(destination)}"
    )
    return destination


def deployed_engine(session):
    return session.run(
        "if [ -x /var/lib/airlock/machine/controller ]; then "
        "printf %s /var/lib/airlock/machine/controller; else "
        "printf %s /run/current-system/sw/bin/airlock-machine-deploy; fi"
    )


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "action",
        choices=[
            "build",
            "plan",
            "apply",
            "status",
            "verify",
            "resume",
            "rollback",
            "guest-plan",
            "guest-apply",
        ],
    )
    parser.add_argument("--target", help="usr-assistant SSH/Tailscale name, e.g. usr-assistant-2")
    parser.add_argument(
        "--bridge-vm", help="Qubes VM name (defaults to usr-assistant, or sys-agent for the rename transition)"
    )
    parser.add_argument(
        "--profile", default="usb-lab", choices=["usb-lab", "usb-gui-lab"]
    )
    parser.add_argument(
        "--receipt", help="path to the inactive admission receipt inside usr-assistant"
    )
    parser.add_argument(
        "--update-coordinator",
        action="store_true",
        help="build and transfer the committed coordinator for guest activation or recovery",
    )
    parser.add_argument("--legacy-dom0", action="store_true",
                        help="use the development shell and host-built closure transport")
    parser.add_argument("--host-repair", action="store_true",
                        help="Repair dom0 while retaining the verified source baseline and current guest artifacts.")
    parser.add_argument(
        "--assistant-rename", action="store_true",
        help="build the one-shot sys-agent to usr-assistant private-volume migration",
    )
    parser.add_argument(
        "--controller-upgrade", action="store_true",
        help="bootstrap rename support with the existing sys-agent client and a sys-ctrl guest update",
    )
    args = parser.parse_args(argv)
    args.bridge_vm = args.bridge_vm or (
        "sys-agent" if args.assistant_rename else "usr-assistant"
    )
    if args.action != "build" and not args.target:
        parser.error("--target is required")
    if args.host_repair and (not args.legacy_dom0 or args.action not in {"plan", "apply"} or args.profile != "usb-lab"):
        parser.error("--host-repair requires legacy-dom0 plan/apply for usb-lab")
    if args.action.startswith("guest-") and not args.receipt:
        parser.error("--receipt is required for guest activation")
    if args.update_coordinator and not (
        args.action.startswith("guest-")
        or (args.legacy_dom0 and args.action in {"resume", "rollback"})
    ):
        parser.error("--update-coordinator requires guest activation or legacy-dom0 resume/rollback")
    if args.assistant_rename and (
        args.profile != "usb-lab" or args.action not in {"build", "plan", "apply"}
    ):
        parser.error("--assistant-rename requires usb-lab build, plan, or apply")
    if args.controller_upgrade and (
        args.profile != "usb-lab" or args.action not in {"plan", "apply"}
        or args.legacy_dom0 or args.assistant_rename
    ):
        parser.error("--controller-upgrade requires usb-lab controller plan/apply without rename or legacy flags")
    if (not args.legacy_dom0 and args.profile == "usb-lab"
            and args.action in {"plan", "apply", "status", "verify", "resume", "rollback"}):
        try:
            if args.action in {"plan", "apply"}:
                environment = os.environ.copy()
                if args.assistant_rename or args.controller_upgrade:
                    environment.setdefault("AIRLOCK_DEPLOY_SOURCE_COMMAND", "airlock-sys-agent-machine-source")
                subprocess.run([sys.executable, str(ROOT / "lifecycle/misc/tools/scripts/usr-assistant-machine-source.py"),
                                "push", "--target", args.target], check=True, env=environment)
            if args.controller_upgrade:
                return subprocess.call([
                    "ssh", args.target, "airlock-sys-agent-machine-deploy",
                    "guest-" + args.action, "--target", "sys-ctrl",
                ])
            if args.assistant_rename:
                return subprocess.call([
                    "ssh", args.target, "python3",
                    "/home/user/airlock-machine/lifecycle/runtime/tools/airlock.py",
                    "machine", args.action, "--assistant-rename",
                ])
            return subprocess.call(["ssh", args.target,
                                    os.environ.get("AIRLOCK_DEPLOY_REMOTE_CLIENT", "airlock"), "machine", args.action])
        except (OSError, subprocess.SubprocessError) as exc:
            print(f"machine-deploy: controller submission failed: {exc}", file=sys.stderr)
            return 1
    session = None
    try:
        if args.action in ("build", "plan", "apply"):
            if args.host_repair:
                session = Dom0(args.target)
                release, os_revision, dev_revision = build_host_repair(args.profile, session)
            else:
                release, os_revision, dev_revision = (
                build(args.profile, assistant_rename=True) if args.assistant_rename
                else build(args.profile)
                )
            print(
                json.dumps(
                    {
                        "release": release,
                        "osRevision": os_revision,
                        "devRevision": dev_revision,
                    }
                ),
                flush=True,
            )
            if args.action == "build":
                return 0
        if session is None:
            session = Dom0(args.target)
        updated_engine = None
        if args.update_coordinator:
            if command("git", "status", "--porcelain", cwd=ROOT):
                raise RuntimeError("commit repository changes before updating the coordinator")
            result = json.loads(
                command(
                    str(NIX), "build", "./airlock-machine#airlock-machine-deploy",
                    "--json", "--out-link",
                    str(ROOT / ".airlock-logs" / "machine-coordinator"), cwd=ROOT,
                )
            )
            package = result[0]["outputs"]["out"]
            transfer(args.target, args.bridge_vm, package, session)
            updated_engine = package + "/bin/airlock-machine-deploy"
        if args.action in ("guest-plan", "guest-apply"):
            engine = updated_engine or deployed_engine(session)
            receipt = remote_receipt(args.target, args.receipt)
            receipt_path = install_receipt(session, receipt)
            preview = session.run(
                shlex.join([engine, "guest-plan", receipt_path]), timeout=900
            )
            print(preview, flush=True)
            if args.action == "guest-plan":
                return 0
            if (
                input(
                    "Activate this admitted guest generation (restarts affected VMs)? [y/N] "
                ).lower()
                != "y"
            ):
                return 0
            if args.legacy_dom0:
                candidate = json.loads(preview).get("candidate")
                if (not isinstance(candidate, str)
                        or not candidate.startswith("/var/lib/airlock/machine/candidates/")):
                    raise RuntimeError("guest plan did not return a local machine candidate")
                action = [engine, "apply", candidate, "--administrative"]
            else:
                action = [engine, "guest-apply", receipt_path]
        elif args.action in ("plan", "apply"):
            transfer(args.target, args.bridge_vm, release, session)
            manifest = json.loads(
                session.run("cat " + shlex.quote(release + "/release.json"))
            )
            if (
                manifest["revision"] != os_revision
                or manifest["profile"] != args.profile
            ):
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
            if args.legacy_dom0:
                action.append("--administrative")
        else:
            action = [updated_engine or "/var/lib/airlock/machine/controller", args.action]
        if args.action in ("status", "verify"):
            script = shlex.join(action)
            if args.action == "status":
                # A pristine image has no selected release/controller yet.
                # Do not hide a broken controller symlink or controller failure.
                controller = shlex.quote(action[0])
                script = (
                    f"if [ -e {controller} ] || [ -L {controller} ]; then {script}; "
                    "else /run/current-system/sw/bin/airlock-machine-deploy status; fi"
                )
            print(session.run(script, timeout=900))
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
