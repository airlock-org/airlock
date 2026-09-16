#!/usr/bin/env python3
"""Root-owned, journalled whole-machine Nix deployment coordinator.

Transport and configuration evaluation deliberately live outside this module.
Qubes remains responsible for VM lifecycle, devices, policy and storage.
"""

import argparse
import contextlib
import fcntl
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

STATE = Path("/var/lib/airlock/machine")
SCHEMA = "airlock.machine-release.v1"


class DeploymentError(RuntimeError):
    pass


def read_json(path):
    return json.loads(Path(path).read_text())


def atomic_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(".tmp")
    with temporary.open("w") as stream:
        json.dump(value, stream, sort_keys=True, indent=2)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    temporary.replace(path)
    fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def run(*args, **kwargs):
    return subprocess.run(
        args, check=True, text=True, stdout=subprocess.PIPE, **kwargs
    ).stdout.strip()


def load_release(path):
    path = Path(path).resolve()
    release = read_json(path / "release.json")
    if release.get("schema") != SCHEMA:
        raise DeploymentError("unsupported machine release schema")
    if not re.fullmatch(r"[a-zA-Z0-9_-]+", release["profile"]):
        raise DeploymentError("invalid machine profile")
    if not release["host"].startswith("/nix/store/"):
        raise DeploymentError("host must be a built Nix system")
    topology = read_json(path / "topology.json")
    if topology.get("schema") != "airlock.qubes-topology.v1":
        raise DeploymentError("unsupported topology schema")
    return release, topology


def active_profile():
    marker = Path("/etc/airlock/machine-profile")
    if marker.exists():
        return marker.read_text().strip()
    # Compatibility with images predating the deployment coordinator.
    for profile in ("usb-lab", "usb-gui-lab"):
        if Path("/etc/airlock", profile).exists():
            return profile
    raise DeploymentError("this boot profile does not support machine deployment")


def dependencies(topology):
    domains = topology["domains"]
    result = {}
    defaults = topology.get("prefs", {})
    for name, domain in domains.items():
        prefs = domain.get("prefs", {})
        refs = [domain.get("netvm"), domain.get("template")]
        refs += [
            prefs.get(key, defaults.get("default_" + key))
            for key in ("guivm", "audiovm")
        ]
        for assignments in domain.get("devices", {}).values():
            refs += [device.get("backend") for device in assignments]
        result[name] = {ref for ref in refs if ref in domains and ref != name}
    return result


def ordered(topology):
    pending = dependencies(topology)
    result = []
    while pending:
        ready = sorted(name for name, deps in pending.items() if not deps - set(result))
        if not ready:
            raise DeploymentError("VM dependency cycle: " + ", ".join(sorted(pending)))
        result.extend(ready)
        for name in ready:
            del pending[name]
    return result


def plan(old, new, running, retain_removed=False):
    """Conservative lifecycle plan; never silently ignore destructive changes."""
    before, after = old["domains"], new["domains"]
    removed = before.keys() - after.keys()
    if removed and not retain_removed:
        raise DeploymentError("VM removal requires a migration: " + ", ".join(removed))
    for name, domain in after.items():
        backend = domain.get("backend", {})
        if backend.get("type") != "nix-closure":
            raise DeploymentError(
                f"{name}: only Nix-closure VM deployment is supported"
            )
        if backend.get("store", {}).get("mode") != "shared-closure":
            raise DeploymentError(f"{name}: shared-closure storage is required")
        if name not in before:
            continue
        previous = before[name]
        for key in ("class", "template", "devices"):
            if previous.get(key) != domain.get(key):
                raise DeploymentError(f"{name}: changing {key} requires a migration")
        for key in ("type", "state"):
            if previous.get("backend", {}).get(key) != backend.get(key):
                raise DeploymentError(
                    f"{name}: changing backend {key} requires a migration"
                )
        for key in ("prefs", "features", "services"):
            missing = previous.get(key, {}).keys() - domain.get(key, {}).keys()
            if missing:
                raise DeploymentError(
                    f"{name}: removing {key} requires explicit reset values"
                )
    # setup-config is a converger, not a garbage collector.
    for key in (("prefs",) if retain_removed else ("kernels", "templates", "prefs")):
        if old.get(key, {}).keys() - new.get(key, {}).keys():
            raise DeploymentError(f"removing topology {key} requires a migration")
    changed = {name for name in after if before.get(name) != after[name]}
    if old.get("kernels") != new.get("kernels") or old.get("prefs") != new.get("prefs"):
        changed.update(after)
    affected = set(changed) | removed
    deps = dependencies(old)
    while True:
        expanded = affected | {name for name, refs in deps.items() if refs & affected}
        if expanded == affected:
            break
        affected = expanded
    start = {
        name
        for name in after
        if name in running or after[name].get("prefs", {}).get("autostart") is True
    }
    new_deps = dependencies(new)
    while True:
        expanded = start | set().union(*(new_deps[name] for name in start))
        if expanded == start:
            break
        start = expanded
    return {
        "changed": sorted(changed),
        "stage": sorted(
            name
            for name in changed
            if before.get(name, {}).get("backend") != after[name].get("backend")
        ),
        "stop": [name for name in reversed(ordered(old)) if name in affected & running],
        "start": [name for name in ordered(new) if name in start],
        "affected": sorted(affected),
        "retained_undeclared": sorted(removed),
    }


class Coordinator:
    def __init__(self, state=STATE, command=run):
        self.state = Path(state)
        self.command = command

    @contextlib.contextmanager
    def locked(self):
        self.state.mkdir(parents=True, exist_ok=True, mode=0o700)
        with (self.state / "lock").open("a") as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as exc:
                raise DeploymentError("another machine deployment is running") from exc
            yield lock

    def pointer(self):
        path = self.state / "selection.json"
        return read_json(path) if path.exists() else None

    def running(self):
        text = self.command("qvm-ls", "--raw-data", "--fields", "NAME,STATE")
        return {
            line.split("|")[0]
            for line in text.splitlines()
            if line.endswith("|Running") and not line.startswith("dom0|")
        }

    def pin(self, name, path):
        root = self.state / "roots" / name
        root.parent.mkdir(exist_ok=True)
        self.command("nix-store", "--add-root", str(root), "--realise", str(path))

    def baseline(self):
        selected = self.pointer()
        if selected:
            return Path(selected["release"])
        baseline = self.state / "baseline"
        baseline.mkdir(exist_ok=True)
        source = Path("/var/lib/airlock/system/current-topology.json")
        if not source.exists():
            raise DeploymentError("bootstrap has not completed; no current topology")
        profile = active_profile()
        host = str(Path("/run/current-system").resolve())
        setup = str(Path("/run/current-system/sw/bin/airlock-setup-config").resolve())
        atomic_json(baseline / "topology.json", read_json(source))
        atomic_json(
            baseline / "release.json",
            {
                "schema": SCHEMA,
                "profile": profile,
                "host": host,
                "setup": setup,
                "revision": "booted-image-baseline",
                "lockHash": None,
            },
        )
        self.pin("baseline-host", host)
        return baseline

    def inspect(self, release):
        desired, new = load_release(release)
        current = self.pointer()
        if current:
            previous, old = load_release(current["release"])
            profile = previous["profile"]
        else:
            profile = active_profile()
            old = read_json("/var/lib/airlock/system/current-topology.json")
        if profile != desired["profile"]:
            raise DeploymentError(
                f"profile mismatch: booted {profile}, release {desired['profile']}"
            )
        result = plan(old, new, self.running())
        result["host_reboot_required"] = (
            str(Path("/run/current-system").resolve()) != desired["host"]
        )
        result["revision"] = desired["revision"]
        return result

    def install_boot_hook(self, release, baseline):
        """Repository-owned compatibility bridge for already-flashed images."""
        manifest, _ = load_release(release)
        fallback, _ = load_release(baseline)
        controller = manifest.get("coordinator")
        if controller:
            link = self.state / "controller.new"
            link.unlink(missing_ok=True)
            link.symlink_to(controller)
            link.replace(self.state / "controller")
        for unit, scope in (
            ("airlock-setup-config", ""),
            ("airlock-setup-config-deferred", " --deferred"),
        ):
            # Do not enable units absent from this profile.
            if (
                self.command(
                    "systemctl", "show", unit, "--property=LoadState", "--value"
                )
                == "not-found"
            ):
                continue
            dropin = Path(
                "/etc/systemd/system", unit + ".service.d", "90-airlock-machine.conf"
            )
            dropin.parent.mkdir(parents=True, exist_ok=True)
            dropin.write_text(
                "# Generated by repository-owned airlock-machine-deploy.\n"
                "[Service]\nExecStart=\nExecStart="
                + str(self.state / "controller")
                + " boot-setup "
                + fallback["setup"]
                + " --apply"
                + scope
                + "\n"
            )
        self.command("systemctl", "daemon-reload")

    def stage(self, release, topology, names, directory):
        staged = {}
        for name in names:
            backend = topology["domains"][name]["backend"]
            store = backend["store"]
            artifact = store["source"]
            # Image rollback uses the retained, already admitted seed descriptor.
            if (Path(artifact) / "prepared.json").exists():
                continue
            output = self.command(
                "airlock-closure-store",
                "admit",
                "--domain",
                name,
                "--system",
                backend["system"]["source"],
                "--closure-source",
                artifact,
                "--root-source",
                backend["root"]["source"],
                "--kernel-name",
                backend["kernel"]["name"],
                "--store-device",
                store["device"],
                "--store-blob-device",
                store["blobDevice"],
                "--store-mountpoint",
                store["mountpoint"],
                "--view-backend",
                store["viewBackend"],
                "--approval-source",
                artifact,
                "--activate",
                "false",
                "--retain",
                "true",
            )
            staged[name] = json.loads(output)["generation"]
            atomic_json(directory / "staged.json", staged)
        atomic_json(directory / "staged.json", staged)

    def setup(self, release, directory):
        manifest, _ = load_release(release)
        env = dict(
            os.environ,
            AIRLOCK_LOG_DIR=str(directory / "setup"),
            AIRLOCK_STAGED_GENERATIONS=str(directory / "staged.json"),
        )
        self.command(
            manifest["setup"],
            "--apply",
            "--topology",
            str(release / "topology.json"),
            env=env,
        )
        result = read_json(directory / "setup" / "setup-config.json")
        if result.get("unsupported_domains_count") or result.get("restart_required"):
            raise DeploymentError(
                "setup left unsupported or deferred work; inspect setup-config.json"
            )

    def verify(self, topology, expected_running):
        observed = {}
        running = self.running()
        for name, domain in topology["domains"].items():
            expected = domain["backend"]["system"]["source"]
            selected = self.command("qvm-features", name, "airlock-system-source")
            if selected != expected:
                raise DeploymentError(
                    f"{name}: selected system {selected!r}, expected {expected}"
                )
            actual = None
            if name in expected_running:
                if name not in running:
                    raise DeploymentError(f"{name}: failed to start")
                actual = self.command(
                    "qrexec-client",
                    "-d",
                    name,
                    "user:readlink -f /run/current-system",
                    timeout=180,
                )
                if actual != expected:
                    raise DeploymentError(
                        f"{name}: running system differs from release: {actual}"
                    )
            observed[name] = {"selected": selected, "running": actual}
        return observed

    def apply(self, release, recover=False):
        release = Path(release).resolve()
        manifest, topology = load_release(release)
        with self.locked():
            latest = self.state / "latest.json"
            if latest.exists() and not recover:
                existing = read_json(latest)
                if existing["phase"] in ("complete", "reboot-required") and existing[
                    "release"
                ] == str(release):
                    self.verify(topology, set(existing["plan"]["start"]))
                    return existing
            if (
                latest.exists()
                and read_json(latest)["phase"] not in ("complete", "reboot-required")
                and not recover
            ):
                raise DeploymentError(
                    "unfinished deployment: inspect status, then use resume or rollback"
                )
            baseline = self.baseline()
            interrupted = read_json(latest) if recover and latest.exists() else None
            # Resume compares against the pre-transaction topology, not the
            # selected-but-possibly-only-half-installed topology.
            if interrupted and str(release) == interrupted["release"]:
                baseline = Path(interrupted["previous"])
            previous, old = load_release(baseline)
            profile = active_profile()
            if manifest["profile"] != profile or previous["profile"] != profile:
                raise DeploymentError("deployment profile differs from booted profile")
            runtime = self.running()
            wanted_running = runtime | (
                set(interrupted["plan"]["start"]) if interrupted else set()
            )
            rollback = (
                interrupted is not None and str(release) != interrupted["release"]
            )
            changes = plan(old, topology, wanted_running, retain_removed=rollback)
            changes["stop"] = [name for name in changes["stop"] if name in runtime]
            transaction = str(time.time_ns())
            directory = self.state / "transactions" / transaction
            directory.mkdir(parents=True)
            record = {
                "id": transaction,
                "release": str(release),
                "previous": str(baseline),
                "revision": manifest["revision"],
                "plan": changes,
                "phase": "preflight",
            }

            def phase(value, **extra):
                record.update(phase=value, **extra)
                atomic_json(directory / "status.json", record)
                atomic_json(latest, record)
                print(
                    json.dumps({"transaction": transaction, "phase": value}), flush=True
                )

            phase("staging")
            try:
                if str(release).startswith("/nix/store/"):
                    self.pin(transaction, release)
                self.install_boot_hook(release, baseline)
                self.stage(release, topology, changes["stage"], directory)
                # Selection is written before disruption. Boot fails closed on an
                # interrupted transaction instead of restoring the image topology.
                atomic_json(
                    self.state / "selection.json",
                    {
                        "release": str(release),
                        "previous": str(baseline),
                        "transaction": transaction,
                        "profile": manifest["profile"],
                    },
                )
                phase("stopping")
                for name in changes["stop"]:
                    self.command(
                        "qvm-shutdown", "--wait", "--timeout", "120", name, timeout=150
                    )
                phase("configuring")
                self.setup(release, directory)
                phase("host-boot-selection")
                self.command(
                    "nix-env",
                    "-p",
                    "/nix/var/nix/profiles/system",
                    "--set",
                    manifest["host"],
                )
                self.command(
                    manifest["host"] + "/bin/switch-to-configuration",
                    "boot",
                    timeout=600,
                )
                phase("starting")
                for name in changes["start"]:
                    if name not in self.running():
                        self.command("qvm-start", name, timeout=240)
                phase("verifying")
                observed = self.verify(topology, set(changes["start"]))
                pending = str(Path("/run/current-system").resolve()) != manifest["host"]
                phase(
                    "reboot-required" if pending else "complete",
                    observed=observed,
                    host_running=str(Path("/run/current-system").resolve()),
                    host_selected=manifest["host"],
                )
            except Exception as exc:
                phase("failed", error=str(exc), failed_phase=record["phase"])
                raise
        return record

    def status(self):
        path = self.state / "latest.json"
        result = read_json(path) if path.exists() else {"phase": "image-baseline"}
        result["selection"] = self.pointer()
        result["host_running_now"] = str(Path("/run/current-system").resolve())
        result["host_matches_selection"] = (
            result.get("host_selected") == result["host_running_now"]
        )
        return result

    def verify_selected(self):
        with self.locked():
            selected = self.pointer()
            if not selected:
                raise DeploymentError("no selected deployment")
            directory = self.state / "transactions" / selected["transaction"]
            record = read_json(directory / "status.json")
            if record["phase"] not in ("complete", "reboot-required"):
                raise DeploymentError("unfinished deployment; resume or rollback first")
            manifest, topology = load_release(selected["release"])
            record["observed"] = self.verify(topology, set(record["plan"]["start"]))
            record["host_running"] = str(Path("/run/current-system").resolve())
            record["phase"] = (
                "complete"
                if record["host_running"] == manifest["host"]
                else "reboot-required"
            )
            record["verified_at"] = time.time()
            atomic_json(directory / "status.json", record)
            atomic_json(self.state / "latest.json", record)
            return record

    def boot_setup(self, default_setup, args):
        with self.locked() as lock:
            # Keep the lock across exec through the exact setup wrapper.
            os.set_inheritable(lock.fileno(), True)
            self._boot_setup(default_setup, args)

    def _boot_setup(self, default_setup, args):
        selected = self.pointer()
        if not selected:
            return os.execv(default_setup, [default_setup, *args])
        release, _ = load_release(selected["release"])
        profile = active_profile()
        if selected["profile"] != profile:
            # Recovery/other profiles must not adopt a lab deployment implicitly.
            raise DeploymentError(
                "selected deployment belongs to another boot profile; use recovery"
            )
        status = read_json(
            self.state / "transactions" / selected["transaction"] / "status.json"
        )
        if status["phase"] not in ("complete", "reboot-required"):
            raise DeploymentError(
                "interrupted deployment; use machine-deploy resume/rollback in recovery"
            )
        setup = release["setup"]
        os.execv(
            setup,
            [
                setup,
                *args,
                "--topology",
                str(Path(selected["release"]) / "topology.json"),
            ],
        )


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    for action in ("plan", "apply"):
        sub.add_parser(action).add_argument("release", type=Path)
    for action in ("status", "verify", "resume", "rollback"):
        sub.add_parser(action)
    boot = sub.add_parser("boot-setup")
    boot.add_argument("setup")
    boot.add_argument("args", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    coordinator = Coordinator()
    try:
        if args.action == "plan":
            result = coordinator.inspect(args.release)
        elif args.action == "apply":
            result = coordinator.apply(args.release)
        elif args.action in ("resume", "rollback"):
            status = coordinator.status()
            key = "release" if args.action == "resume" else "previous"
            if key not in status:
                raise DeploymentError("no deployment to resume or roll back")
            result = coordinator.apply(status[key], recover=True)
        elif args.action == "boot-setup":
            coordinator.boot_setup(args.setup, args.args)
            return 0
        elif args.action == "verify":
            result = coordinator.verify_selected()
        else:
            result = coordinator.status()
        print(json.dumps(result, sort_keys=True, indent=2))
        return 0
    except (DeploymentError, OSError, subprocess.SubprocessError, ValueError) as exc:
        print(f"airlock-machine-deploy: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
