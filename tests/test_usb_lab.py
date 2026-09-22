"""USB general-lab topology and its explicit development-only privilege."""

import json
import os
from pathlib import Path
import subprocess

import pytest

ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.parametrize(
    "domain,route,exclusions,expected",
    [
        ("sys-dev", "00_03.0", "", ("00_03.0", "00_04.0")),
        ("sys-dev", "00_03.0", "00_04.0", ("00_03.0",)),
        ("sys-net", "", "", ("00_03.0", "00_04.0")),
        ("sys-net", "00_03.0", "", ("00_04.0",)),
    ],
)
def test_bootstrap_network_pci_selection(tmp_path, domain, route, exclusions, expected):
    from local_pychecks import check_setup_config_auto_sys_net_pci

    check_setup_config_auto_sys_net_pci(
        ROOT,
        tmp_path,
        domain_name=domain,
        default_route=route,
        exclusions=exclusions,
        expected_devices=expected,
    )


@pytest.mark.nix
def test_usb_lab_uses_sys_dev_as_the_tailscale_network_and_dom0_shell_entrypoint():
    expression = f"""
      let
        f = builtins.getFlake "{ROOT}";
        host = f.nixosConfigurations.usb-dev.config.specialisation.usb-lab.configuration;
        dev = f.nixosConfigurations.airlock-domain-sys-dev.config;
        gui = f.nixosConfigurations.airlock-domain-sys-gui-usb-gui-lab.config;
        labGui = f.nixosConfigurations.airlock-domain-sys-gui-usb-lab.config;
        tailscaleLauncher = builtins.head (builtins.filter (package:
          (package.pname or package.name) == "airlock-start-sys-dev-tailscale"
        ) labGui.environment.systemPackages);
        tailscaleDesktop = builtins.head (builtins.filter (package:
          (package.pname or package.name) == "airlock-start-sys-dev-tailscale.desktop"
        ) labGui.environment.systemPackages);
        policy = builtins.head (builtins.filter (p:
          (p.name or "") == "airlock-dev-dom0-terminal-policy"
        ) host.virtualisation.qubes.packages);
        poolSend = builtins.head (builtins.filter (package:
          (package.pname or package.name) == "airlock-pool-send"
        ) dev.environment.systemPackages);
      in {{
        domains = builtins.attrNames host.airlock.qubes.domains;
        bootRole = host.environment.etc."airlock/boot-role".text;
        inherit (host.airlock.qubes.prefs) default_netvm;
        browserNetvm = host.airlock.qubes.domains.usr-browser.netvm;
        devMemory = host.airlock.qubes.domains.sys-dev.prefs.memory;
        devMaxmem = host.airlock.qubes.domains.sys-dev.prefs.maxmem;
        guiPci = host.airlock.qubes.domains.sys-gui.devices.pci;
        guiHostname = gui.networking.hostName;
        labGuiHasDom0Terminal = labGui.airlock.devDom0Terminal.guest.enable;
        labGuiPackages = map (package: package.pname or package.name) labGui.environment.systemPackages;
        labGuiGsettingsOverridePackages = map (package: package.pname or package.name)
          labGui.services.desktopManager.gnome.extraGSettingsOverridePackages;
        tailscaleLauncher = builtins.readFile "${{tailscaleLauncher}}/bin/airlock-start-sys-dev-tailscale";
        tailscaleDesktop = builtins.readFile "${{tailscaleDesktop}}/share/applications/airlock-start-sys-dev-tailscale.desktop";
        tailscaleHandler = builtins.readFile dev.environment.etc."qubes-rpc/airlock.DevTailscaleUp".source;
        tailscaleRpcConfig = dev.environment.etc."qubes/rpc-config/airlock.DevTailscaleUp".text;
        tailscalePolicy = builtins.readFile "${{host.environment.etc."qubes/policy.d".source}}/40-airlock-dev-tailscale.policy";
        hostNetworkManager = host.networking.networkmanager.enable;
        hostTailscale = host.services.tailscale.enable;
        hostSsh = host.services.openssh.enable;
        autostart = {{
          inherit (host.systemd.services.airlock-machine-autostart) after requires wantedBy;
          command = host.systemd.services.airlock-machine-autostart.serviceConfig.ExecStart;
          sync = host.systemd.services.airlock-machine-autostart.serviceConfig.ExecStartPost;
        }};
        bootSupervisor = {{
          enabled = host.airlock.bootSupervisor.enable;
          prepare = {{
            inherit (host.systemd.services.airlock-boot-attempt-prepare) before wantedBy;
          }};
          recoveryCondition = host.systemd.services.airlock-boot-recovery-console.unitConfig.ConditionPathExists;
          setup = {{
            inherit (host.systemd.services.airlock-setup-config) unitConfig serviceConfig;
          }};
          network = {{
            inherit (host.systemd.services.airlock-start-network-providers) unitConfig serviceConfig;
          }};
          gui = {{
            inherit (host.systemd.services.airlock-start-gui-gpu) unitConfig serviceConfig;
          }};
          autostart = {{
            inherit (host.systemd.services.airlock-machine-autostart) unitConfig serviceConfig;
          }};
        }};
        setupSync = host.systemd.services.airlock-setup-config.serviceConfig.ExecStartPost;
        callers = host.airlock.devDom0Terminal.callers;
        policy = builtins.readFile "${{policy}}/etc/qubes/policy.d/40-airlock-dev-dom0-terminal.policy";
        poolAdmission = host.airlock.poolAdmission;
        poolAdmissionHandler = builtins.readFile host.environment.etc."qubes-rpc/airlock.Pool.Admit".source;
        poolAdmissionMode = host.environment.etc."qubes-rpc/airlock.Pool.Admit".mode;
        poolAdmissionPolicy = builtins.readFile "${{host.environment.etc."qubes/policy.d".source}}/42-airlock-pool-admission.policy";
        dev = {{
          role = dev.airlock.qubeGuest.role;
          tailscale = dev.services.tailscale.enable;
          tailscaleSsh = dev.services.tailscale.extraUpFlags;
          dom0Shell = dev.airlock.devDom0Terminal.guest.enable;
          networkManager = dev.networking.networkmanager.enable;
          packages = map (package: package.pname or package.name) dev.environment.systemPackages;
          configNixStoreRoot = dev.airlock.qubeGuest.configNixStoreRoot;
          poolInit = dev.systemd.services.airlock-sys-dev-build-store.serviceConfig;
          poolDevices = dev.systemd.services.airlock-sys-dev-build-store.requires;
          poolSend = builtins.readFile "${{poolSend}}/bin/airlock-pool-send";
          userSelection = dev.airlock.qubeGuest.allowQrexecUserSelection;
          providesNetwork = dev.airlock.qubeGuest.role == "sys-dev";
        }};
      }}
    """
    result = subprocess.run(
        ["nix", "eval", "--impure", "--json", "--expr", expression],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    log_dir = Path(os.environ["AIRLOCK_LOG_DIR"]) / "usb-lab"
    log_dir.mkdir(parents=True, exist_ok=True)
    (log_dir / "topology.json").write_text(result.stdout)
    report = json.loads(result.stdout)

    for prerequisite in (
        "airlock-setup-config.service",
        "airlock-start-gui-gpu.service",
        "airlock-start-network-providers.service",
    ):
        assert prerequisite in report["autostart"]["after"]
        assert prerequisite in report["autostart"]["requires"]
    assert report["autostart"]["wantedBy"] == ["multi-user.target"]
    assert report["autostart"]["command"].endswith(
        "boot-start --require-running sys-gui"
    )
    assert report["autostart"]["sync"][-1].endswith("/bin/journalctl --sync")
    assert "airlock-boot-supervisor complete" in report["autostart"]["sync"][0]
    assert report["setupSync"].endswith("/bin/journalctl --sync")
    supervisor = report["bootSupervisor"]
    assert supervisor["enabled"] is True
    assert "multi-user.target" in supervisor["prepare"]["wantedBy"]
    for dependency in ("xenconsoled.service", "qubesd.service"):
        assert dependency in supervisor["prepare"]["before"]
    assert supervisor["recoveryCondition"].endswith("/recovery-required.json")
    for service, timeout in (("network", 300), ("gui", 300), ("autostart", 600)):
        assert supervisor[service]["serviceConfig"]["TimeoutStartSec"] == timeout
        assert supervisor[service]["serviceConfig"]["ExecCondition"][0].endswith(
            "test ! -e /run/airlock/boot-supervisor/recovery-required.json"
        )
        assert supervisor[service]["unitConfig"]["OnFailure"] == [
            "airlock-boot-failure@%n.service"
        ]
    assert supervisor["setup"]["serviceConfig"]["ExecCondition"][0].endswith(
        "test ! -e /run/airlock/boot-supervisor/recovery-required.json"
    )

    assert report["domains"] == ["sys-dev", "sys-gui", "usr-browser", "usr-home"]
    assert report["bootRole"] == "normal\n"
    assert report["default_netvm"] == "sys-dev"
    assert report["browserNetvm"] == "sys-dev"
    assert report["devMemory"] == 8192
    assert report["devMaxmem"] == 8192
    assert report["guiHostname"] == "sys-gui"
    assert not report["labGuiHasDom0Terminal"]
    assert "airlock-start-sys-dev-tailscale" in report["labGuiPackages"]
    assert "airlock-start-sys-dev-tailscale.desktop" in report["labGuiPackages"]
    assert "nautilus" in report["labGuiPackages"]
    assert report["labGuiGsettingsOverridePackages"] == ["gnome-shell"]
    assert "/bin/xterm -hold " in report["tailscaleLauncher"]
    assert "-fs 16" in report["tailscaleLauncher"]
    assert "force-user='root'" in report["tailscaleRpcConfig"]
    assert (
        "airlock.DevTailscaleUp + sys-gui sys-dev allow user=root"
        in report["tailscalePolicy"]
    )
    assert "airlock.DevTailscaleUp * @anyvm @anyvm deny" in report["tailscalePolicy"]
    assert "/bin/tailscale up --ssh" in report["tailscaleHandler"]
    # Rejected requests must exit before executing Tailscale on the test host.
    for source, argument in [("usr-browser", ""), ("sys-gui", "unexpected")]:
        rejected = subprocess.run(
            ["bash", "-c", report["tailscaleHandler"]],
            env={
                **os.environ,
                "QREXEC_REMOTE_DOMAIN": source,
                "QREXEC_SERVICE_ARGUMENT": argument,
            },
            capture_output=True,
            timeout=5,
        )
        assert rejected.returncode == 1
    assert "Exec=airlock-start-sys-dev-tailscale" in report["tailscaleDesktop"]
    assert report["guiPci"] == [
        {
            "backend": "dom0",
            "id": "00_08.1-00_00.0",
            "match": "port",
            "mode": "required",
            "options": {},
        }
    ]
    assert not report["hostNetworkManager"]
    assert not report["hostTailscale"]
    assert not report["hostSsh"]
    assert report["callers"] == ["sys-dev"]
    assert "airlock.DevDom0Shell + sys-dev dom0 allow" in report["policy"]
    assert "airlock.DevDom0Shell + sys-gui dom0 allow" not in report["policy"]
    assert report["poolAdmission"] == {
        "enable": True,
        "caller": "sys-dev",
        "targets": ["sys-gui", "sys-dev", "usr-home", "usr-browser"],
    }
    assert report["poolAdmissionMode"] == "0555"
    for target in report["poolAdmission"]["targets"]:
        assert (
            f"airlock.Pool.Admit +{target} sys-dev dom0 allow"
            in report["poolAdmissionPolicy"]
        )
    assert "airlock.Pool.Admit * @anyvm @anyvm deny" in report["poolAdmissionPolicy"]
    for source, target, expected_error in [
        ("usr-home", "usr-home", "restricted to sys-dev"),
        ("sys-dev", "dom0", "target is not permitted"),
    ]:
        rejected = subprocess.run(
            ["bash", "-c", report["poolAdmissionHandler"]],
            env={
                **os.environ,
                "QREXEC_REMOTE_DOMAIN": source,
                "QREXEC_SERVICE_ARGUMENT": target,
            },
            capture_output=True,
            text=True,
            timeout=5,
        )
        assert rejected.returncode == 1
        assert expected_error in rejected.stderr
    assert report["dev"]["configNixStoreRoot"] == "/rw/airlock-builder/merged"
    assert report["dev"]["poolInit"]["ExecStart"].endswith(" start")
    assert report["dev"]["poolInit"]["RemainAfterExit"] is True
    assert {
        unit for unit in report["dev"]["poolDevices"] if unit.endswith(".device")
    } == {"dev-xvdg.device", "dev-xvdh.device"}
    assert "--physical-store-root" in report["dev"]["poolSend"]
    assert (
        "--nix-store" not in report["dev"]["poolSend"]
    )  # Query through the overlay-aware wrapper.
    assert "airlock-pool-send" in report["dev"]["packages"]
    assert {
        key: report["dev"][key]
        for key in (
            "role",
            "tailscale",
            "tailscaleSsh",
            "dom0Shell",
            "networkManager",
            "userSelection",
            "providesNetwork",
        )
    } == {
        "role": "sys-dev",
        "tailscale": True,
        "tailscaleSsh": ["--ssh"],
        "dom0Shell": True,
        "networkManager": True,
        "userSelection": True,
        "providesNetwork": True,
    }
    assert "jq" in report["dev"]["packages"]
