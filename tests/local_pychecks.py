from __future__ import annotations

import json
import os
import platform
import re
import shutil
import subprocess
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
