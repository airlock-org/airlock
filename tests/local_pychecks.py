from __future__ import annotations

import json
import os
import platform
import shutil
import subprocess
from pathlib import Path


def _command_available(command: str) -> bool:
    return shutil.which(command) is not None


def _command_output(command: list[str]) -> str:
    result = subprocess.run(
        command,
        check=False,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    return result.stdout.strip()


def _run(
    command: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    stdout: Path | None = None,
    stderr: Path | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    stdout_handle = stdout.open("w", encoding="utf-8") if stdout is not None else None
    stderr_handle = stderr.open("w", encoding="utf-8") if stderr is not None else None
    try:
        result = subprocess.run(
            command,
            cwd=cwd,
            env=env,
            check=False,
            encoding="utf-8",
            errors="replace",
            stdout=stdout_handle if stdout_handle is not None else subprocess.PIPE,
            stderr=stderr_handle if stderr_handle is not None else subprocess.PIPE,
        )
    finally:
        if stdout_handle is not None:
            stdout_handle.close()
        if stderr_handle is not None:
            stderr_handle.close()
    if check and result.returncode != 0:
        raise AssertionError(
            f"command failed with exit {result.returncode}: {' '.join(command)}\n"
            f"stdout: {result.stdout}\nstderr: {result.stderr}"
        )
    return result


def _run_store_path(repo_root: Path, store_path: str, env: dict[str, str]) -> None:
    _run(
        [
            "bash",
            "-c",
            'source "$1"; nixome_run_store_path "$2"',
            "bash",
            str(repo_root / "scripts/lib/nixome-env.sh"),
            store_path,
        ],
        cwd=repo_root,
        env=env,
    )


def _file_contains_word(path: Path, word: str) -> bool:
    if not path.is_file():
        return False
    return word in path.read_text(encoding="utf-8", errors="replace").split()


def _first_cpu_flags() -> set[str]:
    cpuinfo = Path("/proc/cpuinfo")
    if not cpuinfo.is_file():
        return set()
    for line in cpuinfo.read_text(encoding="utf-8", errors="replace").splitlines():
        if line.startswith(("flags", "Features")) and ":" in line:
            return set(line.split(":", 1)[1].split())
    return set()


def _first_readable(paths: list[Path]) -> str:
    for path in paths:
        if path.is_file():
            return path.read_text(encoding="utf-8", errors="replace").strip()
    return ""


def check_host_capabilities(repo_root: Path, log_dir: Path) -> None:
    del repo_root
    output = log_dir / "host-capabilities.json"
    output.parent.mkdir(parents=True, exist_ok=True)

    xen_capabilities = Path("/proc/xen/capabilities")
    cpu_flags = _first_cpu_flags()
    report = {
        "schema": "nixome.host-capabilities.v1",
        "host": {
            "kernel": platform.release(),
            "architecture": platform.machine(),
            "virtualization": _command_output(["systemd-detect-virt"]),
        },
        "nix": {
            "available": _command_available("nix"),
            "version": _command_output(["nix", "--version"]),
        },
        "xen": {
            "proc_xen": Path("/proc/xen").exists(),
            "control_domain": _file_contains_word(xen_capabilities, "control_d"),
            "capabilities": (
                xen_capabilities.read_text(encoding="utf-8", errors="replace").strip()
                if xen_capabilities.is_file()
                else ""
            ),
        },
        "hardware_virtualization": {
            "vmx": "vmx" in cpu_flags,
            "svm": "svm" in cpu_flags,
            "ept": "ept" in cpu_flags,
            "npt": "npt" in cpu_flags,
            "iommu_groups": any(Path("/sys/kernel/iommu_groups").glob("*")),
            "kvm_nested": _first_readable(
                [
                    Path("/sys/module/kvm_intel/parameters/nested"),
                    Path("/sys/module/kvm_amd/parameters/nested"),
                ]
            ),
        },
        "qubes_tools": {
            "qvm_run": _command_available("qvm-run"),
            "qvm_create": _command_available("qvm-create"),
            "qubes_prefs": _command_available("qubes-prefs"),
        },
    }
    output.write_text(json.dumps(report, sort_keys=True) + "\n", encoding="utf-8")
    assert report["schema"] == "nixome.host-capabilities.v1"


def check_qrexec_policy_path(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    static_dir = work_dir / "static-policy.d"
    runtime_dir = work_dir / "runtime-policy.d"
    script_path_file = work_dir / "helper-path"

    shutil.rmtree(work_dir, ignore_errors=True)
    (static_dir / "include").mkdir(parents=True)
    runtime_dir.mkdir(parents=True)

    (static_dir / "10-static.policy").write_text(
        "nixome.Static * dom0 config-vault allow\n", encoding="utf-8"
    )
    (static_dir / "include/example.policy").write_text(
        "nixome.Include * dom0 config-vault allow\n", encoding="utf-8"
    )
    (static_dir / "90-nixome.policy").write_text(
        "this static managed-name file must be ignored\n", encoding="utf-8"
    )
    (runtime_dir / "old.policy").write_text("stale\n", encoding="utf-8")

    _run(
        [
            "nix",
            "build",
            "--no-link",
            f'{repo_root}#nixosConfigurations.vm-gui-config.config.systemd.units."nixome-qubes-policy-path.service".unit',
        ],
        cwd=repo_root,
    )
    helper = _run(
        [
            "nix",
            "eval",
            "--raw",
            f"{repo_root}#nixosConfigurations.vm-gui-config.config.systemd.services.nixome-qubes-policy-path.serviceConfig.ExecStart",
        ],
        cwd=repo_root,
    ).stdout
    script_path_file.write_text(helper, encoding="utf-8")

    env = os.environ.copy()
    env["NIXOME_QUBES_STATIC_POLICY_DIR"] = str(static_dir)
    env["NIXOME_QUBES_POLICY_DIR"] = str(runtime_dir)
    _run_store_path(repo_root, helper.strip(), env)

    assert (runtime_dir / "10-static.policy").is_symlink()
    assert (runtime_dir / "10-static.policy").resolve() == (
        static_dir / "10-static.policy"
    ).resolve()
    assert (runtime_dir / "include").is_symlink()
    assert (runtime_dir / "include").resolve() == (static_dir / "include").resolve()
    assert not (runtime_dir / "old.policy").exists()
    assert (runtime_dir / "90-nixome.policy").read_text(encoding="utf-8") == (
        "# Managed by Nixome setup-config.\n"
        "# Source: nixome.qubes.topology.qrexec_policy.\n"
        "# Bootstrap config transport policy remains in 30-nixome-config.policy.\n"
    )

    (runtime_dir / "90-nixome.policy").write_text(
        "# Managed by Nixome setup-config.\n"
        "# Source: nixome.qubes.topology.qrexec_policy.\n"
        "# Bootstrap config transport policy remains in 30-nixome-config.policy.\n"
        "nixome.PolicyProbe * dom0 config-vault allow\n"
        "nixome.PolicyProbe * @anyvm @anyvm deny\n",
        encoding="utf-8",
    )
    (static_dir / "20-static.policy").write_text(
        "nixome.StaticTwo * dom0 config-vault allow\n", encoding="utf-8"
    )
    (runtime_dir / "stale.policy").write_text("stale\n", encoding="utf-8")
    _run_store_path(repo_root, helper.strip(), env)

    assert (
        "nixome.PolicyProbe * dom0 config-vault allow"
        in (runtime_dir / "90-nixome.policy").read_text(encoding="utf-8").splitlines()
    )
    assert (runtime_dir / "20-static.policy").is_symlink()
    assert not (runtime_dir / "stale.policy").exists()

    unmanaged_runtime_dir = work_dir / "unmanaged-runtime-policy.d"
    unmanaged_runtime_dir.mkdir()
    (unmanaged_runtime_dir / "90-nixome.policy").write_text(
        "unmanaged\n", encoding="utf-8"
    )
    env["NIXOME_QUBES_POLICY_DIR"] = str(unmanaged_runtime_dir)
    result = _run(
        [
            "bash",
            "-c",
            'source "$1"; nixome_run_store_path "$2"',
            "bash",
            str(repo_root / "scripts/lib/nixome-env.sh"),
            helper.strip(),
        ],
        cwd=repo_root,
        env=env,
        stdout=work_dir / "unmanaged-stdout.log",
        stderr=work_dir / "unmanaged-stderr.log",
        check=False,
    )
    assert result.returncode != 0
    assert "refusing unmanaged Nixome qrexec policy file" in (
        work_dir / "unmanaged-stderr.log"
    ).read_text(encoding="utf-8")
