from __future__ import annotations

import json
import os
import platform
import re
import shutil
import subprocess
import tarfile
import textwrap
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


def _physical_store_path(repo_root: Path, path: str) -> str:
    return _run(
        [
            "bash",
            "-c",
            'source "$1"; nixome_physical_store_path "$2"',
            "bash",
            str(repo_root / "scripts/lib/nixome-env.sh"),
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
            str(repo_root / "scripts/lib/nixome-env.sh"),
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


def check_qube_guest_policy_probe(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir
    service_path_file = work_dir / "policy-probe-path"
    fake_qdb = work_dir / "qubesdb-read"

    shutil.rmtree(work_dir, ignore_errors=True)
    work_dir.mkdir(parents=True)

    attr = (
        f"{repo_root}#nixosConfigurations.nixome-domain-config-vault.config."
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
        "    printf 'config-vault\\n'\n"
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
            str(repo_root / "scripts/lib/nixome-env.sh"),
            service,
        ],
        cwd=repo_root,
        env=env,
    ).stdout
    (work_dir / "from-name.log").write_text(first, encoding="utf-8")
    assert re.search(r"^nixome-policy-probe role=config-vault user=.+$", first)

    fake_qdb.write_text(
        "#!/usr/bin/env bash\n"
        'case "$1" in\n'
        "  /features/nixome-role)\n"
        "    printf 'config-validator\\n'\n"
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
            str(repo_root / "scripts/lib/nixome-env.sh"),
            service,
        ],
        cwd=repo_root,
        env=env,
    ).stdout
    (work_dir / "from-feature.log").write_text(second, encoding="utf-8")
    assert re.search(r"^nixome-policy-probe role=config-validator user=.+$", second)


def check_nix_closure_domain_volume_list(repo_root: Path, log_dir: Path) -> None:
    script = repo_root / "scripts/nix-closure-domain-smoke.sh"
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

    path_value = _physical_path_list(repo_root, match.group(1))
    required = [
        "awk",
        "chmod",
        "cmp",
        "cp",
        "dd",
        "grep",
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


def _write_flake(path: Path, text: str) -> None:
    path.write_text(textwrap.dedent(text).lstrip(), encoding="utf-8")


def _write_executable(path: Path, text: str) -> None:
    path.write_text(textwrap.dedent(text).lstrip(), encoding="utf-8")
    path.chmod(0o755)


def check_system_schema(repo_root: Path, log_dir: Path) -> None:
    work_dir = log_dir / "work"
    shutil.rmtree(work_dir, ignore_errors=True)
    log_dir.mkdir(parents=True, exist_ok=True)
    for name in ("valid", "reboot", "boot"):
        target = work_dir / name / "nixome"
        target.mkdir(parents=True)
        shutil.copy(repo_root / "lib/nixome/system.nix", target / "system-schema.nix")

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
        repo_root / "lib/nixome/topology.nix", valid_dir / "nixome/topology.nix"
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
            repo_root / "lib/nixome/topology.nix", case_dir / "nixome/topology.nix"
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
            '{ qrexecPolicy.rules = [ { service = "nixome.Config.ArchiveCandidate"; source = "dom0"; target = "config-vault"; action = "allow"; } ]; }',
        ),
        (
            "qrexec-allow-any-source",
            "allow rules must use dom0 or a concrete source domain",
            '{ qrexecPolicy.rules = [ { service = "nixome.PolicyProbe"; source = "@anyvm"; target = "config-vault"; action = "allow"; } ]; }',
        ),
        (
            "qrexec-target-dom0",
            "topology-managed qrexec policy cannot target dom0",
            '{ qrexecPolicy.rules = [ { service = "nixome.PolicyProbe"; source = "config-vault"; target = "dom0"; action = "allow"; } ]; }',
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
        ["bash", str(repo_root / "scripts/config-boot-verify.sh")],
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
        ["bash", str(repo_root / "scripts/config-boot-verify.sh")],
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
    (vm_dir / "nixome-vm-gui-config-image-result").symlink_to(
        store_dir / "11111111111111111111111111111111-vm-image"
    )
    (result_link_dir / "qemu-kvm-result").symlink_to(
        "/nix/store/44444444444444444444444444444444-tool"
    )
    (vm_dir / "nixome-vm-gui-config-image.qcow2").touch()
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
          cat <<'INFO'
        image: test.qcow2
        file format: qcow2
        backing file: /nix/store/22222222222222222222222222222222-backing-image/base.qcow2
        INFO
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
        [str(repo_root / "scripts/gc-nixome-store.sh"), "dry-run"],
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
        [str(repo_root / "scripts/gc-nixome-store.sh"), "run"],
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
        [str(repo_root / "scripts/gc-nixome-store.sh"), "auto"],
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
        [str(repo_root / "scripts/gc-nixome-store.sh"), "dry-run"],
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
        "scripts/run-nested-gui-smoke.sh",
        "scripts/sync-nested-dom0-tools.sh",
        "scripts/run-config-boot-activation-flow.sh",
        "scripts/apply-config-candidate.sh",
        "scripts/nixos-config-vms-smoke.sh",
        "scripts/config-boot-verify.sh",
        "scripts/resolve-vm-image-result.sh",
    ):
        _run(["bash", "-n", str(repo_root / rel_path)], cwd=repo_root)

    texts = {
        "Makefile": (repo_root / "Makefile").read_text(encoding="utf-8"),
        "run_nested": (repo_root / "scripts/run-nested-gui-smoke.sh").read_text(
            encoding="utf-8"
        ),
        "config_vms": (repo_root / "scripts/nixos-config-vms-smoke.sh").read_text(
            encoding="utf-8"
        ),
        "boot_flow": (
            repo_root / "scripts/run-config-boot-activation-flow.sh"
        ).read_text(encoding="utf-8"),
        "run_vm": (repo_root / "scripts/run-nested-vm.sh").read_text(encoding="utf-8"),
        "test_vm": (repo_root / "tests/test_vm.py").read_text(encoding="utf-8"),
        "test_vm_staged": (repo_root / "tests/test_vm_staged.py").read_text(
            encoding="utf-8"
        ),
        "sync_tools": (repo_root / "scripts/sync-nested-dom0-tools.sh").read_text(
            encoding="utf-8"
        ),
        "seed_import": (repo_root / "scripts/import-config-seed-inputs.sh").read_text(
            encoding="utf-8"
        ),
        "qube_guest": (repo_root / "modules/qube-guest.nix").read_text(
            encoding="utf-8"
        ),
        "ssh_vm": (repo_root / "scripts/ssh-nested-vm.sh").read_text(encoding="utf-8"),
        "apply_candidate": (repo_root / "scripts/apply-config-candidate.sh").read_text(
            encoding="utf-8"
        ),
        "closure_smoke": (repo_root / "scripts/nix-closure-domain-smoke.sh").read_text(
            encoding="utf-8"
        ),
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
            "NIXOME_RUN_CONFIG_SEED_INPUTS_IMPORT",
            "NIXOME_NESTED_WARM_TOPOLOGY_PATH",
            "NIXOME_NESTED_WARM_SEED_INPUTS_PATH",
            "NIXOME_NESTED_WARM_STOP_NIX_DOMAINS",
            "qvm-kill",
            "affected domain did not halt",
            'CONFIG_VMS_SMOKE_COMMAND="$REMOTE_WARM_BIN_DIR/nixome-config-vms-smoke"',
            'SETUP_CONFIG_TOPOLOGY="$REMOTE_WARM_TOPOLOGY_PATH"',
            'NIXOME_NIX_DOMAIN_STORAGE_HELPER="$REMOTE_WARM_BIN_DIR/nixome-nix-domain-storage"',
            "NIXOME_NIX_DOMAIN_STORAGE_HELPER \\",
            "NIXOME_SETUP_CONFIG_COMMAND;",
            'NIXOME_CONFIG_SEED_INPUTS_SOURCE="$REMOTE_WARM_SEED_INPUTS_PATH"',
            "NIXOME_CONFIG_SEED_INPUTS_SOURCE \\",
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
            '"$ROOT/scripts/nixome-nix.sh" shell nixpkgs#sshpass',
            'NIX_CLOSURE_DOMAIN_SMOKE_COMMAND="$REMOTE_WARM_BIN_DIR/nixome-nix-closure-domain-smoke"',
        ],
        "config_vms": [
            'if $DOM0_SYSTEM_SWITCH_ACTION_NIX == "switch" then',
            'EXPECTED_SYSTEMD_ACTIONS_STATUS="skipped"',
            "grep -F 'rawLocalConfig = import ./nixome/machine.nix {' flake.nix",
            "grep -F 'nixosConfigurations = nixNativeConfigurations // {' flake.nix",
            "grep -F 'nixome-dom0 = nixomeDom0System;' flake.nix",
            "grep -F 'nixNative.domains.",
            'assert_nix_closure_domain "$VAULT"',
            "full|preflight|stage-candidate|apply-candidate|post-apply",
        ],
        "run_vm": [
            "NIXOME_VM_SKIP_BASE_BUILD",
            "using existing Nixome VM base image result link",
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
            "nixome-config-seed-inputs-import",
            "REMOTE_SEED_INPUTS_COMPAT_PATH",
            'nixos-qubes", .nixosQubes',
            "mount --bind",
            "nixome-config-vms-smoke",
            "nixosConfigurations.vm-gui-config.config.nixome.qubes.topology",
            "nixosConfigurations.nixome-domain-$domain.config.system.build.toplevel",
            "nixome-domain-$domain-store-closure",
            "PYTHONPATH=",
            "systemctl restart qubesd.service",
        ],
        "seed_import": [
            "store-export.nar.gz",
            'nix-store -q --references "$store_root"',
            'cut -f3 "$tmpdir/seed-inputs.tsv"',
            'nix-store --export $(cat "$seed_store_paths")',
            "store_export_bytes",
        ],
        "qube_guest": [
            'nix-store --store "$store_root" --import',
            "input_relative_path=",
            '${input_path#"$input_store_path"}',
            'mount --bind "$physical_path" "$mountpoint_path"',
        ],
        "ssh_vm": ['"$ROOT/scripts/nixome-nix.sh" shell nixpkgs#sshpass'],
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


def check_dev_role(repo_root: Path, log_dir: Path) -> None:
    shutil.rmtree(log_dir, ignore_errors=True)
    log_dir.mkdir(parents=True)
    role_json = log_dir / "role.json"

    expr = f"""
    let
      flake = builtins.getFlake "path:{repo_root}";
      lib = flake.inputs.nixpkgs.lib;
      devCfg = flake.nixosConfigurations.nixome-domain-dev.config;
      builders = import {repo_root}/lib/nixome/qube-template.nix {{
        nixpkgs = flake.inputs.nixpkgs;
        nixpkgsLibvirt105Source = "${{flake.inputs.nixpkgs-libvirt_10_5}}";
        nixosQubesSource = "${{flake.inputs.nixos-qubes}}";
        nixomeSupport = "{repo_root}";
        system = "x86_64-linux";
      }};
      defaultMachine = import {repo_root}/seeds/config-staging/nixome/machine.nix {{
        appliedMessage = "test";
        pkgs = builders.pkgs;
        system = "x86_64-linux";
        templateBuilders = builders;
      }};
      devDomainDefinition = defaultMachine.nixNative.domains.dev;
      stagedNixNativeDomains = defaultMachine.nixNative.domains;
      bootstrapDomains = flake.nixosConfigurations.vm-gui-config.config.nixome.qubes.domains;
      bootstrapDevDomain = bootstrapDomains.dev;
      bootstrapTopologyDevDomain = flake.nixosConfigurations.vm-gui-config.config.nixome.qubes.topology.domains.dev;
      generatedDevDomain = (builders.mkNixNativeDomain {{
        name = "dev";
        guestRole = devDomainDefinition.role;
        modules = [ ];
        rootImageSizeMiB = devDomainDefinition.rootImageSizeMiB or 512;
        kernelName = devDomainDefinition.kernel or "6.12.64-1.fc41";
        label = devDomainDefinition.label or "green";
        netvm = devDomainDefinition.netvm or null;
        prefs = devDomainDefinition.prefs or {{ }};
        features = devDomainDefinition.features or {{ }};
        services = devDomainDefinition.services or {{ }};
        state = devDomainDefinition.state or {{ }};
      }}).domain;
      baseCfg = (builders.mkNixNativeDomain {{
        name = "minimal-base";
        guestRole = "base";
        modules = [ ];
      }}).configuration.config;
      packageNames = cfg: map (package: package.pname or package.name or "") cfg.environment.systemPackages;
      devPackages = packageNames devCfg;
      basePackages = packageNames baseCfg;
      has = name: values: builtins.elem name values;
      hasAnyPrefix = prefix: values: lib.any (value: lib.hasPrefix prefix value) values;
      etcNames = cfg: lib.attrNames cfg.environment.etc;
      forbiddenConfigRpc = [
        "qubes-rpc/nixome.Config.ArchiveCandidate"
        "qubes-rpc/nixome.Config.ArchiveDeployment"
        "qubes-rpc/nixome.Config.CandidateExport"
        "qubes-rpc/nixome.Config.SeedInputsImport"
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
      staged_dev_declared_netvm = devDomainDefinition.netvm;
      staged_sys_firewall_external = !(builtins.hasAttr "sys-firewall" stagedNixNativeDomains);
      generated_dev_domain_netvm = generatedDevDomain.netvm;
      bootstrap_dev_domain_present = builtins.hasAttr "dev" bootstrapDomains;
      bootstrap_sys_firewall_external = !(builtins.hasAttr "sys-firewall" bootstrapDomains);
      bootstrap_dev_domain_netvm = bootstrapDevDomain.netvm;
      bootstrap_dev_domain_backend = bootstrapDevDomain.backend.type;
      bootstrap_dev_domain_role_feature = bootstrapDevDomain.features."nixome-role";
      bootstrap_dev_topology_netvm = bootstrapTopologyDevDomain.netvm;
      bootstrap_dev_topology_backend = bootstrapTopologyDevDomain.backend.type;
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
      dev_allowed_submitters = flake.nixosConfigurations.vm-gui-config.config.nixome.qubes.configAccessPolicy.allowedSubmitters;
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
    assert data["hostname"] == "dev"
    assert data["staged_dev_declared_netvm"] == "sys-firewall"
    assert data["staged_sys_firewall_external"] is True
    assert data["generated_dev_domain_netvm"] == "sys-firewall"
    assert data["bootstrap_dev_domain_present"] is True
    assert data["bootstrap_sys_firewall_external"] is True
    assert data["bootstrap_dev_domain_netvm"] == "sys-firewall"
    assert data["bootstrap_dev_domain_backend"] == "nix-closure"
    assert data["bootstrap_dev_domain_role_feature"] == "dev"
    assert data["bootstrap_dev_topology_netvm"] == "sys-firewall"
    assert data["bootstrap_dev_topology_backend"] == "nix-closure"
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
    assert data["dev_allowed_submitters"] == ["dev"]
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
            str(repo_root / "scripts/lib/nixome-env.sh"),
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
            str(repo_root / "scripts/lib/nixome-env.sh"),
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

    seed = repo_root / "seeds/config-staging"
    shutil.copytree(seed / "dom0-profile", work_dir / "dom0-profile")
    for name in (
        "flake.nix",
        "nixome/config-vms.nix",
        "nixome/dom0-host.nix",
        "nixome/local.nix",
        "nixome/prebuilt-template-sources.nix",
        "nixome/system.nix",
    ):
        shutil.copy(seed / name, work_dir / name)
    shutil.copy(
        repo_root / "lib/nixome/system.nix", work_dir / "nixome/system-schema.nix"
    )
    shutil.copy(repo_root / "lib/nixome/topology.nix", work_dir / "nixome/topology.nix")

    replacements = {
        "__NIXOME_NIXPKGS_URL__": "path:"
        + _physical_store_path(
            repo_root,
            _run(
                ["nix", "eval", "--raw", f"{repo_root}#nixome.seedInputs.nixpkgs"]
            ).stdout.strip(),
        ),
        "__NIXPKGS_LIBVIRT_10_5_PATH__": _physical_store_path(
            repo_root,
            _run(
                [
                    "nix",
                    "eval",
                    "--raw",
                    f"{repo_root}#nixome.seedInputs.nixpkgsLibvirt105",
                ]
            ).stdout.strip(),
        ),
        "__NIXOME_NIXOS_QUBES_PATH__": _physical_store_path(
            repo_root,
            _run(
                ["nix", "eval", "--raw", f"{repo_root}#nixome.seedInputs.nixosQubes"]
            ).stdout.strip(),
        ),
        "__NIXOME_SUPPORT_PATH__": _physical_store_path(
            repo_root,
            _run(
                ["nix", "eval", "--raw", f"{repo_root}#nixome.seedInputs.nixomeSupport"]
            ).stdout.strip(),
        ),
    }
    flake_path = work_dir / "flake.nix"
    flake_text = flake_path.read_text(encoding="utf-8")
    for needle, replacement in replacements.items():
        flake_text = flake_text.replace(needle, replacement)
    flake_path.write_text(flake_text, encoding="utf-8")

    _write_flake(
        work_dir / "nixome/machine.nix",
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
              nixNative.domains.bad.role = "config-staging";
            }
            """,
            "nixome.nixNative.domains.bad.role: expected one of base, dev",
        ),
    ]
    invalid_log = result_dir / "invalid.log"
    for machine_expr, expected in invalid_cases:
        _write_flake(work_dir / "nixome/machine.nix", machine_expr)
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
        staging = flake.nixosConfigurations."nixome-domain-config-staging".config;
        validator = flake.nixosConfigurations."nixome-domain-config-validator".config;
        vault = flake.nixosConfigurations."nixome-domain-config-vault".config;
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
      noSeedEtc =
        cfg:
        lib.all (name: !(lib.hasPrefix "nixome/config-seed-inputs/" name)) (etcNames cfg);
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
      vault_has_no_config_nix = !(has "nixome-config-nix" (packageNames cfgs.vault));
      stock_runtime_retained = allCfgs (
        cfg:
        (cfg.systemd.package.pname or "") == "systemd"
        && cfg.services.dbus.enable == true
        && cfg.networking.useDHCP == true
      );
      no_guest_seed_input_etc = allCfgs noSeedEtc;
      required_qubes_services_retained = allCfgs (
        cfg:
        lib.all (name: has name (enabledServices cfg)) [
          "nixome-qubes-rw-init"
          "qubes-db"
          "qubes-qrexec-agent"
          "qubes-sysinit"
        ]
      );
      seed_inputs_import_rpc = {{
        staging = cfgs.staging.environment.etc ? "qubes-rpc/nixome.Config.SeedInputsImport";
        validator = cfgs.validator.environment.etc ? "qubes-rpc/nixome.Config.SeedInputsImport";
        vault = cfgs.vault.environment.etc ? "qubes-rpc/nixome.Config.SeedInputsImport";
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
        "vault": False,
    }
    assert data["staging_has_config_nix"] is True
    assert data["validator_has_config_nix"] is True
    assert data["vault_has_no_config_nix"] is True
    assert data["stock_runtime_retained"] is True
    assert data["no_guest_seed_input_etc"] is True
    assert data["required_qubes_services_retained"] is True
    assert data["seed_inputs_import_rpc"] == {
        "staging": True,
        "validator": True,
        "vault": False,
    }
    assert data["qubes_runtime_packages_retained"] is True


def check_nixome_minimal_closure(repo_root: Path, log_dir: Path) -> None:
    shutil.rmtree(log_dir, ignore_errors=True)
    log_dir.mkdir(parents=True)
    roles = ("config-staging", "config-validator", "config-vault")
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

    for role in ("config-staging", "config-validator"):
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
    assert_absent(
        "config-vault",
        r"/nix/store/[A-Za-z0-9]+-nixome-config-nix",
        "Nixome Nix helper",
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
            f'{repo_root}#nixosConfigurations.nixome-domain-config-staging.config.environment.etc."nixome/guest-tools/nixome-config-staging-validate".source',
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
        str(repo_root / "scripts/lib/nixome-env.sh"),
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
