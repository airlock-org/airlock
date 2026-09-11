from __future__ import annotations

import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import textwrap
import types
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
    stderr_handle = None
    if stderr is not None and stderr == stdout:
        stderr_handle = stdout_handle
    elif stderr is not None:
        stderr_handle = stderr.open("w", encoding="utf-8")
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
        if stderr_handle is not None and stderr_handle is not stdout_handle:
            stderr_handle.close()
    if check and result.returncode != 0:
        raise AssertionError(
            f"command failed with exit {result.returncode}: {' '.join(command)}\n"
            f"stdout: {result.stdout}\nstderr: {result.stderr}"
        )
    return result


def _run_store_path(
    repo_root: Path,
    store_path: str,
    env: dict[str, str],
    args: list[str] | None = None,
) -> None:
    _run(
        [
            "bash",
            "-c",
            'source "$1"; nixome_run_store_path "$2" "${@:3}"',
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/lib/nixome-env.sh"),
            store_path,
            *(args or []),
        ],
        cwd=repo_root,
        env=env,
    )


def _physical_store_path(repo_root: Path, path: str) -> str:
    return _run(
        [
            "bash",
            "-c",
            'source "$1"; nixome_physical_store_path "$2"',
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/lib/nixome-env.sh"),
            path,
        ],
        cwd=repo_root,
    ).stdout.strip()


def _physical_path_list(repo_root: Path, path_list: str) -> str:
    return _run(
        [
            "bash",
            "-c",
            'source "$1"; nixome_physical_path_list "$2"',
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/lib/nixome-env.sh"),
            path_list,
        ],
        cwd=repo_root,
    ).stdout.strip()


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
        "nixome.Static * dom0 sys-config-vault allow\n", encoding="utf-8"
    )
    (static_dir / "include/example.policy").write_text(
        "nixome.Include * dom0 sys-config-vault allow\n", encoding="utf-8"
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
        "nixome.PolicyProbe * dom0 sys-config-vault allow\n"
        "nixome.PolicyProbe * @anyvm @anyvm deny\n",
        encoding="utf-8",
    )
    (static_dir / "20-static.policy").write_text(
        "nixome.StaticTwo * dom0 sys-config-vault allow\n", encoding="utf-8"
    )
    (runtime_dir / "stale.policy").write_text("stale\n", encoding="utf-8")
    _run_store_path(repo_root, helper.strip(), env)

    assert (
        "nixome.PolicyProbe * dom0 sys-config-vault allow"
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
            str(repo_root / "lifecycle/misc/tools/scripts/lib/nixome-env.sh"),
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


def check_qube_guest_policy_probe(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    service_path_file = work_dir / "policy-probe-path"
    fake_qdb = work_dir / "qubesdb-read"

    shutil.rmtree(work_dir, ignore_errors=True)
    work_dir.mkdir(parents=True)

    attr = (
        f"{repo_root}#nixosConfigurations.nixome-domain-sys-config-vault.config."
        'environment.etc."qubes-rpc/nixome.PolicyProbe".source'
    )
    _run(["nix", "build", "--no-link", attr], cwd=repo_root)
    service = _run(["nix", "eval", "--raw", attr], cwd=repo_root).stdout.strip()
    service_path_file.write_text(service + "\n", encoding="utf-8")

    fake_qdb.write_text(
        "#!/usr/bin/env bash\n"
        'case "$1" in\n'
        "  /features/nixome-role)\n"
        "    exit 1\n"
        "    ;;\n"
        "  /name)\n"
        "    printf 'sys-config-vault\\n'\n"
        "    ;;\n"
        "esac\n",
        encoding="utf-8",
    )
    fake_qdb.chmod(0o755)

    env = os.environ.copy()
    env["NIXOME_QUBESDB_READ"] = str(fake_qdb)
    first = _run(
        [
            "bash",
            "-c",
            'source "$1"; nixome_run_store_path "$2"',
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/lib/nixome-env.sh"),
            service,
        ],
        cwd=repo_root,
        env=env,
    ).stdout
    (work_dir / "from-name.log").write_text(first, encoding="utf-8")
    assert re.search(r"^nixome-policy-probe role=sys-config-vault user=.+$", first)

    fake_qdb.write_text(
        "#!/usr/bin/env bash\n"
        'case "$1" in\n'
        "  /features/nixome-role)\n"
        "    printf 'sys-config-validator\\n'\n"
        "    ;;\n"
        "  /name)\n"
        "    printf 'other-name\\n'\n"
        "    ;;\n"
        "esac\n",
        encoding="utf-8",
    )
    second = _run(
        [
            "bash",
            "-c",
            'source "$1"; nixome_run_store_path "$2"',
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/lib/nixome-env.sh"),
            service,
        ],
        cwd=repo_root,
        env=env,
    ).stdout
    (work_dir / "from-feature.log").write_text(second, encoding="utf-8")
    assert re.search(r"^nixome-policy-probe role=sys-config-validator user=.+$", second)


def check_nix_closure_domain_volume_list(repo_root: Path, log_dir: Path) -> None:
    script = repo_root / "lifecycle/misc/tools/scripts/nix-closure-domain-smoke.sh"
    work_dir = log_dir
    work_dir.mkdir(parents=True, exist_ok=True)
    volume_list = work_dir / "qvm-volume-list.txt"
    volume_list.write_text(
        "POOL:VOLUME                                       VMNAME                "
        "VOLUME_NAME  REVERT_POSSIBLE\n"
        "linux-kernel:6.12.64-1.fc41                       nixome-closure-smoke  "
        "kernel       No\n"
        "varlibqubes:appvms/nixome-closure-smoke/private   "
        "nixome-closure-smoke  private      No\n"
        "varlibqubes:appvms/nixome-closure-smoke/root      "
        "nixome-closure-smoke  root         No\n"
        "varlibqubes:appvms/nixome-closure-smoke/store     "
        "nixome-closure-smoke  store        No\n"
        "varlibqubes:appvms/nixome-closure-smoke/store-data "
        "nixome-closure-smoke store-data   No\n"
        "varlibqubes:appvms/nixome-closure-smoke/volatile  "
        "nixome-closure-smoke  volatile     No\n",
        encoding="utf-8",
    )

    rows = volume_list.read_text(encoding="utf-8").splitlines()[1:]
    assert any(
        len(parts := row.split()) >= 3
        and (parts[2] == "store" or parts[0].endswith("/store"))
        for row in rows
    )

    script_text = script.read_text(encoding="utf-8")
    assert 'grep -F "$DOMAIN:store"' not in script_text
    assert 'qvm_volume_list_has_volume "$DOMAIN" store' in script_text
    assert 'qvm_volume_list_has_volume "$DOMAIN" store-data' in script_text
    assert 'touch "$home_dir/nixome-closure-smoke-home"' in script_text
    assert "touch /rw/nixome-closure-smoke-persistent" not in script_text

    assert re.search(
        r"^store_options=([^,\n]+,)*ro(,|$)",
        "store_options=ro,nodev,relatime\n",
        re.MULTILINE,
    )
    assert not re.search(
        r"^store_options=([^,\n]+,)*ro(,|$)",
        "store_options=nodev,rw,relatime\n",
        re.MULTILINE,
    )


def check_nix_closure_root_image(repo_root: Path, log_dir: Path) -> None:
    result_dir = log_dir
    shutil.rmtree(result_dir, ignore_errors=True)
    result_dir.mkdir(parents=True)

    _run(
        [
            "nix",
            "build",
            "--out-link",
            str(result_dir / "root-image"),
            f"{repo_root}#nixome-nix-closure-root-image-test",
        ],
        cwd=repo_root,
        stdout=result_dir / "nix-build.stdout",
        stderr=result_dir / "nix-build.stderr",
    )
    root_image_out = Path(
        _physical_store_path(repo_root, os.readlink(result_dir / "root-image"))
    )
    manifest = json.loads(
        (root_image_out / "manifest.json").read_text(encoding="utf-8")
    )
    assert manifest["schema"] == "nixome.nix-closure-root-image.v1"
    assert manifest["contains_store"] is False
    assert manifest["store_mountpoint"] == "/nix/store"
    assert manifest["store_device_default"] == "/dev/xvde"

    root_files = (
        (root_image_out / "root-files.txt").read_text(encoding="utf-8").splitlines()
    )
    assert "./sbin/init" in root_files
    assert "./nix/store" in root_files
    assert not any(re.search(r"^\./nix/store/.+", item) for item in root_files)

    _run(
        ["debugfs", "-R", "ls -p /nix/store", str(root_image_out / "root.img")],
        stdout=result_dir / "debugfs-nix-store.txt",
        stderr=result_dir / "debugfs-nix-store.stderr",
    )
    debugfs_output = (result_dir / "debugfs-nix-store.txt").read_text(encoding="utf-8")
    assert not re.search(r"/[A-Za-z0-9]{32}-", debugfs_output)


def check_setup_config_runtime_path(repo_root: Path, log_dir: Path) -> None:
    log_dir.mkdir(parents=True, exist_ok=True)
    out = _run(
        [
            "nix",
            "build",
            f"{repo_root}#packages.x86_64-linux.nixome-setup-config",
            "--no-link",
            "--print-out-paths",
        ],
        cwd=repo_root,
    ).stdout.strip()
    wrapper = Path(_physical_store_path(repo_root, out)) / "bin/nixome-setup-config"
    wrapper_text = wrapper.read_text(encoding="utf-8")
    match = re.search(r'^export PATH="(.*):\$PATH"$', wrapper_text, re.MULTILINE)
    assert match is not None, f"could not extract runtime PATH from {wrapper}"
    qubesadmin_python_match = re.search(
        r"^export NIXOME_QUBESADMIN_PYTHON=\$\{NIXOME_QUBESADMIN_PYTHON:-([^}]+)\}$",
        wrapper_text,
        re.MULTILINE,
    )
    assert (
        qubesadmin_python_match is not None
    ), f"could not extract NIXOME_QUBESADMIN_PYTHON from {wrapper}"

    path_value = _physical_path_list(repo_root, match.group(1))
    qubesadmin_python = qubesadmin_python_match.group(1)
    required = [
        "awk",
        "chmod",
        "cmp",
        "cp",
        "dd",
        "grep",
        "ip",
        "jq",
        "mkdir",
        "mkfs.erofs",
        "mktemp",
        "dmsetup",
        "losetup",
        "qubes-prefs",
        "qubesd-query",
        "python3",
        "qvm-create",
        "qvm-features",
        "qvm-ls",
        "qvm-prefs",
        "qvm-service",
        "qvm-template-postprocess",
        "qvm-volume",
        "rm",
        "runuser",
        "sudo",
        "systemctl",
    ]
    resolved = {command: shutil.which(command, path=path_value) for command in required}
    (log_dir / "setup-config-runtime-path.commands").write_text(
        "".join(
            f"{command}: {path or '<missing>'}\n" for command, path in resolved.items()
        ),
        encoding="utf-8",
    )
    missing = [command for command, path in resolved.items() if path is None]
    assert not missing, "setup-config runtime PATH is missing helper(s): " + ", ".join(
        missing
    )
    _run_store_path(
        repo_root,
        qubesadmin_python,
        os.environ.copy(),
        ["-c", "import qubesadmin"],
    )


def _write_flake(path: Path, text: str) -> None:
    path.write_text(textwrap.dedent(text).lstrip(), encoding="utf-8")


def _write_executable(path: Path, text: str) -> None:
    path.write_text(textwrap.dedent(text).lstrip(), encoding="utf-8")
    path.chmod(0o755)


def _setup_fake_command_dir(bin_dir: Path, commands: dict[str, str]) -> None:
    bin_dir.mkdir(parents=True, exist_ok=True)
    for name, script in commands.items():
        _write_executable(bin_dir / name, script)


def check_start_network_providers(repo_root: Path, log_dir: Path) -> None:
    shutil.rmtree(log_dir, ignore_errors=True)
    log_dir.mkdir(parents=True)
    topology = log_dir / "topology.json"
    qvm_start = log_dir / "qvm-start"
    qvm_start_log = log_dir / "qvm-start.log"
    qvm_start_log.write_text("", encoding="utf-8")

    out = _run(
        [
            "nix",
            "build",
            f"{repo_root}#packages.x86_64-linux.nixome-start-network-providers",
            "--no-link",
            "--print-out-paths",
        ],
        cwd=repo_root,
    ).stdout.strip()
    wrapper = (
        Path(_physical_store_path(repo_root, out))
        / "bin/nixome-start-network-providers"
    )

    topology.write_text(
        json.dumps(
            {
                "schema": "nixome.qubes-topology.v1",
                "domains": {
                    "sys-net": {
                        "netvm": None,
                        "prefs": {
                            "autostart": True,
                            "provides_network": True,
                        },
                    },
                    "sys-firewall": {
                        "netvm": "sys-net",
                        "prefs": {
                            "autostart": True,
                            "provides_network": True,
                        },
                    },
                    "sys-vpn": {
                        "netvm": None,
                        "prefs": {
                            "autostart": False,
                            "provides_network": True,
                        },
                    },
                    "dev": {
                        "netvm": "sys-net",
                        "prefs": {
                            "autostart": True,
                            "provides_network": False,
                        },
                    },
                    "sys-none": {
                        "netvm": "none",
                        "prefs": {
                            "autostart": True,
                            "provides_network": True,
                        },
                    },
                },
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    _write_executable(
        qvm_start,
        """
        #!/usr/bin/env bash
        printf '%s\n' "$*" >> "$QVM_START_LOG"
        """,
    )
    env = os.environ.copy()
    env.update(
        {
            "NIXOME_QUBES_TOPOLOGY": str(topology),
            "NIXOME_QVM_START": str(qvm_start),
            "NIXOME_LOG_DIR": str(log_dir),
            "QVM_START_LOG": str(qvm_start_log),
        }
    )
    _run(
        [str(wrapper)],
        cwd=repo_root,
        env=env,
        stdout=log_dir / "stdout.log",
        stderr=log_dir / "stderr.log",
    )

    result = json.loads(
        (log_dir / "start-network-providers.json").read_text(encoding="utf-8")
    )
    assert result["schema"] == "nixome.start-network-providers.v1"
    assert result["status"] == "passed"
    assert result["providers"] == ["sys-net", "sys-firewall", "sys-none"]
    assert result["started_or_running"] == ["sys-net", "sys-firewall", "sys-none"]
    assert result["failed"] == []
    assert qvm_start_log.read_text(encoding="utf-8").splitlines() == [
        "--skip-if-running -- sys-net",
        "--skip-if-running -- sys-firewall",
        "--skip-if-running -- sys-none",
    ]
    command_log = (log_dir / "start-network-providers.commands.log").read_text(
        encoding="utf-8"
    )
    command_lines = [
        line for line in command_log.splitlines() if line.startswith("+ qvm-start ")
    ]
    assert command_lines == [
        "+ qvm-start --skip-if-running -- sys-net",
        "+ qvm-start --skip-if-running -- sys-firewall",
        "+ qvm-start --skip-if-running -- sys-none",
    ]


def check_system_schema(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir / "work"
    shutil.rmtree(work_dir, ignore_errors=True)
    log_dir.mkdir(parents=True, exist_ok=True)
    for name in ("valid", "reboot", "boot"):
        target = work_dir / name / "nixome"
        target.mkdir(parents=True)
        shutil.copy(
            repo_root / "lifecycle/misc/lib/system.nix",
            target / "system-schema.nix",
        )

    _write_flake(
        work_dir / "valid/flake.nix",
        """
        {
          outputs =
            { self }:
            let
              systemSchema = import ./nixome/system-schema.nix;
            in
            {
              nixome.system = systemSchema.mkSystem {
                dom0 = {
                  profile = {
                    source = {
                      build = "packages.x86_64-linux.nixome-dom0-profile";
                    };
                    activationCommand = "bin/nixome-dom0-profile-activate";
                    message = "hello";
                  };
                  packages = [
                    { name = "nixome-dom0-profile"; }
                  ];
                  systemd.services."nixome-example" = {
                    action = "restart";
                  };
                };
              };
            };
        }
        """,
    )
    _write_flake(
        work_dir / "reboot/flake.nix",
        """
        {
          outputs =
            { self }:
            let
              systemSchema = import ./nixome/system-schema.nix;
            in
            {
              nixome.system = systemSchema.mkSystem {
                dom0 = {
                  nixos = {
                    system = {
                      source = {
                        build = "nixosConfigurations.dom0.config.system.build.toplevel";
                      };
                      switchAction = "switch";
                      reason = "new dom0 system generation";
                    };
                    kernel.enable = true;
                    xen.enable = true;
                    bootloader.enable = true;
                    qubes.enable = true;
                  };
                };
              };
            };
        }
        """,
    )
    _write_flake(
        work_dir / "boot/flake.nix",
        """
        {
          outputs =
            { self }:
            let
              systemSchema = import ./nixome/system-schema.nix;
            in
            {
              nixome.system = systemSchema.mkSystem {
                dom0 = {
                  nixos.system = {
                    source = {
                      build = "nixosConfigurations.dom0.config.system.build.toplevel";
                    };
                    switchAction = "boot";
                    reason = "install next dom0 boot generation";
                  };
                };
              };
            };
        }
        """,
    )

    outputs = {}
    for name in ("valid", "reboot", "boot"):
        out_path = log_dir / f"system-schema-{name}.json"
        _run(
            ["nix", "eval", "--json", f"{work_dir / name}#nixome.system"],
            stdout=out_path,
        )
        outputs[name] = json.loads(out_path.read_text(encoding="utf-8"))

    valid = outputs["valid"]
    assert valid["schema"] == "nixome.system.v1"
    assert (
        valid["dom0"]["profile"]["source_ref"]["attr"]
        == "packages.x86_64-linux.nixome-dom0-profile"
    )
    assert (
        valid["dom0"]["profile"]["activation_command"]
        == "bin/nixome-dom0-profile-activate"
    )
    assert valid["dom0"]["profile"]["message"] == "hello"
    assert valid["dom0"]["packages"][0]["name"] == "nixome-dom0-profile"
    assert valid["dom0"]["systemd"]["services"]["nixome-example"]["action"] == "restart"
    assert valid["plan"]["reboot_required"] is False
    assert valid["plan"]["reboot_actions"] == []
    assert any(
        action["type"] == "dom0-profile" and action["message"] == "hello"
        for action in valid["plan"]["live_actions"]
    )
    assert any(
        action["type"] == "systemd-service"
        and action["name"] == "nixome-example"
        and action["action"] == "restart"
        for action in valid["plan"]["live_actions"]
    )

    reboot = outputs["reboot"]
    assert reboot["schema"] == "nixome.system.v1"
    assert (
        reboot["dom0"]["nixos"]["system"]["source_ref"]["attr"]
        == "nixosConfigurations.dom0.config.system.build.toplevel"
    )
    assert reboot["dom0"]["nixos"]["system"]["switch_action"] == "switch"
    assert reboot["dom0"]["nixos"]["system"]["enable"] is True
    assert reboot["dom0"]["nixos"]["kernel"]["enable"] is True
    assert reboot["dom0"]["nixos"]["xen"]["enable"] is True
    assert reboot["dom0"]["nixos"]["bootloader"]["enable"] is True
    assert reboot["dom0"]["nixos"]["qubes"]["enable"] is True
    assert reboot["plan"]["reboot_required"] is True
    assert any(
        action["type"] == "dom0-nixos-system"
        and action["action"] == "switch"
        and action["reason"] == "new dom0 system generation"
        for action in reboot["plan"]["live_actions"]
    )
    reboot_reasons = {
        "dom0 kernel change requires reboot",
        "dom0 xen change requires reboot",
        "dom0 bootloader change requires reboot",
        "dom0 qubes change requires reboot",
    }
    assert len(reboot["plan"]["reboot_actions"]) == 4
    assert set(reboot["plan"]["reboot_reasons"]) == reboot_reasons
    assert reboot_reasons == {
        action["reason"] for action in reboot["plan"]["reboot_actions"]
    }

    boot = outputs["boot"]
    assert boot["schema"] == "nixome.system.v1"
    assert boot["dom0"]["nixos"]["system"]["switch_action"] == "boot"
    assert boot["plan"]["reboot_required"] is True
    assert any(
        action["type"] == "dom0-nixos-system"
        and action["action"] == "boot"
        and action["reason"] == "install next dom0 boot generation"
        for action in boot["plan"]["live_actions"]
    )
    assert len(boot["plan"]["reboot_actions"]) == 1
    assert any(
        action["type"] == "dom0-nixos-system"
        and action["switch_action"] == "boot"
        and action["reason"] == "install next dom0 boot generation"
        for action in boot["plan"]["reboot_actions"]
    )
    assert boot["plan"]["reboot_reasons"] == ["install next dom0 boot generation"]


def check_topology_schema(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir / "work"
    shutil.rmtree(log_dir, ignore_errors=True)
    valid_dir = work_dir / "valid"
    for path in (
        valid_dir / "nixome",
        valid_dir / "template-source",
        valid_dir / "kernel-source",
        valid_dir / "system-source",
    ):
        path.mkdir(parents=True)
    shutil.copy(
        repo_root / "lifecycle/misc/lib/topology.nix",
        valid_dir / "nixome/topology.nix",
    )

    _write_flake(
        valid_dir / "flake.nix",
        """
        {
          description = "Nixome topology schema valid fixture";

          outputs =
            { self }:
            let
              topology = import ./nixome/topology.nix;
            in
            {
              nixome.topology = topology.mkTopology {
                templates."nixome-template" = {
                  import = true;
                  source = ./template-source;
                };
                templates."nixome-built-template" = {
                  import = true;
                  source = {
                    build = "packages.x86_64-linux.nixome-built-template";
                    subpath = "nixome-built-template";
                  };
                };
                kernels."6.12.64-1.fc41".source = ./kernel-source;
                prefs.default_kernel = "";
                domains."test-qube" = {
                  template = "nixome-template";
                  label = "green";
                  prefs.memory = 1024;
                  features."nixome-role" = "test";
                  services."network-manager" = false;
                  devices.pci = [
                    {
                      id = "00_14.0";
                      options.no-strict-reset = true;
                    }
                  ];
                };
                domains."legacy-fedora" = {
                  backend = {
                    type = "qubes-template";
                    template = "nixome-template";
                  };
                  label = "yellow";
                  netvm = null;
                };
                domains."nix-work" = {
                  backend = {
                    type = "nix-closure";
                    system.source = ./system-source;
                    kernel.name = "6.12.64-1.fc41";
                    root.sourceRef.build = "packages.x86_64-linux.nix-work-root-image";
                    root.sourceRef.subpath = "root.img";
                    store.sourceRef.build = "packages.x86_64-linux.nix-work-store-closure";
                  };
                  label = "blue";
                  prefs.memory = 2048;
                };
                qrexecPolicy.rules = [
                  {
                    service = "nixome.PolicyProbe";
                    source = "dom0";
                    target = "test-qube";
                    action = "allow";
                  }
                ];
              };
            };
        }
        """,
    )

    topology_json = log_dir / "topology.json"
    _run(
        ["nix", "eval", "--json", f"{valid_dir}#nixome.topology"], stdout=topology_json
    )
    topology = json.loads(topology_json.read_text(encoding="utf-8"))

    assert topology["schema"] == "nixome.qubes-topology.v1"
    template = topology["templates"]["nixome-template"]
    assert template["import"] is True
    assert template["skip_start"] is True
    assert template["source"].endswith("/template-source")
    assert template["source_ref"] is None
    built_template = topology["templates"]["nixome-built-template"]
    assert built_template["import"] is True
    assert built_template["source"] is None
    assert built_template["source_ref"] == {
        "type": "flake-output",
        "attr": "packages.x86_64-linux.nixome-built-template",
        "subpath": "nixome-built-template",
    }
    assert topology["kernels"]["6.12.64-1.fc41"]["source"].endswith("/kernel-source")

    test_qube = topology["domains"]["test-qube"]
    assert test_qube["class"] == "AppVM"
    assert test_qube["template"] == "nixome-template"
    assert test_qube["backend"]["type"] == "qubes-template"
    assert test_qube["backend"]["template"] == "nixome-template"
    assert test_qube["netvm"] is None
    assert test_qube["prefs"]["autostart"] is False
    assert test_qube["prefs"]["include_in_backups"] is False
    assert test_qube["prefs"]["memory"] == 1024
    assert test_qube["services"]["network-manager"] is False
    assert test_qube["devices"]["pci"] == [
        {
            "backend": "dom0",
            "id": "00_14.0",
            "match": "port",
            "mode": "required",
            "options": {"no-strict-reset": "True"},
        }
    ]

    legacy = topology["domains"]["legacy-fedora"]
    assert legacy["class"] == "AppVM"
    assert legacy["template"] == "nixome-template"
    assert legacy["label"] == "yellow"
    assert legacy["backend"]["type"] == "qubes-template"
    assert legacy["backend"]["template"] == "nixome-template"

    nix_work = topology["domains"]["nix-work"]
    assert nix_work["class"] == "StandaloneVM"
    assert nix_work["template"] is None
    assert nix_work["label"] == "blue"
    assert nix_work["prefs"]["memory"] == 2048
    assert nix_work["backend"]["type"] == "nix-closure"
    assert nix_work["backend"]["system"]["source"].endswith("/system-source")
    assert nix_work["backend"]["system"]["source_ref"] is None
    assert nix_work["backend"]["kernel"]["name"] == "6.12.64-1.fc41"
    assert nix_work["backend"]["kernel"]["source"] is None
    assert nix_work["backend"]["kernel"]["source_ref"] is None
    assert nix_work["backend"]["initrd"]["name"] is None
    assert nix_work["backend"]["modules"]["name"] is None
    assert nix_work["backend"]["root"]["mode"] == "qubes-root-image"
    assert nix_work["backend"]["root"]["source"] is None
    assert nix_work["backend"]["root"]["source_ref"] == {
        "type": "flake-output",
        "attr": "packages.x86_64-linux.nix-work-root-image",
        "subpath": "root.img",
    }
    assert nix_work["backend"]["root"]["read_only"] is False
    assert nix_work["backend"]["store"]["mode"] == "shared-closure"
    assert nix_work["backend"]["store"]["source"] is None
    assert nix_work["backend"]["store"]["source_ref"] == {
        "type": "flake-output",
        "attr": "packages.x86_64-linux.nix-work-store-closure",
        "subpath": "",
    }
    assert nix_work["backend"]["store"]["mountpoint"] == "/nix/store"
    assert nix_work["backend"]["store"]["device"] == "/dev/xvde"
    assert nix_work["backend"]["store"]["read_only"] is True
    assert nix_work["backend"]["state"]["persistent"] == "nix-work-private"
    assert nix_work["backend"]["state"]["volatile"] is None
    assert topology["qrexec_policy"]["rules"] == [
        {
            "service": "nixome.PolicyProbe",
            "argument": "*",
            "source": "dom0",
            "target": "test-qube",
            "action": "allow",
        },
        {
            "service": "nixome.PolicyProbe",
            "argument": "*",
            "source": "@anyvm",
            "target": "@anyvm",
            "action": "deny",
        },
    ]

    def expect_failure(name: str, needle: str, config_expr: str) -> None:
        case_dir = work_dir / name
        (case_dir / "nixome").mkdir(parents=True)
        shutil.copy(
            repo_root / "lifecycle/misc/lib/topology.nix",
            case_dir / "nixome/topology.nix",
        )
        _write_flake(
            case_dir / "flake.nix",
            f"""
            {{
              description = "Nixome topology schema invalid fixture";

              outputs =
                {{ self }}:
                let
                  topology = import ./nixome/topology.nix;
                in
                {{
                  nixome.topology = topology.mkTopology {config_expr};
                }};
            }}
            """,
        )
        log = log_dir / f"{name}.log"
        result = _run(
            ["nix", "eval", "--json", f"{case_dir}#nixome.topology"],
            stdout=log_dir / f"{name}-topology.json",
            stderr=log,
            check=False,
        )
        assert result.returncode != 0, f"schema accepted invalid config: {name}"
        assert needle in log.read_text(
            encoding="utf-8"
        ), f"schema failure did not mention expected text for {name}: {needle}"

    invalid_cases = [
        (
            "missing-appvm-template",
            "nixome.topology.domains.bad.template: must be set for AppVM domains",
            '{ domains.bad = { label = "green"; }; }',
        ),
        (
            "invalid-label",
            "nixome.topology.domains.bad.label: expected one of",
            '{ domains.bad = { template = "nixome-template"; label = "clear"; }; }',
        ),
        (
            "unknown-option",
            "nixome.topology.domains.bad: unknown option(s): servies",
            '{ domains.bad = { template = "nixome-template"; servies = { }; }; }',
        ),
        (
            "invalid-pci-device-id",
            "invalid PCI device id",
            '{ domains.bad = { template = "nixome-template"; devices.pci = [ { id = "../bad"; } ]; }; }',
        ),
        (
            "invalid-pci-assignment-mode",
            "expected one of auto-attach, ask-to-attach, required",
            '{ domains.bad = { template = "nixome-template"; devices.pci = [ { id = "00_14.0"; mode = "manual"; } ]; }; }',
        ),
        (
            "template-import-without-source",
            "nixome.topology.templates.nixome-template.source: must be set when import is true",
            '{ templates."nixome-template".import = true; }',
        ),
        (
            "template-source-and-source-ref",
            "source and sourceRef cannot both be set",
            '{ templates."nixome-template" = { import = true; source = ./template-source; sourceRef.build = "packages.x86_64-linux.template"; }; }',
        ),
        (
            "template-invalid-build-ref",
            "invalid flake output reference",
            '{ templates."nixome-template" = { import = true; source.build = "../bad"; }; }',
        ),
        (
            "non-nixome-qrexec-service",
            "invalid qrexec service",
            '{ qrexecPolicy.rules = [ { service = "qubes.Filecopy"; source = "dom0"; target = "vault"; action = "allow"; } ]; }',
        ),
        (
            "reserved-qrexec-service",
            "reserved bootstrap qrexec service",
            '{ qrexecPolicy.rules = [ { service = "nixome.Config.ArchiveCandidate"; source = "dom0"; target = "sys-config-vault"; action = "allow"; } ]; }',
        ),
        (
            "qrexec-allow-any-source",
            "allow rules must use dom0 or a concrete source domain",
            '{ qrexecPolicy.rules = [ { service = "nixome.PolicyProbe"; source = "@anyvm"; target = "sys-config-vault"; action = "allow"; } ]; }',
        ),
        (
            "qrexec-target-dom0",
            "topology-managed qrexec policy cannot target dom0",
            '{ qrexecPolicy.rules = [ { service = "nixome.PolicyProbe"; source = "sys-config-vault"; target = "dom0"; action = "allow"; } ]; }',
        ),
        (
            "nix-closure-with-template",
            "template must not be set for nix-closure domains",
            '{ domains.bad = { template = "nixome-template"; backend.type = "nix-closure"; }; }',
        ),
        (
            "nix-closure-missing-system",
            "nixome.topology.domains.bad.backend.system.source: must be set for nix-closure domains",
            '{ domains.bad.backend = { type = "nix-closure"; kernel.name = "6.12.64-1.fc41"; root.sourceRef.build = "packages.x86_64-linux.bad-root"; store.sourceRef.build = "packages.x86_64-linux.bad-store"; }; }',
        ),
        (
            "nix-closure-writable-store",
            "nix-closure store views must be read-only",
            '{ domains.bad.backend = { type = "nix-closure"; system.sourceRef.build = "nixosConfigurations.bad.config.system.build.toplevel"; kernel.name = "6.12.64-1.fc41"; root.sourceRef.build = "packages.x86_64-linux.bad-root"; store = { sourceRef.build = "packages.x86_64-linux.bad-store"; readOnly = false; }; }; }',
        ),
        (
            "nix-closure-unsupported-store-device",
            "nixome.topology.domains.bad.backend.store.device: currently must be /dev/xvde",
            '{ domains.bad.backend = { type = "nix-closure"; system.sourceRef.build = "nixosConfigurations.bad.config.system.build.toplevel"; kernel.name = "6.12.64-1.fc41"; root.sourceRef.build = "packages.x86_64-linux.bad-root"; store = { sourceRef.build = "packages.x86_64-linux.bad-store"; device = "/dev/xvdf"; }; }; }',
        ),
        (
            "nix-closure-missing-root",
            "nixome.topology.domains.bad.backend.root.source: must be set for qubes-root-image materialization",
            '{ domains.bad.backend = { type = "nix-closure"; system.sourceRef.build = "nixosConfigurations.bad.config.system.build.toplevel"; kernel.name = "6.12.64-1.fc41"; store.sourceRef.build = "packages.x86_64-linux.bad-store"; }; }',
        ),
        (
            "nix-closure-readonly-root",
            "qubes-root-image materialization imports a writable Qubes root volume",
            '{ domains.bad.backend = { type = "nix-closure"; system.sourceRef.build = "nixosConfigurations.bad.config.system.build.toplevel"; kernel.name = "6.12.64-1.fc41"; root = { sourceRef.build = "packages.x86_64-linux.bad-root"; readOnly = true; }; store.sourceRef.build = "packages.x86_64-linux.bad-store"; }; }',
        ),
    ]
    for name, needle, config_expr in invalid_cases:
        expect_failure(name, needle, config_expr)


def check_config_boot_verify(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    result_log_dir = work_dir / "logs"
    status_json = work_dir / "status.json"
    failed_status_json = work_dir / "status-failed.json"
    result_json = result_log_dir / "config-boot-verify.json"

    shutil.rmtree(work_dir, ignore_errors=True)
    bin_dir.mkdir(parents=True)
    result_log_dir.mkdir()
    _write_executable(
        bin_dir / "nixome-config-status",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        cat "$NIXOME_FAKE_CONFIG_STATUS"
        """,
    )

    status_json.write_text(
        json.dumps(
            {
                "schema": "nixome.config-status.v1",
                "pending_reboot": {"required": False, "actions": [], "reasons": []},
                "system": {
                    "last_apply": {"host_system_source": "/nix/store/new-system"},
                    "host_system_profile_resolved_target": "/nix/store/new-system",
                    "run_current_system_resolved_target": "/nix/store/new-system",
                },
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "NIXOME_LOG_DIR": str(result_log_dir),
            "NIXOME_FAKE_CONFIG_STATUS": str(status_json),
            "NIXOME_EXPECTED_HOST_SYSTEM_SOURCE": "/nix/store/new-system",
        }
    )
    _run(
        [
            "bash",
            str(repo_root / "lifecycle/runtime/tools/scripts/config-boot-verify.sh"),
        ],
        cwd=repo_root,
        env=env,
        stdout=work_dir / "stdout.log",
    )
    passed = json.loads(result_json.read_text(encoding="utf-8"))
    assert passed["schema"] == "nixome.config-boot-verify.v1"
    assert passed["status"] == "passed"
    assert passed["expected_host_system_source"] == "/nix/store/new-system"
    assert passed["last_apply_host_system_source"] == "/nix/store/new-system"
    assert passed["host_system_profile_resolved_target"] == "/nix/store/new-system"
    assert passed["run_current_system_resolved_target"] == "/nix/store/new-system"
    assert passed["pending_reboot"]["required"] is False
    assert passed["status_json"]["schema"] == "nixome.config-status.v1"

    failed_status_json.write_text(
        json.dumps(
            {
                "schema": "nixome.config-status.v1",
                "pending_reboot": {
                    "required": True,
                    "actions": [
                        {
                            "type": "dom0-nixos-system",
                            "switch_action": "boot",
                            "reason": "install next dom0 boot generation",
                        }
                    ],
                    "reasons": ["install next dom0 boot generation"],
                },
                "system": {
                    "last_apply": {"host_system_source": "/nix/store/new-system"},
                    "host_system_profile_resolved_target": "/nix/store/new-system",
                    "run_current_system_resolved_target": "/nix/store/old-system",
                },
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    env["NIXOME_FAKE_CONFIG_STATUS"] = str(failed_status_json)
    result = _run(
        [
            "bash",
            str(repo_root / "lifecycle/runtime/tools/scripts/config-boot-verify.sh"),
        ],
        cwd=repo_root,
        env=env,
        stdout=work_dir / "failed-stdout.log",
        stderr=work_dir / "failed-stderr.log",
        check=False,
    )
    assert result.returncode != 0
    failed = json.loads(result_json.read_text(encoding="utf-8"))
    assert failed["schema"] == "nixome.config-boot-verify.v1"
    assert failed["status"] == "failed"
    assert failed["reason"] == "running system does not point at expected generation"
    assert failed["pending_reboot"]["required"] is True


def check_gc_nixome_store(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    data_root = work_dir / "data"
    nix_root = data_root / "nix-root"
    store_dir = nix_root / "nix/store"
    vm_dir = data_root / "vm"
    gc_log_dir = data_root / "logs"
    result_link_dir = data_root / "result-links"
    bin_dir = work_dir / "bin"
    mock_log = work_dir / "mock.log"

    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (
        bin_dir,
        store_dir / "11111111111111111111111111111111-vm-image",
        store_dir / "22222222222222222222222222222222-backing-image",
        store_dir / "33333333333333333333333333333333-system",
        store_dir / "44444444444444444444444444444444-tool",
        vm_dir,
        gc_log_dir,
        result_link_dir,
    ):
        path.mkdir(parents=True)
    (vm_dir / "nixome-vm-gui-config-dev-image-result").symlink_to(
        store_dir / "11111111111111111111111111111111-vm-image"
    )
    (result_link_dir / "qemu-kvm-result").symlink_to(
        "/nix/store/44444444444444444444444444444444-tool"
    )
    (vm_dir / "nixome-vm-gui-config-dev-image.qcow2").touch()
    (gc_log_dir / "config-apply.json").write_text(
        json.dumps(
            {
                "schema": "nixome.config-apply.v1",
                "system_apply_result": {
                    "host_system_source": "/nix/store/33333333333333333333333333333333-system"
                },
                "apply_log": (
                    "this noisy log may mention "
                    "/nix/store/99999999999999999999999999999999-do-not-root"
                ),
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    _write_executable(
        bin_dir / "nix-store",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        {
          printf 'NIX_CONFIG=%s\n' "${NIX_CONFIG:-}"
          printf 'ARGS='
          printf '[%s]' "$@"
          printf '\n'
        } >> "$NIXOME_GC_TEST_MOCK_LOG"
        case "$*" in
          "--gc --print-dead")
            printf '/nix/store/deadbeefdeadbeefdeadbeefdeadbeef-dead\n'
            printf 'removing stale temporary roots file fixture\n' >&2
            ;;
          "--gc")
            printf 'freed 0 bytes\n'
            ;;
          *)
            printf 'unexpected nix-store args: %s\n' "$*" >&2
            exit 2
            ;;
        esac
        """,
    )
    _write_executable(
        bin_dir / "qemu-img",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        if [ "$1" = "info" ]; then
          printf '%s\n' 'image: test.qcow2'
          printf '%s\n' 'file format: qcow2'
          printf '%s\n' 'backing file: /nix/store/22222222222222222222222222222222-backing-image/base.qcow2'
          exit 0
        fi
        printf 'unexpected qemu-img args: %s\n' "$*" >&2
        exit 2
        """,
    )

    base_env = os.environ.copy()
    base_env.update(
        {
            "PATH": f"{bin_dir}:{base_env['PATH']}",
            "NIXOME_GC_ALLOW_NONSTANDARD_DATA_ROOT": "1",
            "NIXOME_GC_TEST_MOCK_LOG": str(mock_log),
            "NIXOME_DATA_ROOT": str(data_root),
            "NIXOME_NIX_ROOT": str(nix_root),
            "NIXOME_NIX_STORE": str(nix_root),
            "NIXOME_VM_DIR": str(vm_dir),
            "NIXOME_LOG_DIR": str(gc_log_dir),
            "NIXOME_RESULT_LINK_DIR": str(result_link_dir),
            "NIXOME_NIX_STORE_BIN": str(bin_dir / "nix-store"),
            "NIXOME_QEMU_IMG_BIN": str(bin_dir / "qemu-img"),
        }
    )

    _run(
        [
            str(repo_root / "lifecycle/misc/tools/scripts/gc-nixome-store.sh"),
            "dry-run",
        ],
        cwd=repo_root,
        env=base_env,
        stdout=work_dir / "dry-run.out",
    )
    roots_dir = nix_root / "nix/var/nix/gcroots/nixome"
    manifest_text = (roots_dir / "manifest.tsv").read_text(encoding="utf-8")
    mock_text = mock_log.read_text(encoding="utf-8")
    dry_run_text = (work_dir / "dry-run.out").read_text(encoding="utf-8")
    assert f"store = {nix_root}" in mock_text
    assert "ARGS=[--gc][--print-dead]" in mock_text
    for store_name in (
        "11111111111111111111111111111111-vm-image",
        "22222222222222222222222222222222-backing-image",
        "33333333333333333333333333333333-system",
        "44444444444444444444444444444444-tool",
    ):
        assert f"/nix/store/{store_name}" in manifest_text
    assert "99999999999999999999999999999999-do-not-root" not in manifest_text
    assert "Dead path count:  1" in dry_run_text
    assert "Dead stderr log:" in dry_run_text
    assert "Result JSON:" in dry_run_text
    assert "removing stale temporary roots file fixture" in (
        gc_log_dir / "nixome-store-gc-dead-paths.stderr.log"
    ).read_text(encoding="utf-8")
    gc_json = json.loads(
        (gc_log_dir / "nixome-store-gc.json").read_text(encoding="utf-8")
    )
    assert gc_json["schema"] == "nixome.store-gc.v1"
    assert gc_json["mode"] == "dry-run"
    assert gc_json["nix_root"] == str(nix_root)
    assert gc_json["root_count"] == 4
    assert gc_json["dead_count"] == 1
    assert gc_json["gc_ran"] is False
    assert gc_json["dead_stderr_log"] == str(
        gc_log_dir / "nixome-store-gc-dead-paths.stderr.log"
    )

    _run(
        [str(repo_root / "lifecycle/misc/tools/scripts/gc-nixome-store.sh"), "run"],
        cwd=repo_root,
        env=base_env,
        stdout=work_dir / "run.out",
    )
    run_text = (work_dir / "run.out").read_text(encoding="utf-8")
    assert "ARGS=[--gc]" in mock_log.read_text(encoding="utf-8")
    assert "GC output tail:" in run_text
    assert "freed 0 bytes" in run_text
    gc_json = json.loads(
        (gc_log_dir / "nixome-store-gc.json").read_text(encoding="utf-8")
    )
    assert gc_json["schema"] == "nixome.store-gc.v1"
    assert gc_json["mode"] == "run"
    assert gc_json["gc_ran"] is True

    auto_env = base_env | {"NIXOME_GC_AUTO_THRESHOLD_GIB": "0"}
    _run(
        [str(repo_root / "lifecycle/misc/tools/scripts/gc-nixome-store.sh"), "auto"],
        cwd=repo_root,
        env=auto_env,
        stdout=work_dir / "auto.out",
    )
    auto_text = (work_dir / "auto.out").read_text(encoding="utf-8")
    mock_lines = mock_log.read_text(encoding="utf-8").splitlines()
    assert "Auto decision:    running full GC" in auto_text
    assert mock_lines.count("ARGS=[--gc][--print-dead]") == 2
    assert mock_lines.count("ARGS=[--gc]") == 2
    gc_json = json.loads(
        (gc_log_dir / "nixome-store-gc.json").read_text(encoding="utf-8")
    )
    assert gc_json["schema"] == "nixome.store-gc.v1"
    assert gc_json["mode"] == "auto"
    assert gc_json["gc_ran"] is True
    assert gc_json["dead_count"] == 1

    nonstandard_root = Path(f"/tmp/nixome-gc-nonstandard-{os.getpid()}")
    result = _run(
        [
            str(repo_root / "lifecycle/misc/tools/scripts/gc-nixome-store.sh"),
            "dry-run",
        ],
        cwd=repo_root,
        env={
            **os.environ,
            "NIXOME_DATA_ROOT": str(nonstandard_root),
            "NIXOME_NIX_ROOT": str(nonstandard_root / "nix-root"),
            "NIXOME_NIX_STORE": str(nonstandard_root / "nix-root"),
            "NIXOME_LOG_DIR": str(nonstandard_root / "logs"),
            "NIXOME_VM_DIR": str(nonstandard_root / "vm"),
        },
        stdout=work_dir / "nonstandard.out",
        stderr=work_dir / "nonstandard.out",
        check=False,
    )
    assert result.returncode != 0
    assert "refusing nonstandard data root" in (work_dir / "nonstandard.out").read_text(
        encoding="utf-8"
    )


def check_run_nested_gui_smoke_static(repo_root: Path, log_dir: Path) -> None:
    result_json = log_dir / "run-nested-gui-smoke-static.json"
    log_dir.mkdir(parents=True, exist_ok=True)

    for rel_path in (
        "lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh",
        "lifecycle/misc/tools/scripts/sync-nested-dom0-tools.sh",
        "lifecycle/misc/tools/scripts/sync-nested-codex-auth.sh",
        "lifecycle/misc/tools/scripts/run-config-boot-activation-flow.sh",
        "lifecycle/runtime/tools/scripts/apply-config-candidate.sh",
        "lifecycle/misc/tools/scripts/nixos-config-vms-smoke.sh",
        "lifecycle/runtime/tools/scripts/config-boot-verify.sh",
        "lifecycle/misc/tools/scripts/resolve-vm-image-result.sh",
    ):
        _run(["bash", "-n", str(repo_root / rel_path)], cwd=repo_root)

    texts = {
        "Makefile": (repo_root / "Makefile").read_text(encoding="utf-8"),
        "run_nested": (
            repo_root / "lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh"
        ).read_text(encoding="utf-8"),
        "config_vms": (
            repo_root / "lifecycle/misc/tools/scripts/nixos-config-vms-smoke.sh"
        ).read_text(encoding="utf-8"),
        "boot_flow": (
            repo_root
            / "lifecycle/misc/tools/scripts/run-config-boot-activation-flow.sh"
        ).read_text(encoding="utf-8"),
        "run_vm": (
            repo_root / "lifecycle/misc/tools/scripts/run-nested-vm.sh"
        ).read_text(encoding="utf-8"),
        "test_vm": (repo_root / "tests/test_vm.py").read_text(encoding="utf-8"),
        "test_vm_staged": (repo_root / "tests/test_vm_staged.py").read_text(
            encoding="utf-8"
        ),
        "sync_tools": (
            repo_root / "lifecycle/misc/tools/scripts/sync-nested-dom0-tools.sh"
        ).read_text(encoding="utf-8"),
        "sync_codex_auth": (
            repo_root / "lifecycle/misc/tools/scripts/sync-nested-codex-auth.sh"
        ).read_text(encoding="utf-8"),
        "config_inputs_import": (
            repo_root / "lifecycle/misc/tools/scripts/import-config-inputs.sh"
        ).read_text(encoding="utf-8"),
        "qube_guest": (repo_root / "lifecycle/misc/modules/qube-guest.nix").read_text(
            encoding="utf-8"
        ),
        "ssh_vm": (
            repo_root / "lifecycle/misc/tools/scripts/ssh-nested-vm.sh"
        ).read_text(encoding="utf-8"),
        "apply_candidate": (
            repo_root / "lifecycle/runtime/tools/scripts/apply-config-candidate.sh"
        ).read_text(encoding="utf-8"),
        "closure_smoke": (
            repo_root / "lifecycle/misc/tools/scripts/nix-closure-domain-smoke.sh"
        ).read_text(encoding="utf-8"),
    }

    for key in ("Makefile", "run_nested", "config_vms"):
        assert "NIXOME_CONFIG_BUILD_EXTRA_TEMPLATE" not in texts[key]
    assert (
        re.search(
            r"^(test|test-parallel|test-serial|test-vm|test-vm-parallel|"
            r"test-vm-warm|test-vm-staged|test-list):",
            texts["Makefile"],
            re.MULTILINE,
        )
        is None
    )

    expected_fragments = {
        "Makefile": [
            "vm-config-boot-flow",
            "vm-config-boot-activate-flow",
            "test-quick:",
            "test-warm:",
            "test-slow:",
            "tests/test_vm_staged.py",
            'NIXOME_VM_PARALLEL_LANES="$(TEST_VM_JOBS)"',
            'NIXOME_VM_PARALLEL_SSH_BASE_PORT="$(VM_PARALLEL_SSH_BASE_PORT)"',
            "VM_SLOW_SERIAL_VM_DIR ?=",
            "VM_SLOW_SERIAL_SSH_PORT ?=",
            "VM_SLOW_PARALLEL_FLOW_IDS =",
            "VM_SLOW_SELECTED_PARALLEL_TARGETS =",
            "VM_SLOW_SELECTED_SERIAL_TARGETS =",
            "VM_SLOW_DEFAULT_PARALLEL_FLOW_IDS = vm-config-flow vm-nix-closure-flow",
            "VM_SLOW_SERIAL_FLOW_IDS = vm-config-boot-activate-flow",
            "tests/test_vm.py::test_vm_flow[$(test)]",
            "NIXOME_CONFIG_DOM0_SYSTEM_SWITCH_ACTION=boot",
            "NIXOME_AUTO_GC_AFTER_RUN=1",
            "NIXOME_KEEP_VM=1",
            "NIXOME_REUSE_WARM_VM=1",
            "NIXOME_SYNC_DOM0_TOOLS=1",
            "NIXOME_VM_SKIP_BASE_BUILD=1",
            "vm-warm-start",
            "vm-warm-sync",
            "vm-sync-codex-auth",
            "sync-nested-codex-auth.sh",
            "vm-warm-apply",
            "vm-warm-test-config",
            "vm-parallel-prebuild-image",
            "NIXOME_VM_PARALLEL_IMAGE_LINK",
        ],
        "boot_flow": [
            "NIXOME_RUN_CONFIG_BOOT_VERIFY=1",
            "NIXOME_SHUTDOWN_VM_ON_SUCCESS=1",
            "resolve-vm-image-result.sh",
            "NIXOME_VM_SKIP_BASE_BUILD=1",
            "stage_status: .stage.status",
        ],
        "run_nested": [
            "NIXOME_CONFIG_DOM0_SYSTEM_SWITCH_ACTION",
            "NIXOME_CONFIG_DOM0_SYSTEM_REASON",
            "NIXOME_CONFIG_APPLY_STAGE",
            "NIXOME_CONFIG_VMS_SMOKE_MODE",
            "NIXOME_RUN_CONFIG_BOOT_VERIFY",
            "nixome-config-boot-verify",
            "NIXOME_SETUP_CONFIG_BOOT_WAIT_SECONDS",
            "wait_for_setup_config_boot_unit",
            "systemctl cat nixome-setup-config.service",
            "vm-gui-setup-config-boot.json",
            "copy_boot_setup_config_artifacts",
            "copy_boot_setup_config_result",
            "NIXOME_AUTO_GC_AFTER_RUN",
            "NIXOME_SYNC_DOM0_TOOLS",
            "NIXOME_REUSE_WARM_VM",
            "ssh: reusing existing nested dom0",
            "NIXOME_RUN_CONFIG_INPUTS_IMPORT",
            "NIXOME_NESTED_WARM_TOPOLOGY_PATH",
            "NIXOME_NESTED_WARM_CONFIG_INPUTS_PATH",
            "NIXOME_NESTED_WARM_STOP_NIX_DOMAINS",
            "qvm-kill",
            "affected domain did not halt",
            'CONFIG_VMS_SMOKE_COMMAND="$REMOTE_WARM_BIN_DIR/nixome-config-vms-smoke"',
            'SETUP_CONFIG_TOPOLOGY="$REMOTE_WARM_TOPOLOGY_PATH"',
            'NIXOME_NIX_DOMAIN_STORAGE_HELPER="$REMOTE_WARM_BIN_DIR/nixome-nix-domain-storage"',
            "NIXOME_NIX_DOMAIN_STORAGE_HELPER \\",
            "NIXOME_SETUP_CONFIG_COMMAND;",
            'NIXOME_CONFIG_INPUTS_SOURCE="$REMOTE_WARM_CONFIG_INPUTS_PATH"',
            "NIXOME_CONFIG_INPUTS_SOURCE \\",
            '--topology $(shell_quote "$SETUP_CONFIG_TOPOLOGY")',
            "sync-nested-dom0-tools.sh",
            "prepare_warm_sync_boot_unit",
            "warm tool sync uses explicit setup-config",
            "Keep the boot oneshot in its active/exited state",
            "vm-gui-sync-dom0-tools.json",
            'NIXOME_SETUP_CONFIG_COMMAND="$REMOTE_WARM_BIN_DIR/nixome-setup-config"',
            "CONFIG_VMS_STAGING_UPDATE_LOG",
            "config-vms-config-staging-update.sh",
            "sudo -n $config_vms_env NIXOME_LOG_DIR=/var/log/nixome $config_vms_smoke_command",
            '"$ROOT/lifecycle/misc/tools/scripts/nixome-nix.sh" shell nixpkgs#sshpass',
            'NIX_CLOSURE_DOMAIN_SMOKE_COMMAND="$REMOTE_WARM_BIN_DIR/nixome-nix-closure-domain-smoke"',
        ],
        "config_vms": [
            'if $DOM0_SYSTEM_SWITCH_ACTION_NIX == "switch" then',
            'EXPECTED_SYSTEMD_ACTIONS_STATUS="skipped"',
            "grep -F 'rawLocalConfig = import ./nixome/runtime.nix {' flake.nix",
            "grep -F 'nixosConfigurations = nixNativeConfigurations // {' flake.nix",
            "grep -F 'nixome-dom0 = nixomeDom0System;' flake.nix",
            "grep -F 'nixNative.domains.",
            'assert_nix_closure_domain "$VAULT"',
            "full|preflight|stage-candidate|apply-candidate|post-apply",
        ],
        "run_vm": [
            "NIXOME_VM_SKIP_BASE_BUILD",
            "using existing Nixome VM base image result link",
            "NIXOME_VM_IOMMU",
            "kernel-irqchip=split",
            "intel-iommu,intremap=on,caching-mode=on",
            "virtio-net-pci,disable-legacy=on,iommu_platform=on,vectors=0,netdev=sysnet0",
        ],
        "test_vm": [
            "NIXOME_VM_SKIP_BASE_BUILD",
            "NIXOME_VM_PARALLEL_SSH_BASE_PORT",
            "NIXOME_SKIP_GC_AUTO",
        ],
        "test_vm_staged": [
            "STAGED_VM_STEPS",
            "config-apply-resolve",
            "nix-closure-domain-smoke",
            "NIXOME_NIX_CLOSURE_DOMAIN_SMOKE_COMMAND",
        ],
        "sync_tools": [
            "nix-store --export",
            "sudo -n nix-store --import",
            "nixome-config-inputs-import",
            "REMOTE_CONFIG_INPUTS_SYSTEM_PATH",
            'nixos-qubes", .nixosQubes',
            "mount --bind",
            "nixome-config-vms-smoke",
            "nixosConfigurations.vm-gui-config.config.nixome.qubes.topology",
            "nixosConfigurations.nixome-domain-$domain.config.system.build.toplevel",
            "nixome-domain-$domain-store-closure",
            "PYTHONPATH=",
            "systemctl restart qubesd.service",
        ],
        "sync_codex_auth": [
            "nixome.nested-codex-auth-sync.v1",
            "LOCAL_AUTH_JSON",
            "auth.json",
            "NIXOME_SYNC_CODEX_CONFIG",
            "config.toml",
            "/etc/nixome/dev-mode",
            "nested dom0 SSH is not reachable",
            "remote nested dom0 is not a dev-mode image",
            "install -m 0600",
            "CODEX_HOME:-$HOME/.codex",
            '"$ROOT/lifecycle/misc/tools/scripts/nixome-nix.sh" shell nixpkgs#sshpass',
        ],
        "config_inputs_import": [
            "store-export.nar.gz",
            'nix-store -q --references "$store_root"',
            'cut -f3 "$tmpdir/config-inputs.tsv"',
            'nix-store --export $(cat "$inputs_store_paths")',
            "store_export_bytes",
        ],
        "qube_guest": [
            'nix-store --store "$store_root" --import',
            "input_relative_path=",
            '${input_path#"$input_store_path"}',
            'mount --bind "$physical_path" "$mountpoint_path"',
            "ps.dbus-python",
            "nixome-qubes-setup-dnat-to-ns",
            "nixome-qubes-network-proxy-setup",
        ],
        "ssh_vm": [
            '"$ROOT/lifecycle/misc/tools/scripts/nixome-nix.sh" shell nixpkgs#sshpass'
        ],
        "apply_candidate": [
            "NIXOME_SETUP_CONFIG_COMMAND",
            'CONFIG_APPLY_STAGE="${NIXOME_CONFIG_APPLY_STAGE:-full}"',
            "full|archive|resolve|setup|system|deploy",
        ],
        "closure_smoke": ["NIXOME_SETUP_CONFIG_COMMAND"],
    }
    for key, fragments in expected_fragments.items():
        for fragment in fragments:
            assert fragment in texts[key], f"missing {fragment!r} in {key}"

    assert 'cat "$RESULT_JSON"' not in texts["boot_flow"]
    assert 'cat >> "$STAGING_UPDATE_SCRIPT"' not in texts["config_vms"]
    assert "systemctl stop nixome-setup-config.service" not in texts["run_nested"]
    scripts_text = "\n".join(
        path.read_text(encoding="utf-8", errors="replace")
        for path in (repo_root / "scripts").glob("*.sh")
    )
    assert "nix shell nixpkgs#sshpass" not in scripts_text

    result_json.write_text(
        json.dumps(
            {"schema": "nixome.run-nested-gui-smoke-static.v1", "status": "passed"}
        )
        + "\n",
        encoding="utf-8",
    )


def check_qubes_vm_kernel_stubdomain_boot(repo_root: Path, log_dir: Path) -> None:
    del log_dir
    repack = repo_root / "vendor/nixos-qubes/nix/repackRpmKernel.nix"
    install = (
        repo_root
        / "vendor/nixos-qubes/pkgs/by-name/ni/nixos-qubes-tools/src/nixos-qubes-install-kernel-rpm"
    )

    repack_text = repack.read_text(encoding="utf-8")
    install_text = install.read_text(encoding="utf-8")

    def assert_before_mkfs(text: str, copy_tokens: tuple[str, str]) -> None:
        mkfs_pos = text.index("mkfs.ext3")
        for token in copy_tokens:
            pos = text.index(token)
            assert pos < mkfs_pos

    assert "expects direct-boot artifacts at its root" in repack_text
    assert "srcs = [" in repack_text
    assert "kernelSrc" in repack_text
    assert "modulesSrc" in repack_text
    assert_before_mkfs(
        repack_text,
        (
            'cp "var/lib/qubes/vm-kernels/${version}/vmlinuz" imgroot/vmlinuz',
            'cp "var/lib/qubes/vm-kernels/${version}/initramfs" imgroot/initramfs',
        ),
    )

    assert "expects direct-boot artifacts at its root" in install_text
    assert_before_mkfs(
        install_text,
        (
            'cp "$kerneldir/vmlinuz" "$imgroot/vmlinuz"',
            'cp "$kerneldir/initramfs" "$imgroot/initramfs"',
        ),
    )


def check_qubes_libvirt_hvm_pirq_patch(repo_root: Path, log_dir: Path) -> None:
    del log_dir
    package = (
        repo_root / "vendor/nixos-qubes/pkgs/by-name/qu/qubes-core-libvirt/package.nix"
    )
    patch = package.with_name("0019-libxl-enable-hvm-pirq-for-pci-passthrough.patch")

    package_text = package.read_text(encoding="utf-8")
    patch_text = patch.read_text(encoding="utf-8")

    assert (
        "patch -p1 --input ${./0019-libxl-enable-hvm-pirq-for-pci-passthrough.patch}"
        in package_text
    )
    assert "#ifdef LIBXL_HAVE_HVM_PIRQ" in patch_text
    assert "def->os.type == VIR_DOMAIN_OSTYPE_HVM" in patch_text
    assert "d_config->num_pcidevs > 0" in patch_text
    assert "libxl_defbool_set(&d_config->b_info.u.hvm.pirq, true);" in patch_text


def check_dev_role(repo_root: Path, log_dir: Path) -> None:
    shutil.rmtree(log_dir, ignore_errors=True)
    log_dir.mkdir(parents=True)
    role_json = log_dir / "role.json"

    expr = f"""
    let
      flake = builtins.getFlake "path:{repo_root}";
      lib = flake.inputs.nixpkgs.lib;
      legacyDevDom0Cfg = flake.nixosConfigurations.vm-gui-config.config;
      dom0Cfg = flake.nixosConfigurations."vm-gui-config-dev".config;
      releaseDom0Cfg = flake.nixosConfigurations."vm-gui-config-release".config;
      nonDevDom0Cfg = flake.nixosConfigurations.test.config;
      builders = import {repo_root}/lifecycle/misc/lib/qube-template.nix {{
        nixpkgs = flake.inputs.nixpkgs;
        nixpkgsLibvirt105Source = "${{flake.inputs.nixpkgs-libvirt_10_5}}";
        nixosQubesSource = "${{flake.inputs.nixos-qubes}}";
        nixomeSupport = "{repo_root}";
        system = "x86_64-linux";
      }};
      defaultRuntime = import {repo_root}/lifecycle/bootstrap/initial-runtime-config/nixome/runtime.nix {{
        appliedMessage = "test";
        pkgs = builders.pkgs;
        system = "x86_64-linux";
        templateBuilders = builders;
      }};
      devEntry = builders.mkNixNativeDomain {{
        name = "sample-dev";
        guestRole = "dev";
        modules = [
          ({{ ... }}: {{
            networking.hostName = "sample-dev";
          }})
        ];
        features = {{
          "nixome-role" = "dev";
          "nixome-dev" = "1";
        }};
      }};
      devCfg = devEntry.configuration.config;
      generatedDevDomain = devEntry.domain;
      sysNetDomainDefinition = defaultRuntime.nixNative.domains.sys-net;
      sysFirewallDomainDefinition = defaultRuntime.nixNative.domains.sys-firewall;
      sysGuiDomainDefinition = defaultRuntime.nixNative.domains.sys-gui;
      sysGuiCfg = flake.nixosConfigurations.nixome-domain-sys-gui.config;
      stagedNixNativeDomains = defaultRuntime.nixNative.domains;
      bootstrapDomains = dom0Cfg.nixome.qubes.domains;
      releaseBootstrapDomains = releaseDom0Cfg.nixome.qubes.domains;
      bootstrapSysNetDomain = bootstrapDomains.sys-net;
      bootstrapSysFirewallDomain = bootstrapDomains.sys-firewall;
      bootstrapSysGuiDomain = bootstrapDomains.sys-gui;
      bootstrapTopologySysNetDomain = dom0Cfg.nixome.qubes.topology.domains.sys-net;
      bootstrapTopologySysFirewallDomain =
        dom0Cfg.nixome.qubes.topology.domains.sys-firewall;
      baseCfg = (builders.mkNixNativeDomain {{
        name = "minimal-base";
        guestRole = "base";
        modules = [ ];
      }}).configuration.config;
      packageName = package: package.pname or package.name or "";
      packageNames = cfg: map packageName cfg.environment.systemPackages;
      dom0Packages = packageNames dom0Cfg;
      dom0QubesPackages = map packageName dom0Cfg.virtualisation.qubes.packages;
      legacyDevDom0Packages = packageNames legacyDevDom0Cfg;
      releaseDom0Packages = packageNames releaseDom0Cfg;
      nonDevDom0Packages = packageNames nonDevDom0Cfg;
      devPackages = packageNames devCfg;
      sysGuiPackages = packageNames sysGuiCfg;
      basePackages = packageNames baseCfg;
      has = name: values: builtins.elem name values;
      hasAnyPrefix = prefix: values: lib.any (value: lib.hasPrefix prefix value) values;
      etcNames = cfg: lib.attrNames cfg.environment.etc;
      forbiddenConfigRpc = [
        "qubes-rpc/nixome.Config.ArchiveCandidate"
        "qubes-rpc/nixome.Config.ArchiveDeployment"
        "qubes-rpc/nixome.Config.CandidateExport"
        "qubes-rpc/nixome.Config.InputsImport"
        "qubes-rpc/nixome.Config.ValidateCandidate"
        "qubes-rpc/nixome.PolicyProbe"
      ];
      forbiddenDevPackages = [
        "cargo"
        "clippy"
        "codex"
        "git"
        "nixome"
        "nixome-config-nix"
        "rustc"
        "rustfmt"
        "uv"
        "zellij"
      ];
      secretEnvNames = [
        "CODEX_AUTH_TOKEN"
        "OPENAI_API_KEY"
        "OPENAI_API_KEY_FILE"
      ];
      devServiceScript = devCfg.systemd.services.nixome-dev-guest-init.script;
    in
    {{
      role = devCfg.nixome.qubeGuest.role;
      hostname = devCfg.networking.hostName;
      staged_dev_declared = builtins.hasAttr "dev" stagedNixNativeDomains;
      staged_sys_net_declared = builtins.hasAttr "sys-net" stagedNixNativeDomains;
      staged_sys_net_autostart = sysNetDomainDefinition.prefs.autostart;
      staged_sys_net_virt_mode = sysNetDomainDefinition.prefs.virt_mode;
      staged_sys_net_updates_proxy = sysNetDomainDefinition.services."qubes-updates-proxy";
      staged_sys_firewall_declared = builtins.hasAttr "sys-firewall" stagedNixNativeDomains;
      staged_sys_firewall_netvm = sysFirewallDomainDefinition.netvm;
      staged_sys_firewall_autostart = sysFirewallDomainDefinition.prefs.autostart;
      staged_sys_firewall_provides_network =
        sysFirewallDomainDefinition.prefs.provides_network;
      staged_sys_firewall_servicevm = sysFirewallDomainDefinition.features.servicevm;
      staged_sys_firewall_updates_proxy =
        sysFirewallDomainDefinition.services."qubes-updates-proxy";
      staged_sys_gui_declared = builtins.hasAttr "sys-gui" stagedNixNativeDomains;
      staged_sys_gui_role = sysGuiDomainDefinition.role;
      staged_sys_gui_netvm = sysGuiDomainDefinition.netvm;
      staged_sys_gui_guivm = sysGuiDomainDefinition.prefs.guivm;
      staged_sys_gui_autostart = sysGuiDomainDefinition.prefs.autostart;
      staged_sys_gui_agent_feature = sysGuiDomainDefinition.features.gui;
      staged_sys_gui_service = sysGuiDomainDefinition.services.guivm;
      staged_default_guivm = defaultRuntime.topology.extraPrefs.default_guivm;
      staged_default_netvm = defaultRuntime.topology.extraPrefs.default_netvm;
      generated_dev_domain_netvm = generatedDevDomain.netvm;
      generated_dev_domain_role_feature = generatedDevDomain.features."nixome-role";
      generated_dev_domain_marker_feature = generatedDevDomain.features."nixome-dev";
      bootstrap_dev_domain_present = builtins.hasAttr "dev" bootstrapDomains;
      bootstrap_sys_net_present = builtins.hasAttr "sys-net" bootstrapDomains;
      bootstrap_sys_firewall_present = builtins.hasAttr "sys-firewall" bootstrapDomains;
      bootstrap_sys_net_autostart = bootstrapSysNetDomain.prefs.autostart;
      bootstrap_sys_net_netvm = bootstrapSysNetDomain.netvm;
      bootstrap_sys_net_virt_mode = bootstrapSysNetDomain.prefs.virt_mode;
      bootstrap_sys_net_updates_proxy = bootstrapSysNetDomain.services."qubes-updates-proxy";
      bootstrap_sys_firewall_netvm = bootstrapSysFirewallDomain.netvm;
      bootstrap_sys_firewall_autostart = bootstrapSysFirewallDomain.prefs.autostart;
      bootstrap_sys_firewall_provides_network =
        bootstrapSysFirewallDomain.prefs.provides_network;
      bootstrap_sys_firewall_servicevm = bootstrapSysFirewallDomain.features.servicevm;
      bootstrap_sys_firewall_updates_proxy =
        bootstrapSysFirewallDomain.services."qubes-updates-proxy";
      bootstrap_sys_gui_present = builtins.hasAttr "sys-gui" bootstrapDomains;
      bootstrap_sys_gui_guivm = bootstrapSysGuiDomain.prefs.guivm;
      bootstrap_sys_gui_autostart = bootstrapSysGuiDomain.prefs.autostart;
      bootstrap_sys_gui_agent_feature = bootstrapSysGuiDomain.features.gui;
      bootstrap_default_guivm = dom0Cfg.nixome.qubes.prefs.default_guivm;
      bootstrap_topology_default_guivm = dom0Cfg.nixome.qubes.topology.prefs.default_guivm;
      sys_gui_role_enabled = sysGuiCfg.nixome.sysGuiGuest.enable;
      sys_gui_agent_enabled = builtins.elem "multi-user.target"
        sysGuiCfg.systemd.services.qubes-gui-agent.wantedBy;
      sys_gui_packages_present = lib.all (name: has name sysGuiPackages) [
        "qubes-gui-agent-linux"
        "qubes-gui-daemon"
        "xfce4-session"
      ];
      sys_gui_rpc_present = lib.all (name: builtins.hasAttr name sysGuiCfg.environment.etc) [
        "qubes-rpc/qubes.GuiVMSession"
        "qubes-rpc/qubes.SetMonitorLayout"
        "qubes-rpc/qubes.WindowIconUpdater"
      ];
      dom0_sys_gui_policy_present = has "nixome-sys-gui-policy" dom0QubesPackages;
      dom0_sys_gui_session_present = lib.any (
        package: builtins.elem "sys-gui" (package.providedSessions or [ ])
      ) dom0Cfg.services.displayManager.sessionPackages;
      dom0_default_session = dom0Cfg.services.displayManager.defaultSession;
      dom0_display_manager_after = dom0Cfg.systemd.services.display-manager.after;
      dom0_bootstrap_status_device = dom0Cfg.nixome.qubes.setupConfigOnBoot.statusDevice;
      dom0_bootstrap_status_environment =
        dom0Cfg.systemd.services.nixome-setup-config.environment.NIXOME_SETUP_CONFIG_STATUS_DEVICE;
      bootstrap_default_netvm = dom0Cfg.nixome.qubes.prefs.default_netvm;
      bootstrap_topology_default_netvm = dom0Cfg.nixome.qubes.topology.prefs.default_netvm;
      bootstrap_dev_topology_present = builtins.hasAttr "dev" dom0Cfg.nixome.qubes.topology.domains;
      bootstrap_sys_net_topology_autostart = bootstrapTopologySysNetDomain.prefs.autostart;
      bootstrap_sys_firewall_topology_netvm = bootstrapTopologySysFirewallDomain.netvm;
      bootstrap_sys_firewall_topology_autostart =
        bootstrapTopologySysFirewallDomain.prefs.autostart;
      image_dev_mode_enabled = dom0Cfg.nixome.image.devMode.enable;
      legacy_image_dev_mode_enabled = legacyDevDom0Cfg.nixome.image.devMode.enable;
      dom0_dhcp_enabled = dom0Cfg.networking.useDHCP;
      dom0_dhcpcd_no_gateway = lib.hasInfix "nogateway" dom0Cfg.networking.dhcpcd.extraConfig;
      dom0_proxy_enabled = dom0Cfg.nixome.dom0.devModeNetworkProxy.enable;
      dom0_proxy_target = dom0Cfg.nixome.dom0.devModeNetworkProxy.target;
      dom0_proxy_http = dom0Cfg.environment.sessionVariables.http_proxy or null;
      dom0_proxy_https = dom0Cfg.environment.sessionVariables.https_proxy or null;
      dom0_proxy_no_proxy = dom0Cfg.environment.sessionVariables.no_proxy or null;
      dom0_nix_daemon_proxy_http =
        dom0Cfg.systemd.services.nix-daemon.environment.http_proxy or null;
      dom0_dev_mode_marker = dom0Cfg.environment.etc."nixome/dev-mode".text;
      dom0_observability_enabled = dom0Cfg.nixome.dom0.observability.enable;
      dom0_testing_tools_enabled = dom0Cfg.nixome.dom0.testingTools.enable;
      dom0_dev_tools_present = lib.all (name: has name dom0Packages) [
        "codex"
        "git"
        "zellij"
      ];
      legacy_dev_tools_present = lib.all (name: has name legacyDevDom0Packages) [
        "codex"
        "git"
        "zellij"
      ];
      release_image_dev_mode_enabled = releaseDom0Cfg.nixome.image.devMode.enable;
      release_dom0_dhcp_enabled = releaseDom0Cfg.networking.useDHCP;
      release_dom0_dhcpcd_no_gateway =
        lib.hasInfix "nogateway" releaseDom0Cfg.networking.dhcpcd.extraConfig;
      release_dom0_proxy_enabled = releaseDom0Cfg.nixome.dom0.devModeNetworkProxy.enable;
      release_dom0_proxy_http = releaseDom0Cfg.environment.sessionVariables.http_proxy or null;
      release_dom0_dev_marker_absent =
        !(builtins.hasAttr "nixome/dev-mode" releaseDom0Cfg.environment.etc);
      release_dom0_observability_enabled = releaseDom0Cfg.nixome.dom0.observability.enable;
      release_dom0_testing_tools_enabled = releaseDom0Cfg.nixome.dom0.testingTools.enable;
      release_dom0_dev_tools_absent = lib.all (name: !(has name releaseDom0Packages)) [
        "codex"
        "git"
        "nixome-observe-dom0"
        "nixome-qube-bootstrap"
        "nixome-qube-gui-smoke"
        "nixome-qubes-smoke"
        "zellij"
      ];
      release_bootstrap_dev_domain_present = builtins.hasAttr "dev" releaseBootstrapDomains;
      release_bootstrap_sys_net_present = builtins.hasAttr "sys-net" releaseBootstrapDomains;
      release_bootstrap_sys_net_netvm = releaseBootstrapDomains.sys-net.netvm;
      release_bootstrap_sys_net_autostart = releaseBootstrapDomains.sys-net.prefs.autostart;
      release_bootstrap_sys_net_virt_mode =
        releaseBootstrapDomains.sys-net.prefs.virt_mode;
      release_bootstrap_sys_firewall_present =
        builtins.hasAttr "sys-firewall" releaseBootstrapDomains;
      release_bootstrap_sys_firewall_netvm = releaseBootstrapDomains.sys-firewall.netvm;
      release_bootstrap_sys_firewall_autostart =
        releaseBootstrapDomains.sys-firewall.prefs.autostart;
      non_dev_image_dev_mode_enabled = nonDevDom0Cfg.nixome.image.devMode.enable;
      non_dev_dom0_dhcp_enabled = nonDevDom0Cfg.networking.useDHCP;
      non_dev_dom0_dhcpcd_no_gateway =
        lib.hasInfix "nogateway" nonDevDom0Cfg.networking.dhcpcd.extraConfig;
      non_dev_dom0_proxy_enabled = nonDevDom0Cfg.nixome.dom0.devModeNetworkProxy.enable;
      non_dev_dom0_proxy_http = nonDevDom0Cfg.environment.sessionVariables.http_proxy or null;
      non_dev_dom0_dev_marker_absent =
        !(builtins.hasAttr "nixome/dev-mode" nonDevDom0Cfg.environment.etc);
      non_dev_dom0_observability_enabled = nonDevDom0Cfg.nixome.dom0.observability.enable;
      non_dev_dom0_testing_tools_enabled = nonDevDom0Cfg.nixome.dom0.testingTools.enable;
      non_dev_dom0_dev_tools_absent = lib.all (name: !(has name nonDevDom0Packages)) [
        "codex"
        "git"
        "nixome-observe-dom0"
        "nixome-qube-bootstrap"
        "nixome-qube-gui-smoke"
        "nixome-qubes-smoke"
        "zellij"
      ];
      package_names = devPackages;
      required_tools_present = lib.all (name: has name devPackages) [
        "cargo"
        "clippy"
        "codex"
        "git"
        "gnumake"
        "jq"
        "less"
        "nix"
        "nixfmt"
        "nixome"
        "nixome-config-nix"
        "openssh"
        "ripgrep"
        "rustc"
        "rustfmt"
        "uv"
        "vim"
        "zellij"
      ] && hasAnyPrefix "python3" devPackages;
      network_client_support_present = lib.all (name: has name devPackages) [
        "bind"
        "codex"
        "dhcpcd"
        "git"
        "nix"
        "nss-cacert"
        "openresolv"
        "openssh"
      ];
      private_nix_enabled = devCfg.nixome.minimalGuest.nix.enable;
      nix_features = devCfg.nix.settings.experimental-features;
      nix_allowed_users = devCfg.nix.settings.allowed-users;
      codex_home = devCfg.nixome.qubeGuest.codexHome;
      codex_home_env = devCfg.environment.sessionVariables.CODEX_HOME or null;
      dev_persistent_paths = devCfg.nixome.qubeGuest.devPersistentPaths;
      dev_init_contains_dev_path = lib.hasInfix "/home/user/dev" devServiceScript;
      dev_init_contains_codex_home = lib.hasInfix "/home/user/.codex" devServiceScript;
      dev_init_contains_private_nix_root = lib.hasInfix "/rw/home/user/nixome-config-nix-root" devServiceScript;
      dev_qrexec_requires = devCfg.systemd.services.qubes-qrexec-agent.requires;
      dev_qrexec_after = devCfg.systemd.services.qubes-qrexec-agent.after;
      dev_forbidden_rpc_absent = lib.all (name: !(has name (etcNames devCfg))) forbiddenConfigRpc;
      dev_allowed_submitters = dom0Cfg.nixome.qubes.configAccessPolicy.allowedSubmitters;
      dev_no_secret_env = lib.all (name: !(builtins.hasAttr name devCfg.environment.sessionVariables)) secretEnvNames;
      dev_no_codex_auth_etc = lib.all (
        name:
        !(lib.hasInfix "auth.json" name)
        && !(lib.hasInfix "OPENAI_API_KEY" name)
        && !(lib.hasInfix ".codex" name)
      ) (etcNames devCfg);
      base_role = baseCfg.nixome.qubeGuest.role;
      base_private_nix_enabled = baseCfg.nixome.minimalGuest.nix.enable;
      base_dev_packages_absent = lib.all (name: !(has name basePackages)) forbiddenDevPackages;
      base_forbidden_rpc_absent = lib.all (name: !(has name (etcNames baseCfg))) forbiddenConfigRpc;
    }}
    """
    _run(["nix", "eval", "--impure", "--json", "--expr", expr], stdout=role_json)
    data = json.loads(role_json.read_text(encoding="utf-8"))

    assert data["role"] == "dev"
    assert data["hostname"] == "sample-dev"
    assert data["staged_dev_declared"] is False
    assert data["staged_sys_net_declared"] is True
    assert data["staged_sys_net_autostart"] is True
    assert data["staged_sys_net_virt_mode"] == "hvm"
    assert data["staged_sys_net_updates_proxy"] is True
    assert data["staged_sys_firewall_declared"] is True
    assert data["staged_sys_firewall_netvm"] == "sys-net"
    assert data["staged_sys_firewall_autostart"] is True
    assert data["staged_sys_firewall_provides_network"] is True
    assert data["staged_sys_firewall_servicevm"] == "1"
    assert data["staged_sys_firewall_updates_proxy"] is True
    assert data["staged_sys_gui_declared"] is True
    assert data["staged_sys_gui_role"] == "sys-gui"
    assert data["staged_sys_gui_netvm"] is None
    assert data["staged_sys_gui_guivm"] == "dom0"
    assert data["staged_sys_gui_autostart"] is True
    assert data["staged_sys_gui_agent_feature"] == "1"
    assert data["staged_sys_gui_service"] is True
    assert data["staged_default_guivm"] == "sys-gui"
    assert data["staged_default_netvm"] == "sys-firewall"
    assert data["generated_dev_domain_netvm"] is None
    assert data["generated_dev_domain_role_feature"] == "dev"
    assert data["generated_dev_domain_marker_feature"] == "1"
    assert data["bootstrap_dev_domain_present"] is False
    assert data["bootstrap_sys_net_present"] is True
    assert data["bootstrap_sys_firewall_present"] is True
    assert data["bootstrap_sys_net_autostart"] is True
    assert data["bootstrap_sys_net_netvm"] is None
    assert data["bootstrap_sys_net_virt_mode"] == "hvm"
    assert data["bootstrap_sys_net_updates_proxy"] is True
    assert data["bootstrap_sys_firewall_netvm"] == "sys-net"
    assert data["bootstrap_sys_firewall_autostart"] is True
    assert data["bootstrap_sys_firewall_provides_network"] is True
    assert data["bootstrap_sys_firewall_servicevm"] == "1"
    assert data["bootstrap_sys_firewall_updates_proxy"] is True
    assert data["bootstrap_sys_gui_present"] is True
    assert data["bootstrap_sys_gui_guivm"] == "dom0"
    assert data["bootstrap_sys_gui_autostart"] is True
    assert data["bootstrap_sys_gui_agent_feature"] == "1"
    assert data["bootstrap_default_guivm"] == "sys-gui"
    assert data["bootstrap_topology_default_guivm"] == "sys-gui"
    assert data["sys_gui_role_enabled"] is True
    assert data["sys_gui_agent_enabled"] is True
    assert data["sys_gui_packages_present"] is True
    assert data["sys_gui_rpc_present"] is True
    assert data["dom0_sys_gui_policy_present"] is True
    assert data["dom0_sys_gui_session_present"] is True
    assert data["dom0_default_session"] == "sys-gui"
    assert "nixome-setup-config.service" in data["dom0_display_manager_after"]
    assert data["dom0_bootstrap_status_device"] == "/dev/tty1"
    assert data["dom0_bootstrap_status_environment"] == "/dev/tty1"
    assert data["bootstrap_default_netvm"] == "sys-firewall"
    assert data["bootstrap_topology_default_netvm"] == "sys-firewall"
    assert data["bootstrap_dev_topology_present"] is False
    assert data["bootstrap_sys_net_topology_autostart"] is True
    assert data["bootstrap_sys_firewall_topology_netvm"] == "sys-net"
    assert data["bootstrap_sys_firewall_topology_autostart"] is True
    assert data["image_dev_mode_enabled"] is True
    assert data["legacy_image_dev_mode_enabled"] is True
    assert data["dom0_dhcp_enabled"] is True
    assert data["dom0_dhcpcd_no_gateway"] is True
    assert data["dom0_proxy_enabled"] is True
    assert data["dom0_proxy_target"] == "sys-firewall"
    assert data["dom0_proxy_http"] == "http://127.0.0.1:8082"
    assert data["dom0_proxy_https"] == "http://127.0.0.1:8082"
    assert data["dom0_proxy_no_proxy"] == "127.0.0.1,localhost,::1"
    assert data["dom0_nix_daemon_proxy_http"] == "http://127.0.0.1:8082"
    assert data["dom0_dev_mode_marker"] == "enabled\n"
    assert data["dom0_observability_enabled"] is True
    assert data["dom0_testing_tools_enabled"] is True
    assert data["dom0_dev_tools_present"] is True
    assert data["legacy_dev_tools_present"] is True
    assert data["release_image_dev_mode_enabled"] is False
    assert data["release_dom0_dhcp_enabled"] is False
    assert data["release_dom0_dhcpcd_no_gateway"] is False
    assert data["release_dom0_proxy_enabled"] is False
    assert data["release_dom0_proxy_http"] is None
    assert data["release_dom0_dev_marker_absent"] is True
    assert data["release_dom0_observability_enabled"] is False
    assert data["release_dom0_testing_tools_enabled"] is False
    assert data["release_dom0_dev_tools_absent"] is True
    assert data["release_bootstrap_dev_domain_present"] is False
    assert data["release_bootstrap_sys_net_present"] is True
    assert data["release_bootstrap_sys_net_netvm"] is None
    assert data["release_bootstrap_sys_net_autostart"] is True
    assert data["release_bootstrap_sys_net_virt_mode"] == "hvm"
    assert data["release_bootstrap_sys_firewall_present"] is True
    assert data["release_bootstrap_sys_firewall_netvm"] == "sys-net"
    assert data["release_bootstrap_sys_firewall_autostart"] is True
    assert data["non_dev_image_dev_mode_enabled"] is False
    assert data["non_dev_dom0_dhcp_enabled"] is False
    assert data["non_dev_dom0_dhcpcd_no_gateway"] is False
    assert data["non_dev_dom0_proxy_enabled"] is False
    assert data["non_dev_dom0_proxy_http"] is None
    assert data["non_dev_dom0_dev_marker_absent"] is True
    assert data["non_dev_dom0_observability_enabled"] is False
    assert data["non_dev_dom0_testing_tools_enabled"] is False
    assert data["non_dev_dom0_dev_tools_absent"] is True
    assert data["required_tools_present"] is True
    assert data["network_client_support_present"] is True
    assert data["private_nix_enabled"] is True
    assert "nix-command" in data["nix_features"]
    assert "flakes" in data["nix_features"]
    assert data["nix_allowed_users"] == ["user"]
    assert data["codex_home"] == "/home/user/.codex"
    assert data["codex_home_env"] == "/home/user/.codex"
    assert "/home/user/dev" in data["dev_persistent_paths"]
    assert data["dev_init_contains_dev_path"] is True
    assert data["dev_init_contains_codex_home"] is True
    assert data["dev_init_contains_private_nix_root"] is True
    assert "nixome-dev-guest-init.service" in data["dev_qrexec_requires"]
    assert "nixome-dev-guest-init.service" in data["dev_qrexec_after"]
    assert data["dev_forbidden_rpc_absent"] is True
    assert data["dev_allowed_submitters"] == []
    assert data["dev_no_secret_env"] is True
    assert data["dev_no_codex_auth_etc"] is True
    assert data["base_role"] == "base"
    assert data["base_private_nix_enabled"] is False
    assert data["base_dev_packages_absent"] is True
    assert data["base_forbidden_rpc_absent"] is True

    codex_out = log_dir / "codex-out.txt"
    _run(
        [
            "nix",
            "build",
            f"{repo_root}#packages.x86_64-linux.codex",
            "--no-link",
            "--print-build-logs",
            "--print-out-paths",
        ],
        stdout=codex_out,
    )
    codex_path = codex_out.read_text(encoding="utf-8").splitlines()[-1]
    codex_version = _run(
        [
            "bash",
            "-c",
            'source "$1"; nixome_run_store_path "$2/bin/codex" --version',
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/lib/nixome-env.sh"),
            codex_path,
        ],
        cwd=repo_root,
    ).stdout
    (log_dir / "codex-version.txt").write_text(codex_version, encoding="utf-8")
    assert re.search(r"^codex-cli [0-9]+\.[0-9]+\.[0-9]+$", codex_version)
    codex_physical = Path(_physical_store_path(repo_root, codex_path))
    assert not any(codex_physical.rglob("auth.json"))

    zellij_out = log_dir / "zellij-out.txt"
    _run(
        [
            "nix",
            "build",
            f"{repo_root}#packages.x86_64-linux.zellij",
            "--no-link",
            "--print-build-logs",
            "--print-out-paths",
        ],
        stdout=zellij_out,
    )
    zellij_path = zellij_out.read_text(encoding="utf-8").splitlines()[-1]
    zellij_version = _run(
        [
            "bash",
            "-c",
            'source "$1"; nixome_run_store_path "$2/bin/zellij" --version',
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/lib/nixome-env.sh"),
            zellij_path,
        ],
        cwd=repo_root,
    ).stdout
    (log_dir / "zellij-version.txt").write_text(zellij_version, encoding="utf-8")
    assert re.search(r"^zellij [0-9]+\.[0-9]+\.[0-9]+$", zellij_version)


def check_config_candidate_nix_native(repo_root: Path, log_dir: Path) -> None:
    result_dir = log_dir
    work_dir = log_dir / "work"
    shutil.rmtree(result_dir, ignore_errors=True)
    (work_dir / "nixome").mkdir(parents=True)

    initial_config = repo_root / "lifecycle/bootstrap/initial-runtime-config"
    shutil.copytree(initial_config / "dom0-profile", work_dir / "dom0-profile")
    for name in (
        "flake.nix",
        "nixome/config-vms.nix",
        "nixome/dom0-host.nix",
        "nixome/local.nix",
        "nixome/prebuilt-template-sources.nix",
        "nixome/system.nix",
    ):
        shutil.copy(initial_config / name, work_dir / name)
    shutil.copy(
        repo_root / "lifecycle/misc/lib/system.nix",
        work_dir / "nixome/system-schema.nix",
    )
    shutil.copy(
        repo_root / "lifecycle/misc/lib/topology.nix",
        work_dir / "nixome/topology.nix",
    )

    replacements = {
        "__NIXOME_NIXPKGS_URL__": "path:"
        + _physical_store_path(
            repo_root,
            _run(
                ["nix", "eval", "--raw", f"{repo_root}#nixome.configInputs.nixpkgs"]
            ).stdout.strip(),
        ),
        "__NIXPKGS_LIBVIRT_10_5_PATH__": _physical_store_path(
            repo_root,
            _run(
                [
                    "nix",
                    "eval",
                    "--raw",
                    f"{repo_root}#nixome.configInputs.nixpkgsLibvirt105",
                ]
            ).stdout.strip(),
        ),
        "__NIXOME_NIXOS_QUBES_PATH__": _physical_store_path(
            repo_root,
            _run(
                ["nix", "eval", "--raw", f"{repo_root}#nixome.configInputs.nixosQubes"]
            ).stdout.strip(),
        ),
        "__NIXOME_SUPPORT_PATH__": _physical_store_path(
            repo_root,
            _run(
                [
                    "nix",
                    "eval",
                    "--raw",
                    f"{repo_root}#nixome.configInputs.nixomeSupport",
                ]
            ).stdout.strip(),
        ),
    }
    flake_path = work_dir / "flake.nix"
    flake_text = flake_path.read_text(encoding="utf-8")
    for needle, replacement in replacements.items():
        flake_text = flake_text.replace(needle, replacement)
    flake_path.write_text(flake_text, encoding="utf-8")

    _write_flake(
        work_dir / "nixome/runtime.nix",
        """
        { pkgs, ... }:
        {
          nixNative.domains.nix-native-generated = {
            label = "purple";
            netvm = null;
            hostName = "nix-native-generated";
            packages = with pkgs; [
              jq
            ];
            etc."nixome/nix-native-role".text = "generated\n";
            prefs.memory = 1280;
            features."nixome-role" = "nix-native-generated";
          };
        }
        """,
    )

    nix_flake_flags = [
        "nix",
        "--extra-experimental-features",
        "nix-command",
        "--extra-experimental-features",
        "flakes",
    ]
    _run([*nix_flake_flags, "flake", "lock", "--impure", "--offline"], cwd=work_dir)

    topology_json = result_dir / "topology.json"
    _run(
        [
            *nix_flake_flags,
            "eval",
            "--impure",
            "--offline",
            "--no-write-lock-file",
            "--json",
            f"{work_dir}#nixome.topology",
        ],
        stdout=topology_json,
    )
    hostname = _run(
        [
            *nix_flake_flags,
            "eval",
            "--impure",
            "--offline",
            "--no-write-lock-file",
            "--raw",
            f"{work_dir}#nixosConfigurations.nixome-domain-nix-native-generated.config.networking.hostName",
        ],
    ).stdout.strip()
    (result_dir / "hostname.txt").write_text(hostname + "\n", encoding="utf-8")
    etc_text = _run(
        [
            *nix_flake_flags,
            "eval",
            "--impure",
            "--offline",
            "--no-write-lock-file",
            "--raw",
            f'{work_dir}#nixosConfigurations.nixome-domain-nix-native-generated.config.environment.etc."nixome/nix-native-role".text',
        ],
    ).stdout.strip()
    (result_dir / "etc.txt").write_text(etc_text + "\n", encoding="utf-8")

    runtime_source = _run(
        [
            *nix_flake_flags,
            "build",
            "--impure",
            "--offline",
            "--no-write-lock-file",
            "--no-link",
            "--print-out-paths",
            "--expr",
            f'(builtins.getFlake "path:{work_dir}").nixosConfigurations.nixome-domain-nix-native-generated.config.environment.etc."qubes-rpc/nixome.NixStoreRuntimeCheck".source',
        ],
    ).stdout.strip()
    (result_dir / "runtime-check-source.txt").write_text(
        runtime_source + "\n", encoding="utf-8"
    )
    runtime_check = result_dir / "runtime-check.sh"
    shutil.copy(Path(_physical_store_path(repo_root, runtime_source)), runtime_check)
    runtime_text = runtime_check.read_text(encoding="utf-8")

    assert hostname == "nix-native-generated"
    assert etc_text == "generated"
    assert runtime_text.startswith("#!")
    for fragment in (
        "sed -n 's|^nixome.storedev=||p'",
        'mount_source="$(findmnt -rn -T /nix/store -o SOURCE)"',
        'case ",$mount_options," in',
        "*,ro,*)",
    ):
        assert fragment in runtime_text

    topology = json.loads(topology_json.read_text(encoding="utf-8"))
    domain = topology["domains"]["nix-native-generated"]
    assert domain["class"] == "StandaloneVM"
    assert domain["template"] is None
    assert domain["label"] == "purple"
    assert domain["netvm"] is None
    assert domain["prefs"]["memory"] == 1280
    assert domain["prefs"]["kernel"] == "6.12.64-1.fc41"
    assert "init=/sbin/init" in domain["prefs"]["kernelopts"]
    assert "nixome.system=/nix/store/" in domain["prefs"]["kernelopts"]
    assert "nixome.storedev=/dev/xvde" in domain["prefs"]["kernelopts"]
    assert domain["features"]["nixome-backend"] == "nix-closure"
    assert domain["features"]["nixome-role"] == "nix-native-generated"
    assert domain["backend"]["type"] == "nix-closure"
    assert (
        domain["backend"]["system"]["source_ref"]["attr"]
        == "nixosConfigurations.nixome-domain-nix-native-generated.config.system.build.toplevel"
    )
    assert domain["backend"]["root"]["mode"] == "qubes-root-image"
    assert (
        domain["backend"]["root"]["source_ref"]["attr"]
        == "packages.x86_64-linux.nixome-domain-nix-native-generated-root-image"
    )
    assert domain["backend"]["root"]["source_ref"]["subpath"] == "root.img"
    assert domain["backend"]["store"]["mode"] == "shared-closure"
    assert (
        domain["backend"]["store"]["source_ref"]["attr"]
        == "packages.x86_64-linux.nixome-domain-nix-native-generated-store-closure"
    )
    assert domain["backend"]["store"]["source_ref"]["subpath"] == ""
    assert domain["backend"]["store"]["mountpoint"] == "/nix/store"
    assert domain["backend"]["store"]["device"] == "/dev/xvde"
    assert domain["backend"]["store"]["read_only"] is True

    invalid_cases = [
        (
            """
            { ... }:
            {
              nixNative.domains.bad.pakages = [ ];
            }
            """,
            "nixome.nixNative.domains.bad: unknown option(s): pakages",
        ),
        (
            """
            { ... }:
            {
              nixNative.domains.bad.role = "sys-config-staging";
            }
            """,
            "nixome.nixNative.domains.bad.role: expected one of base, dev",
        ),
    ]
    invalid_log = result_dir / "invalid.log"
    for runtime_expr, expected in invalid_cases:
        _write_flake(work_dir / "nixome/runtime.nix", runtime_expr)
        result = _run(
            [
                *nix_flake_flags,
                "eval",
                "--impure",
                "--offline",
                "--no-write-lock-file",
                "--json",
                f"{work_dir}#nixome.topology",
            ],
            stdout=result_dir / "invalid.json",
            stderr=invalid_log,
            check=False,
        )
        assert result.returncode != 0
        assert expected in invalid_log.read_text(encoding="utf-8")


def check_nixome_minimal_guest(repo_root: Path, log_dir: Path) -> None:
    shutil.rmtree(log_dir, ignore_errors=True)
    log_dir.mkdir(parents=True)
    result_json = log_dir / "result.json"
    expr = f"""
    let
      flake = builtins.getFlake "path:{repo_root}";
      lib = flake.inputs.nixpkgs.lib;
      cfgs = {{
        staging = flake.nixosConfigurations."nixome-domain-sys-config-staging".config;
        validator = flake.nixosConfigurations."nixome-domain-sys-config-validator".config;
        vault = flake.nixosConfigurations."nixome-domain-sys-config-vault".config;
      }};
      cfgList = builtins.attrValues cfgs;
      packageNames = cfg: map (package: package.pname or package.name or "") cfg.environment.systemPackages;
      corePackageNames = cfg: map (package: package.pname or package.name or "") cfg.environment.corePackages;
      enabledServices =
        cfg:
        lib.attrNames (
          lib.filterAttrs (_name: service: service.enable or false) cfg.systemd.services
        );
      etcNames = cfg: lib.attrNames cfg.environment.etc;
      has = name: values: builtins.elem name values;
      allCfgs = pred: lib.all pred cfgList;
      packagesAbsent =
        names: cfg:
        lib.all (name: !(has name (packageNames cfg))) names;
      noConfigInputEtc =
        cfg:
        lib.all (name: !(lib.hasPrefix "nixome/config-inputs/" name)) (etcNames cfg);
    in
    {{
      package_names = lib.mapAttrs (_name: cfg: packageNames cfg) cfgs;
      core_package_names = lib.mapAttrs (_name: cfg: corePackageNames cfg) cfgs;
      enabled_services = lib.mapAttrs (_name: cfg: enabledServices cfg) cfgs;
      service_counts = lib.mapAttrs (_name: cfg: builtins.length (enabledServices cfg)) cfgs;
      minimal_enabled = allCfgs (cfg: cfg.nixome.minimalGuest.enable);
      default_packages_empty = allCfgs (cfg: cfg.environment.defaultPackages == [ ]);
      documentation_disabled = allCfgs (
        cfg:
        cfg.documentation.enable == false
        && cfg.documentation.doc.enable == false
        && cfg.documentation.info.enable == false
        && cfg.documentation.man.enable == false
        && cfg.documentation.nixos.enable == false
      );
      perlless_mechanics_enabled = allCfgs (
        cfg:
        cfg.system.etc.overlay.enable == true
        && cfg.services.userborn.enable == true
      );
      installer_tools_disabled = allCfgs (cfg: cfg.system.disableInstallerTools);
      no_installer_tool_packages = allCfgs (
        packagesAbsent [
          "nixos-build-vms"
          "nixos-enter"
          "nixos-generate-config"
          "nixos-install"
          "nixos-option"
        ]
      );
      nix_cli_capability = {{
        staging = cfgs.staging.nixome.minimalGuest.nix.enable;
        validator = cfgs.validator.nixome.minimalGuest.nix.enable;
        vault = cfgs.vault.nixome.minimalGuest.nix.enable;
      }};
      staging_has_config_nix = has "nix" (packageNames cfgs.staging) && has "nixome-config-nix" (packageNames cfgs.staging);
      validator_has_config_nix = has "nix" (packageNames cfgs.validator) && has "nixome-config-nix" (packageNames cfgs.validator);
      vault_has_config_nix = has "nixome-config-nix" (packageNames cfgs.vault);
      stock_runtime_retained = allCfgs (
        cfg:
        (cfg.systemd.package.pname or "") == "systemd"
        && cfg.services.dbus.enable == true
        && cfg.networking.useDHCP == true
      );
      no_guest_config_input_etc = allCfgs noConfigInputEtc;
      required_qubes_services_retained = allCfgs (
        cfg:
        lib.all (name: has name (enabledServices cfg)) [
          "nixome-qubes-rw-init"
          "qubes-db"
          "qubes-qrexec-agent"
          "qubes-sysinit"
        ]
      );
      config_inputs_import_rpc = {{
        staging = cfgs.staging.environment.etc ? "qubes-rpc/nixome.Config.InputsImport";
        validator = cfgs.validator.environment.etc ? "qubes-rpc/nixome.Config.InputsImport";
        vault = cfgs.vault.environment.etc ? "qubes-rpc/nixome.Config.InputsImport";
      }};
      qubes_runtime_packages_retained = allCfgs (
        cfg:
        has "qubes-core-agent-linux" (packageNames cfg)
        && has "qubes-linux-utils" (packageNames cfg)
        && has "qubes-core-qrexec-base" (packageNames cfg)
        && has "qubes-core-qrexec-domU" (packageNames cfg)
      );
    }}
    """
    _run(["nix", "eval", "--impure", "--json", "--expr", expr], stdout=result_json)
    data = json.loads(result_json.read_text(encoding="utf-8"))
    assert data["minimal_enabled"] is True
    assert data["default_packages_empty"] is True
    assert data["documentation_disabled"] is True
    assert data["perlless_mechanics_enabled"] is True
    assert data["installer_tools_disabled"] is True
    assert data["no_installer_tool_packages"] is True
    assert data["nix_cli_capability"] == {
        "staging": True,
        "validator": True,
        "vault": True,
    }
    assert data["staging_has_config_nix"] is True
    assert data["validator_has_config_nix"] is True
    assert data["vault_has_config_nix"] is True
    assert data["stock_runtime_retained"] is True
    assert data["no_guest_config_input_etc"] is True
    assert data["required_qubes_services_retained"] is True
    assert data["config_inputs_import_rpc"] == {
        "staging": True,
        "validator": True,
        "vault": False,
    }
    assert data["qubes_runtime_packages_retained"] is True


def check_nixome_minimal_closure(repo_root: Path, log_dir: Path) -> None:
    shutil.rmtree(log_dir, ignore_errors=True)
    log_dir.mkdir(parents=True)
    roles = ("sys-config-staging", "sys-config-validator", "sys-config-vault")
    paths = {}
    for role in roles:
        attr = f"{repo_root}#nixosConfigurations.nixome-domain-{role}.config.system.build.toplevel"
        paths[role] = _run(
            [
                "nix",
                "build",
                "--no-link",
                "--print-out-paths",
                attr,
                "--print-build-logs",
                "--show-trace",
            ]
        ).stdout.strip()
        closure_file = log_dir / f"closure-{role}.txt"
        _run(["nix", "path-info", "-r", paths[role]], stdout=closure_file)

    def closure(role: str) -> str:
        return (log_dir / f"closure-{role}.txt").read_text(encoding="utf-8")

    def assert_has(role: str, pattern: str, description: str) -> None:
        assert re.search(
            pattern, closure(role)
        ), f"missing expected {description} in {role} closure: {pattern}"

    def assert_absent(role: str, pattern: str, description: str) -> None:
        assert not re.search(
            pattern, closure(role)
        ), f"unexpected {description} in {role} closure: {pattern}"

    for role in roles:
        assert_has(
            role,
            r"/nix/store/[[:alnum:]]+-qubes-core-qrexec-domU-".replace(
                "[[:alnum:]]", "[A-Za-z0-9]"
            ),
            "qrexec domU runtime",
        )
        assert_has(
            role,
            r"/nix/store/[A-Za-z0-9]+-qubes-core-qubesdb-client-",
            "QubesDB client",
        )
        assert_has(
            role,
            r"/nix/store/[A-Za-z0-9]+-qubes-core-qubesdb-daemon-",
            "QubesDB daemon",
        )
        assert_has(
            role,
            r"/nix/store/[A-Za-z0-9]+-qubes-core-agent-linux-[0-9]",
            "upstream Qubes core agent package",
        )
        assert_has(
            role,
            r"/nix/store/[A-Za-z0-9]+-python[^/]*-qubes-core-qrexec",
            "upstream Python qrexec package",
        )
        assert_has(
            role, r"/nix/store/[A-Za-z0-9]+-systemd-[0-9]", "full systemd package"
        )
        assert_has(
            role,
            r"/nix/store/[A-Za-z0-9]+-userborn-",
            "userborn passwd/group generator",
        )
        assert_absent(
            role,
            r"/nix/store/[A-Za-z0-9]+-nixos-(build-vms|enter|generate-config|install|option)",
            "NixOS installer/admin tooling",
        )

    for role in ("sys-config-vault", "sys-config-staging", "sys-config-validator"):
        assert_has(role, r"/nix/store/[A-Za-z0-9]+-nix-[0-9]", "role-specific Nix CLI")
        assert_has(
            role,
            r"/nix/store/[A-Za-z0-9]+-nix-flake",
            "role-specific nix flake support",
        )
        assert_has(
            role,
            r"/nix/store/[A-Za-z0-9]+-nixome-config-nix",
            "role-specific Nixome Nix helper",
        )

    role_reports = []
    for role in roles:
        closure_lines = closure(role).splitlines()
        path_info = json.loads(
            _run(
                ["nix", "path-info", "--json-format", "1", "--json", "-S", paths[role]]
            ).stdout
        )
        role_reports.append(
            {
                "role": role,
                "path": paths[role],
                "store_paths": len(closure_lines),
                "closure_size_bytes": next(iter(path_info.values()))["closureSize"],
            }
        )
    (log_dir / "result.json").write_text(
        json.dumps(
            {
                "schema": "nixome.minimal-closure-test.v1",
                "status": "passed",
                "roles": role_reports,
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


def _git_commit_all(repo: Path, message: str, *, name: str, email: str) -> str:
    _run(["git", "init", "-b", "main"], cwd=repo)
    _run(["git", "add", "."], cwd=repo)
    _run(
        [
            "git",
            "-c",
            f"user.name={name}",
            "-c",
            f"user.email={email}",
            "commit",
            "-m",
            message,
        ],
        cwd=repo,
    )
    return _run(["git", "rev-parse", "HEAD"], cwd=repo).stdout.strip()


def check_config_staging_validate(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    candidate_dir = work_dir / "candidate"
    validation_dir = work_dir / "validation"
    shutil.rmtree(work_dir, ignore_errors=True)
    (candidate_dir / "dom0-profile/bin").mkdir(parents=True)
    (candidate_dir / "dom0-profile/share/nixome").mkdir(parents=True)
    validation_dir.mkdir()

    _write_executable(
        candidate_dir / "dom0-profile/bin/nixome-dom0-profile-activate",
        """
        #!/bin/sh
        set -eu
        exit 0
        """,
    )
    (candidate_dir / "dom0-profile/share/nixome/profile.json").write_text(
        '{"schema":"nixome.dom0-profile.v1","message":"staging-validate-test"}\n',
        encoding="utf-8",
    )
    _write_flake(
        candidate_dir / "flake.nix",
        """
        {
          description = "Nixome staging validation fixture";

          outputs = { self }: {
            nixome.topology = {
              schema = "nixome.qubes-topology.v1";
              templates = { };
              kernels = { };
              domains = { };
              prefs = { };
              qrexec_policy.rules = [ ];
            };

            nixome.system = {
              schema = "nixome.system.v1";
              dom0 = {
                profile = {
                  source = "dom0-profile";
                  source_ref = null;
                  activation_command = "bin/nixome-dom0-profile-activate";
                  activation_action = "none";
                  message = "staging-validate-test";
                };
                packages = [ ];
                systemd.services = { };
                nixos = {
                  system = {
                    enable = false;
                    reason = "no host switch in staging validation fixture";
                    source = null;
                    source_ref = null;
                    switch_action = "switch";
                  };
                  kernel = {
                    enable = false;
                    reason = "no kernel change";
                    source = null;
                    source_ref = null;
                  };
                  xen = {
                    enable = false;
                    reason = "no xen change";
                    source = null;
                    source_ref = null;
                  };
                  bootloader = {
                    enable = false;
                    reason = "no bootloader change";
                    source = null;
                    source_ref = null;
                  };
                  qubes = {
                    enable = false;
                    reason = "no qubes package change";
                    source = null;
                    source_ref = null;
                  };
                };
              };
              plan = {
                live_actions = [ ];
                reboot_actions = [ ];
                reboot_required = false;
                reboot_reasons = [ ];
              };
            };
          };
        }
        """,
    )
    commit = _git_commit_all(
        candidate_dir,
        "staging validation fixture",
        name="Nixome Staging Test",
        email="nixome-staging-test@localhost",
    )
    validator_package = _run(
        [
            "nix",
            "build",
            "--no-link",
            "--print-out-paths",
            f'{repo_root}#nixosConfigurations.nixome-domain-sys-config-staging.config.environment.etc."nixome/guest-tools/nixome-config-staging-validate".source',
        ]
    ).stdout.strip()
    validator_tool = f"{validator_package}/bin/nixome-config-staging-validate"
    env = os.environ.copy()
    env.update(
        {
            "NIXOME_CONFIG_REPO_PATH": str(candidate_dir),
            "NIXOME_CONFIG_VALIDATION_DIR": str(validation_dir),
            "NIXOME_CONFIG_NIX_STORE_ROOT": str(work_dir / "nix-root"),
        }
    )
    command = [
        "bash",
        "-c",
        'source "$1"; nixome_run_store_path "$2"',
        "bash",
        str(repo_root / "lifecycle/misc/tools/scripts/lib/nixome-env.sh"),
        validator_tool,
    ]
    _run(
        command,
        cwd=repo_root,
        env=env,
        stdout=work_dir / "stdout.log",
        stderr=work_dir / "stderr.log",
    )

    output = json.loads((work_dir / "stdout.log").read_text(encoding="utf-8"))
    assert output["schema"] == "nixome.config-validation.v1"
    assert output["status"] == "passed"
    assert output["commit"] == commit
    assert output["repo"] == str(candidate_dir)
    assert output["validated_candidate"] == str(
        validation_dir / f"validated-candidate-{commit}.tar"
    )
    checks = {check["name"]: check for check in output["checks"]}
    expected_checks = {
        "nix-eval-nixome-topology": "output",
        "nixome-topology-json": "path",
        "nix-eval-nixome-system": "output",
        "nixome-system-json": "path",
        "nix-system-source-refs-dry-run": None,
        "nix-flake-check-no-build": None,
        "nix-flake-check": None,
    }
    for name, path_key in expected_checks.items():
        assert checks[name]["status"] == "passed"
        if path_key == "output" and "topology" in name:
            assert checks[name][path_key] == str(
                validation_dir / f"topology-{commit}.json"
            )
        elif path_key == "path" and "topology" in name:
            assert checks[name][path_key] == str(
                validation_dir / f"topology-{commit}.json"
            )
        elif path_key == "output":
            assert checks[name][path_key] == str(
                validation_dir / f"system-{commit}.json"
            )
        elif path_key == "path":
            assert checks[name][path_key] == str(
                validation_dir / f"system-{commit}.json"
            )

    validation = json.loads(
        (validation_dir / f"{commit}.json").read_text(encoding="utf-8")
    )
    topology = json.loads(
        (validation_dir / f"topology-{commit}.json").read_text(encoding="utf-8")
    )
    system = json.loads(
        (validation_dir / f"system-{commit}.json").read_text(encoding="utf-8")
    )
    assert validation["schema"] == "nixome.config-validation.v1"
    assert validation["status"] == "passed"
    assert topology["schema"] == "nixome.qubes-topology.v1"
    assert topology["domains"] == {}
    assert system["schema"] == "nixome.system.v1"
    assert system["dom0"]["profile"]["message"] == "staging-validate-test"
    with tarfile.open(validation_dir / f"validated-candidate-{commit}.tar") as tar:
        members = sorted(tar.getnames())
    for member in (
        "config.bundle",
        "manifest.json",
        "validation.json",
        "topology.json",
        "system.json",
    ):
        assert member in members

    (candidate_dir / "README.md").write_text("dirty\n", encoding="utf-8")
    result = _run(
        command,
        cwd=repo_root,
        env=env,
        stdout=work_dir / "dirty-stdout.log",
        stderr=work_dir / "dirty-stderr.log",
        check=False,
    )
    assert result.returncode != 0
    dirty = json.loads((work_dir / "dirty-stdout.log").read_text(encoding="utf-8"))
    assert dirty["schema"] == "nixome.config-validation.v1"
    assert dirty["status"] == "failed"
    assert dirty["validator_role"] == "config-staging-local"
    assert any(
        check["name"] == "git-clean" and check["status"] == "failed"
        for check in dirty["checks"]
    )


def check_config_vault_deployment_archive(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    vault_repo = work_dir / "vault"
    deployment_dir = work_dir / "deployment"
    deployment_tar = work_dir / "deployment.tar"
    candidate_commit = "0123456789abcdef0123456789abcdef01234567"
    deployment_id = "20260907T000000Z-0123456789ab"
    shutil.rmtree(work_dir, ignore_errors=True)
    (vault_repo / f"candidates/{candidate_commit}").mkdir(parents=True)
    (deployment_dir / "logs").mkdir(parents=True)
    (vault_repo / f"candidates/{candidate_commit}/README.md").write_text(
        "candidate placeholder\n", encoding="utf-8"
    )
    _git_commit_all(
        vault_repo,
        "archive candidate fixture",
        name="Nixome Vault Deployment Test",
        email="nixome-vault-deployment-test@localhost",
    )

    artifacts = {
        "topology": "topology.json",
        "system": "system.json",
        "resolved_system": "resolved-system.json",
        "system_plan": "system-plan.json",
        "system_apply": "system-apply.json",
        "system_builds": "system-builds.json",
        "systemd_actions": "systemd-actions.json",
        "apply": "apply.json",
        "archive": "archive.json",
        "setup": "setup.json",
        "template_builds": "template-builds.json",
        "nix_domain_builds": "nix-domain-builds.json",
        "nix_domain_materializations": "nix-domain-materializations.json",
        "unsupported_domain_backends": "unsupported-domain-backends.json",
        "validated_topology": "validated-topology.json",
        "validation": "validation.json",
        "manifest": "manifest.json",
    }
    deployment_payloads = {
        "deployment.json": {
            "schema": "nixome.config-deployment.v1",
            "status": "passed",
            "reason": "test deployment",
            "deployment_id": deployment_id,
            "deployed_at": "20260907T000000Z",
            "deployed_by": "dom0",
            "staging": "sys-config-staging",
            "validator": "sys-config-validator",
            "vault": "sys-config-vault",
            "candidate_commit": candidate_commit,
            "artifacts": artifacts,
        },
        "apply.json": {
            "schema": "nixome.config-apply.v1",
            "status": "passed",
            "reason": "test apply",
            "candidate_commit": candidate_commit,
        },
        "archive.json": {
            "schema": "nixome.config-archive.v1",
            "status": "passed",
            "reason": "test archive",
            "candidate_commit": candidate_commit,
        },
        "setup.json": {
            "schema": "nixome.setup-config.v1",
            "status": "passed",
            "mode": "apply",
            "reason": "test setup",
        },
        "topology.json": {
            "schema": "nixome.qubes-topology.v1",
            "templates": {},
            "kernels": {},
            "domains": {},
            "prefs": {},
            "qrexec_policy": {"rules": []},
        },
        "template-builds.json": {"schema": "nixome.template-builds.v1", "builds": []},
        "nix-domain-builds.json": {
            "schema": "nixome.nix-domain-builds.v1",
            "builds": [],
        },
        "nix-domain-materializations.json": {
            "schema": "nixome.nix-domain-materializations.v1",
            "domains": [],
        },
        "unsupported-domain-backends.json": {
            "schema": "nixome.unsupported-domain-backends.v1",
            "domains": [],
        },
        "system.json": {
            "schema": "nixome.system.v1",
            "dom0": {
                "profile": {
                    "source": None,
                    "source_ref": None,
                    "activation_command": "bin/nixome-dom0-profile-activate",
                    "activation_action": "none",
                    "message": "",
                },
                "packages": [],
                "systemd": {"services": {}},
                "nixos": {
                    "system": {
                        "enable": False,
                        "reason": "",
                        "source": None,
                        "source_ref": None,
                        "switch_action": "switch",
                    },
                    "kernel": {
                        "enable": False,
                        "reason": "",
                        "source": None,
                        "source_ref": None,
                    },
                    "xen": {
                        "enable": False,
                        "reason": "",
                        "source": None,
                        "source_ref": None,
                    },
                    "bootloader": {
                        "enable": False,
                        "reason": "",
                        "source": None,
                        "source_ref": None,
                    },
                    "qubes": {
                        "enable": False,
                        "reason": "",
                        "source": None,
                        "source_ref": None,
                    },
                },
            },
            "plan": {
                "live_actions": [],
                "reboot_actions": [],
                "reboot_required": False,
                "reboot_reasons": [],
            },
        },
        "system-apply.json": {
            "schema": "nixome.system-apply.v1",
            "status": "passed",
            "reason": "test system apply",
        },
        "system-builds.json": {"schema": "nixome.system-builds.v1", "builds": []},
        "systemd-actions.json": {
            "schema": "nixome.systemd-actions.v1",
            "status": "passed",
            "reason": "test systemd actions",
            "actions": [],
        },
        "validation.json": {
            "schema": "nixome.config-validation.v1",
            "status": "passed",
            "reason": "test validation",
            "commit": candidate_commit,
            "checks": [],
        },
        "manifest.json": {
            "schema": "nixome.config-candidate.v1",
            "commit": candidate_commit,
            "branch": "main",
        },
    }
    for name, payload in deployment_payloads.items():
        (deployment_dir / name).write_text(
            json.dumps(payload, sort_keys=True) + "\n", encoding="utf-8"
        )
    shutil.copy(
        deployment_dir / "topology.json", deployment_dir / "validated-topology.json"
    )
    shutil.copy(deployment_dir / "system.json", deployment_dir / "resolved-system.json")
    (deployment_dir / "system-plan.json").write_text(
        json.dumps(deployment_payloads["system.json"]["plan"], sort_keys=True) + "\n",
        encoding="utf-8",
    )
    log_names = [
        "apply.commands.log",
        "archive.stdout.log",
        "archive.stderr.log",
        "setup.stdout.log",
        "setup.stderr.log",
        "template-builds.log",
        "nix-domain-builds.log",
        "system-builds.log",
        "system-apply.log",
        "profile-activation.log",
        "host-system-activation.log",
        "systemd-actions.log",
        "clone.log",
        "checkout.log",
    ]
    for name in log_names:
        (deployment_dir / f"logs/{name}").write_text(f"{name}\n", encoding="utf-8")
    with tarfile.open(deployment_tar, "w") as tar:
        for name in [
            "deployment.json",
            "apply.json",
            "archive.json",
            "setup.json",
            "topology.json",
            "validated-topology.json",
            "template-builds.json",
            "nix-domain-builds.json",
            "nix-domain-materializations.json",
            "unsupported-domain-backends.json",
            "system.json",
            "resolved-system.json",
            "system-plan.json",
            "system-apply.json",
            "system-builds.json",
            "systemd-actions.json",
            "validation.json",
            "manifest.json",
            *[f"logs/{name}" for name in log_names],
        ]:
            tar.add(deployment_dir / name, arcname=name)

    archive_cmd = _run(
        [
            "nix",
            "build",
            "--no-link",
            "--print-out-paths",
            f'{repo_root}#nixosConfigurations.nixome-domain-sys-config-vault.config.environment.etc."qubes-rpc/nixome.Config.ArchiveDeployment".source',
        ]
    ).stdout.strip()
    env = os.environ.copy()
    env["NIXOME_CONFIG_ARCHIVE_PATH"] = str(vault_repo)
    with deployment_tar.open("rb") as stdin:
        result = subprocess.run(
            [
                "bash",
                "-c",
                'source "$1"; nixome_run_store_path "$2"',
                "bash",
                str(repo_root / "lifecycle/misc/tools/scripts/lib/nixome-env.sh"),
                archive_cmd,
            ],
            cwd=repo_root,
            env=env,
            check=False,
            stdin=stdin,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    (work_dir / "result.json").write_bytes(result.stdout)
    (work_dir / "result.stderr.log").write_bytes(result.stderr)
    assert result.returncode == 0
    archive_result = json.loads(result.stdout.decode("utf-8"))
    assert archive_result["schema"] == "nixome.config-vault-deployment.v1"
    assert archive_result["status"] == "passed"
    assert archive_result["deployment_status"] == "passed"
    assert archive_result["deployment_id"] == deployment_id
    assert archive_result["candidate_commit"] == candidate_commit

    archived_dir = vault_repo / f"deployments/{deployment_id}"
    for rel_path in (
        "systemd-actions.json",
        "nix-domain-builds.json",
        "nix-domain-materializations.json",
        "unsupported-domain-backends.json",
        "logs/host-system-activation.log",
        "logs/systemd-actions.log",
    ):
        assert (archived_dir / rel_path).is_file()
    assert (vault_repo / "latest-deployment").read_text(
        encoding="utf-8"
    ).strip() == deployment_id
    assert (
        _run(["git", "status", "--short", "--branch"], cwd=vault_repo).stdout.strip()
        == "## main"
    )


def check_setup_config_static(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    kernel_dir = work_dir / "kernel/6.12.64-1.fc41"
    template_dir = work_dir / "template/fedora-44-xfce"
    setup_log_dir = work_dir / "logs"
    kernel_base = work_dir / "vm-kernels"
    policy_dir = work_dir / "policy.d"
    capabilities = work_dir / "xen-capabilities"
    topology = work_dir / "topology.json"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (kernel_dir, template_dir, setup_log_dir, policy_dir):
        path.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    for name in ("vmlinuz", "initramfs", "modules.img"):
        (kernel_dir / name).touch()
    (template_dir / "root.img").touch()
    (template_dir / "template.conf").write_text(
        "qrexec=1\nvirt-mode=pvh\n", encoding="utf-8"
    )
    topology.write_text(
        json.dumps(
            {
                "schema": "nixome.qubes-topology.v1",
                "templates": {
                    "fedora-44-xfce": {
                        "import": True,
                        "skip_start": True,
                        "source": str(template_dir),
                    }
                },
                "kernels": {"6.12.64-1.fc41": {"source": str(kernel_dir)}},
                "domains": {
                    "legacy-fedora-app": {
                        "class": "AppVM",
                        "template": "fedora-44-xfce",
                        "label": "green",
                        "netvm": None,
                        "prefs": {"autostart": False, "memory": 1024},
                        "features": {"nixome-role": "legacy-fedora-app"},
                        "services": {"network-manager": False},
                    }
                },
                "prefs": {"default_kernel": ""},
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    unexpected = """
    #!/usr/bin/env bash
    printf 'unexpected command execution: %s\n' "$0" >&2
    exit 97
    """
    commands = {
        name: unexpected
        for name in (
            "qubes-prefs",
            "qvm-appmenus",
            "qvm-create",
            "qvm-features",
            "qvm-prefs",
            "qvm-service",
            "qvm-template-postprocess",
            "qvm-volume",
            "qubesd-query",
            "runuser",
        )
    }
    commands["qvm-ls"] = "#!/usr/bin/env bash\nexit 0\n"
    commands["systemctl"] = """
    #!/usr/bin/env bash
    if [ "$1" = "is-active" ]; then
      exit 0
    fi
    exit 1
    """
    _setup_fake_command_dir(bin_dir, commands)
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "NIXOME_QUBES_KERNEL_BASE": str(kernel_base),
            "NIXOME_QUBES_POLICY_DIR": str(policy_dir),
            "NIXOME_XEN_CAPABILITIES": str(capabilities),
            "NIXOME_LOG_DIR": str(setup_log_dir),
        }
    )
    _run(
        [
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/setup-config.sh"),
            "--plan",
            "--topology",
            str(topology),
        ],
        cwd=repo_root,
        env=env,
        stdout=setup_log_dir / "stdout.log",
        stderr=setup_log_dir / "stderr.log",
    )
    result = json.loads(
        (setup_log_dir / "setup-config.json").read_text(encoding="utf-8")
    )
    assert result["schema"] == "nixome.setup-config.v1"
    assert result["status"] == "passed"
    assert result["mode"] == "plan"
    assert result["templates_changed"] == 1
    assert result["kernels_changed"] == 1
    assert result["domains_changed"] == 1
    assert result["prefs_changed"] == 1
    assert result["qrexec_policy_changed"] == 1


def check_setup_config_empty_global_pref(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    setup_log_dir = work_dir / "logs"
    policy_dir = work_dir / "policy.d"
    state_dir = work_dir / "state"
    capabilities = work_dir / "xen-capabilities"
    mutation_log = work_dir / "mutations.log"
    topology = work_dir / "topology.json"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (setup_log_dir, policy_dir, state_dir):
        path.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    mutation_log.write_text("", encoding="utf-8")
    topology.write_text(
        json.dumps(
            {
                "schema": "nixome.qubes-topology.v1",
                "templates": {},
                "kernels": {},
                "domains": {},
                "prefs": {"default_kernel": ""},
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    commands = {
        "systemctl": """
        #!/usr/bin/env bash
        if [ "$1" = "is-active" ]; then
          exit 0
        fi
        if [ "$1" = "reload" ] || [ "$1" = "try-restart" ]; then
          exit 0
        fi
        exit 1
        """,
        "qvm-ls": "#!/usr/bin/env bash\nprintf 'dom0\n'\n",
        "qubes-prefs": """
        #!/usr/bin/env bash
        if [ "$#" -eq 1 ] && [ "$1" = "default_kernel" ]; then
          printf '\n'
          exit 0
        fi
        {
          printf 'qubes-prefs'
          for arg in "$@"; do
            printf ' <%s>' "$arg"
          done
          printf '\n'
        } >> "$MUTATION_LOG"
        """,
    }
    unexpected = """
    #!/usr/bin/env bash
    printf 'unexpected command execution: %s\n' "$0" >&2
    exit 97
    """
    for name in (
        "qvm-create",
        "qvm-features",
        "qvm-prefs",
        "qvm-service",
        "qvm-template-postprocess",
        "qvm-volume",
        "qubesd-query",
    ):
        commands[name] = unexpected
    _setup_fake_command_dir(bin_dir, commands)
    for command in ("cmp", "grep", "jq"):
        path = shutil.which(command)
        if path is not None:
            (bin_dir / command).symlink_to(path)
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "MUTATION_LOG": str(mutation_log),
            "NIXOME_XEN_CAPABILITIES": str(capabilities),
            "NIXOME_QUBES_POLICY_DIR": str(policy_dir),
            "NIXOME_SYSTEM_STATE_DIR": str(state_dir),
            "NIXOME_LOG_DIR": str(setup_log_dir),
        }
    )
    _run(
        [
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/setup-config.sh"),
            "--apply",
            "--topology",
            str(topology),
        ],
        cwd=repo_root,
        env=env,
        stdout=setup_log_dir / "stdout.log",
        stderr=setup_log_dir / "stderr.log",
    )
    result = json.loads(
        (setup_log_dir / "setup-config.json").read_text(encoding="utf-8")
    )
    assert result["schema"] == "nixome.setup-config.v1"
    assert result["status"] == "passed"
    assert result["mode"] == "apply"
    assert result["prefs_changed"] == 1
    assert result["qrexec_policy_changed"] == 1
    current = json.loads((state_dir / "current-setup.json").read_text(encoding="utf-8"))
    assert current["schema"] == "nixome.setup-config.v1"
    assert current["status"] == "passed"
    assert current["prefs_changed"] == 1
    assert (
        "qubes-prefs <default_kernel> <>"
        in mutation_log.read_text(encoding="utf-8").splitlines()
    )


def check_setup_config_pref_order(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    setup_log_dir = work_dir / "logs"
    policy_dir = work_dir / "policy.d"
    state_dir = work_dir / "state"
    capabilities = work_dir / "xen-capabilities"
    mutation_log = work_dir / "mutations.log"
    memory_marker = work_dir / "memory-set"
    qubes_xml = work_dir / "qubes.xml"
    topology = work_dir / "topology.json"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (setup_log_dir, policy_dir, state_dir):
        path.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    mutation_log.write_text("", encoding="utf-8")
    qubes_xml.write_text(
        """
        <qubes><domains><domain><properties>
          <property name="name">sys-config-staging</property>
          <property name="netvm"></property>
        </properties></domain></domains></qubes>
        """,
        encoding="utf-8",
    )
    topology.write_text(
        json.dumps(
            {
                "schema": "nixome.qubes-topology.v1",
                "templates": {},
                "kernels": {},
                "domains": {
                    "sys-config-staging": {
                        "class": "AppVM",
                        "template": "fedora-44-xfce",
                        "label": "gray",
                        "netvm": None,
                        "prefs": {
                            "autostart": False,
                            "maxmem": 4096,
                            "memory": 2048,
                            "vcpus": 2,
                        },
                        "features": {},
                        "services": {},
                    }
                },
                "prefs": {},
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    commands = {
        "qvm-ls": """
        #!/usr/bin/env bash
        case "$*" in
          *NAME,STATE*)
            printf 'dom0|Running\nconfig-staging|Halted\n'
            ;;
          *)
            printf 'dom0\nconfig-staging\n'
            ;;
        esac
        """,
        "qvm-prefs": """
        #!/usr/bin/env bash
        if [ "$#" -eq 2 ]; then
          case "$1:$2" in
            sys-config-staging:label) printf 'gray\n' ;;
            sys-config-staging:netvm) printf 'none\n' ;;
            sys-config-staging:autostart) printf 'True\n' ;;
            sys-config-staging:memory) printf '400\n' ;;
            sys-config-staging:maxmem) printf '4000\n' ;;
            sys-config-staging:vcpus) printf '1\n' ;;
            *) exit 1 ;;
          esac
          exit 0
        fi
        if [ "$#" -eq 3 ] && [ "$1" = "sys-config-staging" ]; then
          if [ "$2" = "maxmem" ] && [ ! -e "$MEMORY_SET_MARKER" ]; then
            printf 'maxmem was set before memory\n' >&2
            exit 42
          fi
          if [ "$2" = "memory" ]; then
            : > "$MEMORY_SET_MARKER"
          fi
          printf 'qvm-prefs %s\n' "$*" >> "$MUTATION_LOG"
          exit 0
        fi
        exit 1
        """,
        "systemctl": """
        #!/usr/bin/env bash
        case "$1" in
          is-active|reload|try-restart)
            exit 0
            ;;
        esac
        exit 1
        """,
    }
    for name in (
        "qubes-prefs",
        "qvm-create",
        "qvm-features",
        "qvm-service",
        "qvm-template-postprocess",
        "qvm-volume",
        "qubesd-query",
    ):
        commands[name] = "#!/usr/bin/env bash\nexit 0\n"
    _setup_fake_command_dir(bin_dir, commands)
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "MEMORY_SET_MARKER": str(memory_marker),
            "MUTATION_LOG": str(mutation_log),
            "NIXOME_QUBES_XML": str(qubes_xml),
            "NIXOME_QUBES_POLICY_DIR": str(policy_dir),
            "NIXOME_SYSTEM_STATE_DIR": str(state_dir),
            "NIXOME_XEN_CAPABILITIES": str(capabilities),
            "NIXOME_LOG_DIR": str(setup_log_dir),
        }
    )
    _run(
        [
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/setup-config.sh"),
            "--apply",
            "--topology",
            str(topology),
        ],
        cwd=repo_root,
        env=env,
        stdout=setup_log_dir / "stdout.log",
        stderr=setup_log_dir / "stderr.log",
    )
    command_lines = (
        (setup_log_dir / "setup-config.commands.log")
        .read_text(encoding="utf-8")
        .splitlines()
    )
    assert command_lines.index("+ qvm-prefs sys-config-staging memory 2048") < (
        command_lines.index("+ qvm-prefs sys-config-staging maxmem 4096")
    )
    result = json.loads(
        (setup_log_dir / "setup-config.json").read_text(encoding="utf-8")
    )
    assert result["schema"] == "nixome.setup-config.v1"
    assert result["status"] == "passed"
    assert result["prefs_changed"] == 4


def check_setup_config_external_netvm(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    setup_log_dir = work_dir / "logs"
    policy_dir = work_dir / "policy.d"
    capabilities = work_dir / "xen-capabilities"
    mutation_log = work_dir / "mutations.log"
    topology = work_dir / "topology.json"
    shutil.rmtree(work_dir, ignore_errors=True)
    setup_log_dir.mkdir(parents=True)
    policy_dir.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    mutation_log.write_text("", encoding="utf-8")
    topology.write_text(
        json.dumps(
            {
                "schema": "nixome.qubes-topology.v1",
                "templates": {},
                "kernels": {},
                "domains": {
                    "dev": {
                        "class": "StandaloneVM",
                        "template": None,
                        "label": "green",
                        "netvm": "sys-firewall",
                        "prefs": {"autostart": False},
                        "features": {"nixome-role": "dev"},
                        "services": {},
                    }
                },
                "prefs": {"default_kernel": ""},
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    commands = {
        "qvm-ls": """
        #!/usr/bin/env bash
        case "$*" in
          "--raw-data --fields NAME --no-spinner")
            printf 'dom0\n'
            printf 'dev\n'
            ;;
          "--raw-data --fields NAME,STATE --no-spinner")
            printf 'dom0|Running\n'
            printf 'dev|Halted\n'
            ;;
          "--raw-data --fields NAME,CLASS --no-spinner")
            printf 'dom0|AdminVM\n'
            printf 'dev|StandaloneVM\n'
            ;;
          *)
            printf 'unexpected qvm-ls args: %s\n' "$*" >&2
            exit 1
            ;;
        esac
        """,
        "systemctl": """
        #!/usr/bin/env bash
        if [ "$1" = "is-active" ]; then
          exit 0
        fi
        if [ "$1" = "reload" ] || [ "$1" = "try-restart" ]; then
          printf 'systemctl %s\n' "$*" >> "$MUTATION_LOG"
          exit 0
        fi
        exit 1
        """,
        "qubes-prefs": """
        #!/usr/bin/env bash
        if [ "$#" -eq 1 ] && [ "$1" = "default_kernel" ]; then
          printf '\n'
          exit 0
        fi
        printf 'qubes-prefs %s\n' "$*" >> "$MUTATION_LOG"
        exit 0
        """,
        "qvm-create": """
        #!/usr/bin/env bash
        printf 'qvm-create %s\n' "$*" >> "$MUTATION_LOG"
        exit 0
        """,
        "qvm-prefs": """
        #!/usr/bin/env bash
        if [ "$#" -eq 2 ]; then
          case "$1:$2" in
            dev:label) printf 'green\n' ;;
            dev:autostart) printf 'False\n' ;;
            *) exit 1 ;;
          esac
          exit 0
        fi
        if [ "$#" -eq 3 ] && [ "$1:$2:$3" = "dev:netvm:sys-firewall" ]; then
          printf 'setup-config should defer missing external sys-firewall instead of setting it\n' >&2
          exit 97
        fi
        printf 'qvm-prefs %s\n' "$*" >> "$MUTATION_LOG"
        exit 0
        """,
    }
    for name in (
        "qvm-appmenus",
        "qvm-features",
        "qvm-service",
        "qvm-template-postprocess",
        "qvm-volume",
        "qubesd-query",
        "runuser",
    ):
        commands[name] = """
        #!/usr/bin/env bash
        printf '%s %s\n' "$(basename "$0")" "$*" >> "$MUTATION_LOG"
        exit 0
        """
    _setup_fake_command_dir(bin_dir, commands)
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "MUTATION_LOG": str(mutation_log),
            "NIXOME_QUBES_POLICY_DIR": str(policy_dir),
            "NIXOME_SETUP_CONFIG_PERSIST_STATE": "0",
            "NIXOME_XEN_CAPABILITIES": str(capabilities),
            "NIXOME_LOG_DIR": str(setup_log_dir),
        }
    )
    _run(
        [
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/setup-config.sh"),
            "--apply",
            "--topology",
            str(topology),
        ],
        cwd=repo_root,
        env=env,
        stdout=setup_log_dir / "stdout.log",
        stderr=setup_log_dir / "stderr.log",
    )
    result = json.loads(
        (setup_log_dir / "setup-config.json").read_text(encoding="utf-8")
    )
    assert result["schema"] == "nixome.setup-config.v1"
    assert result["status"] == "passed"
    assert result["domains_changed"] == 0
    assert result["prefs_changed"] == 1
    assert any(
        action["type"] == "domain-pref"
        and action["action"] == "defer"
        and action["domain"] == "dev"
        and action["field"] == "netvm"
        and action["value"] == "sys-firewall"
        for action in result["live_actions"]
    )
    assert (
        "qvm-prefs dev netvm sys-firewall"
        not in mutation_log.read_text(encoding="utf-8").splitlines()
    )


def check_setup_config_auto_sys_net_pci(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    setup_log_dir = work_dir / "logs"
    policy_dir = work_dir / "policy.d"
    capabilities = work_dir / "xen-capabilities"
    topology = work_dir / "topology.json"
    qubesadmin_python = work_dir / "qubesadmin-python"
    qubesadmin_python_log = work_dir / "qubesadmin-python.log"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (setup_log_dir, policy_dir):
        path.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    qubesadmin_python_log.write_text("", encoding="utf-8")
    _write_executable(
        qubesadmin_python,
        """
        #!/usr/bin/env bash
        cat >> "$QUBESADMIN_PYTHON_LOG"
        printf '00_03.0\n00_04.0\n'
        """,
    )
    topology.write_text(
        json.dumps(
            {
                "schema": "nixome.qubes-topology.v1",
                "templates": {},
                "kernels": {},
                "domains": {
                    "sys-net": {
                        "class": "StandaloneVM",
                        "template": None,
                        "label": "red",
                        "netvm": None,
                        "prefs": {
                            "autostart": False,
                            "provides_network": True,
                        },
                        "features": {"nixome-role": "sys-net"},
                        "services": {
                            "network-manager": True,
                            "qubes-firewall": True,
                            "qubes-network": True,
                        },
                        "devices": {"pci": []},
                    }
                },
                "prefs": {},
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    commands = {
        "qvm-ls": """
        #!/usr/bin/env bash
        case "$*" in
          "--raw-data --fields NAME --no-spinner")
            printf 'dom0\nsys-net\n'
            ;;
          "--raw-data --fields NAME,STATE --no-spinner")
            printf 'dom0|Running\nsys-net|Halted\n'
            ;;
          *)
            printf 'unexpected qvm-ls args: %s\n' "$*" >&2
            exit 1
            ;;
        esac
        """,
        "systemctl": """
        #!/usr/bin/env bash
        if [ "$1" = "is-active" ]; then
          exit 0
        fi
        exit 1
        """,
        "ip": "#!/usr/bin/env bash\nexit 0\n",
        "readlink": "#!/usr/bin/env bash\nexit 0\n",
    }
    for name in (
        "qubes-prefs",
        "qvm-create",
        "qvm-device",
        "qvm-features",
        "qvm-prefs",
        "qvm-service",
        "qvm-template-postprocess",
        "qvm-volume",
        "qubesd-query",
    ):
        commands[name] = "#!/usr/bin/env bash\nexit 0\n"
    _setup_fake_command_dir(bin_dir, commands)
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "NIXOME_DOM0_DEFAULT_ROUTE_PCI_DEVICES": "00_03.0",
            "NIXOME_QUBESADMIN_PYTHON": str(qubesadmin_python),
            "QUBESADMIN_PYTHON_LOG": str(qubesadmin_python_log),
            "NIXOME_QUBES_POLICY_DIR": str(policy_dir),
            "NIXOME_XEN_CAPABILITIES": str(capabilities),
            "NIXOME_LOG_DIR": str(setup_log_dir),
        }
    )
    _run(
        [
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/setup-config.sh"),
            "--plan",
            "--topology",
            str(topology),
        ],
        cwd=repo_root,
        env=env,
        stdout=setup_log_dir / "stdout.log",
        stderr=setup_log_dir / "stderr.log",
    )
    result = json.loads(
        (setup_log_dir / "setup-config.json").read_text(encoding="utf-8")
    )
    command_lines = (
        (setup_log_dir / "setup-config.commands.log")
        .read_text(encoding="utf-8")
        .splitlines()
    )

    assert result["schema"] == "nixome.setup-config.v1"
    assert result["status"] == "passed"
    assert result["mode"] == "plan"
    assert result["devices_changed"] == 1
    assert any(
        action["type"] == "domain-device"
        and action["action"] == "assign"
        and action["domain"] == "sys-net"
        and action["field"] == "pci"
        and action["value"] == "dom0:00_04.0 mode=required match=port"
        for action in result["live_actions"]
    )
    assert (
        "+ qvm-device pci assign --required --port sys-net dom0:00_04.0"
        in command_lines
    )
    assert not any("dom0:00_03.0" in line for line in command_lines)
    qubesadmin_script = qubesadmin_python_log.read_text(encoding="utf-8")
    assert qubesadmin_script.count("import qubesadmin") == 2


def check_setup_config_sys_net_order(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    setup_log_dir = work_dir / "logs"
    policy_dir = work_dir / "policy.d"
    state_dir = work_dir / "state"
    capabilities = work_dir / "xen-capabilities"
    qubes_xml = work_dir / "qubes.xml"
    topology = work_dir / "topology.json"
    mutation_log = work_dir / "mutations.log"
    created_domains = work_dir / "created-domains"
    status_device = work_dir / "bootstrap-status"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (setup_log_dir, policy_dir, state_dir):
        path.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    mutation_log.write_text("", encoding="utf-8")
    created_domains.write_text("", encoding="utf-8")
    status_device.write_text("", encoding="utf-8")
    qubes_xml.write_text(
        """
        <qubes>
          <domains>
            <domain id="domain-1" class="StandaloneVM">
              <properties>
                <property name="name">sys-net</property>
              </properties>
            </domain>
          </domains>
        </qubes>
        """,
        encoding="utf-8",
    )
    topology.write_text(
        json.dumps(
            {
                "schema": "nixome.qubes-topology.v1",
                "templates": {},
                "kernels": {},
                "domains": {
                    "sys-config-staging": {
                        "class": "StandaloneVM",
                        "template": None,
                        "label": "gray",
                        "netvm": None,
                        "prefs": {"autostart": False, "guivm": None},
                        "features": {},
                        "services": {},
                    },
                    "dev": {
                        "class": "StandaloneVM",
                        "template": None,
                        "label": "green",
                        "netvm": "sys-firewall",
                        "prefs": {"autostart": False},
                        "features": {},
                        "services": {},
                    },
                    "sys-firewall": {
                        "class": "StandaloneVM",
                        "template": None,
                        "label": "green",
                        "netvm": "sys-net",
                        "prefs": {
                            "autostart": True,
                            "guivm": None,
                            "memory": 500,
                            "provides_network": True,
                        },
                        "features": {
                            "nixome-role": "sys-firewall",
                            "servicevm": "1",
                        },
                        "services": {
                            "network-manager": False,
                            "qubes-firewall": True,
                            "qubes-network": True,
                            "qubes-updates-proxy": True,
                        },
                        "devices": {"pci": []},
                    },
                    "sys-gui": {
                        "class": "StandaloneVM",
                        "template": None,
                        "label": "black",
                        "netvm": None,
                        "prefs": {
                            "autostart": True,
                            "guivm": "dom0",
                        },
                        "features": {"nixome-role": "sys-gui"},
                        "services": {
                            "guivm": True,
                            "guivm-gui-agent": True,
                            "network-manager": False,
                        },
                        "devices": {"pci": []},
                    },
                    "sys-net": {
                        "class": "StandaloneVM",
                        "template": None,
                        "label": "red",
                        "netvm": None,
                        "prefs": {
                            "autostart": True,
                            "guivm": None,
                            "provides_network": True,
                        },
                        "features": {"nixome-role": "sys-net"},
                        "services": {
                            "network-manager": True,
                            "qubes-firewall": True,
                            "qubes-network": True,
                            "qubes-updates-proxy": True,
                        },
                        "devices": {"pci": []},
                    },
                },
                "prefs": {
                    "default_guivm": "sys-gui",
                    "default_netvm": "sys-firewall",
                },
                "qrexec_policy": {"rules": []},
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    commands = {
        "qvm-ls": """
        #!/usr/bin/env bash
        case "$*" in
          "--raw-data --fields NAME --no-spinner")
            printf 'dom0\n'
            cat "$CREATED_DOMAINS"
            ;;
          "--raw-data --fields NAME,STATE --no-spinner")
            printf 'dom0|Running\n'
            while IFS= read -r domain; do
              [ -n "$domain" ] || continue
              printf '%s|Halted\n' "$domain"
            done < "$CREATED_DOMAINS"
            ;;
          "--raw-data --fields NAME,CLASS --no-spinner")
            printf 'dom0|AdminVM\n'
            while IFS= read -r domain; do
              [ -n "$domain" ] || continue
              printf '%s|StandaloneVM\n' "$domain"
            done < "$CREATED_DOMAINS"
            ;;
          *)
            printf 'unexpected qvm-ls args: %s\n' "$*" >&2
            exit 1
            ;;
        esac
        """,
        "qvm-create": """
        #!/usr/bin/env bash
        printf 'qvm-create %s\n' "$*" >> "$MUTATION_LOG"
        printf '%s\n' "${!#}" >> "$CREATED_DOMAINS"
        """,
        "qvm-device": """
        #!/usr/bin/env bash
        if [ "$1" = "pci" ] && [ "$2" = "list" ]; then
          exit 0
        fi
        printf 'qvm-device %s\n' "$*" >> "$MUTATION_LOG"
        """,
        "qvm-prefs": """
        #!/usr/bin/env bash
        if [ "$#" -eq 2 ]; then
          if [ "$1" = "sys-net" ] && [ "$2" = "netvm" ]; then
            printf 'none\n'
            exit 0
          fi
          if [ "$1" = "sys-gui" ] && [ "$2" = "guivm" ]; then
            printf 'dom0\n'
            exit 0
          fi
          exit 1
        fi
        printf 'qvm-prefs %s\n' "$*" >> "$MUTATION_LOG"
        """,
        "qvm-features": """
        #!/usr/bin/env bash
        if [ "$#" -eq 2 ]; then
          exit 1
        fi
        printf 'qvm-features %s\n' "$*" >> "$MUTATION_LOG"
        """,
        "qvm-service": """
        #!/usr/bin/env bash
        if [ "$#" -eq 2 ]; then
          exit 1
        fi
        printf 'qvm-service %s\n' "$*" >> "$MUTATION_LOG"
        """,
        "systemctl": """
        #!/usr/bin/env bash
        case "$1" in
          is-active|reload|try-restart)
            exit 0
            ;;
        esac
        exit 1
        """,
        "qubes-prefs": """
        #!/usr/bin/env bash
        if [ "$#" -eq 1 ]; then
          exit 1
        fi
        printf 'qubes-prefs %s\n' "$*" >> "$MUTATION_LOG"
        """,
    }
    for name in (
        "qvm-template-postprocess",
        "qvm-volume",
        "qubesd-query",
    ):
        commands[name] = "#!/usr/bin/env bash\nexit 0\n"
    _setup_fake_command_dir(bin_dir, commands)
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "CREATED_DOMAINS": str(created_domains),
            "MUTATION_LOG": str(mutation_log),
            "NIXOME_DOM0_NETWORK_PCI_DEVICES": "00_02.0 00_03.0",
            "NIXOME_DOM0_DEFAULT_ROUTE_PCI_DEVICES": "00_02.0",
            "NIXOME_QUBES_POLICY_DIR": str(policy_dir),
            "NIXOME_QUBES_XML": str(qubes_xml),
            "NIXOME_SETUP_CONFIG_STATUS_DEVICE": str(status_device),
            "NIXOME_SYSTEM_STATE_DIR": str(state_dir),
            "NIXOME_XEN_CAPABILITIES": str(capabilities),
            "NIXOME_LOG_DIR": str(setup_log_dir),
        }
    )
    _run(
        [
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/setup-config.sh"),
            "--apply",
            "--boot-critical",
            "--topology",
            str(topology),
        ],
        cwd=repo_root,
        env=env,
        stdout=setup_log_dir / "stdout.log",
        stderr=setup_log_dir / "stderr.log",
    )
    boot_command_lines = (
        (setup_log_dir / "setup-config.commands.log")
        .read_text(encoding="utf-8")
        .splitlines()
    )
    boot_result = json.loads(
        (setup_log_dir / "setup-config.json").read_text(encoding="utf-8")
    )
    assert boot_result["scope"] == "boot-critical"
    assert not any("sys-config-staging" in line for line in boot_command_lines)
    assert not any(" dev" in line for line in boot_command_lines)
    assert "+ qvm-prefs sys-net guivm none" in boot_command_lines
    assert "+ qvm-prefs sys-firewall guivm none" in boot_command_lines
    assert "+ qvm-prefs sys-gui guivm dom0" in boot_command_lines
    assert "+ qvm-prefs sys-gui netvm none" in boot_command_lines
    assert "Starting sys-gui desktop" in status_device.read_text(encoding="utf-8")

    _run(
        [
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/setup-config.sh"),
            "--apply",
            "--deferred",
            "--topology",
            str(topology),
        ],
        cwd=repo_root,
        env=env,
        stdout=setup_log_dir / "deferred-stdout.log",
        stderr=setup_log_dir / "deferred-stderr.log",
    )
    deferred_command_lines = (
        (setup_log_dir / "setup-config.commands.log")
        .read_text(encoding="utf-8")
        .splitlines()
    )
    sys_net_create = boot_command_lines.index(
        "+ qvm-create --class StandaloneVM --label red sys-net"
    )
    sys_firewall_create = boot_command_lines.index(
        "+ qvm-create --class StandaloneVM --label green sys-firewall"
    )
    sys_gui_create = boot_command_lines.index(
        "+ qvm-create --class StandaloneVM --label black sys-gui"
    )
    config_create = deferred_command_lines.index(
        "+ qvm-create --class StandaloneVM --label gray sys-config-staging"
    )
    dev_create = deferred_command_lines.index(
        "+ qvm-create --class StandaloneVM --label green dev"
    )
    sys_net_netvm = boot_command_lines.index("+ qvm-prefs sys-net netvm none")
    sys_net_pci = boot_command_lines.index(
        "+ qvm-device pci assign --required --port sys-net dom0:00_03.0"
    )
    sys_firewall_netvm = boot_command_lines.index(
        "+ qvm-prefs sys-firewall netvm sys-net"
    )
    sys_gui_guivm = boot_command_lines.index("+ qvm-prefs sys-gui guivm dom0")
    sys_gui_netvm = boot_command_lines.index("+ qvm-prefs sys-gui netvm none")
    default_guivm = boot_command_lines.index("+ qubes-prefs default_guivm sys-gui")
    assert "+ qvm-prefs dev netvm sys-firewall" in deferred_command_lines
    assert "+ qvm-prefs sys-config-staging guivm none" in deferred_command_lines
    assert "+ qubes-prefs default_netvm sys-firewall" in boot_command_lines

    assert sys_net_create < sys_firewall_create
    assert sys_gui_create < sys_gui_guivm
    assert sys_gui_create < sys_gui_netvm
    assert sys_gui_guivm < default_guivm
    assert sys_gui_netvm < default_guivm
    assert sys_net_netvm < sys_net_pci
    assert sys_net_pci < sys_firewall_netvm
    assert config_create < dev_create

    result = json.loads(
        (setup_log_dir / "setup-config.json").read_text(encoding="utf-8")
    )
    assert result["schema"] == "nixome.setup-config.v1"
    assert result["status"] == "passed"
    assert result["scope"] == "deferred"
    assert boot_result["domains_changed"] == 3
    assert boot_result["devices_changed"] == 1
    assert result["domains_changed"] == 2
    assert result["devices_changed"] == 0


def check_setup_config_qrexec_policy(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    setup_log_dir = work_dir / "logs"
    policy_dir = work_dir / "policy.d"
    policy_file = policy_dir / "90-nixome.policy"
    state_dir = work_dir / "state"
    capabilities = work_dir / "xen-capabilities"
    mutation_log = work_dir / "mutations.log"
    systemctl_log = work_dir / "systemctl.log"
    topology = work_dir / "topology.json"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (setup_log_dir, policy_dir, state_dir):
        path.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    mutation_log.write_text("", encoding="utf-8")
    systemctl_log.write_text("", encoding="utf-8")
    topology.write_text(
        json.dumps(
            {
                "schema": "nixome.qubes-topology.v1",
                "templates": {},
                "kernels": {},
                "domains": {},
                "prefs": {},
                "qrexec_policy": {
                    "rules": [
                        {
                            "service": "nixome.PolicyProbe",
                            "argument": "*",
                            "source": "dom0",
                            "target": "sys-config-vault",
                            "action": "allow",
                        },
                        {
                            "service": "nixome.PolicyProbe",
                            "argument": "*",
                            "source": "@anyvm",
                            "target": "@anyvm",
                            "action": "deny",
                        },
                    ]
                },
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    unexpected = """
    #!/usr/bin/env bash
    printf 'unexpected command execution: %s %s\n' "$0" "$*" >&2
    exit 97
    """
    commands = {
        name: unexpected
        for name in (
            "qvm-create",
            "qvm-features",
            "qvm-prefs",
            "qvm-service",
            "qvm-template-postprocess",
            "qvm-volume",
            "qubes-prefs",
            "qubesd-query",
        )
    }
    commands["qvm-ls"] = "#!/usr/bin/env bash\nprintf 'dom0\n'\n"
    commands["systemctl"] = """
    #!/usr/bin/env bash
    case "$1" in
      is-active)
        exit 0
        ;;
      reload)
        printf 'systemctl %s\n' "$*" >> "$SYSTEMCTL_LOG"
        exit 1
        ;;
      try-restart)
        printf 'systemctl %s\n' "$*" >> "$SYSTEMCTL_LOG"
        exit 0
        ;;
    esac
    exit 1
    """
    _setup_fake_command_dir(bin_dir, commands)
    for command_name in ("cmp", "cp", "grep", "jq", "mkdir", "chmod", "rm"):
        path = shutil.which(command_name)
        if path is not None and not (bin_dir / command_name).exists():
            (bin_dir / command_name).symlink_to(path)
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "MUTATION_LOG": str(mutation_log),
            "SYSTEMCTL_LOG": str(systemctl_log),
            "NIXOME_XEN_CAPABILITIES": str(capabilities),
            "NIXOME_QUBES_POLICY_DIR": str(policy_dir),
            "NIXOME_SYSTEM_STATE_DIR": str(state_dir),
            "NIXOME_LOG_DIR": str(setup_log_dir),
        }
    )
    command = [
        "bash",
        str(repo_root / "lifecycle/misc/tools/scripts/setup-config.sh"),
        "--apply",
        "--topology",
        str(topology),
    ]
    _run(
        command,
        cwd=repo_root,
        env=env,
        stdout=setup_log_dir / "first-stdout.log",
        stderr=setup_log_dir / "first-stderr.log",
    )
    result = json.loads(
        (setup_log_dir / "setup-config.json").read_text(encoding="utf-8")
    )
    assert result["schema"] == "nixome.setup-config.v1"
    assert result["status"] == "passed"
    assert result["mode"] == "apply"
    assert result["qrexec_policy_changed"] == 1
    assert result["qrexec_policy_file"] == str(policy_file)
    assert policy_file.read_text(encoding="utf-8") == (
        "# Managed by Nixome setup-config.\n"
        "# Source: nixome.qubes.topology.qrexec_policy.\n"
        "# Bootstrap config transport policy remains in 30-nixome-config.policy.\n"
        "nixome.PolicyProbe * dom0 sys-config-vault allow\n"
        "nixome.PolicyProbe * @anyvm @anyvm deny\n"
    )
    systemctl_lines = systemctl_log.read_text(encoding="utf-8").splitlines()
    assert "systemctl reload qubes-qrexec-policy-daemon.service" in systemctl_lines
    assert "systemctl try-restart qubes-qrexec-policy-daemon.service" in systemctl_lines

    systemctl_log.write_text("", encoding="utf-8")
    _run(
        command,
        cwd=repo_root,
        env=env,
        stdout=setup_log_dir / "second-stdout.log",
        stderr=setup_log_dir / "second-stderr.log",
    )
    result = json.loads(
        (setup_log_dir / "setup-config.json").read_text(encoding="utf-8")
    )
    assert result["schema"] == "nixome.setup-config.v1"
    assert result["status"] == "passed"
    assert result["mode"] == "apply"
    assert result["qrexec_policy_changed"] == 0
    current = json.loads((state_dir / "current-setup.json").read_text(encoding="utf-8"))
    assert current["schema"] == "nixome.setup-config.v1"
    assert current["status"] == "passed"
    assert current["qrexec_policy_changed"] == 0
    current_topology = json.loads(
        (state_dir / "current-topology.json").read_text(encoding="utf-8")
    )
    assert current_topology["schema"] == "nixome.qubes-topology.v1"
    assert len(current_topology["qrexec_policy"]["rules"]) == 2
    assert systemctl_log.read_text(encoding="utf-8") == ""
    assert mutation_log.read_text(encoding="utf-8") == ""


def check_setup_config_restart_actions(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    kernel_dir = work_dir / "kernel/6.12.64-1.fc41"
    kernel_target = work_dir / "vm-kernels/6.12.64-1.fc41"
    setup_log_dir = work_dir / "logs"
    policy_dir = work_dir / "policy.d"
    state_dir = work_dir / "state"
    capabilities = work_dir / "xen-capabilities"
    mutation_log = work_dir / "mutations.log"
    qubes_xml = work_dir / "qubes.xml"
    topology = work_dir / "topology.json"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (kernel_dir, kernel_target, setup_log_dir, policy_dir, state_dir):
        path.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    for name in ("vmlinuz", "initramfs", "modules.img"):
        (kernel_dir / name).write_text(f"{name}\n", encoding="utf-8")
        shutil.copy(kernel_dir / name, kernel_target / name)
    (kernel_target / "MANAGED_BY_NIXOME").write_text(
        f"source={kernel_dir}\nschema=nixome.qubes-kernel.v1\n", encoding="utf-8"
    )
    mutation_log.write_text("", encoding="utf-8")
    qubes_xml.write_text(
        """
        <qubes>
          <properties><property name="default_kernel"></property></properties>
          <domains><domain><properties>
            <property name="name">legacy-fedora-app</property>
            <property name="netvm"></property>
          </properties></domain></domains>
        </qubes>
        """,
        encoding="utf-8",
    )
    (policy_dir / "90-nixome.policy").write_text(
        "# Managed by Nixome setup-config.\n"
        "# Source: nixome.qubes.topology.qrexec_policy.\n"
        "# Bootstrap config transport policy remains in 30-nixome-config.policy.\n",
        encoding="utf-8",
    )
    topology.write_text(
        json.dumps(
            {
                "schema": "nixome.qubes-topology.v1",
                "templates": {},
                "kernels": {"6.12.64-1.fc41": {"source": str(kernel_dir)}},
                "domains": {
                    "legacy-fedora-app": {
                        "class": "AppVM",
                        "template": "fedora-44-xfce",
                        "label": "green",
                        "netvm": None,
                        "prefs": {"autostart": False, "memory": 1024},
                        "features": {"nixome-role": "legacy-fedora-app"},
                        "services": {"network-manager": False},
                    }
                },
                "prefs": {"default_kernel": ""},
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    commands = {
        "qvm-ls": """
        #!/usr/bin/env bash
        case "$*" in
          *"--fields NAME,STATE"*)
            printf 'dom0|Running\n'
            printf 'fedora-44-xfce|Halted\n'
            printf 'legacy-fedora-app|Running\n'
            ;;
          *)
            printf 'dom0\n'
            printf 'fedora-44-xfce\n'
            printf 'legacy-fedora-app\n'
            ;;
        esac
        """,
        "systemctl": """
        #!/usr/bin/env bash
        if [ "$1" = "is-active" ]; then
          exit 0
        fi
        if [ "$1" = "reload" ] || [ "$1" = "try-restart" ]; then
          printf 'systemctl %s\n' "$*" >> "$MUTATION_LOG"
          exit 0
        fi
        exit 1
        """,
        "qubes-prefs": """
        #!/usr/bin/env bash
        if [ "$#" -eq 1 ] && [ "$1" = "default_kernel" ]; then
          printf '\n'
          exit 0
        fi
        printf 'qubes-prefs %s\n' "$*" >> "$MUTATION_LOG"
        exit 0
        """,
        "qvm-prefs": """
        #!/usr/bin/env bash
        if [ "$#" -eq 2 ]; then
          case "$1:$2" in
            legacy-fedora-app:label) printf 'green\n' ;;
            legacy-fedora-app:netvm) printf 'none\n' ;;
            legacy-fedora-app:autostart) printf 'False\n' ;;
            legacy-fedora-app:memory) printf '512\n' ;;
            *) exit 1 ;;
          esac
          exit 0
        fi
        printf 'qvm-prefs %s\n' "$*" >> "$MUTATION_LOG"
        exit 0
        """,
        "qvm-features": """
        #!/usr/bin/env bash
        if [ "$#" -eq 2 ] && [ "$1:$2" = "legacy-fedora-app:nixome-role" ]; then
          printf 'legacy-fedora-app\n'
          exit 0
        fi
        printf 'qvm-features %s\n' "$*" >> "$MUTATION_LOG"
        exit 0
        """,
        "qvm-service": """
        #!/usr/bin/env bash
        if [ "$#" -eq 2 ] && [ "$1:$2" = "legacy-fedora-app:network-manager" ]; then
          printf 'on\n'
          exit 0
        fi
        printf 'qvm-service %s\n' "$*" >> "$MUTATION_LOG"
        exit 0
        """,
        "qubesd-query": "#!/usr/bin/env bash\nexit 0\n",
    }
    unexpected = """
    #!/usr/bin/env bash
    printf '%s %s\n' "$0" "$*" >> "$MUTATION_LOG"
    exit 99
    """
    for name in ("qvm-create", "qvm-template-postprocess", "qvm-volume"):
        commands[name] = unexpected
    _setup_fake_command_dir(bin_dir, commands)
    for command_name in (
        "awk",
        "chmod",
        "cmp",
        "cp",
        "grep",
        "jq",
        "mkdir",
        "python3",
        "rm",
    ):
        path = shutil.which(command_name)
        if path is not None and not (bin_dir / command_name).exists():
            (bin_dir / command_name).symlink_to(path)
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "MUTATION_LOG": str(mutation_log),
            "NIXOME_QUBES_KERNEL_BASE": str(work_dir / "vm-kernels"),
            "NIXOME_QUBES_XML": str(qubes_xml),
            "NIXOME_QUBES_POLICY_DIR": str(policy_dir),
            "NIXOME_SYSTEM_STATE_DIR": str(state_dir),
            "NIXOME_XEN_CAPABILITIES": str(capabilities),
            "NIXOME_LOG_DIR": str(setup_log_dir),
        }
    )
    _run(
        [
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/setup-config.sh"),
            "--apply",
            "--topology",
            str(topology),
        ],
        cwd=repo_root,
        env=env,
        stdout=setup_log_dir / "stdout.log",
        stderr=setup_log_dir / "stderr.log",
    )
    result = json.loads(
        (setup_log_dir / "setup-config.json").read_text(encoding="utf-8")
    )
    assert result["schema"] == "nixome.setup-config.v1"
    assert result["status"] == "passed"
    assert result["mode"] == "apply"
    assert result["templates_changed"] == 0
    assert result["kernels_changed"] == 0
    assert result["domains_changed"] == 0
    assert result["prefs_changed"] == 1
    assert result["features_changed"] == 0
    assert result["services_changed"] == 1
    assert result["qrexec_policy_changed"] == 0
    assert result["restart_required"] is True
    assert len(result["live_actions"]) == 2
    assert len(result["restart_actions"]) == 2
    assert any(
        action["type"] == "domain-pref"
        and action["action"] == "set"
        and action["domain"] == "legacy-fedora-app"
        and action["field"] == "memory"
        and action["value"] == "1024"
        for action in result["live_actions"]
    )
    assert any(
        action["type"] == "domain-service"
        and action["action"] == "set"
        and action["domain"] == "legacy-fedora-app"
        and action["field"] == "network-manager"
        and action["value"] == "off"
        for action in result["live_actions"]
    )
    for field, value in (("memory", "1024"), ("network-manager", "off")):
        assert any(
            action["type"] in {"domain-pref", "domain-service"}
            and action["action"] == "restart"
            and action["domain"] == "legacy-fedora-app"
            and action["field"] == field
            and action["value"] == value
            and action["state"] == "Running"
            and action["reason"]
            == "domain is running; restart required for this change to fully take effect"
            for action in result["restart_actions"]
        )
    current = json.loads((state_dir / "current-setup.json").read_text(encoding="utf-8"))
    assert current["schema"] == "nixome.setup-config.v1"
    assert current["status"] == "passed"
    assert current["restart_required"] is True
    assert len(current["restart_actions"]) == 2
    current_topology = json.loads(
        (state_dir / "current-topology.json").read_text(encoding="utf-8")
    )
    assert current_topology["schema"] == "nixome.qubes-topology.v1"
    assert current_topology["domains"]["legacy-fedora-app"]["prefs"]["memory"] == 1024
    assert (
        current_topology["domains"]["legacy-fedora-app"]["services"]["network-manager"]
        is False
    )
    mutation_lines = mutation_log.read_text(encoding="utf-8").splitlines()
    assert "qvm-prefs legacy-fedora-app memory 1024" in mutation_lines
    assert "qvm-service legacy-fedora-app network-manager off" in mutation_lines


def check_config_apply_template_build_ref(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    candidate_dir = work_dir / "candidate"
    apply_log_dir = work_dir / "logs"
    capabilities = work_dir / "xen-capabilities"
    validated_tar = work_dir / "config-candidate-validated.tar"
    setup_capture = work_dir / "setup-topology.json"
    state_dir = work_dir / "state"
    etc_dir = work_dir / "etc"
    system_profile = work_dir / "nixome-system-profile"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (bin_dir, candidate_dir, apply_log_dir):
        path.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    nixpkgs_path = _run(
        ["nix", "eval", "--raw", f"{repo_root}#nixome.configInputs.nixpkgs"]
    ).stdout.strip()

    _write_flake(
        candidate_dir / "flake.nix",
        f"""
        {{
          description = "Nixome config apply template build-ref fixture";

          inputs.nixpkgs.url = "path:{nixpkgs_path}";

          outputs =
            {{ self, nixpkgs }}:
            let
              system = "x86_64-linux";
              pkgs = nixpkgs.legacyPackages.${{system}};
            in
            {{
              packages.${{system}}.test-template =
                pkgs.runCommand "test-template"
                  {{
                    preferLocalBuild = true;
                    allowSubstitutes = false;
                  }}
                  ''
                    mkdir -p "$out/test-template"
                    printf 'root image fixture\n' > "$out/test-template/root.img"
                    printf 'qrexec=1\nvirt-mode=pvh\n' > "$out/test-template/template.conf"
                  '';

              nixome.topology = {{
                schema = "nixome.qubes-topology.v1";
                templates.test-template = {{
                  import = true;
                  source = null;
                  source_ref = {{
                    type = "flake-output";
                    attr = "packages.x86_64-linux.test-template";
                    subpath = "test-template";
                  }};
                  skip_start = true;
                }};
                kernels = {{ }};
                domains = {{ }};
                prefs = {{ }};
                qrexec_policy.rules = [ ];
              }};

              nixome.system = {{
                schema = "nixome.system.v1";
                dom0 = {{
                  profile = {{
                    source = null;
                    source_ref = null;
                    activation_command = "bin/nixome-dom0-profile-activate";
                    activation_action = "none";
                    message = "";
                  }};
                  packages = [ ];
                  systemd.services = {{ }};
                  nixos = {{
                    system = {{
                      enable = false;
                      reason = "dom0 system change requires reboot";
                      source = null;
                      source_ref = null;
                    }};
                    kernel = {{
                      enable = false;
                      reason = "dom0 kernel change requires reboot";
                      source = null;
                      source_ref = null;
                    }};
                    xen = {{
                      enable = false;
                      reason = "dom0 xen change requires reboot";
                      source = null;
                      source_ref = null;
                    }};
                    bootloader = {{
                      enable = false;
                      reason = "dom0 bootloader change requires reboot";
                      source = null;
                      source_ref = null;
                    }};
                    qubes = {{
                      enable = false;
                      reason = "dom0 qubes change requires reboot";
                      source = null;
                      source_ref = null;
                    }};
                  }};
                }};
                plan = {{
                  live_actions = [ ];
                  reboot_actions = [ ];
                  reboot_required = false;
                  reboot_reasons = [ ];
                }};
              }};
            }};
        }}
        """,
    )
    nix_flags = [
        "nix",
        "--extra-experimental-features",
        "nix-command",
        "--extra-experimental-features",
        "flakes",
    ]
    _run([*nix_flags, "flake", "lock", "--impure", "--offline"], cwd=candidate_dir)
    _git_commit_all(
        candidate_dir,
        "fixture candidate",
        name="Nixome Test",
        email="nixome-test@localhost",
    )
    candidate_commit = _run(
        ["git", "rev-parse", "HEAD"], cwd=candidate_dir
    ).stdout.strip()
    _run(
        [
            "git",
            "bundle",
            "create",
            str(work_dir / "config.bundle"),
            "HEAD",
            "refs/heads/main",
        ],
        cwd=candidate_dir,
    )
    (work_dir / "validation.json").write_text(
        json.dumps(
            {
                "schema": "nixome.config-validation.v1",
                "status": "passed",
                "commit": candidate_commit,
                "checks": [],
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    (work_dir / "manifest.json").write_text(
        json.dumps(
            {
                "schema": "nixome.config-candidate.v1",
                "commit": candidate_commit,
                "branch": "main",
                "source_role": "sys-config-staging",
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    _run(
        [
            *nix_flags,
            "eval",
            "--impure",
            "--offline",
            "--no-write-lock-file",
            "--json",
            f"{candidate_dir}#nixome.topology",
        ],
        stdout=work_dir / "topology.json",
    )
    _run(
        [
            *nix_flags,
            "eval",
            "--impure",
            "--offline",
            "--no-write-lock-file",
            "--json",
            f"{candidate_dir}#nixome.system",
        ],
        stdout=work_dir / "system.json",
    )
    with tarfile.open(validated_tar, "w") as tar:
        for name in (
            "config.bundle",
            "validation.json",
            "manifest.json",
            "topology.json",
            "system.json",
        ):
            tar.add(work_dir / name, arcname=name)

    _write_executable(
        bin_dir / "nixome-config-archive-candidate",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        cp "$VALIDATED_TAR_FIXTURE" "$NIXOME_LOG_DIR/config-candidate-validated.tar"
        jq -c -n \
          --arg schema "nixome.config-archive.v1" \
          --arg status "passed" \
          --arg candidate_commit "$CANDIDATE_COMMIT_FIXTURE" \
          '{
            schema: $schema,
            status: $status,
            candidate_commit: $candidate_commit,
            validation_commit: $candidate_commit,
            validation: {
              schema: "nixome.config-validation.v1",
              status: "passed",
              commit: $candidate_commit
            },
            vault_archive: {
              schema: "nixome.config-vault-archive.v1",
              status: "passed",
              candidate_commit: $candidate_commit
            }
          }' > "$NIXOME_LOG_DIR/config-archive.json"
        printf 'archive fixture ok\n'
        """,
    )
    _write_executable(
        bin_dir / "nixome-setup-config",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        topology=""
        while [ "$#" -gt 0 ]; do
          case "$1" in
            --topology)
              shift
              topology="$1"
              ;;
          esac
          shift
        done
        [ -n "$topology" ] || exit 2
        cp "$topology" "$SETUP_CAPTURE_FIXTURE"
        jq -e '
          .templates."test-template".source != null and
          .templates."test-template".source_ref == null and
          (.templates."test-template".source | test("^/nix/store/")) and
          (.templates."test-template".source | test("/test-template$"))
        ' "$topology" >/dev/null
        jq -c -n \
          --arg schema "nixome.setup-config.v1" \
          --arg status "passed" \
          '{schema: $schema, status: $status, domains_changed: 0, templates_changed: 1}' \
          > "$NIXOME_LOG_DIR/setup-config.json"
        printf 'setup fixture ok\n'
        """,
    )
    _write_executable(
        bin_dir / "qrexec-client",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        cat >/dev/null
        jq -c -n \
          --arg schema "nixome.config-vault-deployment.v1" \
          --arg status "passed" \
          --arg candidate_commit "$CANDIDATE_COMMIT_FIXTURE" \
          '{
            schema: $schema,
            status: $status,
            deployment_status: "passed",
            candidate_commit: $candidate_commit,
            vault_commit: "fixture-vault"
          }'
        """,
    )
    _write_executable(
        bin_dir / "systemctl",
        """
        #!/usr/bin/env bash
        if [ "$1" = "is-active" ]; then
          exit 0
        fi
        exit 1
        """,
    )
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "CANDIDATE_COMMIT_FIXTURE": candidate_commit,
            "SETUP_CAPTURE_FIXTURE": str(setup_capture),
            "VALIDATED_TAR_FIXTURE": str(validated_tar),
            "NIXOME_XEN_CAPABILITIES": str(capabilities),
            "NIXOME_LOG_DIR": str(apply_log_dir),
            "NIXOME_SYSTEM_STATE_DIR": str(state_dir),
            "NIXOME_SYSTEM_ETC_DIR": str(etc_dir),
            "NIXOME_SYSTEM_PROFILE": str(system_profile),
        }
    )
    _run(
        [
            "bash",
            str(
                repo_root / "lifecycle/runtime/tools/scripts/apply-config-candidate.sh"
            ),
            "--apply",
        ],
        cwd=repo_root,
        env=env,
        stdout=work_dir / "stdout.log",
        stderr=work_dir / "stderr.log",
    )
    result = json.loads(
        (apply_log_dir / "config-apply.json").read_text(encoding="utf-8")
    )
    assert result["schema"] == "nixome.config-apply.v1"
    assert result["status"] == "passed"
    assert result["candidate_commit"] == candidate_commit
    assert result["setup"]["status"] == "passed"
    assert result["deployment"]["status"] == "passed"
    template_builds = json.loads(
        (apply_log_dir / "config-apply-template-builds.json").read_text(
            encoding="utf-8"
        )
    )
    assert template_builds["schema"] == "nixome.template-builds.v1"
    assert len(template_builds["builds"]) == 1
    build = template_builds["builds"][0]
    assert build["template"] == "test-template"
    assert build["attr"] == "packages.x86_64-linux.test-template"
    assert build["status"] == "passed"
    assert re.search(r"^/nix/store/.*/test-template$", build["source_path"])
    system_builds = json.loads(
        (apply_log_dir / "config-apply-system-builds.json").read_text(encoding="utf-8")
    )
    assert system_builds["schema"] == "nixome.system-builds.v1"
    assert system_builds["builds"] == []
    system_apply = json.loads(
        (apply_log_dir / "config-system-apply.json").read_text(encoding="utf-8")
    )
    assert system_apply["schema"] == "nixome.system-apply.v1"
    assert system_apply["status"] == "passed"
    assert system_apply["profile_status"] == "skipped"
    assert system_apply["reboot_required"] is False
    captured_topology = json.loads(setup_capture.read_text(encoding="utf-8"))
    assert captured_topology["templates"]["test-template"]["source"] is not None
    assert captured_topology["templates"]["test-template"]["source_ref"] is None


def check_nixome_cli(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    repo = work_dir / "repo"
    captured_tar = work_dir / "captured-candidate.tar"
    qrexec_log = work_dir / "qrexec.log"
    status_json = work_dir / "status.json"
    shutil.rmtree(work_dir, ignore_errors=True)
    (repo / "modules").mkdir(parents=True)
    bin_dir.mkdir(parents=True)
    status_json.write_text(
        json.dumps(
            {
                "schema": "nixome.config-status.v1",
                "status": "applied",
                "deployment": {
                    "current": {
                        "schema": "nixome.config-deployment.v1",
                        "candidate_commit": "base000000000000000000000000000000000000",
                    }
                },
                "host_generation": {
                    "active": True,
                    "current_source": "/nix/store/base-system",
                },
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    _write_executable(
        bin_dir / "qrexec-client-vm",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        target="$1"
        service="$2"
        printf '%s %s\n' "$target" "$service" >> "$QREXEC_LOG"
        case "$service" in
          nixome.Config.Status)
            cat "$STATUS_JSON"
            ;;
          nixome.Config.Check)
            cat > "$CAPTURED_TAR"
            jq -c -n --arg schema "nixome.config-archive.v1" --arg status "passed" \
              '{schema: $schema, status: $status}'
            ;;
          nixome.Config.Apply)
            cat > "$CAPTURED_TAR"
            jq -c -n --arg schema "nixome.config-apply.v1" --arg status "passed" \
              '{schema: $schema, status: $status}'
            ;;
          nixome.Config.Rollback)
            jq -c -n --arg schema "nixome.config-rollback.v1" --arg status "passed" \
              '{schema: $schema, status: $status}'
            ;;
          *)
            printf 'unexpected service: %s\n' "$service" >&2
            exit 2
            ;;
        esac
        """,
    )
    _write_flake(
        repo / "flake.nix",
        """
        {
          description = "Nixome CLI test repo";
          outputs = { self }: {
            nixome.topology = {
              schema = "nixome.qubes-topology.v1";
              templates = {};
              kernels = {};
              domains = {};
              prefs = {};
              qrexec_policy.rules = [];
            };
          };
        }
        """,
    )
    (repo / "modules/controller.nix").write_text(
        "initial controller\n", encoding="utf-8"
    )
    _git_commit_all(
        repo,
        "initial candidate",
        name="Nixome CLI Test",
        email="nixome-cli-test@localhost",
    )
    (repo / "modules/controller.nix").write_text(
        "self-hosted controller change\n", encoding="utf-8"
    )
    _run(["git", "add", "modules/controller.nix"], cwd=repo)
    _run(
        [
            "git",
            "-c",
            "user.name=Nixome CLI Test",
            "-c",
            "user.email=nixome-cli-test@localhost",
            "commit",
            "-m",
            "change controller source",
        ],
        cwd=repo,
    )
    expected_commit = _run(["git", "rev-parse", "HEAD"], cwd=repo).stdout.strip()
    expected_tree = _run(["git", "rev-parse", "HEAD^{tree}"], cwd=repo).stdout.strip()
    nixome_out = _run(
        [
            "nix",
            "build",
            f"{repo_root}#packages.x86_64-linux.nixome-cli",
            "--no-link",
            "--print-out-paths",
        ]
    ).stdout.strip()
    nixome_bin = Path(_physical_store_path(repo_root, nixome_out)) / "bin/nixome"
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "NIXOME_QREXEC_CLIENT": str(bin_dir / "qrexec-client-vm"),
            "QREXEC_LOG": str(qrexec_log),
            "STATUS_JSON": str(status_json),
            "CAPTURED_TAR": str(captured_tar),
        }
    )

    for subcommand, output_path, schema in (
        ("status", work_dir / "status-output.json", "nixome.config-status.v1"),
        ("check", work_dir / "check.json", "nixome.config-archive.v1"),
        ("apply", work_dir / "apply.json", "nixome.config-apply.v1"),
        ("rollback", work_dir / "rollback.json", "nixome.config-rollback.v1"),
    ):
        _run(
            [str(nixome_bin), subcommand],
            cwd=repo,
            env=env,
            stdout=output_path,
        )
        output = json.loads(output_path.read_text(encoding="utf-8"))
        assert output["schema"] == schema
        expected_status = "applied" if subcommand == "status" else "passed"
        assert output["status"] == expected_status

        if subcommand == "check":
            with tarfile.open(captured_tar) as tar:
                members = sorted(tar.getnames())
                tar.extractall(work_dir / "candidate")
            assert "config.bundle" in members
            assert "manifest.json" in members
            manifest = json.loads(
                (work_dir / "candidate/manifest.json").read_text(encoding="utf-8")
            )
            assert manifest["schema"] == "nixome.config-candidate.v1"
            assert manifest["commit"] == expected_commit
            assert manifest["tree"] == expected_tree
            assert manifest["branch"] == "main"
            assert manifest["source_role"] == "dev"
            assert manifest["transport"] == "qrexec"
            assert manifest["base_generation"]["schema"] == "nixome.config-status.v1"
            assert (
                manifest["base_generation"]["deployment"]["current"]["candidate_commit"]
                == "base000000000000000000000000000000000000"
            )
            assert manifest["requested_targets"] == ["nixome"]
            _run(
                [
                    "git",
                    "clone",
                    str(work_dir / "candidate/config.bundle"),
                    str(work_dir / "cloned"),
                ]
            )
            assert (work_dir / "cloned/modules/controller.nix").read_text(
                encoding="utf-8"
            ).strip() == "self-hosted controller change"

    with (repo / "modules/controller.nix").open("a", encoding="utf-8") as handle:
        handle.write("dirty\n")
    result = _run(
        [str(nixome_bin), "check"],
        cwd=repo,
        env=env,
        stdout=work_dir / "dirty.json",
        stderr=work_dir / "dirty.stderr",
        check=False,
    )
    assert result.returncode != 0
    assert "worktree is dirty" in (work_dir / "dirty.stderr").read_text(
        encoding="utf-8"
    )
    qrexec_lines = qrexec_log.read_text(encoding="utf-8").splitlines()
    for line in (
        "dom0 nixome.Config.Status",
        "dom0 nixome.Config.Check",
        "dom0 nixome.Config.Apply",
        "dom0 nixome.Config.Rollback",
    ):
        assert line in qrexec_lines


def check_config_control(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    control_log_dir = work_dir / "control-logs"
    state_dir = work_dir / "state"
    host_profile = work_dir / "host-system-profile"
    host_current = work_dir / "run-current-system"
    payload_marker = work_dir / "payload-executed"
    activation_marker = work_dir / "activated"
    fake_call_log = work_dir / "fake-calls.log"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (bin_dir, control_log_dir, state_dir):
        path.mkdir(parents=True)

    _write_executable(
        bin_dir / "nixome-config-status",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        jq -c -n --arg schema "nixome.config-status.v1" --arg status "applied" \
          '{schema: $schema, status: $status, deployment: {current: {candidate_commit: "base"}}}'
        """,
    )
    _write_executable(
        bin_dir / "nixome-config-archive-candidate",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        candidate_tar=""
        validate_only=0
        while [ "$#" -gt 0 ]; do
          case "$1" in
            --candidate-tar) shift; candidate_tar="$1" ;;
            --validate-only) validate_only=1 ;;
          esac
          shift
        done
        [ -n "$candidate_tar" ] || exit 2
        [ "$validate_only" = 1 ] || exit 3
        printf 'archive %s validate-only=%s\n' "$candidate_tar" "$validate_only" >> "$FAKE_CALL_LOG"
        tmpdir="$(mktemp -d)"
        trap 'rm -rf "$tmpdir"' EXIT
        tar -C "$tmpdir" -xf "$candidate_tar"
        commit="$(jq -r '.commit' "$tmpdir/manifest.json")"
        scenario="$(jq -r '.scenario // "valid"' "$tmpdir/manifest.json")"
        if [ "$scenario" = "invalid-eval" ]; then
          jq -c -n --arg schema "nixome.config-archive.v1" --arg status "failed" --arg commit "$commit" \
            '{schema: $schema, status: $status, reason: "candidate validation failed", candidate_commit: $commit, validation: {schema: "nixome.config-validation.v1", status: "failed", commit: $commit}}' \
            > "$NIXOME_LOG_DIR/config-archive.json"
          exit 1
        fi
        jq -c -n --arg schema "nixome.config-archive.v1" --arg status "passed" --arg commit "$commit" \
          '{schema: $schema, status: $status, reason: "validated by fake validator", candidate_commit: $commit, validation_commit: $commit, validation: {schema: "nixome.config-validation.v1", status: "passed", commit: $commit}}' \
          > "$NIXOME_LOG_DIR/config-archive.json"
        printf 'fake archive ok\n'
        """,
    )
    _write_executable(
        bin_dir / "nixome-config-apply",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        candidate_tar=""
        while [ "$#" -gt 0 ]; do
          case "$1" in
            --candidate-tar) shift; candidate_tar="$1" ;;
          esac
          shift
        done
        [ -n "$candidate_tar" ] || exit 2
        printf 'apply %s\n' "$candidate_tar" >> "$FAKE_CALL_LOG"
        tmpdir="$(mktemp -d)"
        trap 'rm -rf "$tmpdir"' EXIT
        tar -C "$tmpdir" -xf "$candidate_tar"
        commit="$(jq -r '.commit' "$tmpdir/manifest.json")"
        scenario="$(jq -r '.scenario // "valid"' "$tmpdir/manifest.json")"
        case "$scenario" in
          valid)
            printf '%s\n' "$commit" > "$ACTIVATION_MARKER"
            jq -c -n --arg schema "nixome.config-apply.v1" --arg status "passed" --arg commit "$commit" \
              '{schema: $schema, status: $status, candidate_commit: $commit, system_apply_result: {status: "passed"}}' \
              > "$NIXOME_LOG_DIR/config-apply.json"
            ;;
          build-failed)
            jq -c -n --arg schema "nixome.config-apply.v1" --arg status "failed" --arg commit "$commit" \
              '{schema: $schema, status: $status, reason: "system build failed", candidate_commit: $commit}' \
              > "$NIXOME_LOG_DIR/config-apply.json"
            exit 1
            ;;
          test-failed)
            jq -c -n --arg schema "nixome.config-apply.v1" --arg status "failed" --arg commit "$commit" \
              '{schema: $schema, status: $status, reason: "candidate tests failed", candidate_commit: $commit}' \
              > "$NIXOME_LOG_DIR/config-apply.json"
            exit 1
            ;;
          invalid-eval)
            jq -c -n --arg schema "nixome.config-apply.v1" --arg status "failed" --arg commit "$commit" \
              '{schema: $schema, status: $status, reason: "candidate validation failed", candidate_commit: $commit}' \
              > "$NIXOME_LOG_DIR/config-apply.json"
            exit 1
            ;;
        esac
        printf 'fake apply ok\n'
        """,
    )
    _write_executable(
        bin_dir / "nix-env",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        printf 'nix-env %s\n' "$*" >> "$FAKE_CALL_LOG"
        exit 1
        """,
    )

    def make_candidate(scenario: str, output: Path) -> str:
        tmpdir = work_dir / f"candidate-{scenario}"
        repo = tmpdir / "repo"
        repo.mkdir(parents=True)
        _write_flake(
            repo / "flake.nix",
            """
            {
              outputs = { self }: {
                nixome.topology = {
                  schema = "nixome.qubes-topology.v1";
                  templates = {};
                  kernels = {};
                  domains = {};
                  prefs = {};
                  qrexec_policy.rules = [];
                };
              };
            }
            """,
        )
        commit = _git_commit_all(
            repo,
            f"{scenario} candidate",
            name="Nixome Control Test",
            email="nixome-control-test@localhost",
        )
        _run(
            [
                "git",
                "bundle",
                "create",
                str(tmpdir / "config.bundle"),
                "HEAD",
                "refs/heads/main",
            ],
            cwd=repo,
        )
        (tmpdir / "manifest.json").write_text(
            json.dumps(
                {
                    "schema": "nixome.config-candidate.v1",
                    "commit": commit,
                    "branch": "main",
                    "source_role": "dev",
                    "scenario": scenario,
                    "payload": f"touch {payload_marker}",
                },
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
        with tarfile.open(output, "w") as tar:
            tar.add(tmpdir / "config.bundle", arcname="config.bundle")
            tar.add(tmpdir / "manifest.json", arcname="manifest.json")
        return commit

    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "NIXOME_CONFIG_CONTROL_LOG_DIR": str(control_log_dir),
            "NIXOME_SYSTEM_STATE_DIR": str(state_dir),
            "NIXOME_HOST_SYSTEM_PROFILE": str(host_profile),
            "NIXOME_HOST_CURRENT_SYSTEM_LINK": str(host_current),
            "FAKE_CALL_LOG": str(fake_call_log),
            "ACTIVATION_MARKER": str(activation_marker),
        }
    )
    script = repo_root / "lifecycle/runtime/tools/scripts/config-control.sh"
    _run(
        ["bash", str(script), "status"],
        cwd=repo_root,
        env=env,
        stdout=work_dir / "status.json",
    )
    status = json.loads((work_dir / "status.json").read_text(encoding="utf-8"))
    assert status["schema"] == "nixome.config-status.v1"
    assert status["status"] == "applied"

    valid_tar = work_dir / "valid.tar"
    make_candidate("valid", valid_tar)
    for mode, output_path in (
        ("check", work_dir / "check-valid.json"),
        ("apply", work_dir / "apply-valid.json"),
    ):
        activation_marker.unlink(missing_ok=True)
        with valid_tar.open("rb") as stdin:
            result = subprocess.run(
                ["bash", str(script), mode],
                cwd=repo_root,
                env=env,
                check=False,
                stdin=stdin,
                stdout=subprocess.PIPE,
            )
        output_path.write_bytes(result.stdout)
        assert result.returncode == 0
        payload = json.loads(result.stdout.decode("utf-8"))
        assert payload["status"] == "passed"
        if mode == "check":
            assert payload["schema"] == "nixome.config-archive.v1"
            assert "validate-only=1" in fake_call_log.read_text(encoding="utf-8")
            assert not payload_marker.exists()
        else:
            assert payload["schema"] == "nixome.config-apply.v1"
            assert activation_marker.is_file()
            assert not payload_marker.exists()

    for scenario in ("invalid-eval", "build-failed", "test-failed"):
        activation_marker.unlink(missing_ok=True)
        tar_path = work_dir / f"{scenario}.tar"
        make_candidate(scenario, tar_path)
        with tar_path.open("rb") as stdin:
            result = subprocess.run(
                ["bash", str(script), "apply"],
                cwd=repo_root,
                env=env,
                check=False,
                stdin=stdin,
                stdout=subprocess.PIPE,
            )
        (work_dir / f"apply-{scenario}.json").write_bytes(result.stdout)
        assert result.returncode != 0
        failed = json.loads(result.stdout.decode("utf-8"))
        assert failed["schema"] == "nixome.config-apply.v1"
        assert failed["status"] == "failed"
        assert not activation_marker.exists()

    previous_system = work_dir / "previous-system"
    current_system = work_dir / "current-system-source"
    (previous_system / "bin").mkdir(parents=True)
    current_system.mkdir()
    _write_executable(
        previous_system / "bin/switch-to-configuration",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        printf '%s\n' "$1" > "$NIXOME_SYSTEM_STATE_DIR/rollback-switch-action"
        ln -sfn "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)" "$NIXOME_HOST_CURRENT_SYSTEM_LINK"
        """,
    )
    host_profile.symlink_to(current_system)
    (state_dir / "previous-host-system").write_text(
        str(previous_system) + "\n", encoding="utf-8"
    )
    _run(
        ["bash", str(script), "rollback"],
        cwd=repo_root,
        env=env,
        stdout=work_dir / "rollback.json",
    )
    rollback = json.loads((work_dir / "rollback.json").read_text(encoding="utf-8"))
    assert rollback["schema"] == "nixome.config-rollback.v1"
    assert rollback["status"] == "passed"
    assert rollback["method"] == "state"
    assert rollback["target_system"] == str(previous_system)
    assert os.readlink(host_profile) == str(previous_system)
    assert os.readlink(host_current) == str(previous_system)
    assert (state_dir / "rollback-switch-action").read_text(
        encoding="utf-8"
    ).strip() == "switch"

    with tarfile.open(work_dir / "bad.tar", "w") as tar:
        tar.add(valid_tar, arcname="valid.tar")
    with (work_dir / "bad.tar").open("rb") as stdin:
        result = subprocess.run(
            ["bash", str(script), "check"],
            cwd=repo_root,
            env=env,
            check=False,
            stdin=stdin,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    (work_dir / "bad.json").write_bytes(result.stdout)
    (work_dir / "bad.stderr").write_bytes(result.stderr)
    assert result.returncode != 0
    bad = json.loads(result.stdout.decode("utf-8"))
    assert bad["schema"] == "nixome.config-control.v1"
    assert bad["status"] == "failed"


def _write_domain_reference_candidate(
    work_dir: Path, source_role: str
) -> tuple[str, Path]:
    candidate_dir = work_dir / "candidate"
    candidate_dir.mkdir(parents=True)
    _write_flake(
        candidate_dir / "flake.nix",
        """
        {
          outputs = { self }: { };
        }
        """,
    )
    commit = _git_commit_all(
        candidate_dir,
        "fixture candidate",
        name="Nixome Test",
        email="nixome-test@localhost",
    )
    _run(
        [
            "git",
            "bundle",
            "create",
            str(work_dir / "config.bundle"),
            "HEAD",
            "refs/heads/main",
        ],
        cwd=candidate_dir,
    )
    (work_dir / "topology.json").write_text(
        json.dumps(
            {
                "schema": "nixome.qubes-topology.v1",
                "templates": {},
                "kernels": {},
                "domains": {
                    "dev": {
                        "class": "StandaloneVM",
                        "template": None,
                        "label": "green",
                        "netvm": "sys-firewall",
                        "prefs": {},
                        "features": {"nixome-role": "dev"},
                        "services": {},
                        "backend": {"type": "qubes-standalone", "template": None},
                    }
                },
                "prefs": {},
                "qrexec_policy": {"rules": []},
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    (work_dir / "system.json").write_text(
        json.dumps(
            {
                "schema": "nixome.system.v1",
                "dom0": {
                    "profile": {
                        "source": None,
                        "source_ref": None,
                        "activation_command": "bin/nixome-dom0-profile-activate",
                        "activation_action": "none",
                        "message": "",
                    },
                    "packages": [],
                    "systemd": {"services": {}},
                    "nixos": {
                        name: {
                            "enable": False,
                            "reason": "",
                            "source": None,
                            "source_ref": None,
                            **({"switch_action": "switch"} if name == "system" else {}),
                        }
                        for name in ("system", "kernel", "xen", "bootloader", "qubes")
                    },
                },
                "plan": {
                    "live_actions": [],
                    "reboot_actions": [],
                    "reboot_required": False,
                    "reboot_reasons": [],
                },
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    (work_dir / "validation.json").write_text(
        json.dumps(
            {
                "schema": "nixome.config-validation.v1",
                "status": "passed",
                "commit": commit,
                "checks": [],
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    (work_dir / "manifest.json").write_text(
        json.dumps(
            {
                "schema": "nixome.config-candidate.v1",
                "commit": commit,
                "branch": "main",
                "source_role": source_role,
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    validated_tar = work_dir / "config-candidate-validated.tar"
    with tarfile.open(validated_tar, "w") as tar:
        for member in (
            "config.bundle",
            "validation.json",
            "manifest.json",
            "topology.json",
            "system.json",
        ):
            tar.add(work_dir / member, arcname=member)
    return commit, validated_tar


def _assert_domain_reference_result(
    payload: dict[str, object],
    schema: str,
    expected_status: str,
    expected_reason: str = "",
) -> None:
    assert payload["schema"] == schema
    assert payload["status"] == expected_status
    result = payload["domain_references_result"]
    assert isinstance(result, dict)
    assert result["status"] == expected_status
    if expected_status == "passed":
        references = result["references"]
        assert any(
            reference["kind"] == "netvm"
            and reference["domain"] == "dev"
            and reference["target"] == "sys-firewall"
            for reference in references
        )
        return

    assert payload["reason"] == expected_reason
    missing = result["missing"]
    if expected_reason == "topology references missing domains":
        assert missing == [
            {
                "kind": "netvm",
                "domain": "dev",
                "field": "netvm",
                "target": "sys-firewall",
            }
        ]
    else:
        assert any(
            reference["kind"] == "stock-network-chain"
            and reference["domain"] == "sys-firewall"
            and reference["field"] == "netvm"
            and reference["target"] == "sys-net"
            and reference["reason"] == "stock sys-firewall upstream sys-net is missing"
            for reference in missing
        )


def check_config_archive_domain_references(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    capabilities = work_dir / "xen-capabilities"
    shutil.rmtree(work_dir, ignore_errors=True)
    work_dir.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    _commit, validated_tar = _write_domain_reference_candidate(work_dir, "dev")
    staged_tar = work_dir / "config-candidate-staged.tar"
    with tarfile.open(staged_tar, "w") as tar:
        tar.add(work_dir / "config.bundle", arcname="config.bundle")
        tar.add(work_dir / "manifest.json", arcname="manifest.json")

    for case_name, sys_firewall_present, sys_net_present, status, reason in (
        (
            "missing-sys-firewall",
            False,
            False,
            "failed",
            "topology references missing domains",
        ),
        (
            "missing-sys-net",
            True,
            False,
            "failed",
            "topology references unusable network domains",
        ),
        ("present", True, True, "passed", ""),
    ):
        case_dir = work_dir / case_name
        bin_dir = case_dir / "bin"
        case_log_dir = case_dir / "logs"
        bin_dir.mkdir(parents=True)
        case_log_dir.mkdir()
        qvm_ls_sys_firewall = (
            "printf 'sys-firewall\\n'" if sys_firewall_present else ":"
        )
        qvm_ls_sys_net = "printf 'sys-net\\n'" if sys_net_present else ":"
        _setup_fake_command_dir(
            bin_dir,
            {
                "qrexec-client": f"""
                    #!/usr/bin/env bash
                    set -euo pipefail
                    case "$*" in
                      *"nixome.Config.ValidateCandidate"*)
                        cat >/dev/null
                        cat {validated_tar}
                        ;;
                      *)
                        printf 'unexpected qrexec-client args: %s\\n' "$*" >&2
                        exit 1
                        ;;
                    esac
                """,
                "qvm-check": """
                    #!/usr/bin/env bash
                    set -euo pipefail
                    [ "${1:-}" = "sys-config-validator" ]
                """,
                "qvm-prefs": f"""
                    #!/usr/bin/env bash
                    set -euo pipefail
                    case "${{1:-}}:${{2:-}}" in
                      sys-config-validator:netvm)
                        printf '\\n'
                        ;;
                      sys-config-validator:provides_network)
                        printf 'False\\n'
                        ;;
                      sys-firewall:provides_network)
                        {"true" if sys_firewall_present else "false"} || exit 1
                        printf 'True\\n'
                        ;;
                      sys-firewall:netvm)
                        {"true" if sys_firewall_present else "false"} || exit 1
                        printf 'sys-net\\n'
                        ;;
                      sys-net:provides_network)
                        {"true" if sys_net_present else "false"} || exit 1
                        printf 'True\\n'
                        ;;
                      sys-net:netvm)
                        {"true" if sys_net_present else "false"} || exit 1
                        printf '\\n'
                        ;;
                      *)
                        printf 'unexpected qvm-prefs args: %s\\n' "$*" >&2
                        exit 1
                        ;;
                    esac
                """,
                "qvm-start": """
                    #!/usr/bin/env bash
                    set -euo pipefail
                    [ "${1:-}" = "sys-config-validator" ]
                """,
                "qvm-ls": f"""
                    #!/usr/bin/env bash
                    if [ "$*" = "--raw-data --fields NAME --no-spinner" ]; then
                      printf 'dom0\\n'
                      printf 'dev\\n'
                      {qvm_ls_sys_firewall}
                      {qvm_ls_sys_net}
                      exit 0
                    fi
                    printf 'unexpected qvm-ls args: %s\\n' "$*" >&2
                    exit 1
                """,
                "systemctl": """
                    #!/usr/bin/env bash
                    if [ "$1" = "is-active" ]; then
                      exit 0
                    fi
                    exit 1
                """,
            },
        )
        env = os.environ.copy()
        env.update(
            {
                "PATH": f"{bin_dir}:{env['PATH']}",
                "NIXOME_XEN_CAPABILITIES": str(capabilities),
                "NIXOME_LOG_DIR": str(case_log_dir),
            }
        )
        result = _run(
            [
                "bash",
                str(
                    repo_root
                    / "lifecycle/runtime/tools/scripts/archive-config-candidate.sh"
                ),
                "--candidate-tar",
                str(staged_tar),
                "--validate-only",
            ],
            cwd=repo_root,
            env=env,
            stdout=case_dir / "stdout.log",
            stderr=case_dir / "stderr.log",
            check=False,
        )
        if status == "passed":
            assert result.returncode == 0
        else:
            assert result.returncode != 0
        payload = json.loads((case_log_dir / "config-archive.json").read_text())
        _assert_domain_reference_result(
            payload, "nixome.config-archive.v1", status, reason
        )


def check_config_apply_domain_references(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    capabilities = work_dir / "xen-capabilities"
    shutil.rmtree(work_dir, ignore_errors=True)
    work_dir.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    commit, validated_tar = _write_domain_reference_candidate(
        work_dir, "sys-config-staging"
    )

    for case_name, sys_firewall_present, sys_net_present, status, reason in (
        (
            "missing-sys-firewall",
            False,
            False,
            "failed",
            "topology references missing domains",
        ),
        (
            "missing-sys-net",
            True,
            False,
            "failed",
            "topology references unusable network domains",
        ),
        ("present", True, True, "passed", ""),
    ):
        case_dir = work_dir / case_name
        bin_dir = case_dir / "bin"
        case_log_dir = case_dir / "logs"
        state_dir = case_dir / "state"
        etc_dir = case_dir / "etc"
        setup_capture = case_dir / "setup-topology.json"
        for path in (bin_dir, case_log_dir, state_dir, etc_dir):
            path.mkdir(parents=True)
        qvm_ls_sys_firewall = (
            "printf 'sys-firewall\\n'" if sys_firewall_present else ":"
        )
        qvm_ls_sys_net = "printf 'sys-net\\n'" if sys_net_present else ":"
        _setup_fake_command_dir(
            bin_dir,
            {
                "nixome-config-archive-candidate": f"""
                    #!/usr/bin/env bash
                    set -euo pipefail
                    cp {validated_tar} "$NIXOME_LOG_DIR/config-candidate-validated.tar"
                    jq -c -n \\
                      --arg schema "nixome.config-archive.v1" \\
                      --arg status "passed" \\
                      --arg candidate_commit "{commit}" \\
                      '{{
                        schema: $schema,
                        status: $status,
                        candidate_commit: $candidate_commit,
                        validation_commit: $candidate_commit,
                        validation: {{
                          schema: "nixome.config-validation.v1",
                          status: "passed",
                          commit: $candidate_commit
                        }},
                        vault_archive: {{
                          schema: "nixome.config-vault-archive.v1",
                          status: "passed",
                          candidate_commit: $candidate_commit
                        }}
                      }}' > "$NIXOME_LOG_DIR/config-archive.json"
                    printf 'archive fixture ok\\n'
                """,
                "nixome-setup-config": f"""
                    #!/usr/bin/env bash
                    set -euo pipefail
                    topology=""
                    while [ "$#" -gt 0 ]; do
                      case "$1" in
                        --topology)
                          shift
                          topology="$1"
                          ;;
                      esac
                      shift
                    done
                    [ -n "$topology" ] || exit 2
                    cp "$topology" {setup_capture}
                    jq -e '.domains.dev.netvm == "sys-firewall"' "$topology" >/dev/null
                    jq -c -n \\
                      --arg schema "nixome.setup-config.v1" \\
                      '{{schema: $schema, status: "passed", domains_changed: 0, templates_changed: 0}}' \\
                      > "$NIXOME_LOG_DIR/setup-config.json"
                    printf 'setup fixture ok\\n'
                """,
                "qrexec-client": f"""
                    #!/usr/bin/env bash
                    set -euo pipefail
                    cat >/dev/null
                    jq -c -n \\
                      --arg schema "nixome.config-vault-deployment.v1" \\
                      --arg status "passed" \\
                      --arg candidate_commit "{commit}" \\
                      '{{
                        schema: $schema,
                        status: $status,
                        deployment_status: "passed",
                        candidate_commit: $candidate_commit,
                        vault_commit: "fixture-vault"
                      }}'
                """,
                "qvm-ls": f"""
                    #!/usr/bin/env bash
                    if [ "$*" = "--raw-data --fields NAME --no-spinner" ]; then
                      printf 'dom0\\n'
                      printf 'dev\\n'
                      {qvm_ls_sys_firewall}
                      {qvm_ls_sys_net}
                      exit 0
                    fi
                    printf 'unexpected qvm-ls args: %s\\n' "$*" >&2
                    exit 1
                """,
                "qvm-prefs": f"""
                    #!/usr/bin/env bash
                    set -euo pipefail
                    case "${{1:-}}:${{2:-}}" in
                      sys-firewall:provides_network)
                        {"true" if sys_firewall_present else "false"} || exit 1
                        printf 'True\\n'
                        ;;
                      sys-firewall:netvm)
                        {"true" if sys_firewall_present else "false"} || exit 1
                        printf 'sys-net\\n'
                        ;;
                      sys-net:provides_network)
                        {"true" if sys_net_present else "false"} || exit 1
                        printf 'True\\n'
                        ;;
                      sys-net:netvm)
                        {"true" if sys_net_present else "false"} || exit 1
                        printf '\\n'
                        ;;
                      *)
                        printf 'unexpected qvm-prefs args: %s\\n' "$*" >&2
                        exit 1
                        ;;
                    esac
                """,
                "systemctl": """
                    #!/usr/bin/env bash
                    if [ "$1" = "is-active" ]; then
                      exit 0
                    fi
                    exit 1
                """,
            },
        )
        env = os.environ.copy()
        env.update(
            {
                "PATH": f"{bin_dir}:{env['PATH']}",
                "NIXOME_XEN_CAPABILITIES": str(capabilities),
                "NIXOME_LOG_DIR": str(case_log_dir),
                "NIXOME_USE_HOST_NIX_STORE": "1",
                "NIXOME_SYSTEM_STATE_DIR": str(state_dir),
                "NIXOME_SYSTEM_ETC_DIR": str(etc_dir),
            }
        )
        result = _run(
            [
                "bash",
                str(
                    repo_root
                    / "lifecycle/runtime/tools/scripts/apply-config-candidate.sh"
                ),
                "--apply",
            ],
            cwd=repo_root,
            env=env,
            stdout=case_dir / "stdout.log",
            stderr=case_dir / "stderr.log",
            check=False,
        )
        if status == "passed":
            assert result.returncode == 0
            assert setup_capture.is_file()
        else:
            assert result.returncode != 0
            assert not setup_capture.exists()
        payload = json.loads((case_log_dir / "config-apply.json").read_text())
        _assert_domain_reference_result(
            payload, "nixome.config-apply.v1", status, reason
        )


def check_config_apply_nix_domain_materialization(
    repo_root: Path, log_dir: Path
) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    candidate_dir = work_dir / "candidate"
    apply_log_dir = work_dir / "logs"
    capabilities = work_dir / "xen-capabilities"
    state_dir = work_dir / "state"
    etc_dir = work_dir / "etc"
    validated_tar = work_dir / "config-candidate-validated.tar"
    deployment_tar_capture = work_dir / "deployment.tar"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (
        bin_dir,
        candidate_dir / "system-source",
        candidate_dir / "root-source",
        candidate_dir / "store-source",
        apply_log_dir,
        state_dir,
        etc_dir,
    ):
        path.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")

    _write_flake(
        candidate_dir / "flake.nix",
        """
        {
          description = "Nixome Nix-native domain materialization fixture";
          outputs = { self }: { };
        }
        """,
    )
    system_source = candidate_dir / "system-source"
    root_source = candidate_dir / "root-source"
    store_source = candidate_dir / "store-source"
    _write_executable(
        system_source / "init",
        """
        #!/bin/sh
        exec /init
        """,
    )
    (root_source / "root.img").touch()
    (store_source / "store-paths").write_text(
        str(system_source) + "\n", encoding="utf-8"
    )
    (store_source / "manifest.json").write_text(
        json.dumps(
            {
                "schema": "nixome.store-closure-manifest.v1",
                "name": "fixture-store",
                "store_paths_file": "store-paths",
                "read_only": True,
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    commit = _git_commit_all(
        candidate_dir,
        "fixture candidate",
        name="Nixome Test",
        email="nixome-test@localhost",
    )
    _run(
        [
            "git",
            "bundle",
            "create",
            str(work_dir / "config.bundle"),
            "HEAD",
            "refs/heads/main",
        ],
        cwd=candidate_dir,
    )
    topology = {
        "schema": "nixome.qubes-topology.v1",
        "templates": {},
        "kernels": {},
        "domains": {
            "nix-native-work": {
                "class": "StandaloneVM",
                "template": None,
                "label": "green",
                "netvm": None,
                "prefs": {
                    "autostart": False,
                    "include_in_backups": True,
                    "kernelopts": "console=hvc0 init=/sbin/init",
                    "memory": 1536,
                    "virt_mode": "pvh",
                },
                "features": {
                    "nixome-managed": "1",
                    "nixome-role": "nix-native-work",
                    "qrexec": "1",
                },
                "services": {},
                "backend": {
                    "type": "nix-closure",
                    "template": None,
                    "system": {"source": "system-source", "source_ref": None},
                    "kernel": {
                        "name": "6.12.64-1.fc41",
                        "source": None,
                        "source_ref": None,
                    },
                    "initrd": {"name": None, "source": None, "source_ref": None},
                    "modules": {"name": None, "source": None, "source_ref": None},
                    "root": {
                        "mode": "qubes-root-image",
                        "source": "root-source",
                        "source_ref": None,
                        "read_only": False,
                    },
                    "store": {
                        "mode": "shared-closure",
                        "source": "store-source",
                        "source_ref": None,
                        "mountpoint": "/nix/store",
                        "device": "/dev/xvde",
                        "blob_device": "/dev/xvdf",
                        "view_backend": "erofs-objectmap",
                        "read_only": True,
                    },
                    "state": {
                        "persistent": "nix-native-work-private",
                        "volatile": None,
                    },
                },
            }
        },
        "prefs": {},
        "qrexec_policy": {"rules": []},
    }
    (work_dir / "topology.json").write_text(
        json.dumps(topology, sort_keys=True) + "\n", encoding="utf-8"
    )
    (work_dir / "system.json").write_text(
        json.dumps(
            {
                "schema": "nixome.system.v1",
                "dom0": {
                    "profile": {
                        "source": None,
                        "source_ref": None,
                        "activation_command": "bin/nixome-dom0-profile-activate",
                        "activation_action": "none",
                        "message": "",
                    },
                    "packages": [],
                    "systemd": {"services": {}},
                    "nixos": {
                        name: {
                            "enable": False,
                            "reason": "",
                            "source": None,
                            "source_ref": None,
                            **({"switch_action": "switch"} if name == "system" else {}),
                        }
                        for name in ("system", "kernel", "xen", "bootloader", "qubes")
                    },
                },
                "plan": {
                    "live_actions": [],
                    "reboot_actions": [],
                    "reboot_required": False,
                    "reboot_reasons": [],
                },
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    (work_dir / "validation.json").write_text(
        json.dumps(
            {
                "schema": "nixome.config-validation.v1",
                "status": "passed",
                "commit": commit,
                "checks": [],
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    (work_dir / "manifest.json").write_text(
        json.dumps(
            {
                "schema": "nixome.config-candidate.v1",
                "commit": commit,
                "branch": "main",
                "source_role": "sys-config-staging",
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    with tarfile.open(validated_tar, "w") as tar:
        for member in (
            "config.bundle",
            "validation.json",
            "manifest.json",
            "topology.json",
            "system.json",
        ):
            tar.add(work_dir / member, arcname=member)

    _setup_fake_command_dir(
        bin_dir,
        {
            "nixome-config-archive-candidate": f"""
                #!/usr/bin/env bash
                set -euo pipefail
                cp {validated_tar} "$NIXOME_LOG_DIR/config-candidate-validated.tar"
                jq -c -n \\
                  --arg schema "nixome.config-archive.v1" \\
                  --arg status "passed" \\
                  --arg candidate_commit "{commit}" \\
                  '{{
                    schema: $schema,
                    status: $status,
                    candidate_commit: $candidate_commit,
                    validation_commit: $candidate_commit,
                    validation: {{
                      schema: "nixome.config-validation.v1",
                      status: "passed",
                      commit: $candidate_commit
                    }},
                    vault_archive: {{
                      schema: "nixome.config-vault-archive.v1",
                      status: "passed",
                      candidate_commit: $candidate_commit
                    }}
                  }}' > "$NIXOME_LOG_DIR/config-archive.json"
                printf 'archive fixture ok\\n'
            """,
            "nixome-setup-config": """
                #!/usr/bin/env bash
                set -euo pipefail
                topology=""
                while [ "$#" -gt 0 ]; do
                  case "$1" in
                    --topology)
                      shift
                      topology="$1"
                      ;;
                  esac
                  shift
                done
                [ -n "$topology" ]
                jq -e \
                  --arg state "$SYSTEM_STATE_DIR_FIXTURE" \
                  --arg commit "$CANDIDATE_COMMIT_FIXTURE" \
                  '
                    .domains."nix-native-work".backend.type == "nix-closure" and
                    .domains."nix-native-work".backend.system.source_ref == null and
                    .domains."nix-native-work".backend.root.source_ref == null and
                    .domains."nix-native-work".backend.store.source_ref == null and
                    (.domains."nix-native-work".backend.system.source | startswith($state + "/sources/" + $commit + "/nix-domain-nix-native-work-system")) and
                    (.domains."nix-native-work".backend.root.source | startswith($state + "/sources/" + $commit + "/nix-domain-nix-native-work-root")) and
                    (.domains."nix-native-work".backend.store.source | startswith($state + "/sources/" + $commit + "/nix-domain-nix-native-work-store"))
                  ' "$topology" >/dev/null
                jq -c -n \
                  --arg schema "nixome.setup-config.v1" \
                  '{
                    schema: $schema,
                    status: "passed",
                    mode: "apply",
                    reason: "fixture setup passed",
                    templates_changed: 0,
                    kernels_changed: 0,
                    domains_changed: 1,
                    prefs_changed: 0,
                    features_changed: 0,
                    services_changed: 0,
                    qrexec_policy_changed: 0,
                    root_images_changed: 1,
                    live_actions: [],
                    restart_actions: [],
                    unsupported_domains: [],
                    unsupported_domains_count: 0,
                    restart_required: false
                  }' > "$NIXOME_LOG_DIR/setup-config.json"
                cat "$NIXOME_LOG_DIR/setup-config.json"
            """,
            "qrexec-client": """
                #!/usr/bin/env bash
                set -euo pipefail
                cat > "$DEPLOYMENT_TAR_CAPTURE_FIXTURE"
                jq -c -n \
                  --arg schema "nixome.config-vault-deployment.v1" \
                  --arg status "passed" \
                  --arg candidate_commit "$CANDIDATE_COMMIT_FIXTURE" \
                  '{
                    schema: $schema,
                    status: $status,
                    deployment_status: "passed",
                    candidate_commit: $candidate_commit,
                    vault_commit: "fixture-vault"
                  }'
            """,
            "systemctl": """
                #!/usr/bin/env bash
                if [ "$1" = "is-active" ]; then
                  exit 0
                fi
                exit 1
            """,
        },
    )
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "CANDIDATE_COMMIT_FIXTURE": commit,
            "DEPLOYMENT_TAR_CAPTURE_FIXTURE": str(deployment_tar_capture),
            "SYSTEM_STATE_DIR_FIXTURE": str(state_dir),
            "NIXOME_XEN_CAPABILITIES": str(capabilities),
            "NIXOME_LOG_DIR": str(apply_log_dir),
            "NIXOME_USE_HOST_NIX_STORE": "1",
            "NIXOME_SYSTEM_STATE_DIR": str(state_dir),
            "NIXOME_SYSTEM_ETC_DIR": str(etc_dir),
        }
    )
    _run(
        [
            "bash",
            str(
                repo_root / "lifecycle/runtime/tools/scripts/apply-config-candidate.sh"
            ),
            "--apply",
        ],
        cwd=repo_root,
        env=env,
        stdout=work_dir / "stdout.log",
        stderr=work_dir / "stderr.log",
    )

    apply_payload = json.loads((apply_log_dir / "config-apply.json").read_text())
    assert apply_payload["schema"] == "nixome.config-apply.v1"
    assert apply_payload["status"] == "passed"
    assert apply_payload["candidate_commit"] == commit
    assert apply_payload["reason"] == "validated config candidate applied"
    assert apply_payload["setup"]["status"] == "passed"
    assert apply_payload["system_apply_result"]["status"] == "passed"
    assert apply_payload["nix_domain_builds_result"]["schema"] == (
        "nixome.nix-domain-builds.v1"
    )
    materializations = apply_payload["nix_domain_materializations_result"]
    assert materializations["schema"] == "nixome.nix-domain-materializations.v1"
    assert materializations["domains"][0]["domain"] == "nix-native-work"
    assert materializations["domains"][0]["root"]["source"].startswith(
        f"{state_dir}/sources/{commit}/nix-domain-nix-native-work-root"
    )
    assert apply_payload["unsupported_domain_backends_result"]["schema"] == (
        "nixome.unsupported-domain-backends.v1"
    )
    assert apply_payload["unsupported_domain_backends_result"]["domains"] == []
    assert apply_payload["deployment"]["status"] == "passed"
    assert (
        apply_payload["deployment"]["nix_domain_materializations"]["domains"][0][
            "domain"
        ]
        == "nix-native-work"
    )
    assert apply_payload["deployment_archive"]["status"] == "passed"

    current_deployment = json.loads((state_dir / "current-deployment.json").read_text())
    assert current_deployment["schema"] == "nixome.config-deployment.v1"
    assert current_deployment["status"] == "passed"
    assert current_deployment["candidate_commit"] == commit
    assert (
        current_deployment["nix_domain_builds"]["schema"]
        == "nixome.nix-domain-builds.v1"
    )
    assert current_deployment["nix_domain_materializations"]["schema"] == (
        "nixome.nix-domain-materializations.v1"
    )

    with tarfile.open(deployment_tar_capture) as tar:
        members = sorted(tar.getnames())
        assert "nix-domain-builds.json" in members
        assert "nix-domain-materializations.json" in members
        assert "unsupported-domain-backends.json" in members
        assert "logs/nix-domain-builds.log" in members
        nix_domain_builds = json.load(tar.extractfile("nix-domain-builds.json"))
        extracted_materializations = json.load(
            tar.extractfile("nix-domain-materializations.json")
        )
        unsupported = json.load(tar.extractfile("unsupported-domain-backends.json"))
        deployment = json.load(tar.extractfile("deployment.json"))

    assert nix_domain_builds["schema"] == "nixome.nix-domain-builds.v1"
    assert (
        extracted_materializations["schema"] == "nixome.nix-domain-materializations.v1"
    )
    assert extracted_materializations["domains"][0]["domain"] == "nix-native-work"
    assert extracted_materializations["domains"][0]["root"]["mode"] == (
        "qubes-root-image"
    )
    assert extracted_materializations["domains"][0]["store"]["mode"] == (
        "shared-closure"
    )
    assert extracted_materializations["domains"][0]["store"]["view_backend"] == (
        "erofs-objectmap"
    )
    assert unsupported["schema"] == "nixome.unsupported-domain-backends.v1"
    assert unsupported["domains"] == []
    assert deployment["artifacts"]["nix_domain_builds"] == "nix-domain-builds.json"
    assert (
        deployment["artifacts"]["nix_domain_materializations"]
        == "nix-domain-materializations.json"
    )
    assert (
        deployment["artifacts"]["unsupported_domain_backends"]
        == "unsupported-domain-backends.json"
    )


def check_setup_config_idempotent(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    kernel_dir = work_dir / "kernel/6.12.64-1.fc41"
    kernel_base = work_dir / "vm-kernels"
    kernel_target = kernel_base / "6.12.64-1.fc41"
    template_dir = work_dir / "template/fedora-44-xfce"
    topology_path = work_dir / "topology.json"
    capabilities = work_dir / "xen-capabilities"
    setup_log_dir = work_dir / "logs"
    mutation_log = work_dir / "mutations.log"
    qubes_xml = work_dir / "qubes.xml"
    policy_dir = work_dir / "policy.d"
    policy_file = policy_dir / "90-nixome.policy"
    state_dir = work_dir / "state"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (
        bin_dir,
        kernel_dir,
        kernel_target,
        template_dir,
        setup_log_dir,
        policy_dir,
        state_dir,
    ):
        path.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    for filename in ("vmlinuz", "initramfs", "modules.img"):
        (kernel_dir / filename).write_text(filename + "\n", encoding="utf-8")
        shutil.copy2(kernel_dir / filename, kernel_target / filename)
    (kernel_target / "MANAGED_BY_NIXOME").write_text(
        f"source={kernel_dir}\nschema=nixome.qubes-kernel.v1\n",
        encoding="utf-8",
    )
    (template_dir / "root.img").touch()
    (template_dir / "template.conf").write_text(
        "qrexec=1\nvirt-mode=pvh\n", encoding="utf-8"
    )
    mutation_log.touch()
    qubes_xml.write_text(
        """
        <qubes>
          <properties>
            <property name="default_kernel"></property>
          </properties>
          <domains><domain><properties>
            <property name="name">legacy-fedora-app</property>
            <property name="netvm"></property>
          </properties></domain></domains>
        </qubes>
        """,
        encoding="utf-8",
    )
    policy_file.write_text(
        "\n".join(
            (
                "# Managed by Nixome setup-config.",
                "# Source: nixome.qubes.topology.qrexec_policy.",
                "# Bootstrap config transport policy remains in 30-nixome-config.policy.",
                "",
            )
        ),
        encoding="utf-8",
    )
    topology_path.write_text(
        json.dumps(
            {
                "schema": "nixome.qubes-topology.v1",
                "templates": {
                    "fedora-44-xfce": {
                        "import": True,
                        "skip_start": True,
                        "source": str(template_dir),
                    }
                },
                "kernels": {"6.12.64-1.fc41": {"source": str(kernel_dir)}},
                "domains": {
                    "legacy-fedora-app": {
                        "class": "AppVM",
                        "template": "fedora-44-xfce",
                        "label": "green",
                        "netvm": None,
                        "prefs": {"autostart": False, "memory": 1024},
                        "features": {"nixome-role": "legacy-fedora-app"},
                        "services": {"network-manager": False},
                    }
                },
                "prefs": {"default_kernel": ""},
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    _setup_fake_command_dir(
        bin_dir,
        {
            "qvm-ls": """
                #!/usr/bin/env bash
                printf 'dom0\\nfedora-44-xfce\\nlegacy-fedora-app\\n'
            """,
            "systemctl": """
                #!/usr/bin/env bash
                if [ "$1" = "is-active" ]; then
                  exit 0
                fi
                if [ "$1" = "reload" ] || [ "$1" = "try-restart" ]; then
                  printf 'systemctl %s\\n' "$*" >> "$MUTATION_LOG"
                  exit 0
                fi
                exit 1
            """,
            "qubes-prefs": """
                #!/usr/bin/env bash
                if [ "$#" -eq 1 ] && [ "$1" = "default_kernel" ]; then
                  printf '\\n'
                  exit 0
                fi
                printf 'qubes-prefs %s\\n' "$*" >> "$MUTATION_LOG"
                exit 0
            """,
            "qvm-prefs": """
                #!/usr/bin/env bash
                if [ "$#" -eq 2 ]; then
                  case "$1:$2" in
                    legacy-fedora-app:label) printf 'green\\n' ;;
                    legacy-fedora-app:netvm) printf 'none\\n' ;;
                    legacy-fedora-app:autostart) printf 'False\\n' ;;
                    legacy-fedora-app:memory) printf '1024\\n' ;;
                    *) exit 1 ;;
                  esac
                  exit 0
                fi
                printf 'qvm-prefs %s\\n' "$*" >> "$MUTATION_LOG"
                exit 0
            """,
            "qvm-features": """
                #!/usr/bin/env bash
                if [ "$#" -eq 2 ] && [ "$1:$2" = "legacy-fedora-app:nixome-role" ]; then
                  printf 'legacy-fedora-app\\n'
                  exit 0
                fi
                printf 'qvm-features %s\\n' "$*" >> "$MUTATION_LOG"
                exit 0
            """,
            "qvm-service": """
                #!/usr/bin/env bash
                if [ "$#" -eq 2 ] && [ "$1:$2" = "legacy-fedora-app:network-manager" ]; then
                  printf 'off\\n'
                  exit 0
                fi
                printf 'qvm-service %s\\n' "$*" >> "$MUTATION_LOG"
                exit 0
            """,
            "qvm-create": """
                #!/usr/bin/env bash
                printf '%s %s\\n' "$0" "$*" >> "$MUTATION_LOG"
                exit 99
            """,
            "qvm-template-postprocess": """
                #!/usr/bin/env bash
                printf '%s %s\\n' "$0" "$*" >> "$MUTATION_LOG"
                exit 99
            """,
            "qvm-volume": """
                #!/usr/bin/env bash
                printf '%s %s\\n' "$0" "$*" >> "$MUTATION_LOG"
                exit 99
            """,
            "qubesd-query": """
                #!/usr/bin/env bash
                exit 0
            """,
        },
    )
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "MUTATION_LOG": str(mutation_log),
            "NIXOME_QUBES_KERNEL_BASE": str(kernel_base),
            "NIXOME_QUBES_XML": str(qubes_xml),
            "NIXOME_QUBES_POLICY_DIR": str(policy_dir),
            "NIXOME_SYSTEM_STATE_DIR": str(state_dir),
            "NIXOME_XEN_CAPABILITIES": str(capabilities),
            "NIXOME_LOG_DIR": str(setup_log_dir),
        }
    )
    _run(
        [
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/setup-config.sh"),
            "--apply",
            "--topology",
            str(topology_path),
        ],
        cwd=repo_root,
        env=env,
        stdout=setup_log_dir / "stdout.log",
        stderr=setup_log_dir / "stderr.log",
    )
    setup = json.loads((setup_log_dir / "setup-config.json").read_text())
    assert setup["schema"] == "nixome.setup-config.v1"
    assert setup["status"] == "passed"
    assert setup["mode"] == "apply"
    for key in (
        "templates_changed",
        "kernels_changed",
        "domains_changed",
        "prefs_changed",
        "features_changed",
        "services_changed",
        "qrexec_policy_changed",
    ):
        assert setup[key] == 0
    current_setup = json.loads((state_dir / "current-setup.json").read_text())
    assert current_setup["schema"] == "nixome.setup-config.v1"
    assert current_setup["status"] == "passed"
    assert current_setup["restart_required"] is False
    assert current_setup["restart_actions"] == []
    current_topology = json.loads((state_dir / "current-topology.json").read_text())
    assert current_topology["schema"] == "nixome.qubes-topology.v1"
    assert current_topology["domains"]["legacy-fedora-app"]["prefs"]["memory"] == 1024
    assert mutation_log.read_text(encoding="utf-8") == ""


def check_closure_volume_filtered_device(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir / "work"
    result_path = log_dir / "result.json"
    shutil.rmtree(log_dir, ignore_errors=True)
    (work_dir / "pool/objects/segments").mkdir(parents=True)
    storage_src = repo_root / "lifecycle/runtime/tools/pkgs/nixome-qubes-storage/src"
    sys.path.insert(0, str(storage_src))
    original_modules = {
        name: sys.modules.get(name) for name in ("qubes", "qubes.exc", "qubes.storage")
    }

    class StoragePoolException(Exception):
        pass

    class FakePoolBase:
        def __init__(
            self,
            *,
            name: str,
            revisions_to_keep: int = 0,
            ephemeral_volatile: bool = False,
            **_kwargs: object,
        ) -> None:
            self.name = name
            self.revisions_to_keep = revisions_to_keep
            self.ephemeral_volatile = ephemeral_volatile

    class FakeBlockDevice:
        def __init__(
            self,
            path: str,
            name: str,
            script: str | None = None,
            rw: bool = True,
            domain: str | None = None,
            devtype: str = "disk",
        ) -> None:
            self.path = path
            self.name = name
            self.script = script
            self.rw = rw
            self.domain = domain
            self.devtype = devtype

    class FakeVolumeBase:
        def __init__(self, **kwargs: object) -> None:
            self.name = kwargs.get("name", "")
            self.pool = kwargs.get("pool")
            self.vid = kwargs.get("vid", "")
            self.rw = bool(kwargs.get("rw", True))
            self.snap_on_start = bool(kwargs.get("snap_on_start", False))
            self.save_on_stop = bool(kwargs.get("save_on_stop", False))
            self.source = kwargs.get("source")
            self.revisions_to_keep = kwargs.get("revisions_to_keep", 0)
            self._size = int(kwargs.get("size", 0))
            self.path = kwargs.get("path")

        @property
        def config(self) -> dict[str, object]:
            pool = self.pool.name if hasattr(self.pool, "name") else self.pool
            return {
                "name": self.name,
                "pool": pool,
                "vid": self.vid,
                "rw": self.rw,
                "snap_on_start": self.snap_on_start,
                "save_on_stop": self.save_on_stop,
                "source": self.source,
                "revisions_to_keep": self.revisions_to_keep,
                "size": self._size,
            }

        def block_device(self) -> FakeBlockDevice:
            return FakeBlockDevice(self.path, self.name, rw=self.rw)

    qubes_module = types.ModuleType("qubes")
    exc_module = types.ModuleType("qubes.exc")
    storage_module = types.ModuleType("qubes.storage")
    exc_module.StoragePoolException = StoragePoolException
    storage_module.Pool = FakePoolBase
    storage_module.Volume = FakeVolumeBase
    storage_module.BlockDevice = FakeBlockDevice
    qubes_module.exc = exc_module
    qubes_module.storage = storage_module
    sys.modules["qubes"] = qubes_module
    sys.modules["qubes.exc"] = exc_module
    sys.modules["qubes.storage"] = storage_module

    try:
        from nixome_qubes_storage import engine
        from nixome_qubes_storage.qubes import ClosurePool

        pool_root = work_dir / "pool"
        segment_blob = pool_root / "objects/segments/000001.blob"
        segment_blob.write_bytes(b"segment")
        blob_map = (
            pool_root / "domains/sys-config-vault/generations/1/store.blob-map.json"
        )
        blob_map.parent.mkdir(parents=True, exist_ok=True)
        blob_map.write_text(
            json.dumps(
                {
                    "schema": "nixome.closure-store.blob-map.v1",
                    "segments": [
                        {
                            "id": "000001",
                            "blob": str(segment_blob),
                            "size": segment_blob.stat().st_size,
                        }
                    ],
                    "logical_size": 512,
                    "extents": [],
                }
            ),
            encoding="utf-8",
        )
        manifest = blob_map.parent / "manifest.json"
        manifest.write_text("{}", encoding="utf-8")

        pool = ClosurePool(name="nixome-closure", root_dir=str(pool_root))
        vm = types.SimpleNamespace(name="sys-config-vault")
        volume = pool.init_volume(
            vm,
            {
                "name": "store-data",
                "pool": "nixome-closure",
                "vid": "domains/sys-config-vault/generations/1/store-data",
                "size": 512,
                "rw": False,
                "snap_on_start": False,
                "save_on_stop": True,
                "source": None,
                "revisions_to_keep": 0,
                "domain_name": "sys-config-vault",
                "generation": "1",
                "closure_hash": "abc",
                "view_kind": "erofs-blob-map",
                "view_path": "/dev/mapper/stale-nixome-config-vault-1-abc",
                "blob_map_path": str(blob_map),
                "blob_logical_size": 512,
                "manifest_path": str(manifest),
            },
        )
        original_attach = engine.attach_blob_view
        calls: list[dict[str, str]] = []

        def filtered_attach(
            root_dir: str, domain: str, generation: str
        ) -> dict[str, object]:
            calls.append(
                {"root_dir": root_dir, "domain": domain, "generation": generation}
            )
            return {
                "device": "/dev/mapper/nixome-config-vault-1-abc",
                "backing_paths": [str(segment_blob)],
            }

        engine.attach_blob_view = filtered_attach
        device = volume.block_device()
        assert device.path == "/dev/mapper/nixome-config-vault-1-abc"
        assert os.path.realpath(device.path) != os.path.realpath(segment_blob)
        assert calls == [
            {
                "root_dir": str(pool_root),
                "domain": "sys-config-vault",
                "generation": "1",
            }
        ]
        assert "view_path" not in volume.config

        volume.path = "/dev/mapper/stale-nixome-config-vault-1-abc"
        device = volume.block_device()
        assert device.path == "/dev/mapper/nixome-config-vault-1-abc"
        assert calls == [
            {
                "root_dir": str(pool_root),
                "domain": "sys-config-vault",
                "generation": "1",
            },
            {
                "root_dir": str(pool_root),
                "domain": "sys-config-vault",
                "generation": "1",
            },
        ]

        def unsafe_attach(
            _root_dir: str, _domain: str, _generation: str
        ) -> dict[str, object]:
            return {
                "device": str(segment_blob),
                "backing_paths": [str(segment_blob)],
            }

        engine.attach_blob_view = unsafe_attach
        volume.path = None
        try:
            volume.block_device()
        except StoragePoolException as exc:
            assert "global Nixome backing storage" in str(exc)
        else:
            raise AssertionError("global backing storage was exposed to the guest")
        finally:
            engine.attach_blob_view = original_attach

        result_path.write_text(
            json.dumps(
                {
                    "schema": "nixome.closure-volume-filtered-device.v1",
                    "status": "passed",
                    "guest_device": device.path,
                    "backing_paths": [str(segment_blob)],
                    "backing_storage_exposed": False,
                },
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
        result = json.loads(result_path.read_text())
        assert result["schema"] == "nixome.closure-volume-filtered-device.v1"
        assert result["status"] == "passed"
        assert result["guest_device"] == "/dev/mapper/nixome-config-vault-1-abc"
        assert result["backing_storage_exposed"] is False
    finally:
        for name, module in original_modules.items():
            if module is None:
                sys.modules.pop(name, None)
            else:
                sys.modules[name] = module


def check_config_system_apply_boot(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    candidate_dir = work_dir / "candidate"
    apply_log_dir = work_dir / "logs"
    capabilities = work_dir / "xen-capabilities"
    host_system_source = candidate_dir / "dom0-system"
    state_dir = work_dir / "state"
    etc_dir = work_dir / "etc"
    system_profile = work_dir / "nixome-system-profile"
    host_system_profile = work_dir / "host-system-profile"
    host_current_system_link = work_dir / "current-system"
    validated_tar = work_dir / "config-candidate-validated.tar"
    deployment_tar_capture = work_dir / "deployment.tar"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (
        bin_dir,
        candidate_dir / "nixome",
        apply_log_dir,
        host_system_source / "bin",
        etc_dir,
    ):
        path.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    shutil.copy2(
        repo_root / "lifecycle/misc/lib/system.nix",
        candidate_dir / "nixome/system-schema.nix",
    )
    _write_executable(
        host_system_source / "bin/switch-to-configuration",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        printf '%s\n' "$1" > "$NIXOME_SYSTEM_STATE_DIR/host-switch-action"
        printf '%s\n' "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)" > "$NIXOME_SYSTEM_STATE_DIR/host-switch-source"
        if [ "$1" != "boot" ]; then
          ln -sfn "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)" "$NIXOME_HOST_CURRENT_SYSTEM_LINK"
        fi
        """,
    )
    _write_flake(
        candidate_dir / "flake.nix",
        """
        {
          outputs =
            { self }:
            let
              systemSchema = import ./nixome/system-schema.nix;
            in
            {
              nixome.topology = {
                schema = "nixome.qubes-topology.v1";
                templates = { };
                kernels = { };
                domains = { };
                prefs = { };
                qrexec_policy.rules = [ ];
              };

              nixome.system = systemSchema.mkSystem {
                dom0.nixos.system = {
                  source = "dom0-system";
                  switchAction = "boot";
                  reason = "install next dom0 boot generation";
                };
              };
            };
        }
        """,
    )
    commit = _git_commit_all(
        candidate_dir,
        "fixture boot candidate",
        name="Nixome Test",
        email="nixome-test@localhost",
    )
    expected_host_system_source = state_dir / f"sources/{commit}/dom0.nixos.system"
    _run(
        [
            "git",
            "bundle",
            "create",
            str(work_dir / "config.bundle"),
            "HEAD",
            "refs/heads/main",
        ],
        cwd=candidate_dir,
    )
    _run(
        ["nix", "eval", "--json", f"{candidate_dir}#nixome.topology"],
        stdout=work_dir / "topology.json",
    )
    _run(
        ["nix", "eval", "--json", f"{candidate_dir}#nixome.system"],
        stdout=work_dir / "system.json",
    )
    (work_dir / "validation.json").write_text(
        json.dumps(
            {
                "schema": "nixome.config-validation.v1",
                "status": "passed",
                "commit": commit,
                "checks": [],
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    (work_dir / "manifest.json").write_text(
        json.dumps(
            {
                "schema": "nixome.config-candidate.v1",
                "commit": commit,
                "branch": "main",
                "source_role": "sys-config-staging",
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    with tarfile.open(validated_tar, "w") as tar:
        for member in (
            "config.bundle",
            "validation.json",
            "manifest.json",
            "topology.json",
            "system.json",
        ):
            tar.add(work_dir / member, arcname=member)

    _setup_fake_command_dir(
        bin_dir,
        {
            "nixome-config-archive-candidate": f"""
                #!/usr/bin/env bash
                set -euo pipefail
                cp {validated_tar} "$NIXOME_LOG_DIR/config-candidate-validated.tar"
                jq -c -n \\
                  --arg schema "nixome.config-archive.v1" \\
                  --arg status "passed" \\
                  --arg candidate_commit "{commit}" \\
                  '{{
                    schema: $schema,
                    status: $status,
                    candidate_commit: $candidate_commit,
                    validation_commit: $candidate_commit,
                    validation: {{
                      schema: "nixome.config-validation.v1",
                      status: "passed",
                      commit: $candidate_commit
                    }},
                    vault_archive: {{
                      schema: "nixome.config-vault-archive.v1",
                      status: "passed",
                      candidate_commit: $candidate_commit
                    }}
                  }}' > "$NIXOME_LOG_DIR/config-archive.json"
            """,
            "nixome-setup-config": """
                #!/usr/bin/env bash
                set -euo pipefail
                jq -c -n \
                  --arg schema "nixome.setup-config.v1" \
                  --arg status "passed" \
                  '{schema: $schema, status: $status, domains_changed: 0, templates_changed: 0}' \
                  > "$NIXOME_LOG_DIR/setup-config.json"
            """,
            "qrexec-client": """
                #!/usr/bin/env bash
                set -euo pipefail
                cat > "$DEPLOYMENT_TAR_CAPTURE_FIXTURE"
                jq -c -n \
                  --arg schema "nixome.config-vault-deployment.v1" \
                  --arg status "passed" \
                  --arg candidate_commit "$CANDIDATE_COMMIT_FIXTURE" \
                  '{
                    schema: $schema,
                    status: $status,
                    deployment_status: "passed",
                    candidate_commit: $candidate_commit,
                    vault_commit: "fixture-vault"
                  }'
            """,
            "systemctl": """
                #!/usr/bin/env bash
                set -euo pipefail
                case "$1" in
                is-active)
                  exit 0
                  ;;
                esac
                printf 'unexpected systemctl args:' >&2
                printf ' [%s]' "$@" >&2
                printf '\n' >&2
                exit 1
            """,
        },
    )
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "CANDIDATE_COMMIT_FIXTURE": commit,
            "DEPLOYMENT_TAR_CAPTURE_FIXTURE": str(deployment_tar_capture),
            "NIXOME_XEN_CAPABILITIES": str(capabilities),
            "NIXOME_LOG_DIR": str(apply_log_dir),
            "NIXOME_USE_HOST_NIX_STORE": "1",
            "NIXOME_SYSTEM_STATE_DIR": str(state_dir),
            "NIXOME_SYSTEM_ETC_DIR": str(etc_dir),
            "NIXOME_SYSTEM_PROFILE": str(system_profile),
            "NIXOME_HOST_SYSTEM_PROFILE": str(host_system_profile),
            "NIXOME_HOST_CURRENT_SYSTEM_LINK": str(host_current_system_link),
        }
    )
    _run(
        [
            "bash",
            str(
                repo_root / "lifecycle/runtime/tools/scripts/apply-config-candidate.sh"
            ),
            "--apply",
        ],
        cwd=repo_root,
        env=env,
        stdout=work_dir / "stdout.log",
        stderr=work_dir / "stderr.log",
    )

    apply_payload = json.loads((apply_log_dir / "config-apply.json").read_text())
    assert apply_payload["schema"] == "nixome.config-apply.v1"
    assert apply_payload["status"] == "passed"
    assert apply_payload["candidate_commit"] == commit
    system_apply = apply_payload["system_apply_result"]
    assert system_apply["status"] == "passed"
    assert system_apply["profile_status"] == "skipped"
    assert system_apply["host_system_status"] == "passed"
    assert system_apply["host_system_source"] == str(expected_host_system_source)
    assert system_apply["host_system_switch_action"] == "boot"
    assert system_apply["host_current_system"] == ""
    assert system_apply["reboot_required"] is True
    assert len(system_apply["reboot_actions"]) == 1
    assert system_apply["reboot_actions"][0]["type"] == "dom0-nixos-system"
    assert system_apply["reboot_reasons"] == ["install next dom0 boot generation"]
    assert apply_payload["deployment"]["status"] == "passed"
    assert apply_payload["deployment"]["system_apply"]["status"] == "passed"
    assert apply_payload["deployment"]["system_apply"]["reboot_required"] is True
    assert len(apply_payload["deployment"]["system_apply"]["reboot_actions"]) == 1

    last_system = json.loads((state_dir / "last-system.json").read_text())
    assert last_system["schema"] == "nixome.system.v1"
    assert last_system["dom0"]["nixos"]["system"]["switch_action"] == "boot"
    assert last_system["plan"]["reboot_required"] is True
    assert len(last_system["plan"]["reboot_actions"]) == 1
    assert last_system["plan"]["reboot_actions"][0]["type"] == "dom0-nixos-system"
    current_deployment = json.loads((state_dir / "current-deployment.json").read_text())
    assert current_deployment["schema"] == "nixome.config-deployment.v1"
    assert current_deployment["status"] == "passed"
    assert current_deployment["system_apply"]["reboot_required"] is True
    assert len(current_deployment["system_apply"]["reboot_actions"]) == 1
    assert (state_dir / "current-host-system-switch-action").read_text().strip() == (
        "boot"
    )
    assert (state_dir / "host-switch-action").read_text().strip() == "boot"
    assert (state_dir / "current-host-system").read_text().strip() == str(
        expected_host_system_source
    )
    assert (state_dir / "host-switch-source").read_text().strip() == str(
        expected_host_system_source
    )
    assert os.readlink(host_system_profile) == str(expected_host_system_source)
    assert not host_current_system_link.exists()


def check_setup_config_nix_native_running_storage(
    repo_root: Path, log_dir: Path
) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    topology_path = work_dir / "topology.json"
    capabilities = work_dir / "xen-capabilities"
    setup_log_dir = work_dir / "logs"
    policy_dir = work_dir / "policy.d"
    policy_file = policy_dir / "90-nixome.policy"
    state_dir = work_dir / "state"
    mutation_log = work_dir / "mutations.log"
    qubes_xml = work_dir / "qubes.xml"
    old_system_source = work_dir / "old/system-source"
    old_root_source = work_dir / "old/root-source"
    old_store_source = work_dir / "old/store-source"
    new_system_source = work_dir / "new/system-source"
    new_root_source = work_dir / "new/root-source"
    new_store_source = work_dir / "new/store-source"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (
        bin_dir,
        setup_log_dir,
        policy_dir,
        state_dir,
        old_system_source,
        old_root_source,
        old_store_source,
        new_system_source,
        new_root_source,
        new_store_source,
    ):
        path.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    for source in (old_system_source, new_system_source):
        _write_executable(
            source / "init",
            """
            #!/bin/sh
            exec /init
            """,
        )
    (old_root_source / "root.img").write_text("old-root\n", encoding="utf-8")
    (new_root_source / "root.img").write_text("new-root\n", encoding="utf-8")
    (old_store_source / "store-paths").write_text(
        str(old_system_source) + "\n", encoding="utf-8"
    )
    (new_store_source / "store-paths").write_text(
        str(new_system_source) + "\n", encoding="utf-8"
    )
    for store_source, name in (
        (old_store_source, "old-store"),
        (new_store_source, "new-store"),
    ):
        (store_source / "manifest.json").write_text(
            json.dumps(
                {
                    "schema": "nixome.store-closure-manifest.v1",
                    "name": name,
                    "read_only": True,
                },
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
    mutation_log.touch()
    qubes_xml.write_text(
        """
        <qubes><domains><domain><properties>
          <property name="name">sys-config-staging</property>
          <property name="netvm"></property>
        </properties></domain></domains></qubes>
        """,
        encoding="utf-8",
    )
    policy_file.write_text(
        "\n".join(
            (
                "# Managed by Nixome setup-config.",
                "# Source: nixome.qubes.topology.qrexec_policy.",
                "# Bootstrap config transport policy remains in 30-nixome-config.policy.",
                "",
            )
        ),
        encoding="utf-8",
    )
    topology_path.write_text(
        json.dumps(
            {
                "schema": "nixome.qubes-topology.v1",
                "templates": {},
                "kernels": {},
                "domains": {
                    "sys-config-staging": {
                        "class": "StandaloneVM",
                        "template": None,
                        "label": "green",
                        "netvm": None,
                        "prefs": {
                            "autostart": False,
                            "memory": 1024,
                            "virt_mode": "pvh",
                        },
                        "features": {
                            "nixome-managed": "1",
                            "nixome-role": "sys-config-staging",
                            "qrexec": "1",
                        },
                        "services": {},
                        "backend": {
                            "type": "nix-closure",
                            "template": None,
                            "system": {
                                "source": str(new_system_source),
                                "source_ref": None,
                            },
                            "kernel": {
                                "name": "old-kernel",
                                "source": None,
                                "source_ref": None,
                            },
                            "initrd": {
                                "name": None,
                                "source": None,
                                "source_ref": None,
                            },
                            "modules": {
                                "name": None,
                                "source": None,
                                "source_ref": None,
                            },
                            "root": {
                                "mode": "qubes-root-image",
                                "source": str(new_root_source),
                                "source_ref": None,
                                "read_only": False,
                            },
                            "store": {
                                "mode": "shared-closure",
                                "source": str(new_store_source),
                                "source_ref": None,
                                "view_backend": "erofs-objectmap",
                                "mountpoint": "/nix/store",
                                "blob_device": "/dev/xvdf",
                                "chunk_size": 65536,
                                "device": "/dev/xvde",
                                "read_only": True,
                            },
                            "state": {
                                "persistent": "config-staging-private",
                                "volatile": None,
                            },
                        },
                    }
                },
                "prefs": {},
                "qrexec_policy": {"rules": []},
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    _setup_fake_command_dir(
        bin_dir,
        {
            "qvm-ls": """
                #!/usr/bin/env bash
                case "$*" in
                  *"--fields NAME,STATE"*)
                    printf 'sys-config-staging|Running\\n'
                    ;;
                  *"--fields NAME,CLASS"*)
                    printf 'sys-config-staging|StandaloneVM\\n'
                    ;;
                  *)
                    printf 'sys-config-staging\\n'
                    ;;
                esac
            """,
            "qvm-prefs": f"""
                #!/usr/bin/env bash
                if [ "$#" -eq 2 ]; then
                  case "$1:$2" in
                    sys-config-staging:label) printf 'green\\n' ;;
                    sys-config-staging:netvm) printf 'none\\n' ;;
                    sys-config-staging:autostart) printf 'False\\n' ;;
                    sys-config-staging:memory) printf '1024\\n' ;;
                    sys-config-staging:virt_mode) printf 'pvh\\n' ;;
                    sys-config-staging:kernel) printf 'old-kernel\\n' ;;
                    sys-config-staging:kernelopts) printf 'console=hvc0 init=/sbin/init nixome.system={old_system_source} nixome.storedev=/dev/xvde nixome.storefmt=erofs nixome.storeblobdev=/dev/xvdf\\n' ;;
                    *) exit 1 ;;
                  esac
                  exit 0
                fi
                printf 'qvm-prefs %s\\n' "$*" >> "$MUTATION_LOG"
            """,
            "qvm-features": f"""
                #!/usr/bin/env bash
                if [ "$#" -eq 2 ]; then
                  case "$1:$2" in
                    sys-config-staging:nixome-backend) printf 'nix-closure\\n' ;;
                    sys-config-staging:nixome-system-source) printf '{old_system_source}\\n' ;;
                    sys-config-staging:nixome-root-source) printf '{old_root_source}\\n' ;;
                    sys-config-staging:nixome-store-source) printf '{old_store_source}\\n' ;;
                    sys-config-staging:nixome-store-mode) printf 'shared-closure\\n' ;;
                    sys-config-staging:nixome-store-view-backend) printf 'erofs-objectmap\\n' ;;
                    sys-config-staging:nixome-store-generation) printf '1\\n' ;;
                    sys-config-staging:nixome-store-closure-hash) printf 'old-closure\\n' ;;
                    sys-config-staging:nixome-store-view-image) printf '/var/lib/nixome/closure-store/domains/sys-config-staging/generations/1/store.erofs\\n' ;;
                    sys-config-staging:nixome-store-filesystem) printf 'erofs\\n' ;;
                    sys-config-staging:nixome-store-blob-map) printf '/var/lib/nixome/closure-store/domains/sys-config-staging/generations/1/store.blob-map.json\\n' ;;
                    sys-config-staging:nixome-store-blob-volume) printf 'store-data\\n' ;;
                    sys-config-staging:nixome-store-blob-device) printf '/dev/xvdf\\n' ;;
                    sys-config-staging:nixome-store-mountpoint) printf '/nix/store\\n' ;;
                    sys-config-staging:nixome-store-device) printf '/dev/xvde\\n' ;;
                    sys-config-staging:nixome-store-read-only) printf 'True\\n' ;;
                    sys-config-staging:nixome-managed) printf '1\\n' ;;
                    sys-config-staging:nixome-role) printf 'sys-config-staging\\n' ;;
                    sys-config-staging:qrexec) printf '1\\n' ;;
                    *) exit 1 ;;
                  esac
                  exit 0
                fi
                printf 'qvm-features %s\\n' "$*" >> "$MUTATION_LOG"
            """,
            "systemctl": """
                #!/usr/bin/env bash
                case "$1" in
                  is-active|reload|try-restart)
                    exit 0
                    ;;
                esac
                exit 1
            """,
            "qvm-create": """
                #!/usr/bin/env bash
                printf 'unexpected mutation: %s %s\\n' "$0" "$*" >> "$MUTATION_LOG"
                exit 97
            """,
            "qvm-template-postprocess": """
                #!/usr/bin/env bash
                printf 'unexpected mutation: %s %s\\n' "$0" "$*" >> "$MUTATION_LOG"
                exit 97
            """,
            "qvm-volume": """
                #!/usr/bin/env bash
                printf 'unexpected mutation: %s %s\\n' "$0" "$*" >> "$MUTATION_LOG"
                exit 97
            """,
            "nixome-nix-domain-storage": """
                #!/usr/bin/env bash
                printf 'unexpected mutation: %s %s\\n' "$0" "$*" >> "$MUTATION_LOG"
                exit 97
            """,
            "qvm-service": """
                #!/usr/bin/env bash
                if [ "$#" -eq 2 ]; then
                  exit 1
                fi
                printf 'qvm-service %s\\n' "$*" >> "$MUTATION_LOG"
            """,
            "qubes-prefs": """
                #!/usr/bin/env bash
                if [ "$#" -eq 1 ]; then
                  exit 1
                fi
                printf 'qubes-prefs %s\\n' "$*" >> "$MUTATION_LOG"
            """,
            "qubesd-query": """
                #!/usr/bin/env bash
                exit 0
            """,
        },
    )
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "MUTATION_LOG": str(mutation_log),
            "NIXOME_QUBES_XML": str(qubes_xml),
            "NIXOME_QUBES_POLICY_DIR": str(policy_dir),
            "NIXOME_SYSTEM_STATE_DIR": str(state_dir),
            "NIXOME_XEN_CAPABILITIES": str(capabilities),
            "NIXOME_LOG_DIR": str(setup_log_dir),
        }
    )
    _run(
        [
            "bash",
            str(repo_root / "lifecycle/misc/tools/scripts/setup-config.sh"),
            "--apply",
            "--topology",
            str(topology_path),
        ],
        cwd=repo_root,
        env=env,
        stdout=setup_log_dir / "stdout.log",
        stderr=setup_log_dir / "stderr.log",
    )
    setup = json.loads((setup_log_dir / "setup-config.json").read_text())
    assert setup["schema"] == "nixome.setup-config.v1"
    assert setup["status"] == "passed"
    assert setup["mode"] == "apply"
    assert setup["domains_changed"] == 0
    assert setup["root_images_changed"] == 0
    assert setup["store_volumes_changed"] == 0
    assert setup["store_views_changed"] == 0
    assert setup["restart_required"] is True
    assert setup["live_actions"] == []
    assert len(setup["restart_actions"]) == 3
    restart_reason = (
        "domain is running; stop it and re-run setup-config for this storage change "
        "to take effect"
    )
    for expected_type, expected_value in (
        ("domain-root", str(new_root_source)),
        ("domain-store", str(new_store_source)),
        ("domain-system", str(new_system_source)),
    ):
        assert any(
            action["type"] == expected_type
            and action["action"] == "restart"
            and action["domain"] == "sys-config-staging"
            and action["field"] == "source"
            and action["value"] == expected_value
            and action["state"] == "Running"
            and action["reason"] == restart_reason
            for action in setup["restart_actions"]
        )

    current_setup = json.loads((state_dir / "current-setup.json").read_text())
    assert current_setup["schema"] == "nixome.setup-config.v1"
    assert current_setup["status"] == "passed"
    assert current_setup["restart_required"] is True
    assert len(current_setup["restart_actions"]) == 3
    assert mutation_log.read_text(encoding="utf-8") == ""
    commands_log = (setup_log_dir / "setup-config.commands.log").read_text(
        encoding="utf-8"
    )
    assert "+ qvm-volume import sys-config-staging:root" not in commands_log
    assert (
        "+ nixome-nix-domain-storage ensure-store sys-config-staging"
        not in commands_log
    )
    assert "+ qvm-prefs sys-config-staging kernelopts" not in commands_log


def check_setup_config_nix_native(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    topology_path = work_dir / "topology.json"
    capabilities = work_dir / "xen-capabilities"
    plan_log_dir = work_dir / "plan-logs"
    apply_log_dir = work_dir / "apply-logs"
    policy_dir = work_dir / "policy.d"
    kernel_base = work_dir / "vm-kernels"
    system_source = work_dir / "system-source"
    root_source = work_dir / "root-source"
    store_source = work_dir / "store-source"
    root_image = root_source / "root.img"
    mutation_log = work_dir / "mutations.log"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (
        bin_dir,
        plan_log_dir,
        apply_log_dir,
        policy_dir,
        kernel_base,
        system_source,
        root_source,
        store_source,
    ):
        path.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    _write_executable(
        system_source / "init",
        """
        #!/bin/sh
        exec /init
        """,
    )
    root_image.touch()
    (store_source / "store-paths").write_text(
        str(system_source) + "\n", encoding="utf-8"
    )
    (store_source / "manifest.json").write_text(
        json.dumps(
            {
                "schema": "nixome.store-closure-manifest.v1",
                "name": "test-store",
                "read_only": True,
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    mutation_log.touch()
    topology_path.write_text(
        json.dumps(
            {
                "schema": "nixome.qubes-topology.v1",
                "templates": {},
                "kernels": {},
                "domains": {
                    "legacy-fedora-app": {
                        "class": "AppVM",
                        "template": "fedora-44-xfce",
                        "label": "red",
                        "netvm": None,
                        "prefs": {"autostart": False},
                        "features": {
                            "nixome-managed": "1",
                            "nixome-role": "legacy-fedora-app",
                        },
                        "services": {},
                        "backend": {
                            "type": "qubes-template",
                            "template": "fedora-44-xfce",
                        },
                    },
                    "nix-native-work": {
                        "class": "StandaloneVM",
                        "template": None,
                        "label": "green",
                        "netvm": None,
                        "prefs": {
                            "autostart": False,
                            "include_in_backups": True,
                            "kernelopts": "console=hvc0 init=/sbin/init",
                            "memory": 1536,
                            "virt_mode": "pvh",
                        },
                        "features": {
                            "nixome-managed": "1",
                            "nixome-role": "nix-native-work",
                            "qrexec": "1",
                        },
                        "services": {},
                        "backend": {
                            "type": "nix-closure",
                            "template": None,
                            "system": {
                                "source": str(system_source),
                                "source_ref": None,
                            },
                            "kernel": {
                                "name": "6.12.64-1.fc41",
                                "source": None,
                                "source_ref": None,
                            },
                            "initrd": {
                                "name": None,
                                "source": None,
                                "source_ref": None,
                            },
                            "modules": {
                                "name": None,
                                "source": None,
                                "source_ref": None,
                            },
                            "root": {
                                "mode": "qubes-root-image",
                                "source": str(root_source),
                                "source_ref": None,
                                "read_only": False,
                            },
                            "store": {
                                "mode": "shared-closure",
                                "source": str(store_source),
                                "source_ref": None,
                                "view_backend": "erofs-objectmap",
                                "blob_device": "/dev/xvdf",
                                "chunk_size": 65536,
                                "mountpoint": "/nix/store",
                                "read_only": True,
                            },
                            "state": {
                                "persistent": "nix-native-work-private",
                                "persistent_size": 8589934592,
                                "volatile": None,
                            },
                        },
                    },
                },
                "prefs": {},
                "qrexec_policy": {"rules": []},
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    _setup_fake_command_dir(
        bin_dir,
        {
            "qvm-ls": """
                #!/usr/bin/env bash
                exit 0
            """,
            "qvm-create": """
                #!/usr/bin/env bash
                printf 'qvm-create %s\\n' "$*" >> "$MUTATION_LOG"
            """,
            "qvm-volume": """
                #!/usr/bin/env bash
                if [ "$1" = info ]; then
                  cat <<'INFO'
                pool               varlibqubes
                vid                appvms/nix-native-work/private
                rw                 True
                size               2147483648
                usage              0
                INFO
                  exit 0
                fi
                printf 'qvm-volume %s\\n' "$*" >> "$MUTATION_LOG"
            """,
            "qvm-prefs": """
                #!/usr/bin/env bash
                if [ "$#" = 2 ]; then
                  exit 1
                fi
                printf 'qvm-prefs %s\\n' "$*" >> "$MUTATION_LOG"
            """,
            "qvm-features": """
                #!/usr/bin/env bash
                if [ "$#" = 2 ]; then
                  exit 1
                fi
                printf 'qvm-features %s\\n' "$*" >> "$MUTATION_LOG"
            """,
            "qvm-service": """
                #!/usr/bin/env bash
                if [ "$#" = 2 ]; then
                  exit 1
                fi
                printf 'qvm-service %s\\n' "$*" >> "$MUTATION_LOG"
            """,
            "systemctl": """
                #!/usr/bin/env bash
                case "$1" in
                  is-active|reload|try-restart)
                    exit 0
                    ;;
                esac
                exit 1
            """,
            "qubes-prefs": """
                #!/usr/bin/env bash
                printf 'unexpected command execution: %s\\n' "$0" >&2
                exit 97
            """,
            "qvm-template-postprocess": """
                #!/usr/bin/env bash
                printf 'unexpected command execution: %s\\n' "$0" >&2
                exit 97
            """,
            "qubesd-query": """
                #!/usr/bin/env bash
                printf 'unexpected command execution: %s\\n' "$0" >&2
                exit 97
            """,
            "nixome-nix-domain-storage": """
                #!/usr/bin/env bash
                printf 'nixome-nix-domain-storage %s\\n' "$*" >> "$MUTATION_LOG"
                cat <<JSON
                {"schema":"nixome.nix-domain-storage.v1","domain":"$2","volume":"store","mode":"shared-closure","source":"$4","read_only":true,"volume_created":true,"volumes_created":["store","store-data"],"view_created":true,"metadata_changed":true,"qubesd_restart_required":false,"qubesd_restarted":true,"generation":1,"closure_hash":"fixture","filesystem":"erofs","view_backend":"erofs-objectmap","view_image":"/var/lib/nixome/closure-store/domains/$2/generations/1/store.erofs","metadata_image":"/var/lib/nixome/closure-store/domains/$2/generations/1/store.erofs","blob_map":"/var/lib/nixome/closure-store/domains/$2/generations/1/store.blob-map.json","blob_volume":"store-data","blob_device":"/dev/xvdf","blob_logical_size":65536}
                JSON
            """,
        },
    )

    def run_setup(mode: str, output_dir: Path, state_dir: Path) -> None:
        env = os.environ.copy()
        env.update(
            {
                "PATH": f"{bin_dir}:{env['PATH']}",
                "MUTATION_LOG": str(mutation_log),
                "NIXOME_QUBES_KERNEL_BASE": str(kernel_base),
                "NIXOME_QUBES_POLICY_DIR": str(policy_dir),
                "NIXOME_XEN_CAPABILITIES": str(capabilities),
                "NIXOME_LOG_DIR": str(output_dir),
                "NIXOME_SYSTEM_STATE_DIR": str(state_dir),
            }
        )
        _run(
            [
                "bash",
                str(repo_root / "lifecycle/misc/tools/scripts/setup-config.sh"),
                f"--{mode}",
                "--topology",
                str(topology_path),
            ],
            cwd=repo_root,
            env=env,
            stdout=output_dir / "stdout.log",
            stderr=output_dir / "stderr.log",
        )

    run_setup("plan", plan_log_dir, work_dir / "state-plan")
    plan = json.loads((plan_log_dir / "setup-config.json").read_text())
    assert plan["schema"] == "nixome.setup-config.v1"
    assert plan["status"] == "passed"
    assert plan["mode"] == "plan"
    assert plan["domains_changed"] == 2
    assert plan["root_images_changed"] == 1
    assert plan["store_volumes_changed"] == 2
    assert plan["state_volumes_changed"] == 1
    assert plan["store_views_changed"] == 1
    assert plan["unsupported_domains_count"] == 0
    plan_commands = (plan_log_dir / "setup-config.commands.log").read_text(
        encoding="utf-8"
    )
    expected_store_command = (
        f"+ nixome-nix-domain-storage ensure-store nix-native-work {store_source} "
        f"{store_source} --mode shared-closure --system {system_source} --root "
        f"{root_source} --kernel 6.12.64-1.fc41 --device /dev/xvde --blob-device "
        "/dev/xvdf --mountpoint /nix/store --view-backend erofs-objectmap"
    )
    for expected in (
        "+ qvm-create --class StandaloneVM --label green nix-native-work",
        "+ qvm-create --class AppVM --label red --template fedora-44-xfce "
        "legacy-fedora-app",
        f"+ qvm-volume import nix-native-work:root {root_image}",
        "+ qvm-volume resize nix-native-work:private 8589934592",
        expected_store_command,
    ):
        assert expected in plan_commands
    assert plan_commands.count("+ nixome-nix-domain-storage ensure-store ") == 1

    run_setup("apply", apply_log_dir, work_dir / "state-apply")
    apply = json.loads((apply_log_dir / "setup-config.json").read_text())
    assert apply["schema"] == "nixome.setup-config.v1"
    assert apply["status"] == "passed"
    assert apply["mode"] == "apply"
    assert apply["domains_changed"] == 2
    assert apply["root_images_changed"] == 1
    assert apply["store_volumes_changed"] == 2
    assert apply["state_volumes_changed"] == 1
    assert apply["store_views_changed"] == 1
    assert apply["qubesd_restarts"] == 1
    assert apply["unsupported_domains_count"] == 0
    live_actions = apply["live_actions"]
    assert any(
        action["type"] == "domain-root"
        and action["action"] == "import"
        and action["domain"] == "nix-native-work"
        for action in live_actions
    )
    assert any(
        action["type"] == "domain-state-volume"
        and action["action"] == "resize"
        and action["domain"] == "nix-native-work"
        and action["field"] == "private"
        and action["value"] == "8589934592"
        for action in live_actions
    )
    assert any(
        action["type"] == "domain-store-view"
        and action["action"] == "materialize"
        and action["domain"] == "nix-native-work"
        for action in live_actions
    )
    for volume in ("store", "store-data"):
        assert any(
            action["type"] == "domain-store-volume"
            and action["action"] == "create"
            and action["domain"] == "nix-native-work"
            and action["value"] == volume
            for action in live_actions
        )
    assert any(
        action["type"] == "systemd"
        and action["action"] == "restart"
        and action["field"] == "service"
        and action["value"] == "qubesd.service"
        for action in live_actions
    )
    mutation_lines = mutation_log.read_text(encoding="utf-8").splitlines()
    expected_mutations = [
        "qvm-create --class AppVM --label red --template fedora-44-xfce "
        "legacy-fedora-app",
        "qvm-create --class StandaloneVM --label green nix-native-work",
        f"qvm-volume import nix-native-work:root {root_image}",
        "qvm-volume resize nix-native-work:private 8589934592",
        "qvm-prefs nix-native-work kernel 6.12.64-1.fc41",
        f"qvm-prefs nix-native-work kernelopts console=hvc0 init=/sbin/init "
        f"nixome.system={system_source} nixome.storedev=/dev/xvde "
        "nixome.storefmt=erofs nixome.storeblobdev=/dev/xvdf",
        f"qvm-features nix-native-work nixome-root-source {root_source}",
        f"qvm-features nix-native-work nixome-store-source {store_source}",
        "qvm-features nix-native-work nixome-store-mode shared-closure",
        "qvm-features nix-native-work nixome-store-view-backend erofs-objectmap",
        "qvm-features nix-native-work nixome-store-filesystem erofs",
        "qvm-features nix-native-work nixome-store-generation 1",
        "qvm-features nix-native-work nixome-store-closure-hash fixture",
        "qvm-features nix-native-work nixome-store-view-image "
        "/var/lib/nixome/closure-store/domains/nix-native-work/generations/1/store.erofs",
        "qvm-features nix-native-work nixome-store-blob-map "
        "/var/lib/nixome/closure-store/domains/nix-native-work/generations/1/store.blob-map.json",
        "qvm-features nix-native-work nixome-store-blob-volume store-data",
        "qvm-features nix-native-work nixome-store-device /dev/xvde",
        "qvm-features nix-native-work nixome-store-blob-device /dev/xvdf",
        expected_store_command.removeprefix("+ "),
    ]
    for expected in expected_mutations:
        assert expected in mutation_lines
    assert (
        sum(
            line.startswith("nixome-nix-domain-storage ensure-store ")
            for line in mutation_lines
        )
        == 1
    )


def check_config_status(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    status_log_dir = work_dir / "logs"
    state_dir = work_dir / "state"
    policy_dir = work_dir / "policy.d"
    topology_json = work_dir / "topology.json"
    current_topology_json = state_dir / "current-topology.json"
    profile_dir = work_dir / "profiles"
    store_dir = work_dir / "store"
    system_profile = profile_dir / "nixome-system"
    host_system_profile = profile_dir / "system"
    run_current_system = work_dir / "run-current-system"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (
        bin_dir,
        status_log_dir,
        state_dir,
        policy_dir,
        profile_dir,
        store_dir,
    ):
        path.mkdir(parents=True)

    topology = {
        "schema": "nixome.qubes-topology.v1",
        "templates": {},
        "kernels": {},
        "domains": {
            "sys-config-vault": {
                "class": "StandaloneVM",
                "template": None,
                "backend": {"type": "nix-closure"},
            },
            "work-fedora": {"class": "AppVM", "template": "fedora-44-xfce"},
        },
        "prefs": {},
        "qrexec_policy": {
            "rules": [
                {
                    "service": "nixome.PolicyProbe",
                    "argument": "*",
                    "source": "dom0",
                    "target": "sys-config-vault",
                    "action": "allow",
                }
            ]
        },
    }
    current_topology = json.loads(json.dumps(topology))
    current_topology["domains"]["work-fedora-from-current"] = {
        "class": "AppVM",
        "template": "fedora-44-xfce",
    }
    topology_json.write_text(
        json.dumps(topology, sort_keys=True) + "\n", encoding="utf-8"
    )
    current_topology_json.write_text(
        json.dumps(current_topology, sort_keys=True) + "\n", encoding="utf-8"
    )
    (status_log_dir / "setup-config.json").write_text(
        json.dumps(
            {
                "schema": "nixome.setup-config.v1",
                "status": "passed",
                "domains_changed": 2,
                "restart_required": True,
                "restart_actions": [
                    {
                        "type": "domain-pref",
                        "action": "restart",
                        "domain": "sys-config-vault",
                        "field": "memory",
                        "value": "1024",
                        "reason": "domain is running; restart required for this change to fully take effect",
                        "state": "Running",
                    }
                ],
                "live_actions": [
                    {
                        "type": "domain-pref",
                        "action": "set",
                        "domain": "sys-config-vault",
                        "field": "memory",
                        "value": "1024",
                    }
                ],
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    (status_log_dir / "config-apply.json").write_text(
        '{"schema":"nixome.config-apply.v1","status":"passed","candidate_commit":"abc123"}\n',
        encoding="utf-8",
    )
    (state_dir / "current-system.json").write_text(
        '{"schema":"nixome.system.v1","dom0":{"profile":{"message":"from-test"}}}\n',
        encoding="utf-8",
    )
    (state_dir / "current-plan.json").write_text(
        json.dumps(
            {
                "live_actions": [],
                "reboot_actions": [],
                "reboot_required": False,
                "reboot_reasons": [],
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    (state_dir / "last-apply.json").write_text(
        json.dumps(
            {
                "schema": "nixome.system-apply.v1",
                "status": "passed",
                "host_system_status": "passed",
                "host_system_source": str(store_dir / "host-system"),
                "host_system_switch_action": "switch",
                "reboot_required": False,
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    for filename, payload in (
        (
            "current-deployment.json",
            {
                "schema": "nixome.config-deployment.v1",
                "status": "passed",
                "deployment_id": "deployment-current",
            },
        ),
        (
            "last-deployment.json",
            {
                "schema": "nixome.config-deployment.v1",
                "status": "failed",
                "deployment_id": "deployment-last-attempt",
            },
        ),
        (
            "current-config-apply.json",
            {
                "schema": "nixome.config-apply.v1",
                "status": "passed",
                "deployment_id": "deployment-current",
            },
        ),
        (
            "last-config-apply.json",
            {
                "schema": "nixome.config-apply.v1",
                "status": "failed",
                "deployment_id": "deployment-last-attempt",
            },
        ),
    ):
        (state_dir / filename).write_text(
            json.dumps(payload, sort_keys=True) + "\n", encoding="utf-8"
        )
    for name in ("nixome-profile", "host-system", "old-host-system"):
        (store_dir / name).mkdir()
    system_profile.symlink_to(store_dir / "nixome-profile")
    (profile_dir / "system-2-link").symlink_to(store_dir / "host-system")
    host_system_profile.symlink_to("system-2-link")
    run_current_system.symlink_to(store_dir / "host-system")
    (policy_dir / "90-nixome.policy").write_text(
        "\n".join(
            (
                "# Managed by Nixome setup-config.",
                "nixome.PolicyProbe * dom0 sys-config-vault allow",
                "nixome.PolicyProbe * @anyvm @anyvm deny",
                "",
            )
        ),
        encoding="utf-8",
    )
    _setup_fake_command_dir(
        bin_dir,
        {
            "qvm-ls": """
                #!/usr/bin/env bash
                set -euo pipefail
                case "$*" in
                  "--raw-data --fields NAME,CLASS,STATE,TEMPLATE,LABEL --no-spinner")
                    cat <<'QVM'
                sys-config-vault|StandaloneVM|Running||black
                work-fedora|AppVM|Halted|fedora-44-xfce|green
                other|AppVM|Halted|fedora-43-xfce|red
                QVM
                    ;;
                  *)
                    printf 'unexpected qvm-ls args: %s\\n' "$*" >&2
                    exit 2
                    ;;
                esac
            """,
        },
    )
    out = _run(
        [
            "nix",
            "build",
            f"{repo_root}#packages.x86_64-linux.nixome-config-status",
            "--no-link",
            "--print-out-paths",
        ],
        cwd=repo_root,
    ).stdout.strip()
    wrapper = Path(_physical_store_path(repo_root, out)) / "bin/nixome-config-status"

    def run_status(output: Path, *, boot_topology: bool) -> dict[str, object]:
        env = os.environ.copy()
        env.update(
            {
                "PATH": f"{bin_dir}:{env['PATH']}",
                "NIXOME_QVM_LS_BIN": str(bin_dir / "qvm-ls"),
                "NIXOME_LOG_DIR": str(status_log_dir),
                "NIXOME_SYSTEM_STATE_DIR": str(state_dir),
                "NIXOME_QUBES_POLICY_DIR": str(policy_dir),
                "NIXOME_CONFIG_STATUS_SYSTEM_PROFILE": str(system_profile),
                "NIXOME_CONFIG_STATUS_HOST_SYSTEM_PROFILE": str(host_system_profile),
                "NIXOME_CONFIG_STATUS_RUN_CURRENT_SYSTEM": str(run_current_system),
            }
        )
        if boot_topology:
            env["NIXOME_CONFIG_STATUS_BOOT_TOPOLOGY_JSON"] = str(topology_json)
        else:
            env["NIXOME_CONFIG_STATUS_TOPOLOGY_JSON"] = str(topology_json)
        _run([str(wrapper)], cwd=repo_root, env=env, stdout=output)
        return json.loads(output.read_text())

    result = run_status(work_dir / "status.json", boot_topology=False)
    assert result["schema"] == "nixome.config-status.v1"
    assert result["status"] == "applied"
    assert result["topology"]["present"] is True
    assert "sys-config-vault" in result["topology"]["domain_names"]
    assert result["topology"]["template_names"] == []
    assert result["topology"]["qrexec_policy_rule_count"] == 1
    assert result["qvm"]["available"] is True
    assert result["qvm"]["status"] == "ok"
    assert len(result["qvm"]["domains"]) == 3
    assert len(result["qvm"]["managed_domains"]) == 2
    assert result["qrexec_policy"]["exists"] is True
    assert result["qrexec_policy"]["rule_count"] == 2
    assert result["pending_reboot"]["required"] is False
    assert result["pending_reboot"]["actions"] == []
    assert result["pending_reboot"]["reasons"] == []
    host_system_target = str(store_dir / "host-system")
    profile_target = str(store_dir / "nixome-profile")
    assert result["host_generation"]["active"] is True
    assert result["host_generation"]["current_source"] == host_system_target
    assert result["host_generation"]["staged_source"] == ""
    assert result["host_generation"]["profile"]["resolved_target"] == host_system_target
    assert result["host_generation"]["running"]["resolved_target"] == host_system_target
    assert result["host_generation"]["last_apply"]["source"] == host_system_target
    assert result["host_generation"]["last_apply"]["switch_action"] == "switch"
    assert result["host_generation"]["pending_reboot"]["required"] is False
    pending_vm_restarts = result["pending_vm_restarts"]
    assert pending_vm_restarts["required"] is True
    assert len(pending_vm_restarts["actions"]) == 1
    assert pending_vm_restarts["actions"][0]["domain"] == "sys-config-vault"
    assert pending_vm_restarts["domains"] == ["sys-config-vault"]
    assert pending_vm_restarts["reasons"] == [
        "domain is running; restart required for this change to fully take effect"
    ]
    assert result["setup_source"] == "log"
    assert result["setup"]["status"] == "passed"
    assert len(result["setup"]["restart_actions"]) == 1
    assert result["apply"]["status"] == "passed"
    assert result["deployment"]["current"]["deployment_id"] == "deployment-current"
    assert (
        result["deployment"]["last_attempt"]["deployment_id"]
        == "deployment-last-attempt"
    )
    assert result["deployment"]["current_apply"]["status"] == "passed"
    assert result["deployment"]["last_apply_result"]["status"] == "failed"
    system = result["system"]
    assert system["current_system"]["dom0"]["profile"]["message"] == "from-test"
    assert system["current_plan"]["reboot_actions"] == []
    assert system["current_plan"]["reboot_required"] is False
    assert system["pending_reboot"]["required"] is False
    assert system["host_generation"]["active"] is True
    assert system["host_generation"]["current_source"] == host_system_target
    assert system["host_generation"]["last_apply"]["source"] == host_system_target
    assert system["last_apply"]["host_system_status"] == "passed"
    assert system["profile_target"] == profile_target
    assert system["profile_resolved_target"] == profile_target
    assert system["host_system_profile_target"] == "system-2-link"
    assert system["host_system_profile_resolved_target"] == host_system_target
    assert system["run_current_system_target"] == host_system_target
    assert system["run_current_system_resolved_target"] == host_system_target

    current_result = run_status(work_dir / "status-current.json", boot_topology=True)
    assert current_result["schema"] == "nixome.config-status.v1"
    assert current_result["topology"]["source"] == "current"
    assert (
        current_result["paths"]["topology"]
        == current_result["paths"]["current_topology"]
    )
    assert current_result["topology"]["boot_present"] is True
    assert current_result["topology"]["current_present"] is True
    assert "work-fedora-from-current" in current_result["topology"]["domain_names"]
    assert len(current_result["qvm"]["managed_domains"]) == 2

    (state_dir / "current-plan.json").write_text(
        json.dumps(
            {
                "live_actions": [],
                "reboot_actions": [
                    {
                        "type": "dom0-nixos-system",
                        "action": "boot",
                        "reason": "install next dom0 boot generation",
                    }
                ],
                "reboot_required": True,
                "reboot_reasons": ["install next dom0 boot generation"],
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    run_current_system.unlink()
    run_current_system.symlink_to(store_dir / "old-host-system")
    reboot_result = run_status(work_dir / "status-reboot.json", boot_topology=True)
    assert reboot_result["schema"] == "nixome.config-status.v1"
    assert reboot_result["pending_reboot"]["required"] is True
    assert len(reboot_result["pending_reboot"]["actions"]) == 1
    assert reboot_result["pending_reboot"]["actions"][0]["type"] == (
        "dom0-nixos-system"
    )
    assert reboot_result["pending_reboot"]["reasons"] == [
        "install next dom0 boot generation"
    ]
    assert reboot_result["host_generation"]["active"] is False
    assert reboot_result["host_generation"]["current_source"] == str(
        store_dir / "old-host-system"
    )
    assert reboot_result["host_generation"]["staged_source"] == host_system_target
    assert reboot_result["host_generation"]["pending_reboot"]["required"] is True
    assert reboot_result["system"]["pending_reboot"]["required"] is True
    assert len(reboot_result["system"]["pending_reboot"]["actions"]) == 1
    assert reboot_result["system"]["host_generation"]["active"] is False

    run_current_system.unlink()
    run_current_system.symlink_to(store_dir / "host-system")
    consumed_result = run_status(
        work_dir / "status-reboot-consumed.json", boot_topology=True
    )
    assert consumed_result["schema"] == "nixome.config-status.v1"
    assert consumed_result["pending_reboot"]["required"] is False
    assert consumed_result["pending_reboot"]["actions"] == []
    assert consumed_result["pending_reboot"]["reasons"] == []
    assert consumed_result["host_generation"]["active"] is True
    assert consumed_result["host_generation"]["current_source"] == host_system_target
    assert consumed_result["host_generation"]["staged_source"] == ""
    assert consumed_result["host_generation"]["pending_reboot"]["required"] is False
    assert consumed_result["pending_vm_restarts"]["required"] is True
    assert consumed_result["system"]["pending_reboot"]["required"] is False
    assert consumed_result["system"]["pending_reboot"]["actions"] == []
    assert consumed_result["system"]["pending_reboot"]["reasons"] == []
    assert consumed_result["system"]["host_generation"]["active"] is True
    assert consumed_result["system"]["current_plan"]["reboot_required"] is True
    assert len(consumed_result["system"]["current_plan"]["reboot_actions"]) == 1

    (status_log_dir / "setup-config.json").rename(
        status_log_dir / "setup-config.json.saved"
    )
    (state_dir / "current-setup.json").write_text(
        json.dumps(
            {
                "schema": "nixome.setup-config.v1",
                "status": "passed",
                "domains_changed": 0,
                "restart_required": False,
                "restart_actions": [],
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    current_setup_result = run_status(
        work_dir / "status-current-setup.json", boot_topology=True
    )
    assert current_setup_result["schema"] == "nixome.config-status.v1"
    assert current_setup_result["setup_source"] == "current"
    assert current_setup_result["paths"]["current_setup"] != ""
    assert current_setup_result["setup"]["status"] == "passed"
    assert current_setup_result["setup_log"] is None
    assert current_setup_result["current_setup"]["status"] == "passed"
    assert current_setup_result["pending_vm_restarts"]["required"] is False
    assert current_setup_result["pending_vm_restarts"]["actions"] == []
    assert current_setup_result["pending_vm_restarts"]["domains"] == []
    assert current_setup_result["pending_vm_restarts"]["reasons"] == []


def check_config_system_apply(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    bin_dir = work_dir / "bin"
    candidate_dir = work_dir / "candidate"
    apply_log_dir = work_dir / "logs"
    capabilities = work_dir / "xen-capabilities"
    profile_source = candidate_dir / "dom0-profile"
    host_system_source = candidate_dir / "dom0-system"
    state_dir = work_dir / "state"
    etc_dir = work_dir / "etc"
    system_profile = work_dir / "nixome-system-profile"
    host_system_profile = work_dir / "host-system-profile"
    host_current_system_link = work_dir / "current-system"
    validated_tar = work_dir / "config-candidate-validated.tar"
    deployment_tar_capture = work_dir / "deployment.tar"
    read_only_topology_target = work_dir / "read-only-boot-topology.json"
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (
        bin_dir,
        candidate_dir,
        apply_log_dir,
        etc_dir,
        profile_source / "bin",
        profile_source / "share/nixome",
        host_system_source / "bin",
        host_system_source / "etc",
    ):
        path.mkdir(parents=True)
    capabilities.write_text("control_d\n", encoding="utf-8")
    read_only_topology_target.write_text(
        '{"schema":"nixome.qubes-topology.v1","domains":{}}\n', encoding="utf-8"
    )
    read_only_topology_target.chmod(0o444)
    (etc_dir / "qubes-topology.json").symlink_to(read_only_topology_target)
    (profile_source / "share/nixome/profile.json").write_text(
        '{"schema":"nixome.dom0-profile.v1","message":"profile-from-test","built_by":"config-system-apply-test"}\n',
        encoding="utf-8",
    )
    _write_executable(
        profile_source / "bin/nixome-dom0-profile-activate",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        install -d -m 0755 "$NIXOME_SYSTEM_STATE_DIR" "$NIXOME_SYSTEM_ETC_DIR"
        cp "$(dirname "$0")/../share/nixome/profile.json" "$NIXOME_SYSTEM_STATE_DIR/current-profile.json"
        cp "$NIXOME_SYSTEM_JSON" "$NIXOME_SYSTEM_STATE_DIR/current-system.json"
        cp "$NIXOME_SYSTEM_PLAN_JSON" "$NIXOME_SYSTEM_STATE_DIR/current-plan.json"
        cp "$(dirname "$0")/../share/nixome/profile.json" "$NIXOME_SYSTEM_ETC_DIR/system-profile.json"
        printf 'activated\n' > "$NIXOME_SYSTEM_STATE_DIR/profile-activated"
        """,
    )
    (host_system_source / "etc/inside-config-message").write_text(
        "profile-from-test\n", encoding="utf-8"
    )
    _write_executable(
        host_system_source / "bin/switch-to-configuration",
        """
        #!/usr/bin/env bash
        set -euo pipefail
        if [ "${NIXOME_FAIL_HOST_SWITCH:-0}" = "1" ]; then
          printf 'fixture host switch failed\n' >&2
          exit 42
        fi
        printf '%s\n' "$1" > "$NIXOME_SYSTEM_STATE_DIR/host-switch-action"
        printf '%s\n' "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)" > "$NIXOME_SYSTEM_STATE_DIR/host-switch-source"
        ln -sfn "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)" "$NIXOME_HOST_CURRENT_SYSTEM_LINK"
        cp "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)/etc/inside-config-message" "$NIXOME_SYSTEM_ETC_DIR/inside-config-applied-message"
        """,
    )
    _write_flake(
        candidate_dir / "flake.nix",
        """
        {
          outputs =
            { self }:
            {
              nixome.topology = {
                schema = "nixome.qubes-topology.v1";
                templates = { };
                kernels = { };
                domains = { };
                prefs = { };
                qrexec_policy.rules = [ ];
              };

              nixome.system = {
                schema = "nixome.system.v1";
                dom0 = {
                  profile = {
                    source = "dom0-profile";
                    source_ref = null;
                    activation_command = "bin/nixome-dom0-profile-activate";
                    activation_action = "run";
                    message = "profile-from-test";
                  };
                  packages = [
                    { name = "nixome-dom0-profile"; }
                  ];
                  systemd.services = {
                    "nixome-test-restart.service" = {
                      action = "restart";
                      reason = "restart fixture service";
                    };
                    "nixome-test-reload.service" = {
                      action = "reload";
                      reason = "reload fixture service";
                    };
                  };
                  nixos = {
                    system = {
                      enable = true;
                      reason = "dom0 system change requires reboot";
                      source = "dom0-system";
                      source_ref = null;
                      switch_action = "switch";
                    };
                    kernel = {
                      enable = false;
                      reason = "dom0 kernel change requires reboot";
                      source = null;
                      source_ref = null;
                    };
                    xen = {
                      enable = false;
                      reason = "dom0 xen change requires reboot";
                      source = null;
                      source_ref = null;
                    };
                    bootloader = {
                      enable = false;
                      reason = "dom0 bootloader change requires reboot";
                      source = null;
                      source_ref = null;
                    };
                    qubes = {
                      enable = false;
                      reason = "dom0 qubes change requires reboot";
                      source = null;
                      source_ref = null;
                    };
                  };
                };
                plan = {
                  live_actions = [
                    {
                      type = "dom0-profile";
                      action = "run";
                      source = "dom0-profile";
                      source_ref = null;
                      activation_command = "bin/nixome-dom0-profile-activate";
                      message = "profile-from-test";
                    }
                    {
                      type = "dom0-nixos-system";
                      action = "switch";
                      source = "dom0-system";
                      source_ref = null;
                      reason = "dom0 system change requires reboot";
                    }
                    {
                      type = "systemd-service";
                      name = "nixome-test-restart.service";
                      action = "restart";
                      reason = "restart fixture service";
                    }
                    {
                      type = "systemd-service";
                      name = "nixome-test-reload.service";
                      action = "reload";
                      reason = "reload fixture service";
                    }
                  ];
                  reboot_actions = [ ];
                  reboot_required = false;
                  reboot_reasons = [ ];
                };
              };
            };
        }
        """,
    )
    commit = _git_commit_all(
        candidate_dir,
        "fixture candidate",
        name="Nixome Test",
        email="nixome-test@localhost",
    )
    expected_profile_source = state_dir / f"sources/{commit}/dom0.profile"
    expected_host_system_source = state_dir / f"sources/{commit}/dom0.nixos.system"
    _run(
        [
            "git",
            "bundle",
            "create",
            str(work_dir / "config.bundle"),
            "HEAD",
            "refs/heads/main",
        ],
        cwd=candidate_dir,
    )
    _run(
        ["nix", "eval", "--json", f"{candidate_dir}#nixome.topology"],
        stdout=work_dir / "topology.json",
    )
    _run(
        ["nix", "eval", "--json", f"{candidate_dir}#nixome.system"],
        stdout=work_dir / "system.json",
    )
    for filename, payload in (
        (
            "validation.json",
            {
                "schema": "nixome.config-validation.v1",
                "status": "passed",
                "commit": commit,
                "checks": [],
            },
        ),
        (
            "manifest.json",
            {
                "schema": "nixome.config-candidate.v1",
                "commit": commit,
                "branch": "main",
                "source_role": "sys-config-staging",
            },
        ),
    ):
        (work_dir / filename).write_text(
            json.dumps(payload, sort_keys=True) + "\n", encoding="utf-8"
        )
    with tarfile.open(validated_tar, "w") as tar:
        for member in (
            "config.bundle",
            "validation.json",
            "manifest.json",
            "topology.json",
            "system.json",
        ):
            tar.add(work_dir / member, arcname=member)
    _setup_fake_command_dir(
        bin_dir,
        {
            "nixome-config-archive-candidate": f"""
                #!/usr/bin/env bash
                set -euo pipefail
                cp {validated_tar} "$NIXOME_LOG_DIR/config-candidate-validated.tar"
                jq -c -n \\
                  --arg schema "nixome.config-archive.v1" \\
                  --arg status "passed" \\
                  --arg candidate_commit "{commit}" \\
                  '{{
                    schema: $schema,
                    status: $status,
                    candidate_commit: $candidate_commit,
                    validation_commit: $candidate_commit,
                    validation: {{
                      schema: "nixome.config-validation.v1",
                      status: "passed",
                      commit: $candidate_commit
                    }},
                    vault_archive: {{
                      schema: "nixome.config-vault-archive.v1",
                      status: "passed",
                      candidate_commit: $candidate_commit
                    }}
                  }}' > "$NIXOME_LOG_DIR/config-archive.json"
                printf 'archive fixture ok\\n'
            """,
            "nixome-setup-config": """
                #!/usr/bin/env bash
                set -euo pipefail
                jq -c -n \
                  --arg schema "nixome.setup-config.v1" \
                  --arg status "passed" \
                  '{schema: $schema, status: $status, domains_changed: 0, templates_changed: 0}' \
                  > "$NIXOME_LOG_DIR/setup-config.json"
                printf 'setup fixture ok\n'
            """,
            "qrexec-client": """
                #!/usr/bin/env bash
                set -euo pipefail
                cat > "$DEPLOYMENT_TAR_CAPTURE_FIXTURE"
                jq -c -n \
                  --arg schema "nixome.config-vault-deployment.v1" \
                  --arg status "passed" \
                  --arg candidate_commit "$CANDIDATE_COMMIT_FIXTURE" \
                  '{
                    schema: $schema,
                    status: $status,
                    deployment_status: "passed",
                    candidate_commit: $candidate_commit,
                    vault_commit: "fixture-vault"
                  }'
            """,
            "systemctl": """
                #!/usr/bin/env bash
                set -euo pipefail
                mkdir -p "$NIXOME_SYSTEM_STATE_DIR"
                case "$1" in
                is-active)
                  exit 0
                  ;;
                restart|reload)
                  printf '%s %s\n' "$1" "$2" >> "$NIXOME_SYSTEM_STATE_DIR/systemctl-calls"
                  printf '%s %s ok\n' "$1" "$2"
                  exit 0
                  ;;
                esac
                printf 'unexpected systemctl args:' >&2
                printf ' [%s]' "$@" >&2
                printf '\n' >&2
                exit 1
            """,
        },
    )

    def run_apply(
        *,
        output_dir: Path,
        state: Path,
        etc: Path,
        system_link: Path,
        host_link: Path,
        current_link: Path,
        deployment_capture: Path,
        fail_host_switch: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        env = os.environ.copy()
        env.update(
            {
                "PATH": f"{bin_dir}:{env['PATH']}",
                "CANDIDATE_COMMIT_FIXTURE": commit,
                "DEPLOYMENT_TAR_CAPTURE_FIXTURE": str(deployment_capture),
                "NIXOME_XEN_CAPABILITIES": str(capabilities),
                "NIXOME_LOG_DIR": str(output_dir),
                "NIXOME_USE_HOST_NIX_STORE": "1",
                "NIXOME_SYSTEM_STATE_DIR": str(state),
                "NIXOME_SYSTEM_ETC_DIR": str(etc),
                "NIXOME_SYSTEM_PROFILE": str(system_link),
                "NIXOME_HOST_SYSTEM_PROFILE": str(host_link),
                "NIXOME_HOST_CURRENT_SYSTEM_LINK": str(current_link),
            }
        )
        if fail_host_switch:
            env["NIXOME_FAIL_HOST_SWITCH"] = "1"
        return _run(
            [
                "bash",
                str(
                    repo_root
                    / "lifecycle/runtime/tools/scripts/apply-config-candidate.sh"
                ),
                "--apply",
            ],
            cwd=repo_root,
            env=env,
            stdout=work_dir / ("fail-stdout.log" if fail_host_switch else "stdout.log"),
            stderr=work_dir / ("fail-stderr.log" if fail_host_switch else "stderr.log"),
            check=False,
        )

    result = run_apply(
        output_dir=apply_log_dir,
        state=state_dir,
        etc=etc_dir,
        system_link=system_profile,
        host_link=host_system_profile,
        current_link=host_current_system_link,
        deployment_capture=deployment_tar_capture,
    )
    assert result.returncode == 0
    apply_payload = json.loads((apply_log_dir / "config-apply.json").read_text())
    assert apply_payload["schema"] == "nixome.config-apply.v1"
    assert apply_payload["status"] == "passed"
    assert apply_payload["candidate_commit"] == commit
    system_apply = apply_payload["system_apply_result"]
    assert system_apply["status"] == "passed"
    assert system_apply["profile_status"] == "passed"
    assert system_apply["host_system_status"] == "passed"
    assert system_apply["systemd_status"] == "passed"
    assert system_apply["host_system_source"] == str(expected_host_system_source)
    assert system_apply["host_system_switch_action"] == "switch"
    assert system_apply["reboot_actions"] == []
    assert system_apply["reboot_required"] is False
    assert apply_payload["systemd_actions_result"]["status"] == "passed"
    assert apply_payload["deployment"]["status"] == "passed"
    assert apply_payload["deployment"]["system_apply"]["status"] == "passed"
    assert apply_payload["deployment"]["systemd_actions"]["status"] == "passed"
    for filename in (
        "current-deployment.json",
        "last-deployment.json",
        "current-config-apply.json",
        "last-config-apply.json",
    ):
        payload = json.loads((state_dir / filename).read_text())
        assert payload["status"] == "passed"
        assert payload["candidate_commit"] == commit
    current_profile = json.loads((state_dir / "current-profile.json").read_text())
    assert current_profile["schema"] == "nixome.dom0-profile.v1"
    assert current_profile["message"] == "profile-from-test"
    current_system = json.loads((state_dir / "current-system.json").read_text())
    assert current_system["schema"] == "nixome.system.v1"
    assert current_system["dom0"]["profile"]["message"] == "profile-from-test"
    assert current_system["dom0"]["nixos"]["system"]["enable"] is True
    assert current_system["dom0"]["nixos"]["system"]["source"] == str(
        expected_host_system_source
    )
    assert current_system["dom0"]["nixos"]["system"]["switch_action"] == "switch"
    assert (
        current_system["dom0"]["systemd"]["services"]["nixome-test-restart.service"][
            "action"
        ]
        == "restart"
    )
    assert (
        current_system["dom0"]["systemd"]["services"]["nixome-test-reload.service"][
            "action"
        ]
        == "reload"
    )
    assert current_system["plan"]["reboot_actions"] == []
    assert current_system["plan"]["reboot_required"] is False
    current_topology = json.loads((state_dir / "current-topology.json").read_text())
    assert current_topology["schema"] == "nixome.qubes-topology.v1"
    assert len(current_topology["domains"]) == 0
    current_setup = json.loads((state_dir / "current-setup.json").read_text())
    assert current_setup["schema"] == "nixome.setup-config.v1"
    assert current_setup["status"] == "passed"
    assert os.readlink(etc_dir / "qubes-topology.json") == str(
        read_only_topology_target
    )
    assert (state_dir / "profile-activated").read_text().strip() == "activated"
    systemctl_calls = (state_dir / "systemctl-calls").read_text().splitlines()
    assert "restart nixome-test-restart.service" in systemctl_calls
    assert "reload nixome-test-reload.service" in systemctl_calls
    assert os.readlink(system_profile) == str(expected_profile_source)
    assert (expected_profile_source / "bin/nixome-dom0-profile-activate").is_file()
    assert os.readlink(host_system_profile) == str(expected_host_system_source)
    assert os.readlink(host_current_system_link) == str(expected_host_system_source)
    assert (expected_host_system_source / "bin/switch-to-configuration").is_file()
    assert (state_dir / "host-switch-action").read_text().strip() == "switch"
    assert (state_dir / "host-switch-source").read_text().strip() == str(
        expected_host_system_source
    )
    assert (
        etc_dir / "inside-config-applied-message"
    ).read_text().strip() == "profile-from-test"
    with tarfile.open(deployment_tar_capture) as tar:
        members = set(tar.getnames())
        for member in (
            "system.json",
            "resolved-system.json",
            "system-plan.json",
            "system-apply.json",
            "system-builds.json",
            "systemd-actions.json",
            "logs/profile-activation.log",
            "logs/systemd-actions.log",
        ):
            assert member in members
        deployment = json.load(tar.extractfile("deployment.json"))
        systemd_actions = json.load(tar.extractfile("systemd-actions.json"))
    assert systemd_actions["schema"] == "nixome.systemd-actions.v1"
    assert systemd_actions["status"] == "passed"
    assert any(
        action["name"] == "nixome-test-restart.service"
        and action["action"] == "restart"
        and action["status"] == "passed"
        for action in systemd_actions["actions"]
    )
    assert any(
        action["name"] == "nixome-test-reload.service"
        and action["action"] == "reload"
        and action["status"] == "passed"
        for action in systemd_actions["actions"]
    )
    assert deployment["artifacts"]["systemd_actions"] == "systemd-actions.json"
    assert deployment["systemd_actions"]["status"] == "passed"

    fail_log_dir = work_dir / "fail-logs"
    fail_state_dir = work_dir / "fail-state"
    fail_etc_dir = work_dir / "fail-etc"
    fail_deployment_tar = work_dir / "deployment-failed.tar"
    fail_log_dir.mkdir()
    failed = run_apply(
        output_dir=fail_log_dir,
        state=fail_state_dir,
        etc=fail_etc_dir,
        system_link=work_dir / "fail-nixome-system-profile",
        host_link=work_dir / "fail-host-system-profile",
        current_link=work_dir / "fail-current-system",
        deployment_capture=fail_deployment_tar,
        fail_host_switch=True,
    )
    assert failed.returncode != 0
    failed_apply = json.loads((fail_log_dir / "config-apply.json").read_text())
    assert failed_apply["schema"] == "nixome.config-apply.v1"
    assert failed_apply["status"] == "failed"
    assert failed_apply["reason"] == "system apply failed"
    assert failed_apply["system_apply_result"]["status"] == "failed"
    assert failed_apply["system_apply_result"]["profile_status"] == "passed"
    assert failed_apply["system_apply_result"]["host_system_status"] == "skipped"
    assert failed_apply["system_apply_result"]["reason"] == (
        "dom0 NixOS switch-to-configuration switch failed"
    )
    assert failed_apply["deployment"]["status"] == "failed"
    assert failed_apply["deployment"]["system_apply"]["status"] == "failed"
    assert failed_apply["deployment_archive"]["status"] == "passed"
    fail_topology = json.loads((fail_state_dir / "current-topology.json").read_text())
    assert fail_topology["schema"] == "nixome.qubes-topology.v1"
    assert len(fail_topology["domains"]) == 0
    last_deployment = json.loads((fail_state_dir / "last-deployment.json").read_text())
    assert last_deployment["schema"] == "nixome.config-deployment.v1"
    assert last_deployment["status"] == "failed"
    assert last_deployment["candidate_commit"] == commit
    last_apply = json.loads((fail_state_dir / "last-config-apply.json").read_text())
    assert last_apply["schema"] == "nixome.config-apply.v1"
    assert last_apply["status"] == "failed"
    assert last_apply["candidate_commit"] == commit
    assert not (fail_state_dir / "current-deployment.json").exists()
    assert not (fail_state_dir / "current-config-apply.json").exists()
    with tarfile.open(fail_deployment_tar) as tar:
        fail_deployment = json.load(tar.extractfile("deployment.json"))
        fail_system_apply = json.load(tar.extractfile("system-apply.json"))
        fail_systemd_actions = json.load(tar.extractfile("systemd-actions.json"))
    assert fail_deployment["schema"] == "nixome.config-deployment.v1"
    assert fail_deployment["status"] == "failed"
    assert fail_deployment["reason"] == "system apply failed"
    assert fail_deployment["system_apply"]["status"] == "failed"
    assert fail_system_apply["schema"] == "nixome.system-apply.v1"
    assert fail_system_apply["status"] == "failed"
    assert fail_system_apply["reason"] == (
        "dom0 NixOS switch-to-configuration switch failed"
    )
    assert fail_systemd_actions["schema"] == "nixome.systemd-actions.v1"
    assert fail_systemd_actions["status"] == "skipped"


def check_config_validator_candidate(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    candidate_dir = work_dir / "candidate"
    portable_source = work_dir / "portable-config-input-source"
    portable_source_link = work_dir / "portable-config-input"
    validation_dir = work_dir / "validation"
    staged_tar = work_dir / "config-candidate-staged.tar"
    validated_tar = work_dir / "config-candidate-validated.tar"
    fail_candidate_dir = work_dir / "failing-check-candidate"
    fail_staged_tar = work_dir / "config-candidate-failing-check.tar"
    fail_validated_tar = work_dir / "config-candidate-failing-check-validated.tar"
    if (work_dir / "nix-root").exists():
        _run(["chmod", "-R", "u+w", str(work_dir / "nix-root")], check=False)
    shutil.rmtree(work_dir, ignore_errors=True)
    for path in (
        candidate_dir / "dom0-profile/bin",
        candidate_dir / "dom0-profile/share/nixome",
        validation_dir,
        portable_source,
    ):
        path.mkdir(parents=True)
    (portable_source / "marker").write_text(
        "portable-validator-test\n", encoding="utf-8"
    )
    portable_source_link.symlink_to(portable_source)
    (candidate_dir / "dom0-profile/share/nixome/profile.json").write_text(
        '{"schema":"nixome.dom0-profile.v1","message":"validator-test","built_by":"config-validator-candidate-test"}\n',
        encoding="utf-8",
    )
    _write_executable(
        candidate_dir / "dom0-profile/bin/nixome-dom0-profile-activate",
        """
        #!/bin/sh
        set -eu
        exit 0
        """,
    )
    _write_flake(
        candidate_dir / "flake.nix",
        f"""
        {{
          description = "Nixome config validator fixture";

          inputs.portable-source = {{
            url = "path:{portable_source_link}";
            flake = false;
          }};

          outputs = {{ self, portable-source }}: let
            mkFixturePackage = name: builtins.derivation {{
              inherit name;
              system = "x86_64-linux";
              builder = "/bin/sh";
              args = [
                "-c"
                "mkdir -p $out"
              ];
            }};
          in {{
            packages.x86_64-linux.nix-work-system = mkFixturePackage "nix-work-system";
            packages.x86_64-linux.nix-work-store-closure = mkFixturePackage "nix-work-store-closure";

            nixome.topology = {{
              schema = "nixome.qubes-topology.v1";
              templates = {{ }};
              kernels = {{ }};
              domains = {{
                nix-native-work = {{
                  class = "StandaloneVM";
                  template = null;
                  label = "green";
                  netvm = null;
                  prefs = {{ }};
                  features = {{ }};
                  services = {{ }};
                  backend = {{
                    type = "nix-closure";
                    template = null;
                    system = {{
                      source = null;
                      source_ref = {{
                        type = "flake-output";
                        attr = "packages.x86_64-linux.nix-work-system";
                        subpath = "";
                      }};
                    }};
                    kernel = {{
                      name = "6.12.64-1.fc41";
                      source = null;
                      source_ref = null;
                    }};
                    initrd = {{
                      name = null;
                      source = null;
                      source_ref = null;
                    }};
                    modules = {{
                      name = null;
                      source = null;
                      source_ref = null;
                    }};
                    store = {{
                      mode = "shared-closure";
                      source = null;
                      source_ref = {{
                        type = "flake-output";
                        attr = "packages.x86_64-linux.nix-work-store-closure";
                        subpath = "";
                      }};
                      mountpoint = "/nix/store";
                      device = "/dev/xvde";
                      blob_device = "/dev/xvdf";
                      view_backend = "erofs-objectmap";
                      read_only = true;
                    }};
                    state = {{
                      persistent = "nix-native-work-private";
                      volatile = null;
                    }};
                  }};
                }};
              }};
              prefs = {{ }};
              qrexec_policy.rules = [ ];
            }};

            nixome.system = {{
              schema = "nixome.system.v1";
              dom0 = {{
                profile = {{
                  source = "dom0-profile";
                  source_ref = null;
                  activation_command = "bin/nixome-dom0-profile-activate";
                  activation_action = "none";
                  message = builtins.readFile "${{portable-source}}/marker";
                }};
                packages = [ ];
                systemd.services = {{ }};
                nixos = {{
                  system = {{
                    enable = false;
                    reason = "dom0 system change requires reboot";
                    source = null;
                    source_ref = null;
                  }};
                  kernel = {{
                    enable = false;
                    reason = "dom0 kernel change requires reboot";
                    source = null;
                    source_ref = null;
                  }};
                  xen = {{
                    enable = false;
                    reason = "dom0 xen change requires reboot";
                    source = null;
                    source_ref = null;
                  }};
                  bootloader = {{
                    enable = false;
                    reason = "dom0 bootloader change requires reboot";
                    source = null;
                    source_ref = null;
                  }};
                  qubes = {{
                    enable = false;
                    reason = "dom0 qubes change requires reboot";
                    source = null;
                    source_ref = null;
                  }};
                }};
              }};
              plan = {{
                live_actions = [ ];
                reboot_actions = [ ];
                reboot_required = false;
                reboot_reasons = [ ];
              }};
            }};
          }};
        }}
        """,
    )
    commit = _git_commit_all(
        candidate_dir,
        "validator fixture",
        name="Nixome Validator Test",
        email="nixome-validator-test@localhost",
    )
    _run(
        ["git", "bundle", "create", str(work_dir / "config.bundle"), "HEAD"],
        cwd=candidate_dir,
    )
    (work_dir / "manifest.json").write_text(
        json.dumps(
            {"schema": "nixome.config-candidate.v1", "commit": commit},
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    with tarfile.open(staged_tar, "w") as tar:
        tar.add(work_dir / "config.bundle", arcname="config.bundle")
        tar.add(work_dir / "manifest.json", arcname="manifest.json")
    validator_cmd = _run(
        [
            "nix",
            "build",
            "--no-link",
            "--print-out-paths",
            f'{repo_root}#nixosConfigurations.nixome-domain-sys-config-validator.config.environment.etc."qubes-rpc/nixome.Config.ValidateCandidate".source',
        ],
        cwd=repo_root,
    ).stdout.strip()

    def run_validator(
        input_tar: Path, output_tar: Path, stderr_path: Path, *, check: bool
    ) -> subprocess.CompletedProcess[str]:
        env = os.environ.copy()
        env.update(
            {
                "NIXOME_CONFIG_VALIDATION_DIR": str(validation_dir),
                "NIXOME_CONFIG_NIX_STORE_ROOT": str(work_dir / "nix-root"),
            }
        )
        with (
            input_tar.open("rb") as stdin,
            output_tar.open("wb") as stdout,
            stderr_path.open("wb") as stderr,
        ):
            result = subprocess.run(
                [
                    "bash",
                    "-c",
                    'source "$1"; nixome_run_store_path "$2"',
                    "bash",
                    str(repo_root / "lifecycle/misc/tools/scripts/lib/nixome-env.sh"),
                    validator_cmd,
                ],
                cwd=repo_root,
                env=env,
                check=False,
                stdin=stdin,
                stdout=stdout,
                stderr=stderr,
            )
        if check and result.returncode != 0:
            raise AssertionError(f"validator failed with exit {result.returncode}")
        return result

    run_validator(
        staged_tar, validated_tar, work_dir / "validator.stderr.log", check=True
    )
    with tarfile.open(validated_tar) as tar:
        members = set(tar.getnames())
        assert {
            "config.bundle",
            "manifest.json",
            "validation.json",
            "topology.json",
            "system.json",
        } <= members
        validation = json.load(tar.extractfile("validation.json"))
        topology = json.load(tar.extractfile("topology.json"))
        system = json.load(tar.extractfile("system.json"))
    assert validation["schema"] == "nixome.config-validation.v1"
    assert validation["status"] == "passed"
    assert any(
        check["name"] == "nix-template-source-refs-dry-run"
        and check["status"] == "passed"
        for check in validation["checks"]
    )
    assert any(
        check["name"] == "nixome-topology-json" and check["status"] == "passed"
        for check in validation["checks"]
    )
    assert any(
        check["name"] == "nixome-system-json" and check["status"] == "passed"
        for check in validation["checks"]
    )
    assert any(
        check["name"] == "nix-flake-check" and check["status"] == "passed"
        for check in validation["checks"]
    )
    assert topology["schema"] == "nixome.qubes-topology.v1"
    assert system["schema"] == "nixome.system.v1"
    assert system["dom0"]["profile"]["message"] == "portable-validator-test\n"
    refs_log = next(
        check["log"]
        for check in validation["checks"]
        if check["name"] == "nix-template-source-refs-dry-run"
    )
    assert "checking nix-domain=nix-native-work.system attr=packages.x86_64-linux.nix-work-system" in Path(
        refs_log
    ).read_text(
        encoding="utf-8"
    )
    assert "checking nix-domain=nix-native-work.store attr=packages.x86_64-linux.nix-work-store-closure" in Path(
        refs_log
    ).read_text(
        encoding="utf-8"
    )

    fail_candidate_dir.mkdir()
    _write_flake(
        fail_candidate_dir / "flake.nix",
        """
        {
          description = "Nixome config validator failing check fixture";

          outputs = { self }: {
            checks.x86_64-linux.failing = derivation {
              name = "nixome-failing-check";
              system = "x86_64-linux";
              builder = "/bin/sh";
              args = [
                "-c"
                "echo failing check >&2; exit 23"
              ];
            };

            nixome.topology = {
              schema = "nixome.qubes-topology.v1";
              templates = { };
              kernels = { };
              domains = { };
              prefs = { };
              qrexec_policy.rules = [ ];
            };

            nixome.system = {
              schema = "nixome.system.v1";
              dom0 = {
                profile = {
                  source = null;
                  source_ref = null;
                  activation_command = "bin/nixome-dom0-profile-activate";
                  activation_action = "none";
                  message = "";
                };
                packages = [ ];
                systemd.services = { };
                nixos = {
                  system = {
                    enable = false;
                    reason = "no host system change";
                    source = null;
                    source_ref = null;
                  };
                  kernel = {
                    enable = false;
                    reason = "no kernel change";
                    source = null;
                    source_ref = null;
                  };
                  xen = {
                    enable = false;
                    reason = "no xen change";
                    source = null;
                    source_ref = null;
                  };
                  bootloader = {
                    enable = false;
                    reason = "no bootloader change";
                    source = null;
                    source_ref = null;
                  };
                  qubes = {
                    enable = false;
                    reason = "no qubes package change";
                    source = null;
                    source_ref = null;
                  };
                };
              };
              plan = {
                live_actions = [ ];
                reboot_actions = [ ];
                reboot_required = false;
                reboot_reasons = [ ];
              };
            };
          };
        }
        """,
    )
    fail_commit = _git_commit_all(
        fail_candidate_dir,
        "failing flake check fixture",
        name="Nixome Validator Test",
        email="nixome-validator-test@localhost",
    )
    _run(
        [
            "git",
            "bundle",
            "create",
            str(work_dir / "failing-check.bundle"),
            "HEAD",
        ],
        cwd=fail_candidate_dir,
    )
    (work_dir / "failing-check-manifest.json").write_text(
        json.dumps(
            {"schema": "nixome.config-candidate.v1", "commit": fail_commit},
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    with tarfile.open(fail_staged_tar, "w") as tar:
        tar.add(work_dir / "failing-check.bundle", arcname="config.bundle")
        tar.add(work_dir / "failing-check-manifest.json", arcname="manifest.json")
    failed = run_validator(
        fail_staged_tar,
        fail_validated_tar,
        work_dir / "validator-failing-check.stderr.log",
        check=False,
    )
    assert failed.returncode != 0
    fail_validation = json.loads((validation_dir / f"{fail_commit}.json").read_text())
    assert fail_validation["schema"] == "nixome.config-validation.v1"
    assert fail_validation["status"] == "failed"
    assert fail_validation["commit"] == fail_commit
    assert any(
        check["name"] == "nix-flake-check" and check["status"] == "failed"
        for check in fail_validation["checks"]
    )
    assert any(
        check["name"] == "nix-flake-check-no-build" and check["status"] == "passed"
        for check in fail_validation["checks"]
    )


def check_closure_store_linux_storage(repo_root: Path, log_dir: Path) -> None:
    shutil.rmtree(log_dir, ignore_errors=True)
    log_dir.mkdir(parents=True)
    result_path = log_dir / "result.json"
    if os.geteuid() != 0:
        result_path.write_text(
            json.dumps(
                {
                    "schema": "nixome.closure-store-linux-storage.v1",
                    "status": "skipped",
                    "reason": "root or passwordless sudo is required for loop, device-mapper, and mount namespace coverage",
                },
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
        return

    # The privileged path needs real loop, device-mapper, and mount namespace
    # operations. Keep the pytest entry explicit rather than silently faking that
    # integration coverage.
    for command in ("dmsetup", "losetup", "mount", "unshare", "findmnt"):
        if shutil.which(command) is None:
            raise AssertionError(f"missing required command: {command}")
    raise AssertionError(
        "privileged closure-store-linux-storage pytest migration is not implemented"
    )


def _nix_eval_json(repo_root: Path, attr: str, *extra: str) -> object:
    return json.loads(
        _run(
            ["nix", "eval", "--json", *extra, f"{repo_root}#{attr}"], cwd=repo_root
        ).stdout.strip()
    )


def _nix_eval_raw(repo_root: Path, attr: str, *extra: str) -> str:
    return _run(
        ["nix", "eval", "--raw", *extra, f"{repo_root}#{attr}"], cwd=repo_root
    ).stdout


def check_config_topology_eval(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    shutil.rmtree(work_dir, ignore_errors=True)
    work_dir.mkdir(parents=True)

    domains = _nix_eval_json(
        repo_root, "nixosConfigurations.vm-gui-config.config.nixome.qubes.domains"
    )
    kernels = _nix_eval_json(
        repo_root, "nixosConfigurations.vm-gui-config.config.nixome.qubes.kernels"
    )
    templates = _nix_eval_json(
        repo_root, "nixosConfigurations.vm-gui-config.config.nixome.qubes.templates"
    )
    topology = _nix_eval_json(
        repo_root, "nixosConfigurations.vm-gui-config.config.nixome.qubes.topology"
    )
    network_debug_domains = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config-network-debug.config.nixome.qubes.domains",
    )
    assert domains["sys-config-vault"]["class"] == "StandaloneVM"
    assert domains["sys-config-vault"]["template"] is None
    assert domains["sys-config-vault"]["netvm"] is None
    assert domains["sys-config-vault"]["prefs"]["guivm"] is None
    assert domains["sys-config-vault"]["backend"]["type"] == "nix-closure"
    assert domains["sys-config-vault"]["backend"]["store"]["mode"] == "shared-closure"
    assert domains["sys-config-vault"]["backend"]["store"]["readOnly"] is True
    assert domains["sys-config-vault"]["features"]["nixome-role"] == "sys-config-vault"
    assert domains["sys-config-staging"]["backend"]["type"] == "nix-closure"
    assert domains["sys-config-staging"]["prefs"]["guivm"] is None
    assert (
        domains["sys-config-staging"]["features"]["nixome-role"] == "sys-config-staging"
    )
    assert domains["sys-config-staging"]["features"][
        "nixome-config-applied-message"
    ] == ("initial")
    assert domains["sys-config-validator"]["backend"]["type"] == "nix-closure"
    assert domains["sys-config-validator"]["prefs"]["guivm"] is None
    assert (
        domains["sys-config-validator"]["features"]["nixome-role"]
        == "sys-config-validator"
    )
    for config_domain in (
        "sys-config-vault",
        "sys-config-staging",
        "sys-config-validator",
    ):
        assert domains[config_domain]["prefs"]["autostart"] is False
    assert domains["sys-net"]["class"] == "StandaloneVM"
    assert domains["sys-net"]["netvm"] is None
    assert domains["sys-net"]["prefs"]["provides_network"] is True
    assert domains["sys-net"]["prefs"]["guivm"] is None
    assert domains["sys-net"]["prefs"]["virt_mode"] == "hvm"
    assert domains["sys-net"]["services"]["network-manager"] is True
    assert domains["sys-net"]["services"]["qubes-updates-proxy"] is True
    assert domains["sys-firewall"]["class"] == "StandaloneVM"
    assert domains["sys-firewall"]["netvm"] == "sys-net"
    assert domains["sys-firewall"]["label"] == "green"
    assert domains["sys-firewall"]["prefs"]["autostart"] is True
    assert domains["sys-firewall"]["prefs"]["memory"] == 500
    assert domains["sys-firewall"]["prefs"]["provides_network"] is True
    assert domains["sys-firewall"]["prefs"]["guivm"] is None
    assert domains["sys-firewall"]["features"]["nixome-role"] == "sys-firewall"
    assert domains["sys-firewall"]["features"]["servicevm"] == "1"
    assert domains["sys-firewall"]["services"]["network-manager"] is False
    assert domains["sys-firewall"]["services"]["qubes-firewall"] is True
    assert domains["sys-firewall"]["services"]["qubes-network"] is True
    assert domains["sys-firewall"]["services"]["qubes-updates-proxy"] is True
    assert network_debug_domains["sys-net"]["prefs"]["autostart"] is True
    assert network_debug_domains["sys-firewall"]["prefs"]["autostart"] is False
    assert network_debug_domains["sys-config-vault"]["prefs"]["autostart"] is False
    assert network_debug_domains["sys-config-staging"]["prefs"]["autostart"] is False
    assert network_debug_domains["sys-config-validator"]["prefs"]["autostart"] is False
    assert (
        _nix_eval_json(
            repo_root,
            "nixosConfigurations.vm-gui-config-network-debug.config.nixome.qubes.configCurrentBootstrapOnBoot.enable",
        )
        is False
    )
    assert (
        _nix_eval_json(
            repo_root,
            "nixosConfigurations.nixome-domain-sys-net-network-debug.config.systemd.services.NetworkManager.wantedBy",
        )
        == []
    )
    assert "dev" not in domains
    assert "qubes-kernel-6.12.64-1.fc41" in kernels["6.12.64-1.fc41"]["source"]
    assert templates == {}
    assert topology["schema"] == "nixome.qubes-topology.v1"
    assert topology["templates"] == {}
    assert topology["prefs"]["default_netvm"] == "sys-firewall"
    systemd_boot_extra_install_commands = _nix_eval_raw(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.boot.loader.systemd-boot.extraInstallCommands",
    )
    assert "xenBootBuilder" in systemd_boot_extra_install_commands
    assert "nixos-*.conf" in systemd_boot_extra_install_commands
    assert "Recovery: without Xen" in systemd_boot_extra_install_commands
    assert systemd_boot_extra_install_commands.index("xenBootBuilder") < (
        systemd_boot_extra_install_commands.index("Recovery: without Xen")
    )
    assert (
        topology["domains"]["sys-config-vault"]["backend"]["root"]["source_ref"] is None
    )
    assert topology["domains"]["sys-config-vault"]["prefs"]["guivm"] is None
    assert re.search(
        r"^/nix/store/.*/root\.img$",
        topology["domains"]["sys-config-vault"]["backend"]["root"]["source"],
    )
    assert (
        topology["domains"]["sys-config-vault"]["backend"]["store"]["read_only"] is True
    )
    assert (
        topology["domains"]["sys-config-staging"]["backend"]["state"]["persistent_size"]
        == 8589934592
    )
    assert (
        topology["domains"]["sys-config-validator"]["backend"]["state"][
            "persistent_size"
        ]
        == 8589934592
    )
    assert (
        topology["domains"]["sys-config-vault"]["backend"]["state"]["persistent_size"]
        is None
    )
    assert topology["domains"]["sys-net"]["devices"]["pci"] == [
        {
            "backend": "dom0",
            "id": "00_03.0",
            "match": "port",
            "mode": "required",
            "options": {"no-strict-reset": "True"},
        }
    ]
    assert topology["domains"]["sys-net"]["prefs"]["virt_mode"] == "hvm"
    assert topology["domains"]["sys-net"]["prefs"]["guivm"] is None
    assert topology["domains"]["sys-firewall"]["netvm"] == "sys-net"
    assert topology["domains"]["sys-firewall"]["prefs"]["autostart"] is True
    assert topology["domains"]["sys-firewall"]["prefs"]["provides_network"] is True
    assert topology["domains"]["sys-firewall"]["prefs"]["guivm"] is None
    assert "dev" not in topology["domains"]

    proxy_eval = json.loads(
        _run(
            [
                "nix",
                "eval",
                "--impure",
                "--json",
                "--expr",
                f"""
                let
                  flake = builtins.getFlake "path:{repo_root}";
                  dom0 = flake.nixosConfigurations.vm-gui-config.config;
                  sysNet = flake.nixosConfigurations.nixome-domain-sys-net.config;
                  sysNetNetworkDebug = flake.nixosConfigurations.nixome-domain-sys-net-network-debug.config;
                  sysFirewall = flake.nixosConfigurations.nixome-domain-sys-firewall.config;
                  packageNames = cfg: map (package: package.pname or package.name or "") cfg.environment.systemPackages;
                  systemdPackageNames = cfg: map (package: package.pname or package.name or "") cfg.systemd.packages;
                in
                {{
                  dom0_proxy_enabled = dom0.nixome.dom0.devModeNetworkProxy.enable;
                  dom0_proxy_target = dom0.nixome.dom0.devModeNetworkProxy.target;
                  dom0_dhcpcd_extra_config = dom0.networking.dhcpcd.extraConfig;
                  socket_listen = dom0.systemd.sockets.nixome-dom0-updates-proxy.listenStreams;
                  socket_wanted_by = dom0.systemd.sockets.nixome-dom0-updates-proxy.wantedBy;
                  socket_accept = dom0.systemd.sockets.nixome-dom0-updates-proxy.socketConfig.Accept;
                  socket_bind_to_device =
                    dom0.systemd.sockets.nixome-dom0-updates-proxy.socketConfig.BindToDevice;
                  service_exec =
                    dom0.systemd.services."nixome-dom0-updates-proxy@".serviceConfig.ExecStart;
                  service_stdin =
                    dom0.systemd.services."nixome-dom0-updates-proxy@".serviceConfig.StandardInput;
                  service_stdout =
                    dom0.systemd.services."nixome-dom0-updates-proxy@".serviceConfig.StandardOutput;
                  service_after = dom0.systemd.services."nixome-dom0-updates-proxy@".after;
                  qubesd_path_names = map
                    (package: package.pname or package.name or "")
                    dom0.systemd.services.qubesd.path;
                  session_proxy = {{
                    http_proxy = dom0.environment.sessionVariables.http_proxy or null;
                    https_proxy = dom0.environment.sessionVariables.https_proxy or null;
                    HTTP_PROXY = dom0.environment.sessionVariables.HTTP_PROXY or null;
                    HTTPS_PROXY = dom0.environment.sessionVariables.HTTPS_PROXY or null;
                    no_proxy = dom0.environment.sessionVariables.no_proxy or null;
                    NO_PROXY = dom0.environment.sessionVariables.NO_PROXY or null;
                  }};
                  nix_daemon_proxy = {{
                    http_proxy = dom0.systemd.services.nix-daemon.environment.http_proxy or null;
                    https_proxy = dom0.systemd.services.nix-daemon.environment.https_proxy or null;
                  }};
                  sys_net_service_exec =
                    sysNet.systemd.services.qubes-updates-proxy.serviceConfig.ExecStart;
                  sys_net_service_wanted_by =
                    sysNet.systemd.services.qubes-updates-proxy.wantedBy;
                  sys_net_service_after = sysNet.systemd.services.qubes-updates-proxy.after;
                  sys_net_service_condition =
                    sysNet.systemd.services.qubes-updates-proxy.unitConfig.ConditionPathExists;
                  sys_net_qrexec_after =
                    sysNet.systemd.services.qubes-qrexec-agent.after;
                  sys_net_qrexec_before =
                    sysNet.systemd.services.qubes-qrexec-agent.before;
                  sys_net_qrexec_unit_after =
                    sysNet.systemd.services.qubes-qrexec-agent.unitConfig.After or [];
                  sys_net_qrexec_unit_text =
                    sysNet.systemd.units."qubes-qrexec-agent.service".text;
                  sys_net_rpc_config_text =
                    sysNet.environment.etc."qubes/rpc-config/qubes.UpdatesProxy".text;
                  sys_net_vmshell_rpc_config_text =
                    sysNet.environment.etc."qubes/rpc-config/qubes.VMShell".text;
                  sys_net_network_debug_vmshell_rpc_config_text =
                    sysNetNetworkDebug.environment.etc."qubes/rpc-config/qubes.VMShell".text;
                  sys_net_rpc_service_mode =
                    sysNet.environment.etc."qubes-rpc/qubes.UpdatesProxy".mode;
                  sys_net_rpc_service_source =
                    toString sysNet.environment.etc."qubes-rpc/qubes.UpdatesProxy".source;
                  sys_net_tinyproxy_user = sysNet.users.users.tinyproxy.isSystemUser;
                  sys_net_tinyproxy_group = sysNet.users.users.tinyproxy.group;
                  sys_net_package_names = packageNames sysNet;
                  sys_net_systemd_package_names = systemdPackageNames sysNet;
                  sys_net_kernel_modules = sysNet.boot.kernelModules;
                  sys_net_xendriverdomain_enable =
                    sysNet.systemd.services.xendriverdomain.enable;
                  sys_net_xendriverdomain_wanted_by =
                    sysNet.systemd.services.xendriverdomain.wantedBy;
                  sys_net_xendriverdomain_after =
                    sysNet.systemd.services.xendriverdomain.after;
                  sys_net_xendriverdomain_requires =
                    sysNet.systemd.services.xendriverdomain.requires;
                  sys_net_xendriverdomain_runtime_directory =
                    sysNet.systemd.services.xendriverdomain.serviceConfig.RuntimeDirectory;
                  sys_net_xendriverdomain_logs_directory =
                    sysNet.systemd.services.xendriverdomain.serviceConfig.LogsDirectory;
                  sys_net_xendriverdomain_unit_text =
                    sysNet.systemd.units."xendriverdomain.service".text;
                  sys_net_xen_etc_source =
                    toString sysNet.environment.etc."xen".source;
                  sys_firewall_role = sysFirewall.nixome.qubeGuest.role;
                  sys_firewall_dhcp = sysFirewall.networking.useDHCP;
                  sys_firewall_networkmanager = sysFirewall.networking.networkmanager.enable;
                  sys_firewall_updates_proxy_exec =
                    sysFirewall.systemd.services.qubes-updates-proxy.serviceConfig.ExecStart;
                  sys_firewall_network_uplink_exec =
                    sysFirewall.systemd.services."qubes-network-uplink@".serviceConfig.ExecStart;
                  sys_firewall_network_uplink_stop =
                    sysFirewall.systemd.services."qubes-network-uplink@".serviceConfig.ExecStop;
                  sys_firewall_qrexec_after =
                    sysFirewall.systemd.services.qubes-qrexec-agent.after;
                  sys_firewall_qrexec_before =
                    sysFirewall.systemd.services.qubes-qrexec-agent.before;
                  sys_firewall_qrexec_unit_after =
                    sysFirewall.systemd.services.qubes-qrexec-agent.unitConfig.After or [];
                  sys_firewall_rpc_config_text =
                    sysFirewall.environment.etc."qubes/rpc-config/qubes.UpdatesProxy".text;
                  sys_firewall_rpc_service_mode =
                    sysFirewall.environment.etc."qubes-rpc/qubes.UpdatesProxy".mode;
                  sys_firewall_tinyproxy_user =
                    sysFirewall.users.users.tinyproxy.isSystemUser;
                  sys_firewall_tinyproxy_group = sysFirewall.users.users.tinyproxy.group;
                  sys_firewall_package_names = packageNames sysFirewall;
                  sys_firewall_systemd_package_names = systemdPackageNames sysFirewall;
                  sys_firewall_kernel_modules = sysFirewall.boot.kernelModules;
                  sys_firewall_xendriverdomain_enable =
                    sysFirewall.systemd.services.xendriverdomain.enable;
                  sys_firewall_xendriverdomain_wanted_by =
                    sysFirewall.systemd.services.xendriverdomain.wantedBy;
                  sys_firewall_xendriverdomain_after =
                    sysFirewall.systemd.services.xendriverdomain.after;
                  sys_firewall_xendriverdomain_requires =
                    sysFirewall.systemd.services.xendriverdomain.requires;
                }}
                """,
            ],
            cwd=repo_root,
        ).stdout
    )
    assert proxy_eval["dom0_proxy_enabled"] is True
    assert proxy_eval["dom0_proxy_target"] == "sys-firewall"
    assert "nogateway" in proxy_eval["dom0_dhcpcd_extra_config"]
    assert proxy_eval["socket_listen"] == ["127.0.0.1:8082"]
    assert "sockets.target" in proxy_eval["socket_wanted_by"]
    assert proxy_eval["socket_accept"] is True
    assert proxy_eval["socket_bind_to_device"] == "lo"
    assert (
        "qrexec-client --use-stdin-socket -d sys-firewall" in proxy_eval["service_exec"]
    )
    assert "DEFAULT:QUBESRPC qubes.UpdatesProxy+ dom0" in proxy_eval["service_exec"]
    assert proxy_eval["service_stdin"] == "socket"
    assert proxy_eval["service_stdout"] == "inherit"
    assert "nixome-start-network-providers.service" in proxy_eval["service_after"]
    assert "qubes-qrexec-policy-daemon.service" in proxy_eval["service_after"]
    assert "sudo" in proxy_eval["qubesd_path_names"]
    expected_proxy = "http://127.0.0.1:8082"
    assert proxy_eval["session_proxy"] == {
        "http_proxy": expected_proxy,
        "https_proxy": expected_proxy,
        "HTTP_PROXY": expected_proxy,
        "HTTPS_PROXY": expected_proxy,
        "no_proxy": "127.0.0.1,localhost,::1",
        "NO_PROXY": "127.0.0.1,localhost,::1",
    }
    assert proxy_eval["nix_daemon_proxy"] == {
        "http_proxy": expected_proxy,
        "https_proxy": expected_proxy,
    }
    assert "tinyproxy -d -c" in proxy_eval["sys_net_service_exec"]
    assert "multi-user.target" in proxy_eval["sys_net_service_wanted_by"]
    assert "qubes-iptables.service" in proxy_eval["sys_net_service_after"]
    assert (
        proxy_eval["sys_net_service_condition"]
        == "|/run/qubes-service/qubes-updates-proxy"
    )
    assert "xendriverdomain.service" in proxy_eval["sys_net_qrexec_after"]
    assert "nixome-qubes-rw-init.service" in proxy_eval["sys_net_qrexec_after"]
    assert "systemd-user-sessions.service" not in proxy_eval["sys_net_qrexec_after"]
    assert "network.target" in proxy_eval["sys_net_qrexec_before"]
    assert (
        proxy_eval["sys_net_qrexec_unit_after"].split()
        == proxy_eval["sys_net_qrexec_after"]
    )
    assert (
        "After=xendriverdomain.service nixome-qubes-rw-init.service"
        in proxy_eval["sys_net_qrexec_unit_text"]
    )
    assert (
        "After=systemd-user-sessions.service"
        not in proxy_eval["sys_net_qrexec_unit_text"]
    )
    assert "Before=network.target" in proxy_eval["sys_net_qrexec_unit_text"]
    assert "xen-netback" in proxy_eval["sys_net_kernel_modules"]
    assert "qubes-vmm-xen" in proxy_eval["sys_net_package_names"]
    assert "qubes-vmm-xen" in proxy_eval["sys_net_systemd_package_names"]
    assert proxy_eval["sys_net_xendriverdomain_enable"] is True
    assert "multi-user.target" in proxy_eval["sys_net_xendriverdomain_wanted_by"]
    assert "proc-xen.mount" in proxy_eval["sys_net_xendriverdomain_after"]
    assert "systemd-modules-load.service" in proxy_eval["sys_net_xendriverdomain_after"]
    assert "proc-xen.mount" in proxy_eval["sys_net_xendriverdomain_requires"]
    assert proxy_eval["sys_net_xendriverdomain_runtime_directory"] == "xen"
    assert proxy_eval["sys_net_xendriverdomain_logs_directory"] == "xen"
    assert "RuntimeDirectory=xen" in proxy_eval["sys_net_xendriverdomain_unit_text"]
    assert "LogsDirectory=xen" in proxy_eval["sys_net_xendriverdomain_unit_text"]
    assert "nixome-xen-driver-domain-etc" in proxy_eval["sys_net_xen_etc_source"]
    assert (
        proxy_eval["sys_net_rpc_config_text"]
        == "skip-service-descriptor=true\nexit-on-service-eof=true\n"
    )
    assert proxy_eval["sys_net_vmshell_rpc_config_text"] == "force-user='user'\n"
    assert proxy_eval["sys_net_network_debug_vmshell_rpc_config_text"] == ""
    assert proxy_eval["sys_net_rpc_service_mode"] == "0555"
    assert (
        "nixome-qubes-updates-proxy-rpc-service"
        in proxy_eval["sys_net_rpc_service_source"]
    )
    rpc_service = _run(
        [
            "nix",
            "build",
            "--no-link",
            "--print-out-paths",
            f'{repo_root}#nixosConfigurations.nixome-domain-sys-net.config.environment.etc."qubes-rpc/qubes.UpdatesProxy".source',
        ],
        cwd=repo_root,
    ).stdout.strip()
    rpc_service_path = Path(_physical_store_path(repo_root, rpc_service))
    assert rpc_service_path.is_file()
    assert not rpc_service_path.is_symlink()
    rpc_service_text = rpc_service_path.read_text(encoding="utf-8")
    assert "socat STDIO TCP:127.0.0.1:8082" in rpc_service_text
    assert proxy_eval["sys_net_tinyproxy_user"] is True
    assert proxy_eval["sys_net_tinyproxy_group"] == "tinyproxy"
    assert "tinyproxy" in proxy_eval["sys_net_package_names"]
    sys_net_system = _run(
        [
            "nix",
            "build",
            "--no-link",
            "--print-build-logs",
            "--print-out-paths",
            "--show-trace",
            f"{repo_root}#nixosConfigurations.nixome-domain-sys-net.config.system.build.toplevel",
        ],
        cwd=repo_root,
    ).stdout.strip()
    sys_net_system_path = Path(_physical_store_path(repo_root, sys_net_system))
    sys_net_packaged_qrexec_unit = Path(
        os.path.realpath(
            sys_net_system_path / "etc/systemd/system/qubes-qrexec-agent.service"
        )
    )
    sys_net_packaged_qrexec_unit_text = sys_net_packaged_qrexec_unit.read_text(
        encoding="utf-8"
    )
    assert "After=xendriverdomain.service\n" in sys_net_packaged_qrexec_unit_text
    assert "systemd-user-sessions.service" not in sys_net_packaged_qrexec_unit_text
    assert (
        "xen-netback"
        in (sys_net_system_path / "etc/modules-load.d/nixos.conf")
        .read_text(encoding="utf-8")
        .split()
    )
    assert (
        sys_net_system_path / "etc/systemd/system/xendriverdomain.service"
    ).is_file()
    assert (
        sys_net_system_path
        / "etc/systemd/system/multi-user.target.wants/xendriverdomain.service"
    ).is_symlink()
    sys_net_vif_route_qubes = (
        sys_net_system_path / "etc/xen/scripts/vif-route-qubes"
    ).read_text(encoding="utf-8")
    assert "/usr/lib/systemd/systemd-sysctl" not in sys_net_vif_route_qubes
    assert "/lib/systemd/systemd-sysctl" in sys_net_vif_route_qubes
    _run(
        [
            "nix",
            "build",
            "--no-link",
            "--print-build-logs",
            "--show-trace",
            f"{repo_root}#nixosConfigurations.nixome-domain-sys-net.config.system.build.etcBasedir",
        ],
        cwd=repo_root,
        stdout=log_dir / "sys-net-etc-lowerdir-build.log",
        stderr=log_dir / "sys-net-etc-lowerdir-build.log",
    )
    assert proxy_eval["sys_firewall_role"] == "sys-firewall"
    assert proxy_eval["sys_firewall_dhcp"] is False
    assert proxy_eval["sys_firewall_networkmanager"] is False
    assert "tinyproxy -d -c" in proxy_eval["sys_firewall_updates_proxy_exec"]
    assert "/bin/setup-ip add %i" in proxy_eval["sys_firewall_network_uplink_exec"]
    assert "/bin/setup-ip remove %i" in proxy_eval["sys_firewall_network_uplink_stop"]
    assert "xendriverdomain.service" in proxy_eval["sys_firewall_qrexec_after"]
    assert "nixome-qubes-rw-init.service" in proxy_eval["sys_firewall_qrexec_after"]
    assert (
        "systemd-user-sessions.service" not in proxy_eval["sys_firewall_qrexec_after"]
    )
    assert "network.target" in proxy_eval["sys_firewall_qrexec_before"]
    assert (
        proxy_eval["sys_firewall_qrexec_unit_after"].split()
        == proxy_eval["sys_firewall_qrexec_after"]
    )
    assert "xen-netback" in proxy_eval["sys_firewall_kernel_modules"]
    assert "qubes-vmm-xen" in proxy_eval["sys_firewall_package_names"]
    assert "qubes-vmm-xen" in proxy_eval["sys_firewall_systemd_package_names"]
    assert proxy_eval["sys_firewall_xendriverdomain_enable"] is True
    assert "multi-user.target" in proxy_eval["sys_firewall_xendriverdomain_wanted_by"]
    assert "proc-xen.mount" in proxy_eval["sys_firewall_xendriverdomain_after"]
    assert "proc-xen.mount" in proxy_eval["sys_firewall_xendriverdomain_requires"]
    assert (
        proxy_eval["sys_firewall_rpc_config_text"]
        == "skip-service-descriptor=true\nexit-on-service-eof=true\n"
    )
    assert proxy_eval["sys_firewall_rpc_service_mode"] == "0555"
    assert proxy_eval["sys_firewall_tinyproxy_user"] is True
    assert proxy_eval["sys_firewall_tinyproxy_group"] == "tinyproxy"
    assert "tinyproxy" in proxy_eval["sys_firewall_package_names"]
    _run(
        [
            "nix",
            "build",
            "--no-link",
            "--print-build-logs",
            "--show-trace",
            f"{repo_root}#nixosConfigurations.nixome-domain-sys-firewall.config.system.build.etcBasedir",
        ],
        cwd=repo_root,
        stdout=log_dir / "sys-firewall-etc-lowerdir-build.log",
        stderr=log_dir / "sys-firewall-etc-lowerdir-build.log",
    )

    guest_init_wanted_by = _nix_eval_json(
        repo_root,
        "nixosConfigurations.nixome-domain-sys-config-staging.config.systemd.services.nixome-config-guest-init.wantedBy",
    )
    guest_init_after = _nix_eval_json(
        repo_root,
        "nixosConfigurations.nixome-domain-sys-config-staging.config.systemd.services.nixome-config-guest-init.after",
    )
    guest_init_before = _nix_eval_json(
        repo_root,
        "nixosConfigurations.nixome-domain-sys-config-staging.config.systemd.services.nixome-config-guest-init.before",
    )
    guest_init_script = _nix_eval_json(
        repo_root,
        "nixosConfigurations.nixome-domain-sys-config-staging.config.systemd.services.nixome-config-guest-init.script",
    )
    guest_qrexec_requires = _nix_eval_json(
        repo_root,
        "nixosConfigurations.nixome-domain-sys-config-staging.config.systemd.services.qubes-qrexec-agent.requires",
    )
    guest_qrexec_after = _nix_eval_json(
        repo_root,
        "nixosConfigurations.nixome-domain-sys-config-staging.config.systemd.services.qubes-qrexec-agent.after",
    )
    assert "multi-user.target" in guest_init_wanted_by
    assert "nixome-qubes-rw-init.service" in guest_init_after
    assert "qubes-db.service" in guest_init_after
    assert "qubes-qrexec-agent.service" in guest_init_before
    assert "HOME=/home/user" in guest_init_script
    assert "nixome-config-nix/bin" in guest_init_script
    assert "nixome-config-guest-init.service" in guest_qrexec_requires
    assert "nixome-config-guest-init.service" in guest_qrexec_after
    assert (
        _nix_eval_raw(
            repo_root,
            "nixosConfigurations.nixome-domain-sys-config-staging.config.nixome.qubeGuest.configNixStoreRoot",
        ).strip()
        == "/rw/home/user/nixome-config-nix-root"
    )

    def has_package(configuration: str, package_name: str) -> bool:
        return json.loads(
            _run(
                [
                    "nix",
                    "eval",
                    "--json",
                    "--apply",
                    f'packages: builtins.any (package: (package.pname or package.name or "") == "{package_name}") packages',
                    f"{repo_root}#nixosConfigurations.{configuration}.config.environment.systemPackages",
                ],
                cwd=repo_root,
            ).stdout
        )

    assert has_package("nixome-domain-sys-config-staging", "nixome-config-nix") is True
    assert (
        has_package("nixome-domain-sys-config-validator", "nixome-config-nix") is True
    )
    assert has_package("nixome-domain-sys-config-vault", "nixome-config-nix") is True
    assert (
        _nix_eval_json(
            repo_root,
            "nixosConfigurations.nixome-domain-sys-config-validator.config.nixome.qubeGuest.role",
        )
        == "sys-config-validator"
    )
    assert (
        _nix_eval_json(
            repo_root,
            "nixosConfigurations.nixome-domain-sys-config-vault.config.networking.hostName",
        )
        == "sys-config-vault"
    )
    assert (
        _nix_eval_json(
            repo_root,
            "nixosConfigurations.nixome-domain-sys-config-staging.config.networking.hostName",
        )
        == "sys-config-staging"
    )
    assert (
        _nix_eval_json(
            repo_root,
            "nixosConfigurations.nixome-domain-sys-config-validator.config.networking.hostName",
        )
        == "sys-config-validator"
    )
    for attr in (
        'nixosConfigurations.nixome-domain-sys-config-vault.config.environment.etc."qubes-rpc/nixome.Config.ExportCurrent".mode',
        'nixosConfigurations.nixome-domain-sys-config-staging.config.environment.etc."qubes-rpc/nixome.Config.ImportCurrent".mode',
        'nixosConfigurations.nixome-domain-sys-config-validator.config.environment.etc."qubes-rpc/nixome.Config.ValidateCandidate".mode',
        'nixosConfigurations.nixome-domain-sys-config-staging.config.environment.etc."qubes-rpc/nixome.Config.InputsImport".mode',
        'nixosConfigurations.nixome-domain-sys-config-validator.config.environment.etc."qubes-rpc/nixome.Config.InputsImport".mode',
        'nixosConfigurations.nixome-domain-sys-config-vault.config.environment.etc."qubes-rpc/nixome.Config.ArchiveDeployment".mode',
        'nixosConfigurations.vm-gui-config.config.environment.etc."qubes-rpc/nixome.Config.Status".mode',
        'nixosConfigurations.vm-gui-config.config.environment.etc."qubes-rpc/nixome.Config.Check".mode',
        'nixosConfigurations.vm-gui-config.config.environment.etc."qubes-rpc/nixome.Config.Apply".mode',
        'nixosConfigurations.vm-gui-config.config.environment.etc."qubes-rpc/nixome.Config.Rollback".mode',
    ):
        assert _nix_eval_json(repo_root, attr) == "0555"
    assert (
        _nix_eval_json(
            repo_root,
            'nixosConfigurations.nixome-domain-sys-config-staging.config.environment.etc."qubes/rpc-config/nixome.Config.InputsImport".text',
        )
        == "force-user='root'\n"
    )
    assert (
        _nix_eval_json(
            repo_root,
            'nixosConfigurations.nixome-domain-sys-config-staging.config.environment.etc."qubes/rpc-config/nixome.Config.ImportCurrent".text',
        )
        == "force-user='user'\n"
    )
    assert (
        _nix_eval_json(
            repo_root,
            'nixosConfigurations.nixome-domain-sys-config-vault.config.environment.etc."qubes/rpc-config/nixome.Config.ExportCurrent".text',
        )
        == "force-user='user'\n"
    )
    vault_inputs_rpc_present = json.loads(
        _run(
            [
                "nix",
                "eval",
                "--impure",
                "--json",
                "--expr",
                f"""
                let
                  cfg = (builtins.getFlake "path:{repo_root}").nixosConfigurations.nixome-domain-sys-config-vault.config.environment.etc;
                in
                  cfg ? "qubes-rpc/nixome.Config.InputsImport"
                """,
            ],
            cwd=repo_root,
        ).stdout
    )
    assert vault_inputs_rpc_present is False

    setup_wanted_by = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config.wantedBy",
    )
    setup_after = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config.after",
    )
    setup_wants = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config.wants",
    )
    setup_execstart = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config.serviceConfig.ExecStart",
    )
    setup_env = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config.environment",
    )
    assert "multi-user.target" in setup_wanted_by
    for service in (
        "libvirtd.service",
        "qubes-db-dom0.service",
        "qubesd.service",
        "qubes-qrexec-policy-daemon.service",
    ):
        assert service in setup_after
        assert service in setup_wants
    assert isinstance(setup_execstart, str)
    assert "nixome-setup-config" in setup_execstart
    assert setup_execstart.endswith(" --apply --boot-critical")
    assert setup_env["NIXOME_LOG_DIR"] == "/var/log/nixome"
    assert (
        _nix_eval_json(
            repo_root,
            "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config.restartIfChanged",
        )
        is False
    )
    assert (
        _nix_eval_json(
            repo_root,
            "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config.stopIfChanged",
        )
        is False
    )
    assert "qubes-qrexec-policy-daemon.service" in _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-qubes-policy-path.before",
    )
    assert "nixome-qubes-policy-path.service" in _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.qubes-qrexec-policy-daemon.after",
    )
    assert "nixome-qubes-policy-path.service" in _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.qubes-qrexec-policy-daemon.requires",
    )
    qrexec_execstart = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.qubes-qrexec-policy-daemon.serviceConfig.ExecStart",
    )
    assert "" in qrexec_execstart
    assert any(
        "qrexec-policy-daemon --policy-path /var/lib/nixome/qubes-policy.d" in item
        for item in qrexec_execstart
    )
    assert has_package("vm-gui-config", "nixome-config-control") is True
    assert has_package("vm-gui-config", "nixome-config-bootstrap-current") is True
    assert has_package("vm-gui-config", "nixome-start-network-providers") is True

    start_network_wanted_by = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-start-network-providers.wantedBy",
    )
    start_network_after = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-start-network-providers.after",
    )
    start_network_before = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-start-network-providers.before",
    )
    start_network_wants = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-start-network-providers.wants",
    )
    start_network_execstart = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-start-network-providers.serviceConfig.ExecStart",
    )
    start_network_env = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-start-network-providers.environment",
    )
    assert "multi-user.target" in start_network_wanted_by
    for service in (
        "libvirtd.service",
        "nixome-setup-config.service",
        "qubes-db-dom0.service",
        "qubesd.service",
        "qubes-qrexec-policy-daemon.service",
    ):
        assert service in start_network_after
        assert service in start_network_wants
    assert "nixome-setup-config-deferred.service" in start_network_before
    assert "nixome-config-bootstrap-current.service" not in start_network_before
    assert isinstance(start_network_execstart, str)
    assert "nixome-start-network-providers" in start_network_execstart
    assert start_network_env["NIXOME_LOG_DIR"] == "/var/log/nixome"
    assert (
        _nix_eval_json(
            repo_root,
            "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-start-network-providers.restartIfChanged",
        )
        is False
    )
    assert (
        _nix_eval_json(
            repo_root,
            "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-start-network-providers.stopIfChanged",
        )
        is False
    )

    deferred_setup_wanted_by = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config-deferred.wantedBy",
    )
    deferred_setup_after = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config-deferred.after",
    )
    deferred_setup_wants = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config-deferred.wants",
    )
    deferred_setup_execstart = _nix_eval_json(
        repo_root,
        "nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config-deferred.serviceConfig.ExecStart",
    )
    assert "multi-user.target" in deferred_setup_wanted_by
    for service in (
        "nixome-setup-config.service",
        "nixome-start-network-providers.service",
        "qubes-db-dom0.service",
        "qubesd.service",
        "qubes-qrexec-policy-daemon.service",
    ):
        assert service in deferred_setup_after
        assert service in deferred_setup_wants
    assert isinstance(deferred_setup_execstart, str)
    assert deferred_setup_execstart.endswith(" --apply --deferred")

    assert (
        _nix_eval_json(
            repo_root,
            "nixosConfigurations.vm-gui-config.config.nixome.qubes.configCurrentBootstrapOnBoot.enable",
        )
        is False
    )
    current_bootstrap_service_present = json.loads(
        _run(
            [
                "nix",
                "eval",
                "--impure",
                "--json",
                "--expr",
                f"""
                let
                  cfg = (builtins.getFlake "path:{repo_root}").nixosConfigurations.vm-gui-config.config;
                in
                  cfg.systemd.services ? "nixome-config-bootstrap-current"
                """,
            ],
            cwd=repo_root,
        ).stdout
    )
    assert current_bootstrap_service_present is False

    for attr, expected in (
        (
            "nixome-domain-sys-config-vault-root-image",
            "nixome-domain-sys-config-vault-root-image",
        ),
        (
            "nixome-domain-sys-config-staging-root-image",
            "nixome-domain-sys-config-staging-root-image",
        ),
        (
            "nixome-domain-sys-config-validator-root-image",
            "nixome-domain-sys-config-validator-root-image",
        ),
        (
            "nixome-domain-sys-config-vault-store-closure",
            "nixome-domain-sys-config-vault-store-closure",
        ),
        (
            "nixome-domain-sys-config-staging-store-closure",
            "nixome-domain-sys-config-staging-store-closure",
        ),
        (
            "nixome-domain-sys-config-validator-store-closure",
            "nixome-domain-sys-config-validator-store-closure",
        ),
    ):
        drv = _run(
            ["nix", "path-info", "--derivation", f"{repo_root}#{attr}"],
            cwd=repo_root,
        ).stdout
        assert expected in drv

    config_access_policy_pkg = _run(
        [
            "nix",
            "build",
            "--impure",
            "--expr",
            f"""
            let
              packages = (builtins.getFlake "git+file://{repo_root}").nixosConfigurations."vm-gui-config".config.virtualisation.qubes.packages;
              matches = builtins.filter (package: package.name == "nixome-config-access-policy") packages;
            in
              builtins.head matches
            """,
            "--no-link",
            "--print-out-paths",
        ],
        cwd=repo_root,
    ).stdout.strip()
    policy_text = (
        Path(_physical_store_path(repo_root, config_access_policy_pkg))
        / "etc/qubes/policy.d/30-nixome-config.policy"
    ).read_text(encoding="utf-8")
    expected_policy = "\n".join(
        (
            "# Managed by Nixome. Only dom0 may fetch config baselines/candidates,",
            "# ask sys-config-validator to validate them, or archive them into sys-config-vault.",
            "nixome.Config.ExportCurrent * dom0 sys-config-vault allow",
            "nixome.Config.ExportCurrent * @anyvm @anyvm deny",
            "nixome.Config.ImportCurrent * dom0 sys-config-staging allow",
            "nixome.Config.ImportCurrent * @anyvm @anyvm deny",
            "nixome.Config.CandidateExport * dom0 sys-config-staging allow",
            "nixome.Config.CandidateExport * @anyvm @anyvm deny",
            "nixome.Config.ValidateCandidate * dom0 sys-config-validator allow",
            "nixome.Config.ValidateCandidate * @anyvm @anyvm deny",
            "nixome.Config.InputsImport * dom0 sys-config-staging allow",
            "nixome.Config.InputsImport * dom0 sys-config-validator allow",
            "nixome.Config.InputsImport * @anyvm @anyvm deny",
            "nixome.Config.ArchiveCandidate * dom0 sys-config-vault allow",
            "nixome.Config.ArchiveCandidate * @anyvm @anyvm deny",
            "nixome.Config.ArchiveDeployment * dom0 sys-config-vault allow",
            "nixome.Config.ArchiveDeployment * @anyvm @anyvm deny",
            "nixome.Config.Status * @anyvm @anyvm deny",
            "nixome.Config.Check * @anyvm @anyvm deny",
            "nixome.Config.Apply * @anyvm @anyvm deny",
            "nixome.Config.Rollback * @anyvm @anyvm deny",
            "",
        )
    )
    assert policy_text == expected_policy

    dom0_proxy_policy_pkg = _run(
        [
            "nix",
            "build",
            "--impure",
            "--expr",
            f"""
            let
              packages = (builtins.getFlake "git+file://{repo_root}").nixosConfigurations."vm-gui-config".config.virtualisation.qubes.packages;
              matches = builtins.filter (package: package.name == "nixome-dom0-network-proxy-policy") packages;
            in
              builtins.head matches
            """,
            "--no-link",
            "--print-out-paths",
        ],
        cwd=repo_root,
    ).stdout.strip()
    dom0_proxy_policy_text = (
        Path(_physical_store_path(repo_root, dom0_proxy_policy_pkg))
        / "etc/qubes/policy.d/35-nixome-dom0-network-proxy.policy"
    ).read_text(encoding="utf-8")
    assert dom0_proxy_policy_text == "\n".join(
        (
            "# Managed by Nixome. Let dev-mode dom0 reach the updates proxy through its configured network qube.",
            "qubes.UpdatesProxy * dom0 sys-firewall allow",
            "",
        )
    )

    staging_init_pkg = _run(
        [
            "nix",
            "build",
            "--impure",
            "--expr",
            f"""
            let
              packages = (builtins.getFlake "git+file://{repo_root}").nixosConfigurations."nixome-domain-sys-config-staging".config.environment.systemPackages;
              matches = builtins.filter (package: package.name == "nixome-config-staging-init") packages;
            in
              builtins.head matches
            """,
            "--no-link",
            "--print-out-paths",
        ],
        cwd=repo_root,
    ).stdout.strip()
    staging_init = (
        Path(_physical_store_path(repo_root, staging_init_pkg))
        / "bin/nixome-config-staging-init"
    ).read_text(encoding="utf-8")
    for expected in (
        'config_inputs_root="${NIXOME_CONFIG_INPUTS_PATH:-/etc/nixome/config-inputs}"',
        "s|__NIXOME_NIXPKGS_URL__|path:$config_inputs_root/nixpkgs|g",
        "s|__NIXPKGS_LIBVIRT_10_5_PATH__|$config_inputs_root/nixpkgs-libvirt_10_5|g",
        "s|__NIXOME_NIXOS_QUBES_PATH__|$config_inputs_root/nixos-qubes|g",
        "s|__NIXOME_SUPPORT_PATH__|$config_inputs_root/nixome-support|g",
    ):
        assert expected in staging_init
    assert "s|__NIXOME_NIXPKGS_URL__|path:/nix/store/" not in staging_init
    assert "s|__NIXOME_SUPPORT_PATH__|/nix/store/" not in staging_init


def check_config_candidate_flake(repo_root: Path, log_dir: Path) -> None:
    result_dir = log_dir
    work_dir = result_dir / "work"
    shutil.rmtree(result_dir, ignore_errors=True)
    (work_dir / "nixome").mkdir(parents=True)
    for relative in (
        "dom0-profile",
        "flake.nix",
        "nixome/config-vms.nix",
        "nixome/dom0-host.nix",
        "nixome/local.nix",
        "nixome/runtime.nix",
        "nixome/prebuilt-template-sources.nix",
        "nixome/system.nix",
    ):
        source = repo_root / "lifecycle/bootstrap/initial-runtime-config" / relative
        target = work_dir / relative
        if source.is_dir():
            shutil.copytree(source, target)
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
    shutil.copy2(
        repo_root / "lifecycle/misc/lib/system.nix",
        work_dir / "nixome/system-schema.nix",
    )
    shutil.copy2(
        repo_root / "lifecycle/misc/lib/topology.nix",
        work_dir / "nixome/topology.nix",
    )

    replacements = {
        "__NIXOME_NIXPKGS_URL__": "path:"
        + _physical_store_path(
            repo_root,
            _run(
                ["nix", "eval", "--raw", f"{repo_root}#nixome.configInputs.nixpkgs"]
            ).stdout.strip(),
        ),
        "__NIXPKGS_LIBVIRT_10_5_PATH__": _physical_store_path(
            repo_root,
            _run(
                [
                    "nix",
                    "eval",
                    "--raw",
                    f"{repo_root}#nixome.configInputs.nixpkgsLibvirt105",
                ]
            ).stdout.strip(),
        ),
        "__NIXOME_NIXOS_QUBES_PATH__": _physical_store_path(
            repo_root,
            _run(
                ["nix", "eval", "--raw", f"{repo_root}#nixome.configInputs.nixosQubes"]
            ).stdout.strip(),
        ),
        "__NIXOME_SUPPORT_PATH__": _physical_store_path(
            repo_root,
            _run(
                [
                    "nix",
                    "eval",
                    "--raw",
                    f"{repo_root}#nixome.configInputs.nixomeSupport",
                ]
            ).stdout.strip(),
        ),
    }
    flake_path = work_dir / "flake.nix"
    flake_text = flake_path.read_text(encoding="utf-8")
    for needle, replacement in replacements.items():
        flake_text = flake_text.replace(needle, replacement)
    flake_path.write_text(flake_text, encoding="utf-8")

    nix_common = [
        "nix",
        "--extra-experimental-features",
        "nix-command",
        "--extra-experimental-features",
        "flakes",
    ]
    _run([*nix_common, "flake", "lock", "--impure", "--offline"], cwd=work_dir)
    _run(
        [
            *nix_common,
            "flake",
            "check",
            "--impure",
            "--offline",
            "--no-write-lock-file",
            str(work_dir),
            "--no-build",
            "--show-trace",
        ],
        cwd=work_dir,
    )

    def eval_work_json(attr: str) -> object:
        return json.loads(
            _run(
                [
                    *nix_common,
                    "eval",
                    "--impure",
                    "--offline",
                    "--no-write-lock-file",
                    "--json",
                    f"{work_dir}#{attr}",
                ],
                cwd=work_dir,
            ).stdout
        )

    def eval_work_raw(attr: str) -> str:
        return _run(
            [
                *nix_common,
                "eval",
                "--impure",
                "--offline",
                "--no-write-lock-file",
                "--raw",
                f"{work_dir}#{attr}",
            ],
            cwd=work_dir,
        ).stdout

    topology = eval_work_json("nixome.topology")
    system = eval_work_json("nixome.system")
    host_message = eval_work_raw(
        'nixosConfigurations.nixome-dom0.config.environment.etc."nixome/inside-config-applied-message".text'
    )
    host_derivations = _run(
        [
            *nix_common,
            "path-info",
            "--impure",
            "--offline",
            "--derivation",
            "--recursive",
            f"{work_dir}#nixosConfigurations.nixome-dom0.config.system.build.toplevel",
        ],
        cwd=work_dir,
    ).stdout
    profile_out = _run(
        [
            *nix_common,
            "build",
            "--impure",
            "--offline",
            "--no-write-lock-file",
            "--out-link",
            str(result_dir / "dom0-profile"),
            f"{work_dir}#packages.x86_64-linux.nixome-dom0-profile",
        ],
        cwd=work_dir,
    ).stdout
    profile_physical = Path(
        _physical_store_path(repo_root, os.readlink(result_dir / "dom0-profile"))
    )
    profile_json = json.loads(
        (profile_physical / "share/nixome/profile.json").read_text(encoding="utf-8")
    )
    assert profile_out is not None
    assert topology["schema"] == "nixome.qubes-topology.v1"
    assert topology["templates"] == {}
    assert topology["domains"]["sys-net"]["netvm"] is None
    assert topology["domains"]["sys-net"]["prefs"]["provides_network"] is True
    assert topology["domains"]["sys-net"]["prefs"]["virt_mode"] == "hvm"
    assert topology["domains"]["sys-firewall"]["netvm"] == "sys-net"
    assert topology["domains"]["sys-firewall"]["prefs"]["autostart"] is True
    assert topology["domains"]["sys-firewall"]["prefs"]["provides_network"] is True
    assert topology["domains"]["sys-firewall"]["features"]["servicevm"] == "1"
    assert topology["prefs"]["default_netvm"] == "sys-firewall"
    assert "dev" not in topology["domains"]
    assert (
        topology["domains"]["sys-config-staging"]["features"][
            "nixome-config-applied-message"
        ]
        == "initial"
    )
    assert system["schema"] == "nixome.system.v1"
    assert system["dom0"]["profile"]["source"] == "dom0-profile"
    assert system["dom0"]["profile"]["activation_action"] == "run"
    assert system["dom0"]["profile"]["message"] == "initial"
    assert system["dom0"]["nixos"]["system"]["enable"] is True
    assert system["dom0"]["nixos"]["system"]["switch_action"] == "switch"
    assert system["plan"]["reboot_required"] is False
    assert system["plan"]["reboot_actions"] == []
    assert host_message.strip() == "initial"
    assert not re.search(
        r"nixome-nixos-(config-system|work)|qubes-template-nixome-nixos",
        host_derivations,
    )
    assert profile_json["schema"] == "nixome.dom0-profile.v1"
    assert profile_json["message"] == "initial"
    assert profile_json["built_by"] == "sys-config-staging"
    assert (profile_physical / "bin/nixome-dom0-profile-activate").is_file()

    runtime_path = work_dir / "nixome/runtime.nix"
    negative_cases = [
        (
            "{ ... }:\n{\n  templates.definitions.nixome-nixos-bad = { };\n}\n",
            "nixome.topology",
            "--json",
            "nixome.runtime: unknown option(s): templates",
        ),
        (
            "{ ... }:\n{\n  configVms.guests.config-stagin = { };\n}\n",
            "nixome.configRoleConfigurations.sys-config-staging.config.networking.hostName",
            "--raw",
            "nixome.configVms.guests: unknown role(s): config-stagin",
        ),
        (
            '{ ... }:\n{\n  configVms.domains.sys-config-vault.template = "old-template";\n}\n',
            "nixome.topology",
            "--json",
            "nixome.configVms.domains.sys-config-vault: unknown option(s): template",
        ),
        (
            '{ ... }:\n{\n  dom0.host.apply.mode = "live";\n}\n',
            "nixome.system",
            "--json",
            "nixome.dom0.host.apply.mode: expected one of auto, switch, boot",
        ),
        (
            "{ ... }:\n{\n  dom0.host.apply.reason = true;\n}\n",
            "nixome.system",
            "--json",
            "nixome.dom0.host.apply.reason: expected a string or path, got bool",
        ),
        (
            "{ ... }:\n{\n  dom0.host.pakages = [ ];\n}\n",
            "nixome.system",
            "--json",
            "nixome.dom0.host: unknown option(s): pakages",
        ),
        (
            "{ ... }:\n{\n  unknowable = true;\n}\n",
            "nixome.topology",
            "--json",
            "nixome.runtime: unknown option(s): unknowable",
        ),
    ]
    for index, (runtime_text, attr, mode, expected_error) in enumerate(negative_cases):
        runtime_path.write_text(runtime_text, encoding="utf-8")
        result = _run(
            [
                *nix_common,
                "eval",
                "--impure",
                "--offline",
                "--no-write-lock-file",
                mode,
                f"{work_dir}#{attr}",
            ],
            cwd=work_dir,
            stderr=result_dir / f"invalid-{index}.log",
            check=False,
        )
        assert result.returncode != 0
        assert expected_error in (result_dir / f"invalid-{index}.log").read_text(
            encoding="utf-8"
        )


def _storage_env(repo_root: Path, extra_path: str = "") -> dict[str, str]:
    env = os.environ.copy()
    env["PYTHONPATH"] = str(
        repo_root / "lifecycle/runtime/tools/pkgs/nixome-qubes-storage/src"
    )
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    path_parts = [extra_path] if extra_path else []
    if shutil.which("mkfs.erofs") is None or shutil.which("fsck.erofs") is None:
        erofs_out = _run(
            ["nix", "build", "--no-link", "--print-out-paths", "nixpkgs#erofs-utils"],
            cwd=repo_root,
        ).stdout.splitlines()
        for output in erofs_out:
            physical = Path(_physical_store_path(repo_root, output))
            if (physical / "bin/mkfs.erofs").is_file():
                path_parts.append(str(physical / "bin"))
                break
    if path_parts:
        env["PATH"] = ":".join(path_parts + [env["PATH"]])
    return env


def _run_storage(
    repo_root: Path,
    result_dir: Path,
    name: str,
    args: list[str],
    *,
    env: dict[str, str] | None = None,
    check: bool = True,
) -> tuple[subprocess.CompletedProcess[str], dict[str, object] | None]:
    result = _run(
        ["python3", "-m", "nixome_qubes_storage.cli", *args],
        cwd=repo_root,
        env=env or _storage_env(repo_root),
        stdout=result_dir / f"{name}.json",
        stderr=result_dir / f"{name}.err",
        check=False,
    )
    if check and result.returncode != 0:
        raise AssertionError(
            f"storage command failed: {' '.join(args)}\n"
            f"{(result_dir / f'{name}.err').read_text(encoding='utf-8')}"
        )
    payload = None
    if (result_dir / f"{name}.json").stat().st_size:
        payload = json.loads((result_dir / f"{name}.json").read_text())
    return result, payload


def _make_storage_object(
    physical_store: Path, entry: str, content: str, size: int = 1024
) -> None:
    target = physical_store / entry / "bin/payload"
    target.parent.mkdir(parents=True)
    data = (content + "\n").encode("utf-8")
    target.write_bytes((data * ((size // len(data)) + 1))[:size])
    (physical_store / entry).chmod(0o555)
    (physical_store / entry / "bin").chmod(0o555)
    target.chmod(0o444)


def _make_storage_closure(work_dir: Path, name: str, entries: list[str]) -> Path:
    closure = work_dir / "closures" / name
    closure.mkdir(parents=True)
    (closure / "store-paths").write_text(
        "".join(f"/nix/store/{entry}\n" for entry in entries), encoding="utf-8"
    )
    (closure / "manifest.json").write_text(
        json.dumps(
            {
                "schema": "nixome.store-closure-manifest.v1",
                "name": name,
                "store_paths_file": "store-paths",
                "read_only": True,
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    return closure


def _object_segment(pool: Path, entry: str) -> str:
    return json.loads((pool / f"objects/blob-metadata/{entry}.json").read_text())[
        "segment_id"
    ]


def check_closure_store_engine(repo_root: Path, log_dir: Path) -> None:
    result_dir = log_dir
    work_dir = result_dir / "work"
    if work_dir.exists():
        _run(["chmod", "-R", "u+w", str(work_dir)], check=False)
    shutil.rmtree(result_dir, ignore_errors=True)
    (work_dir / "physical-store").mkdir(parents=True)
    (work_dir / "closures").mkdir()
    (work_dir / "bin").mkdir()
    result_dir.mkdir(parents=True, exist_ok=True)
    physical_store = work_dir / "physical-store"

    entries = {
        "common": "11111111111111111111111111111111-common",
        "work1": "22222222222222222222222222222222-work-system-1",
        "unique1": "33333333333333333333333333333333-work-unique-1",
        "work2": "44444444444444444444444444444444-work-system-2",
        "unique2": "55555555555555555555555555555555-work-unique-2",
        "small_a": "dddddddddddddddddddddddddddddddd-small-a",
        "small_b": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-small-b",
        "small_c": "ffffffffffffffffffffffffffffffff-small-c",
    }
    for key, entry in entries.items():
        _make_storage_object(
            physical_store, entry, key, 12000 if "work" in key else 1024
        )
    _make_storage_closure(
        work_dir, "work-gen1", [entries["common"], entries["work1"], entries["unique1"]]
    )
    _make_storage_closure(
        work_dir, "work-gen2", [entries["common"], entries["work2"], entries["unique2"]]
    )
    _make_storage_closure(
        work_dir, "fit", [entries["small_a"], entries["small_b"], entries["small_c"]]
    )
    fake_nix = work_dir / "bin/nix"
    _write_executable(
        fake_nix,
        """
        #!/usr/bin/env bash
        printf '%s\n' "$@" > "$NIXOME_FAKE_NIX_LOG"
        """,
    )
    verify_env = _storage_env(repo_root, str(work_dir / "bin"))
    verify_env["NIXOME_FAKE_NIX_LOG"] = str(result_dir / "nix-verify.args")
    _result, verify = _run_storage(
        repo_root,
        result_dir,
        "verify-command",
        [
            "admit",
            "--pool-root",
            str(work_dir / "verify-pool"),
            "--domain",
            "verify-command",
            "--system",
            f"/nix/store/{entries['common']}",
            "--closure-source",
            str(work_dir / "closures/work-gen1"),
            "--physical-store-root",
            str(physical_store),
            "--root-source",
            "root-verify",
            "--kernel-name",
            "kernel-a",
            "--verify-mode",
            "nix-store",
        ],
        env=verify_env,
    )
    assert verify["schema"] == "nixome.closure-store.admission.v1"
    assert verify["status"] == "passed"
    assert verify["verification"]["mode"] == "nix-store"
    assert "--extra-experimental-features" in verify["verification"]["command"]
    assert "nix-command" in (result_dir / "nix-verify.args").read_text().splitlines()

    fit_pool = work_dir / "fit-pool"
    _run_storage(
        repo_root,
        result_dir,
        "fit",
        [
            "admit",
            "--pool-root",
            str(fit_pool),
            "--domain",
            "fit",
            "--system",
            f"/nix/store/{entries['small_a']}",
            "--closure-source",
            str(work_dir / "closures/fit"),
            "--physical-store-root",
            str(physical_store),
            "--root-source",
            "root-fit",
            "--kernel-name",
            "kernel-a",
            "--verify-mode",
            "tree",
            "--segment-target-bytes",
            "1048576",
        ],
    )
    fit_segment = _object_segment(fit_pool, entries["small_a"])
    assert _object_segment(fit_pool, entries["small_b"]) == fit_segment
    assert _object_segment(fit_pool, entries["small_c"]) == fit_segment
    _result, fit_status = _run_storage(
        repo_root, result_dir, "fit-status", ["status", "--pool-root", str(fit_pool)]
    )
    assert fit_status["segment_count"] == 1
    assert fit_status["open_segment_count"] == 1

    pool = work_dir / "pool"
    _result, gen1 = _run_storage(
        repo_root,
        result_dir,
        "work-gen1",
        [
            "admit",
            "--pool-root",
            str(pool),
            "--domain",
            "work",
            "--system",
            f"/nix/store/{entries['work1']}",
            "--closure-source",
            str(work_dir / "closures/work-gen1"),
            "--physical-store-root",
            str(physical_store),
            "--root-source",
            "root-work",
            "--kernel-name",
            "kernel-a",
            "--verify-mode",
            "tree",
            "--segment-target-bytes",
            "32768",
        ],
    )
    _result, gen2 = _run_storage(
        repo_root,
        result_dir,
        "work-gen2",
        [
            "admit",
            "--pool-root",
            str(pool),
            "--domain",
            "work",
            "--system",
            f"/nix/store/{entries['work2']}",
            "--closure-source",
            str(work_dir / "closures/work-gen2"),
            "--physical-store-root",
            str(physical_store),
            "--root-source",
            "root-work",
            "--kernel-name",
            "kernel-a",
            "--verify-mode",
            "tree",
            "--segment-target-bytes",
            "32768",
        ],
    )
    assert gen1["generation"] == 1
    assert gen2["generation"] == 2
    assert gen2["previous_generation"] == 1
    _result, status = _run_storage(
        repo_root, result_dir, "status-gen2", ["status", "--pool-root", str(pool)]
    )
    assert status["schema"] == "nixome.closure-store.status.v1"
    assert status["domains"]["work"]["current"] == 2
    assert len(status["domains"]["work"]["generations"]) == 2
    _result, rollback = _run_storage(
        repo_root,
        result_dir,
        "rollback",
        ["rollback", "--pool-root", str(pool), "--domain", "work", "--generation", "1"],
    )
    assert rollback["status"] == "passed"
    assert rollback["current_generation"] == 1
    _result, switched = _run_storage(
        repo_root,
        result_dir,
        "switch",
        ["switch", "--pool-root", str(pool), "--domain", "work", "--generation", "2"],
    )
    assert switched["status"] == "passed"
    assert switched["current_generation"] == 2
    _run_storage(
        repo_root,
        result_dir,
        "retain-gen1-off",
        [
            "set-retention",
            "--pool-root",
            str(pool),
            "--domain",
            "work",
            "--generation",
            "1",
            "--retain",
            "false",
        ],
    )
    _result, gc = _run_storage(
        repo_root,
        result_dir,
        "gc",
        ["gc", "--pool-root", str(pool), "--keep-generations", "1"],
    )
    assert gc["schema"] == "nixome.closure-store.gc.v1"
    assert any(
        item["domain"] == "work" and item["generation"] == 1
        for item in gc["removed_generations"]
    )


def _run_storage_failpoint(
    repo_root: Path,
    result_dir: Path,
    name: str,
    failpoint: str,
    args: list[str],
    *,
    env: dict[str, str] | None = None,
) -> None:
    fail_env = (env or _storage_env(repo_root)).copy()
    fail_env["NIXOME_CLOSURE_STORE_FAILPOINT"] = failpoint
    result, _payload = _run_storage(
        repo_root, result_dir, name, args, env=fail_env, check=False
    )
    assert result.returncode != 0
    assert f"simulated failure at {failpoint}" in (
        result_dir / f"{name}.err"
    ).read_text(encoding="utf-8")


def check_closure_store_failure_recovery(repo_root: Path, log_dir: Path) -> None:
    result_dir = log_dir
    work_dir = result_dir / "work"
    if work_dir.exists():
        _run(["chmod", "-R", "u+w", str(work_dir)], check=False)
    shutil.rmtree(result_dir, ignore_errors=True)
    (work_dir / "physical-store").mkdir(parents=True)
    (work_dir / "closures").mkdir()
    (work_dir / "bin").mkdir()
    result_dir.mkdir(parents=True, exist_ok=True)
    physical_store = work_dir / "physical-store"
    entries = {
        "common": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-common",
        "one": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-one",
        "two": "cccccccccccccccccccccccccccccccc-two",
        "three": "dddddddddddddddddddddddddddddddd-three",
        "four": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-four",
        "five": "ffffffffffffffffffffffffffffffff-five",
        "six": "11111111111111111111111111111111-six",
        "seven": "22222222222222222222222222222222-seven",
    }
    for entry in entries.values():
        _make_storage_object(physical_store, entry, entry)
    for name in ("one", "two", "three", "four", "five"):
        _make_storage_closure(work_dir, name, [entries[name]])
    _make_storage_closure(work_dir, "six", [entries["common"], entries["six"]])
    _make_storage_closure(work_dir, "seven", [entries["common"], entries["seven"]])
    _write_executable(
        work_dir / "bin/nix",
        """
        #!/usr/bin/env bash
        printf '%s\n' "$@" > "$NIXOME_FAKE_NIX_LOG"
        """,
    )
    verify_env = _storage_env(repo_root, str(work_dir / "bin"))
    verify_env["NIXOME_FAKE_NIX_LOG"] = str(result_dir / "nix-verify.args")
    verify_pool = work_dir / "verify-pool"
    _run_storage_failpoint(
        repo_root,
        result_dir,
        "verification-before-command",
        "verification-before-command",
        [
            "admit",
            "--pool-root",
            str(verify_pool),
            "--domain",
            "verify",
            "--system",
            f"/nix/store/{entries['one']}",
            "--closure-source",
            str(work_dir / "closures/one"),
            "--physical-store-root",
            str(physical_store),
            "--verify-mode",
            "nix-store",
        ],
        env=verify_env,
    )
    _run_storage(
        repo_root,
        result_dir,
        "recover-verify",
        ["recover", "--pool-root", str(verify_pool)],
    )
    _result, status = _run_storage(
        repo_root,
        result_dir,
        "verify-status",
        ["status", "--pool-root", str(verify_pool)],
    )
    assert status["domains"].get("verify") is None

    append_pool = work_dir / "append-pool"
    _run_storage_failpoint(
        repo_root,
        result_dir,
        "object-after-append",
        "object-after-append",
        [
            "admit",
            "--pool-root",
            str(append_pool),
            "--domain",
            "append",
            "--system",
            f"/nix/store/{entries['one']}",
            "--closure-source",
            str(work_dir / "closures/one"),
            "--physical-store-root",
            str(physical_store),
            "--verify-mode",
            "tree",
        ],
    )
    assert not (append_pool / f"objects/blob-metadata/{entries['one']}.json").exists()
    assert any((append_pool / "objects/segments").glob("*.blob"))
    _run_storage(
        repo_root,
        result_dir,
        "recover-append",
        ["recover", "--pool-root", str(append_pool)],
    )
    _result, status = _run_storage(
        repo_root,
        result_dir,
        "append-status",
        ["status", "--pool-root", str(append_pool)],
    )
    assert status["domains"].get("append") is None

    metadata_pool = work_dir / "metadata-pool"
    _run_storage_failpoint(
        repo_root,
        result_dir,
        "object-after-metadata",
        "object-after-metadata",
        [
            "admit",
            "--pool-root",
            str(metadata_pool),
            "--domain",
            "metadata",
            "--system",
            f"/nix/store/{entries['two']}",
            "--closure-source",
            str(work_dir / "closures/two"),
            "--physical-store-root",
            str(physical_store),
            "--verify-mode",
            "tree",
        ],
    )
    assert (metadata_pool / f"objects/blob-metadata/{entries['two']}.json").is_file()
    _result, gc_active = _run_storage(
        repo_root,
        result_dir,
        "gc-active-metadata",
        ["gc", "--pool-root", str(metadata_pool), "--keep-generations", "0"],
    )
    assert entries["two"] in gc_active["active_transaction_objects"]
    assert entries["two"] not in gc_active["removed_object_blobs"]
    _run_storage(
        repo_root,
        result_dir,
        "recover-metadata",
        ["recover", "--pool-root", str(metadata_pool)],
    )
    _result, status = _run_storage(
        repo_root,
        result_dir,
        "metadata-status",
        ["status", "--pool-root", str(metadata_pool)],
    )
    assert status["domains"].get("metadata") is None
    _result, gc_aborted = _run_storage(
        repo_root,
        result_dir,
        "gc-aborted-metadata",
        ["gc", "--pool-root", str(metadata_pool), "--keep-generations", "0"],
    )
    assert entries["two"] in gc_aborted["removed_object_blobs"]

    finalize_pool = work_dir / "finalize-pool"
    _run_storage_failpoint(
        repo_root,
        result_dir,
        "generation-after-tx-commit",
        "generation-after-tx-commit",
        [
            "admit",
            "--pool-root",
            str(finalize_pool),
            "--domain",
            "finalize",
            "--system",
            f"/nix/store/{entries['five']}",
            "--closure-source",
            str(work_dir / "closures/five"),
            "--physical-store-root",
            str(physical_store),
            "--verify-mode",
            "tree",
            "--activate",
            "false",
        ],
    )
    manifest = json.loads(
        (finalize_pool / "domains/finalize/generations/1/manifest.json").read_text()
    )
    assert manifest["status"] == "prepared"
    _result, gc_before = _run_storage(
        repo_root,
        result_dir,
        "gc-finalize-before-recover",
        ["gc", "--pool-root", str(finalize_pool), "--keep-generations", "0"],
    )
    assert any(
        item["domain"] == "finalize"
        and item["generation"] == 1
        and item["transaction_status"] == "committed"
        for item in gc_before["in_progress_generations"]
    )
    _result, recover_finalize = _run_storage(
        repo_root,
        result_dir,
        "recover-finalize",
        ["recover", "--pool-root", str(finalize_pool)],
    )
    assert len(recover_finalize["finalized_generations"]) == 1
    _result, status = _run_storage(
        repo_root,
        result_dir,
        "status-finalize-after",
        ["status", "--pool-root", str(finalize_pool)],
    )
    assert status["domains"]["finalize"]["current"] is None
    assert status["domains"]["finalize"]["generations"][0]["generation"] == 1

    switch_pool = work_dir / "switch-pool"
    _run_storage(
        repo_root,
        result_dir,
        "switch-gen1",
        [
            "admit",
            "--pool-root",
            str(switch_pool),
            "--domain",
            "switch",
            "--system",
            f"/nix/store/{entries['six']}",
            "--closure-source",
            str(work_dir / "closures/six"),
            "--physical-store-root",
            str(physical_store),
            "--verify-mode",
            "tree",
        ],
    )
    _run_storage(
        repo_root,
        result_dir,
        "switch-gen2",
        [
            "admit",
            "--pool-root",
            str(switch_pool),
            "--domain",
            "switch",
            "--system",
            f"/nix/store/{entries['seven']}",
            "--closure-source",
            str(work_dir / "closures/seven"),
            "--physical-store-root",
            str(physical_store),
            "--verify-mode",
            "tree",
            "--activate",
            "false",
        ],
    )
    _run_storage_failpoint(
        repo_root,
        result_dir,
        "switch-before-current-replace",
        "switch-before-current-replace",
        [
            "switch",
            "--pool-root",
            str(switch_pool),
            "--domain",
            "switch",
            "--generation",
            "2",
        ],
    )
    _run_storage(
        repo_root,
        result_dir,
        "recover-switch-before",
        ["recover", "--pool-root", str(switch_pool)],
    )
    _result, status = _run_storage(
        repo_root,
        result_dir,
        "status-switch-before",
        ["status", "--pool-root", str(switch_pool)],
    )
    assert status["domains"]["switch"]["current"] == 1
    _run_storage_failpoint(
        repo_root,
        result_dir,
        "switch-after-current-replace",
        "switch-after-current-replace",
        [
            "switch",
            "--pool-root",
            str(switch_pool),
            "--domain",
            "switch",
            "--generation",
            "2",
        ],
    )
    _run_storage(
        repo_root,
        result_dir,
        "recover-switch-after",
        ["recover", "--pool-root", str(switch_pool)],
    )
    _result, status = _run_storage(
        repo_root,
        result_dir,
        "status-switch-after",
        ["status", "--pool-root", str(switch_pool)],
    )
    assert status["domains"]["switch"]["current"] == 2

    gc_pool = work_dir / "gc-pool"
    _run_storage(
        repo_root,
        result_dir,
        "gc-gen1",
        [
            "admit",
            "--pool-root",
            str(gc_pool),
            "--domain",
            "gc-domain",
            "--system",
            f"/nix/store/{entries['common']}",
            "--closure-source",
            str(work_dir / "closures/six"),
            "--physical-store-root",
            str(physical_store),
            "--verify-mode",
            "tree",
        ],
    )
    _run_storage(
        repo_root,
        result_dir,
        "gc-gen2",
        [
            "admit",
            "--pool-root",
            str(gc_pool),
            "--domain",
            "gc-domain",
            "--system",
            f"/nix/store/{entries['seven']}",
            "--closure-source",
            str(work_dir / "closures/seven"),
            "--physical-store-root",
            str(physical_store),
            "--verify-mode",
            "tree",
            "--retain",
            "false",
        ],
    )
    _run_storage(
        repo_root,
        result_dir,
        "gc-rollback",
        [
            "rollback",
            "--pool-root",
            str(gc_pool),
            "--domain",
            "gc-domain",
            "--generation",
            "1",
        ],
    )
    _run_storage_failpoint(
        repo_root,
        result_dir,
        "gc-after-object-metadata-remove",
        "gc-after-object-metadata-remove",
        ["gc", "--pool-root", str(gc_pool), "--keep-generations", "0"],
    )
    _run_storage(
        repo_root, result_dir, "recover-gc", ["recover", "--pool-root", str(gc_pool)]
    )
    _result, status = _run_storage(
        repo_root, result_dir, "status-gc", ["status", "--pool-root", str(gc_pool)]
    )
    assert status["domains"]["gc-domain"]["current"] == 1
    assert len(status["domains"]["gc-domain"]["generations"]) == 1


def _write_fake_qubes_package(fake_root: Path) -> None:
    qubes_dir = fake_root / "qubes"
    qubes_dir.mkdir(parents=True)
    (qubes_dir / "exc.py").write_text(
        "class StoragePoolException(Exception):\n    pass\n", encoding="utf-8"
    )
    (qubes_dir / "storage.py").write_text(
        textwrap.dedent("""
            class Pool:
                driver = "fake"

                def __init__(self, *, name, revisions_to_keep=1, ephemeral_volatile=False):
                    self.name = name
                    self.revisions_to_keep = revisions_to_keep
                    self.ephemeral_volatile = ephemeral_volatile

                @property
                def config(self):
                    return {
                        "name": self.name,
                        "driver": self.driver,
                        "revisions_to_keep": self.revisions_to_keep,
                        "ephemeral_volatile": self.ephemeral_volatile,
                    }

                def init_volume(self, vm, volume_config):
                    volume_config["pool"] = self
                    volume = Volume(**volume_config)
                    vm.volumes[volume.name] = volume
                    return volume


            class BasicPool(Pool):
                driver = "file"


            class Volume:
                devtype = "disk"
                domain = None
                path = None

                def __init__(
                    self,
                    name,
                    pool,
                    vid=None,
                    *,
                    revisions_to_keep=0,
                    rw=False,
                    save_on_stop=False,
                    size=0,
                    snap_on_start=False,
                    source=None,
                    **kwargs,
                ):
                    self.name = name
                    self.pool = pool
                    self.vid = vid or name
                    self.revisions_to_keep = revisions_to_keep
                    self.rw = rw
                    self.save_on_stop = save_on_stop
                    self._size = int(size)
                    self.snap_on_start = snap_on_start
                    self.source = source
                    for key, value in kwargs.items():
                        setattr(self, key, value)

                @property
                def size(self):
                    return self._size

                @property
                def config(self):
                    payload = {
                        "name": self.name,
                        "pool": self.pool.name,
                        "vid": self.vid,
                        "size": self.size,
                        "rw": self.rw,
                        "snap_on_start": self.snap_on_start,
                        "save_on_stop": self.save_on_stop,
                        "source": self.source,
                        "revisions_to_keep": self.revisions_to_keep,
                    }
                    for key in (
                        "domain_name",
                        "generation",
                        "closure_hash",
                        "view_kind",
                        "view_path",
                        "manifest_path",
                        "blob_map_path",
                        "blob_logical_size",
                    ):
                        if hasattr(self, key):
                            payload[key] = getattr(self, key)
                    return payload

                async def create(self):
                    return self


            class Storage:
                def __init__(self, vm):
                    self.vm = vm

                def init_volume(self, name, volume_config):
                    pool = self.vm.app.get_pool(volume_config["pool"])
                    volume = pool.init_volume(self.vm, volume_config.copy())
                    self.vm.volumes[name] = volume
                    return volume
            """).lstrip(),
        encoding="utf-8",
    )
    (qubes_dir / "__init__.py").write_text(
        textwrap.dedent("""
            import json
            import os

            from . import storage


            class VM:
                klass = "StandaloneVM"

                def __init__(self, app, name, data=None):
                    self.app = app
                    self.name = name
                    self.features = (data or {}).get("features", {})
                    self.volume_config = {}
                    self.volumes = {}
                    self.storage = storage.Storage(self)
                    if data is None:
                        root_pool = app.get_pool("default")
                        root = storage.Volume(
                            name="root",
                            pool=root_pool,
                            vid="root",
                            size=1024,
                            rw=True,
                            snap_on_start=False,
                            save_on_stop=True,
                            source=None,
                            revisions_to_keep=1,
                        )
                        self.volumes["root"] = root
                        self.volume_config["root"] = root.config
                    else:
                        self.volume_config = data.get("volume_config", {})
                        for name, config in self.volume_config.items():
                            self.storage.init_volume(name, config.copy())

                def is_running(self):
                    return os.environ.get("NIXOME_FAKE_QUBES_RUNNING", "0") in (
                        "1",
                        "true",
                        "yes",
                    )


            class Qubes:
                def __init__(self):
                    self.pools = {"default": storage.BasicPool(name="default")}
                    self.domains = {}
                    state = os.environ["NIXOME_FAKE_QUBES_STATE_JSON"]
                    domain = os.environ["NIXOME_FAKE_QUBES_DOMAIN"]
                    if os.path.exists(state):
                        with open(state, "r", encoding="utf-8") as handle:
                            data = json.load(handle)
                        for name, config in data.get("pools", {}).items():
                            if name == "default":
                                continue
                            if config.get("driver") == "nixome-closure":
                                from nixome_qubes_storage.qubes import ClosurePool

                                self.pools[name] = ClosurePool(
                                    name=name,
                                    root_dir=config["root_dir"],
                                    revisions_to_keep=config.get("revisions_to_keep", 0),
                                    ephemeral_volatile=config.get(
                                        "ephemeral_volatile", False
                                    ),
                                )
                        self.domains = {
                            domain: VM(self, domain, data.get("domains", {}).get(domain, {}))
                        }
                    else:
                        self.domains = {domain: VM(self, domain)}

                def get_pool(self, name):
                    return self.pools[name]

                async def add_pool(self, name, **kwargs):
                    if kwargs.get("driver") != "nixome-closure":
                        raise RuntimeError(f"unsupported pool driver {kwargs.get('driver')}")
                    from nixome_qubes_storage.qubes import ClosurePool

                    pool = ClosurePool(
                        name=name,
                        root_dir=kwargs["root_dir"],
                        revisions_to_keep=kwargs.get("revisions_to_keep", 0),
                        ephemeral_volatile=kwargs.get("ephemeral_volatile", False),
                    )
                    await pool.setup()
                    self.pools[name] = pool
                    return pool

                def save(self, lock=True):
                    payload = {
                        "pools": {
                            name: pool.config for name, pool in sorted(self.pools.items())
                        },
                        "domains": {},
                    }
                    for name, vm in sorted(self.domains.items()):
                        payload["domains"][name] = {
                            "features": vm.features,
                            "volume_config": {
                                volume_name: volume.config
                                for volume_name, volume in sorted(vm.volumes.items())
                            },
                        }
                    for target in (
                        os.environ["NIXOME_FAKE_QUBES_SAVE_JSON"],
                        os.environ["NIXOME_FAKE_QUBES_STATE_JSON"],
                    ):
                        with open(target, "w", encoding="utf-8") as handle:
                            json.dump(payload, handle, sort_keys=True)

                def close(self):
                    for vm in self.domains.values():
                        del vm.app
                    return None
            """).lstrip(),
        encoding="utf-8",
    )


def check_nix_domain_storage_helper_shared(repo_root: Path, log_dir: Path) -> None:
    result_dir = log_dir
    work_dir = log_dir / "work"
    nix_root = work_dir / "nix-root"
    physical_store = nix_root / "nix/store"
    pool = work_dir / "pool"
    fake_root = work_dir / "fake"
    fake_bin = work_dir / "bin"
    state_json = work_dir / "fake-qubes-state.json"
    save_json = result_dir / "fake-qubes-save.json"
    systemctl_log = result_dir / "systemctl.log"

    if nix_root.exists():
        _run(["chmod", "-R", "u+w", str(nix_root)], check=False)
    shutil.rmtree(result_dir, ignore_errors=True)
    for path in (physical_store, work_dir / "closures", fake_bin):
        path.mkdir(parents=True)
    _write_fake_qubes_package(fake_root)
    _write_executable(
        fake_bin / "systemctl",
        """
        #!/bin/sh
        printf 'systemctl %s\\n' "$*" >> "$NIXOME_FAKE_SYSTEMCTL_LOG"
        case "$1" in
          is-active|stop|start)
            exit 0
            ;;
        esac
        exit 1
        """,
    )

    entries = {
        "common": "11111111111111111111111111111111-common",
        "system": "22222222222222222222222222222222-system",
        "unique": "33333333333333333333333333333333-unique",
        "system2": "44444444444444444444444444444444-system-2",
        "unique2": "55555555555555555555555555555555-unique-2",
    }
    for entry in entries.values():
        _make_storage_object(physical_store, entry, entry, size=8192)
    closure = _make_storage_closure(
        work_dir,
        "helper-shared",
        [entries["common"], entries["system"], entries["unique"]],
    )
    closure_gen2 = _make_storage_closure(
        work_dir,
        "helper-shared-gen2",
        [entries["common"], entries["system2"], entries["unique2"]],
    )

    env = _storage_env(repo_root, str(fake_bin))
    env.update(
        {
            "PYTHONPATH": f"{fake_root}:{repo_root / 'lifecycle/runtime/tools/pkgs/nixome-qubes-storage/src'}",
            "NIXOME_NIX_ROOT": str(nix_root),
            "NIXOME_CLOSURE_STORE_VERIFY": "tree",
            "NIXOME_FAKE_QUBES_DOMAIN": "nix-native-work",
            "NIXOME_FAKE_QUBES_SAVE_JSON": str(save_json),
            "NIXOME_FAKE_QUBES_STATE_JSON": str(state_json),
            "NIXOME_FAKE_SYSTEMCTL_LOG": str(systemctl_log),
        }
    )

    def run_helper(
        name: str,
        args: list[str],
        *,
        extra_env: dict[str, str] | None = None,
        check: bool = True,
    ) -> tuple[subprocess.CompletedProcess[str], dict[str, object] | None]:
        command_env = env.copy()
        if extra_env:
            command_env.update(extra_env)
        result = _run(
            [
                "python3",
                str(
                    repo_root / "lifecycle/runtime/tools/scripts/nix-domain-storage.py"
                ),
                *args,
            ],
            cwd=repo_root,
            env=command_env,
            stdout=result_dir / f"{name}.json",
            stderr=result_dir / f"{name}.err",
            check=False,
        )
        if check and result.returncode != 0:
            raise AssertionError(
                f"helper command failed: {' '.join(args)}\n"
                f"{(result_dir / f'{name}.err').read_text(encoding='utf-8')}"
            )
        payload = None
        if (result_dir / f"{name}.json").stat().st_size:
            payload = json.loads((result_dir / f"{name}.json").read_text())
        return result, payload

    ensure_args = [
        "ensure-store",
        "nix-native-work",
        str(closure),
        "/nix/store/example-source",
        "--mode",
        "shared-closure",
        "--system",
        f"/nix/store/{entries['system']}",
        "--root",
        "/nix/store/example-root",
        "--kernel",
        "kernel-a",
        "--pool-root",
        str(pool),
    ]
    _result, first = run_helper("helper-output", ensure_args)
    assert first is not None
    assert first["schema"] == "nixome.nix-domain-storage.v1"
    assert first["domain"] == "nix-native-work"
    assert first["mode"] == "shared-closure"
    assert first["pool"] == "nixome-closure"
    assert first["pool_created"] is True
    assert first["volume_created"] is True
    assert first["view_created"] is True
    assert first["metadata_changed"] is True
    assert first["qubesd_restart_required"] is False
    assert first["qubesd_restarted"] is True
    assert first["generation"] == 1
    assert first["filesystem"] == "erofs"
    assert first["view_backend"] == "erofs-objectmap"
    assert first["objects_total"] == 3
    assert first["objects_admitted"] == 3
    assert first["objects_reused"] == 0

    view_image = Path(str(first["metadata_image"]))
    blob_map = Path(str(first["blob_map"]))
    assert view_image.is_file()
    assert blob_map.is_file()
    assert view_image.stat().st_mode & 0o222 == 0

    saved = json.loads(save_json.read_text(encoding="utf-8"))
    volumes = saved["domains"]["nix-native-work"]["volume_config"]
    assert saved["pools"]["nixome-closure"]["driver"] == "nixome-closure"
    assert volumes["store"]["pool"] == "nixome-closure"
    assert volumes["store"]["rw"] is False
    assert volumes["store"]["snap_on_start"] is False
    assert volumes["store"]["save_on_stop"] is True
    assert volumes["store"]["source"] is None
    assert volumes["store"]["generation"] == "1"
    assert volumes["store"]["view_kind"] == "metadata-image"
    assert volumes["store"]["view_path"]
    assert volumes["store"]["manifest_path"]
    assert volumes["store-data"]["pool"] == "nixome-closure"
    assert volumes["store-data"]["rw"] is False
    assert volumes["store-data"]["snap_on_start"] is False
    assert volumes["store-data"]["save_on_stop"] is True
    assert volumes["store-data"]["source"] is None
    assert volumes["store-data"]["generation"] == "1"
    assert volumes["store-data"]["view_kind"] == "erofs-blob-map"
    assert volumes["store-data"]["blob_map_path"]
    assert int(volumes["store-data"]["blob_logical_size"]) > 0

    state = json.loads(state_json.read_text(encoding="utf-8"))
    state["domains"]["nix-native-work"]["features"] = {
        "nixome-store-source": "/nix/store/example-source",
        "nixome-store-mode": "shared-closure",
        "nixome-store-view-backend": str(first["view_backend"]),
        "nixome-store-generation": str(first["generation"]),
        "nixome-store-closure-hash": str(first["closure_hash"]),
        "nixome-store-view-image": str(first["view_image"]),
    }
    state_json.write_text(json.dumps(state, sort_keys=True) + "\n", encoding="utf-8")

    _result, second = run_helper("helper-output-2", ensure_args)
    assert second is not None
    assert second["mode"] == "shared-closure"
    assert second["volume_created"] is False
    assert second["view_created"] is False
    assert second["metadata_changed"] is False
    assert second["qubesd_restarted"] is False
    assert second["view_backend"] == "erofs-objectmap"
    assert str(second["generation"]) == "1"

    ensure_gen2_args = [
        "ensure-store",
        "nix-native-work",
        str(closure_gen2),
        "/nix/store/example-source-2",
        "--mode",
        "shared-closure",
        "--system",
        f"/nix/store/{entries['system2']}",
        "--root",
        "/nix/store/example-root",
        "--kernel",
        "kernel-b",
        "--pool-root",
        str(pool),
    ]
    _result, gen2 = run_helper("helper-output-gen2", ensure_gen2_args)
    assert gen2 is not None
    assert gen2["mode"] == "shared-closure"
    assert gen2["generation"] == 2
    assert gen2["objects_total"] == 3
    assert gen2["objects_admitted"] == 2
    assert gen2["objects_reused"] == 1
    assert gen2["view_backend"] == "erofs-objectmap"

    running, _payload = run_helper(
        "helper-switch-running",
        ["switch-store", "nix-native-work", "1", "--pool-root", str(pool)],
        extra_env={"NIXOME_FAKE_QUBES_RUNNING": "1"},
        check=False,
    )
    assert running.returncode != 0
    assert "shared store generation switch requested but domain is running" in (
        result_dir / "helper-switch-running.err"
    ).read_text(encoding="utf-8")

    _result, switched = run_helper(
        "helper-switch-back",
        ["switch-store", "nix-native-work", "1", "--pool-root", str(pool)],
    )
    assert switched is not None
    assert switched["mode"] == "shared-closure"
    assert switched["generation"] == 1
    assert switched["engine_current_generation"] == 1
    assert switched["volume_created"] is False
    assert switched["metadata_changed"] is True
    assert switched["qubesd_restarted"] is True
    assert switched["system"] == f"/nix/store/{entries['system']}"
    assert switched["kernel_name"] == "kernel-a"

    state = json.loads(state_json.read_text(encoding="utf-8"))
    volumes = state["domains"]["nix-native-work"]["volume_config"]
    assert volumes["store"]["generation"] == "1"
    assert volumes["store"]["view_path"] == str(view_image)
    assert volumes["store-data"]["generation"] == "1"
    assert volumes["store-data"]["blob_map_path"] == str(blob_map)

    state["domains"]["nix-native-work"]["features"] = {
        "nixome-store-source": "/nix/store/example-source-2",
        "nixome-store-mode": "shared-closure",
        "nixome-store-view-backend": "erofs-objectmap",
        "nixome-store-generation": str(gen2["generation"]),
        "nixome-store-closure-hash": str(gen2["closure_hash"]),
        "nixome-store-view-image": str(gen2["view_image"]),
        "nixome-store-blob-map": str(gen2["blob_map"]),
    }
    state_json.write_text(json.dumps(state, sort_keys=True) + "\n", encoding="utf-8")

    _result, repaired = run_helper("helper-stale-repair", ensure_gen2_args)
    assert repaired is not None
    assert repaired["mode"] == "shared-closure"
    assert repaired["generation"] == 2
    assert repaired["engine_current_generation"] == 2
    assert repaired["volume_created"] is False
    assert repaired["view_created"] is False
    assert repaired["metadata_changed"] is True
    assert repaired["qubesd_restarted"] is True
    assert repaired["objects_admitted"] == 0
    assert repaired["objects_reused"] == 3

    state = json.loads(state_json.read_text(encoding="utf-8"))
    volumes = state["domains"]["nix-native-work"]["volume_config"]
    assert volumes["store"]["generation"] == "2"
    assert volumes["store"]["view_path"] == str(gen2["view_image"])
    assert volumes["store-data"]["generation"] == "2"
    assert volumes["store-data"]["blob_map_path"] == str(gen2["blob_map"])

    _result, status = _run_storage(
        repo_root,
        result_dir,
        "engine-status-after-switch",
        ["status", "--pool-root", str(pool)],
    )
    assert status["domains"]["nix-native-work"]["current"] == 2
    assert len(status["domains"]["nix-native-work"]["generations"]) == 2
    assert systemctl_log.read_text(encoding="utf-8") == (
        "systemctl is-active --quiet qubesd.service\n"
        "systemctl stop qubesd.service\n"
        "systemctl start qubesd.service\n"
        "systemctl is-active --quiet qubesd.service\n"
        "systemctl stop qubesd.service\n"
        "systemctl start qubesd.service\n"
        "systemctl is-active --quiet qubesd.service\n"
        "systemctl stop qubesd.service\n"
        "systemctl start qubesd.service\n"
        "systemctl is-active --quiet qubesd.service\n"
        "systemctl stop qubesd.service\n"
        "systemctl start qubesd.service\n"
    )
