#!/usr/bin/env python3
"""Build representative guest changes using only the image's pinned inputs."""

import argparse
from io import BytesIO
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile
import time


STORE_PATH = re.compile(r"/nix/store/[0-9abcdfghijklmnpqrsvwxyz]{32}-[^/]+")
SOURCE_PATH = re.compile(
    r"/nix/store/[0-9abcdfghijklmnpqrsvwxyz]{32}-[^/]+(?:/[^/.][^/]*)*"
)
INSTALLABLE = "nixosConfigurations.airlock-domain-usr-home.config.system.build.toplevel"
GUEST_MODULE = Path("lifecycle/misc/modules/usr-home-guest.nix")
PACKAGE_LINE = "  environment.systemPackages = map (application: application.package) applications;"
SCENARIOS = {
    "etc-marker": '  environment.etc."airlock-offline-gate".text = "offline gate marker";',
    "service-setting": '  systemd.services.qubes-qrexec-agent.serviceConfig.TimeoutStartSec = "90s";',
}


def checked(command, **kwargs):
    result = subprocess.run(command, capture_output=True, text=True, **kwargs)
    if result.returncode:
        raise RuntimeError(
            f"{' '.join(map(str, command[:4]))}: {result.stderr.strip()}"
        )
    return result.stdout.strip()


def store_root(path):
    path = str(path).split("/", 4)
    return "/".join(path[:4])


def physical(source_root, logical):
    return source_root / logical.lstrip("/")


def prepare_store(store, source_root, paths, registration, flake_sources):
    target = store / "nix/store"
    target.mkdir(parents=True)
    for logical in paths:
        original = physical(source_root, logical)
        if not original.exists():
            raise RuntimeError(
                f"pinned input is absent from the build store: {logical}"
            )
        staged = target / Path(logical).name
        if logical in flake_sources:
            if not original.is_dir():
                raise RuntimeError(f"flake input is not a directory: {logical}")
            staged.mkdir()
        else:
            staged.symlink_to(original, target_is_directory=original.is_dir())
    checked(["nix-store", "--store", str(store), "--load-db"], input=registration)


def checkout_with_change(machine, target, change):
    archive = subprocess.run(
        ["git", "-C", str(machine), "archive", "--format=tar", "HEAD"],
        capture_output=True,
        check=True,
    ).stdout
    target.mkdir()
    with tarfile.open(fileobj=BytesIO(archive)) as entries:
        entries.extractall(target, filter="data")
    module = target / GUEST_MODULE
    text = module.read_text()
    if text.count(PACKAGE_LINE) != 1:
        raise RuntimeError(
            "usr-home guest module changed; update offline gate examples"
        )
    module.write_text(text.replace(PACKAGE_LINE, PACKAGE_LINE + "\n" + change, 1))


def run_trial(args, name, change, paths, registration, inputs, log_dir, temporary):
    trial = temporary / name
    trial.mkdir()
    store = trial / "store"
    flake_sources = {store_root(path) for path in inputs.values()}
    prepare_store(store, args.source_store_root, paths, registration, flake_sources)
    checkout = trial / "machine"
    checkout_with_change(args.machine, checkout, change)
    for directory in (trial / "home", trial / "cache", trial / "tmp"):
        directory.mkdir()
    overrides = []
    for input_name, path in sorted(inputs.items()):
        overrides.extend(("--override-input", input_name, f"path:{path}"))
    flake_mounts = []
    for path in sorted(flake_sources):
        source = str(physical(args.source_store_root, path))
        flake_mounts.extend(("--ro-bind", source, str(store / path.lstrip("/"))))
        flake_mounts.extend(("--ro-bind", source, path))
    command = [
        "bwrap",
        "--die-with-parent",
        "--unshare-net",
        "--ro-bind",
        "/",
        "/",
        "--bind",
        str(trial),
        str(trial),
        "--bind",
        str(store / "nix/store"),
        "/nix/store",
        *flake_mounts,
        "--dev-bind",
        "/dev",
        "/dev",
        "--proc",
        "/proc",
        "--clearenv",
        "--setenv",
        "HOME",
        str(trial / "home"),
        "--setenv",
        "XDG_CACHE_HOME",
        str(trial / "cache"),
        "--setenv",
        "TMPDIR",
        str(trial / "tmp"),
        "--setenv",
        "NIX_CONFIG",
        f"store = {store}\nsubstituters = \n",
        "--",
        str(args.nix_bin),
        "--extra-experimental-features",
        "nix-command flakes",
        "--store",
        str(store),
        "--option",
        "build-hook",
        "",
        "build",
        "--offline",
        "--no-write-lock-file",
        *overrides,
        f"path:{checkout}#{INSTALLABLE}",
        "--no-link",
        "--json",
    ]
    started = time.monotonic()
    try:
        result = subprocess.run(
            command, capture_output=True, text=True, timeout=args.timeout
        )
    except subprocess.TimeoutExpired as exc:

        def decoded(output):
            return (
                output.decode(errors="replace")
                if isinstance(output, bytes)
                else (output or "")
            )

        (log_dir / f"{name}.stdout").write_text(decoded(exc.stdout))
        (log_dir / f"{name}.stderr").write_text(decoded(exc.stderr))
        raise RuntimeError(
            f"{name} offline build timed out after {args.timeout}s"
        ) from exc
    (log_dir / f"{name}.stdout").write_text(result.stdout)
    (log_dir / f"{name}.stderr").write_text(result.stderr)
    if result.returncode:
        raise RuntimeError(
            f"{name} offline build failed; see {log_dir / f'{name}.stderr'}"
        )
    try:
        system = json.loads(result.stdout)[0]["outputs"]["out"]
    except (ValueError, IndexError, KeyError, TypeError) as exc:
        raise RuntimeError(f"{name} returned no system path") from exc
    if system in paths:
        raise RuntimeError(f"{name} reused a preseeded final system: {system}")
    if not isinstance(system, str) or not STORE_PATH.fullmatch(system):
        raise RuntimeError(f"{name} returned an invalid system path: {system}")
    for logical in paths:
        staged = store / logical.lstrip("/")
        if logical in flake_sources:
            unchanged = staged.is_dir() and not any(staged.iterdir())
        else:
            unchanged = staged.is_symlink()
        if not unchanged:
            raise RuntimeError(f"{name} replaced a pinned input: {logical}")
    return {"system": system, "elapsed_seconds": round(time.monotonic() - started, 2)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--closure", required=True, type=Path)
    parser.add_argument("--machine", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--source-store-root", required=True, type=Path)
    parser.add_argument("--nix-bin", required=True, type=Path)
    parser.add_argument("--log-dir", required=True, type=Path)
    parser.add_argument("--temporary-root", required=True, type=Path)
    parser.add_argument("--timeout", type=int, default=1800)
    args = parser.parse_args()
    args.log_dir.mkdir(parents=True, exist_ok=True)
    args.temporary_root.mkdir(parents=True, exist_ok=True)
    for name in SCENARIOS:
        for suffix in ("stdout", "stderr"):
            (args.log_dir / f"{name}.{suffix}").unlink(missing_ok=True)
    report = {
        "schema": "airlock.offline-builder-gate.v1",
        "status": "running",
        "scenarios": {},
    }
    try:
        if checked(["git", "-C", str(args.machine), "status", "--porcelain"]):
            raise RuntimeError(
                "machine source must be committed before the offline gate"
            )
        revision = checked(["git", "-C", str(args.machine), "rev-parse", "HEAD"])
        lock_hash = hashlib.sha256(
            (args.machine / "flake.lock").read_bytes()
        ).hexdigest()
        manifest = json.loads(args.manifest.read_text())
        if manifest.get("schema") != "airlock.sys-dev-offline-inputs.v1":
            raise RuntimeError("invalid offline input manifest")
        if manifest.get("lock_hash") != lock_hash:
            raise RuntimeError("offline input manifest differs from machine lock")
        inputs = manifest.get("inputs")
        if not isinstance(inputs, dict) or not inputs:
            raise RuntimeError("offline input manifest has no inputs")
        if any(
            not isinstance(name, str)
            or not isinstance(path, str)
            or not SOURCE_PATH.fullmatch(path)
            for name, path in inputs.items()
        ):
            raise RuntimeError("offline input manifest has invalid paths")
        paths = set(
            (physical(args.source_store_root, str(args.closure)) / "store-paths")
            .read_text()
            .splitlines()
        )
        if not paths or any(not STORE_PATH.fullmatch(path) for path in paths):
            raise RuntimeError("invalid builder input closure")
        if any(store_root(path) not in paths for path in inputs.values()):
            raise RuntimeError("flake source is absent from builder input closure")
        nix_path = store_root(args.nix_bin)
        if nix_path not in paths or not args.nix_bin.as_posix().endswith("/bin/nix"):
            raise RuntimeError("guest Nix is absent from builder input closure")
        registration = (
            physical(args.source_store_root, str(args.closure)) / "registration"
        ).read_text()
        report.update(
            {"revision": revision, "lock_hash": lock_hash, "baseline_paths": len(paths)}
        )
        with tempfile.TemporaryDirectory(
            prefix="offline-builder-gate-", dir=args.temporary_root
        ) as directory:
            temporary = Path(directory)
            for name, change in SCENARIOS.items():
                report["scenarios"][name] = run_trial(
                    args,
                    name,
                    change,
                    paths,
                    registration,
                    inputs,
                    args.log_dir,
                    temporary,
                )
        report["status"] = "passed"
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
        report["status"] = "failed"
        report["error"] = str(exc)
    (args.log_dir / "report.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n"
    )
    print(json.dumps(report, sort_keys=True))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
