#!/usr/bin/env python3
"""Cached offline acceptance trials using only the image's pinned inputs."""

import argparse
import fcntl
from io import BytesIO
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import time
import uuid


STORE_PATH = re.compile(r"/nix/store/[0-9abcdfghijklmnpqrsvwxyz]{32}-[^/]+")
SOURCE_PATH = re.compile(
    r"/nix/store/[0-9abcdfghijklmnpqrsvwxyz]{32}-[^/]+(?:/[^/.][^/]*)*"
)
INSTALLABLE = "nixosConfigurations.airlock-domain-usr-home.config.system.build.toplevel"
GUEST_MODULE = Path("lifecycle/misc/modules/usr-home-guest.nix")
SCENARIOS = {
    "etc-marker": '  environment.etc."airlock-offline-gate".text = "offline gate marker";',
    "service-setting": '  systemd.services.qubes-qrexec-agent.serviceConfig.TimeoutStartSec = "90s";',
}
RECEIPT_SCHEMA = "airlock.offline-builder-receipt.v1"
REPORT_SCHEMA = "airlock.offline-builder-gate.v2"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def atomic_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")
        temporary = Path(stream.name)
    temporary.replace(path)


def input_identity(args, revision, lock_hash, paths, registration, manifest):
    """Hash exact prepared bytes and registered Nix closure metadata."""
    closure_dir = physical(args.source_store_root, str(args.closure))
    bwrap = shutil.which("bwrap")
    if not bwrap:
        raise RuntimeError("bubblewrap is unavailable")
    sources = {
        "seed_archive": digest(args.seed_archive.read_bytes()),
        "manifest": digest(args.manifest.read_bytes()),
        "closure_paths": digest((closure_dir / "store-paths").read_bytes()),
        "registration": digest(registration.encode()),
        "runner": digest(Path(__file__).read_bytes()),
        "nix_binary": digest(physical_store_file(args.source_store_root, str(args.nix_bin)).read_bytes()),
        "bwrap_binary": digest(Path(bwrap).read_bytes()),
        "image_flake": digest(Path("flake.nix").read_bytes()),
        "prepared_store": digest(Path("lifecycle/image/prepared-guest-store.nix").read_bytes()),
    }
    identity = {
        "schema": RECEIPT_SCHEMA,
        "revision": revision,
        "lock_hash": lock_hash,
        "closure": str(args.closure),
        "nix_bin": str(args.nix_bin),
        "host_nix_version": checked(["nix", "--version"]),
        "target_system": "x86_64-linux",
        "trial_timeout_seconds": args.timeout,
        "source_store_root": str(args.source_store_root.resolve()),
        "isolation": "bwrap-unshare-net-empty-store-no-substitutes-sandbox-v1",
        "paths": len(paths),
        "hashes": sources,
    }
    return identity, digest(json.dumps(identity, sort_keys=True).encode())


def valid_receipt(path, fingerprint, identity=None):
    try:
        receipt = json.loads(path.read_text())
    except (OSError, ValueError):
        return None
    if not isinstance(receipt, dict) or receipt.get("schema") != RECEIPT_SCHEMA:
        return None
    if receipt.get("fingerprint") != fingerprint or receipt.get("status") != "passed":
        return None
    if identity is not None and receipt.get("identity") != identity:
        return None
    scenarios = receipt.get("scenarios")
    if not isinstance(scenarios, dict) or set(scenarios) != set(SCENARIOS):
        return None
    if any(
        not isinstance(result, dict)
        or not isinstance(result.get("system"), str)
        or not STORE_PATH.fullmatch(result["system"])
        or not isinstance(result.get("elapsed_seconds"), (int, float))
        for result in scenarios.values()
    ):
        return None
    evidence = receipt.get("evidence")
    if not isinstance(evidence, dict) or set(evidence) != set(SCENARIOS):
        return None
    if any(
        not isinstance(files, dict)
        or set(files) != {"stdout", "stderr"}
        or any(not isinstance(item, str) or not Path(item).is_file() for item in files.values())
        for files in evidence.values()
    ):
        return None
    return receipt


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
    return source_root / str(logical).lstrip("/")


def physical_store_file(source_root, logical):
    """Follow absolute Nix store links inside the selected physical store."""
    path = physical(source_root, logical)
    for _ in range(16):
        if not path.is_symlink():
            return path
        target = path.readlink()
        if target.is_absolute():
            if not str(target).startswith("/nix/store/"):
                raise RuntimeError(f"Nix tool links outside its store: {logical}")
            path = physical(source_root, target)
        else:
            path = path.parent / target
    raise RuntimeError(f"Nix tool has too many symbolic links: {logical}")


def prepare_store(store, source_root, paths, registration, flake_sources):
    target = store / "nix/store"
    target.mkdir(parents=True)
    for logical in paths:
        original = physical(source_root, logical)
        if not original.exists() and not original.is_symlink():
            raise RuntimeError(
                f"pinned input is absent from the build store: {logical}"
            )
        staged = target / Path(logical).name
        if logical in flake_sources and not original.is_dir():
            raise RuntimeError(f"flake input is not a directory: {logical}")
        # These are mount points, not links to the host's physical store.
        # Preserve genuine store-root symlinks with their original targets.
        if original.is_symlink():
            staged.symlink_to(original.readlink())
        elif original.is_dir():
            staged.mkdir()
        elif original.is_file():
            staged.touch()
        else:
            raise RuntimeError(f"unsupported pinned input type: {logical}")
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
    baseline = module.with_name("usr-home-guest-offline-baseline.nix")
    if baseline.exists():
        raise RuntimeError(f"offline trial baseline already exists: {baseline}")
    # Preserve the committed module and its relative imports unchanged. A
    # separate importing module adds the trial option without depending on the
    # formatting or application list in the original source.
    module.rename(baseline)
    module.write_text(
        "{\n"
        f"  imports = [ ./{baseline.name} ];\n"
        f"{change}\n"
        "}\n"
    )


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
    input_mounts = []
    for path in sorted(paths):
        original = physical(args.source_store_root, path)
        if original.is_symlink():
            continue
        source = str(original)
        # /nix is the trial's entire store and database inside bubblewrap.
        # Each selected object needs only one read-only mount under it.
        input_mounts.extend(("--ro-bind", source, path))
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
        str(store / "nix"),
        "/nix",
        *input_mounts,
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
        "store = /\nsubstituters = \n",
        "--",
        str(args.nix_bin),
        "--extra-experimental-features",
        "nix-command flakes",
        "--store",
        "/",
        "--option",
        "build-hook",
        "",
        "--option",
        "sandbox",
        "true",
        "--option",
        "sandbox-fallback",
        "false",
        "--option",
        "extra-sandbox-paths",
        "",
        "build",
        "--offline",
        "--no-write-lock-file",
        *overrides,
        f"path:{checkout}#{INSTALLABLE}",
        "--no-link",
        "--json",
    ]
    if len(command) - 1 >= 9000:
        raise RuntimeError(
            f"{name} needs too many bubblewrap arguments for "
            f"{len(paths)} pinned store paths"
        )
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
        detail = "\n".join(result.stderr.rstrip().splitlines()[-12:])
        raise RuntimeError(
            f"{name} offline build failed; see {log_dir / f'{name}.stderr'}\n{detail}"
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
        original = physical(args.source_store_root, logical)
        if original.is_symlink():
            unchanged = staged.is_symlink() and staged.readlink() == original.readlink()
        elif original.is_dir():
            unchanged = (
                not staged.is_symlink()
                and staged.is_dir()
                and not any(staged.iterdir())
            )
        else:
            unchanged = (
                not staged.is_symlink()
                and staged.is_file()
                and staged.stat().st_size == 0
            )
        if not unchanged:
            raise RuntimeError(f"{name} replaced a pinned input: {logical}")
    return {"system": system, "elapsed_seconds": round(time.monotonic() - started, 2)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--closure", required=True, type=Path)
    parser.add_argument("--machine", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--seed-archive", required=True, type=Path)
    parser.add_argument("--source-store-root", required=True, type=Path)
    parser.add_argument("--nix-bin", required=True, type=Path)
    parser.add_argument("--log-dir", required=True, type=Path)
    parser.add_argument("--cache-dir", required=True, type=Path)
    parser.add_argument("--temporary-root", required=True, type=Path)
    parser.add_argument("--timeout", type=int, default=1800)
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--image-command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    args.log_dir.mkdir(parents=True, exist_ok=True)
    args.cache_dir.mkdir(parents=True, exist_ok=True)
    args.temporary_root.mkdir(parents=True, exist_ok=True)
    report = {
        "schema": REPORT_SCHEMA,
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
        if manifest.get("schema") != "airlock.sys-agent-offline-inputs.v1":
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
        # A cached proof never replaces the input realization required by the image.
        checked(["nix-store", "--store", str(args.source_store_root), "--realise", *sorted(paths)])
        if any(not physical(args.source_store_root, path).exists() for path in paths):
            raise RuntimeError("builder input closure is incomplete after realization")
        if any(store_root(path) not in paths for path in inputs.values()):
            raise RuntimeError("flake source is absent from builder input closure")
        nix_path = store_root(args.nix_bin)
        if nix_path not in paths or not args.nix_bin.as_posix().endswith("/bin/nix"):
            raise RuntimeError("guest Nix is absent from builder input closure")
        registration = (
            physical(args.source_store_root, str(args.closure)) / "registration"
        ).read_text()
        identity, fingerprint = input_identity(
            args, revision, lock_hash, paths, registration, manifest
        )
        report["fingerprint"] = fingerprint
        report.update(
            {"revision": revision, "lock_hash": lock_hash, "baseline_paths": len(paths)}
        )
        receipt_path = args.cache_dir / f"{fingerprint}.json"
        lock_path = args.cache_dir / f"{fingerprint}.lock"
        with lock_path.open("w") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            cached = valid_receipt(receipt_path, fingerprint, identity)
            if cached and not args.force:
                report["cache"] = "hit"
                report["scenarios"] = cached["scenarios"]
                report["evidence"] = cached["evidence"]
                print(f"offline builder acceptance: cache hit {fingerprint}", flush=True)
            else:
                report["cache"] = "forced" if args.force else "miss"
                print(f"offline builder acceptance: {report['cache']} {fingerprint}", flush=True)
                # Invalidate before a forced run so interruption cannot leave old proof.
                receipt_path.unlink(missing_ok=True)
                run_dir = args.log_dir / fingerprint / uuid.uuid4().hex
                run_dir.mkdir(parents=True)
                with tempfile.TemporaryDirectory(
                    prefix="offline-builder-gate-", dir=args.temporary_root
                ) as directory:
                    temporary = Path(directory)
                    for name, change in SCENARIOS.items():
                        report["scenarios"][name] = run_trial(
                            args, name, change, paths, registration, inputs, run_dir, temporary
                        )
                report["evidence"] = {
                    name: {
                        suffix: str((run_dir / f"{name}.{suffix}").resolve())
                        for suffix in ("stdout", "stderr")
                    }
                    for name in SCENARIOS
                }
                # Check mutable preparation files and source once more before proof.
                if checked(["git", "-C", str(args.machine), "status", "--porcelain"]):
                    raise RuntimeError("machine source changed during offline trials")
                if input_identity(args, revision, lock_hash, paths, registration, manifest)[1] != fingerprint:
                    raise RuntimeError("builder inputs changed during offline trials")
                atomic_json(receipt_path, {
                    "schema": RECEIPT_SCHEMA,
                    "status": "passed",
                    "fingerprint": fingerprint,
                    "identity": identity,
                    "scenarios": report["scenarios"],
                    "evidence": report["evidence"],
                })
            report["status"] = "passed"
            report["receipt"] = str(receipt_path)
            if args.image_command:
                if args.image_command[0] == "--":
                    args.image_command.pop(0)
                if not args.image_command:
                    raise RuntimeError("empty image command")
                # The image consumes a private copy of the exact prepared bytes.
                snapshot = args.cache_dir / "snapshots" / fingerprint
                snapshot.mkdir(parents=True, exist_ok=True)
                for name, source in (("seed.tar.gz", args.seed_archive), ("inputs.json", args.manifest)):
                    target = snapshot / name
                    if not target.exists() or digest(target.read_bytes()) != digest(source.read_bytes()):
                        with tempfile.NamedTemporaryFile(dir=snapshot, delete=False) as stream:
                            stream.write(source.read_bytes())
                            temporary = Path(stream.name)
                        temporary.replace(target)
                environment = os.environ.copy()
                environment["AIRLOCK_MACHINE_SEED_ARCHIVE"] = str(snapshot / "seed.tar.gz")
                environment["AIRLOCK_MACHINE_OFFLINE_INPUTS"] = str(snapshot / "inputs.json")
                expected_snapshot = {
                    "seed.tar.gz": identity["hashes"]["seed_archive"],
                    "inputs.json": identity["hashes"]["manifest"],
                }

                def snapshot_matches():
                    return all(
                        digest((snapshot / name).read_bytes()) == expected
                        for name, expected in expected_snapshot.items()
                    )

                if input_identity(args, revision, lock_hash, paths, registration, manifest)[1] != fingerprint:
                    raise RuntimeError("builder inputs changed before image assembly")
                if not snapshot_matches():
                    raise RuntimeError("image input snapshot changed before assembly")
                print("offline builder acceptance: assembling image", flush=True)
                command = list(args.image_command)
                try:
                    link_index = command.index("--out-link") + 1
                    final_link = Path(command[link_index])
                except (ValueError, IndexError) as exc:
                    raise RuntimeError("image command needs --out-link") from exc
                temporary_link = final_link.with_name(final_link.name + f".checked-{uuid.uuid4().hex}")
                command[link_index] = str(temporary_link)
                result = subprocess.run(command, env=environment)
                if result.returncode:
                    temporary_link.unlink(missing_ok=True)
                    raise RuntimeError(f"image assembly failed with exit code {result.returncode}")
                if input_identity(args, revision, lock_hash, paths, registration, manifest)[1] != fingerprint:
                    temporary_link.unlink(missing_ok=True)
                    raise RuntimeError("builder inputs changed during image assembly")
                if not snapshot_matches():
                    temporary_link.unlink(missing_ok=True)
                    raise RuntimeError("image input snapshot changed during assembly")
                if checked(["git", "-C", str(args.machine), "status", "--porcelain"]):
                    temporary_link.unlink(missing_ok=True)
                    raise RuntimeError("machine source changed during image assembly")
                temporary_link.replace(final_link)
                report["image_status"] = "passed"
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
        report["status"] = "failed"
        report["error"] = str(exc)
        print(f"offline builder acceptance: failure: {exc}", flush=True)
    atomic_json(args.log_dir / "report.json", report)
    print(json.dumps(report, sort_keys=True))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
