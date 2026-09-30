"""USB general-lab topology and its explicit development-only privilege."""

import json
import os
from pathlib import Path
import subprocess

import pytest

ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.nix
def test_usb_lab_routes_sys_dev_through_firewall_and_retains_development_access():
    expression = f"""
      let
        f = builtins.getFlake "{ROOT}";
        host = f.nixosConfigurations.usb-dev.config.specialisation.usb-lab.configuration;
        dev = f.nixosConfigurations.airlock-domain-sys-dev.config;
        ctrl = f.nixosConfigurations.airlock-domain-sys-ctrl.config;
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
        retiredPolicy = builtins.head (builtins.filter (p:
          builtins.pathExists "${{p}}/etc/qubes/policy.d/05-airlock-retired.policy"
        ) host.virtualisation.qubes.packages);
        poolSend = builtins.head (builtins.filter (package:
          (package.pname or package.name) == "airlock-pool-send"
        ) dev.environment.systemPackages);
      in {{
        domains = builtins.attrNames host.airlock.qubes.domains;
        bootRole = host.environment.etc."airlock/boot-role".text;
        inherit (host.airlock.qubes.prefs) default_netvm;
        browserNetvm = host.airlock.qubes.domains.usr-browser.netvm;
        devNetvm = host.airlock.qubes.domains.sys-dev.netvm;
        ctrlNetvm = host.airlock.qubes.domains.sys-ctrl.netvm;
        firewallNetvm = host.airlock.qubes.domains.sys-firewall.netvm;
        netNetvm = host.airlock.qubes.domains.sys-net.netvm;
        homeNetvm = host.airlock.qubes.domains.usr-home.netvm;
        devProvider = host.airlock.qubes.domains.sys-dev.prefs.provides_network;
        netProvider = host.airlock.qubes.domains.sys-net.prefs.provides_network;
        firewallProvider = host.airlock.qubes.domains.sys-firewall.prefs.provides_network;
        devPci = host.airlock.qubes.domains.sys-dev.devices.pci;
        netPci = host.airlock.qubes.domains.sys-net.devices.pci;
        devAutoPci = host.airlock.qubes.domains.sys-dev.features."airlock-auto-network-pci";
        netAutoPci = host.airlock.qubes.domains.sys-net.features."airlock-auto-network-pci";
        devMemory = host.airlock.qubes.domains.sys-dev.prefs.memory;
        devMaxmem = host.airlock.qubes.domains.sys-dev.prefs.maxmem;
        ctrlMemory = host.airlock.qubes.domains.sys-ctrl.prefs.memory;
        ctrlMaxmem = host.airlock.qubes.domains.sys-ctrl.prefs.maxmem;
        ctrlBuildJobs = ctrl.nix.settings.max-jobs;
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
          failureCapture = host.systemd.services."airlock-boot-failure@".serviceConfig.ExecStart;
          failureFinally = host.systemd.services."airlock-boot-failure@".serviceConfig.ExecStopPost;
          tty1 = host.systemd.units."getty@tty1.service".enable;
          autoTty1 = host.systemd.units."autovt@tty1.service".enable;
          failureConsole = host.systemd.services.airlock-boot-failure-console.serviceConfig;
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
        retiredPolicy = builtins.readFile "${{retiredPolicy}}/etc/qubes/policy.d/05-airlock-retired.policy";
        retireDomains = host.airlock.machineDeployment.retireDomains;
        trashRetentionDays = host.airlock.machineDeployment.trashRetentionDays;
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
          providesNetwork = host.airlock.qubes.domains.sys-dev.prefs.provides_network;
        }};
      }}
    """
    result = subprocess.run(
        ["nix", "eval", "--impure", "--json", "--expr", expression],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
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
    assert report["retireDomains"] == []
    assert report["trashRetentionDays"] == 30
    assert "* * @anyvm @tag:airlock-retired deny" in report["retiredPolicy"]
    supervisor = report["bootSupervisor"]
    assert supervisor["enabled"] is True
    assert "multi-user.target" in supervisor["prepare"]["wantedBy"]
    for dependency in ("xenconsoled.service", "qubesd.service"):
        assert dependency in supervisor["prepare"]["before"]
    assert supervisor["recoveryCondition"].endswith("/recovery-required.json")
    assert "airlock-boot-supervisor fail" in supervisor["failureCapture"]
    assert "airlock-boot-failure-console.service" in supervisor["failureFinally"]
    assert supervisor["tty1"] is False
    assert supervisor["autoTty1"] is False
    assert supervisor["failureConsole"]["TTYPath"] == "/dev/tty1"
    assert supervisor["failureConsole"]["Restart"] == "always"
    for service, timeout in (("network", 300), ("gui", 300), ("autostart", 1800)):
        assert supervisor[service]["serviceConfig"]["TimeoutStartSec"] == timeout
        assert supervisor[service]["serviceConfig"]["ExecCondition"][0].endswith(
            "test ! -e /run/airlock/boot-supervisor/recovery-required.json"
        )
        expected_failure = (
            "airlock-boot-background-failure.service"
            if service == "autostart"
            else "airlock-boot-failure@%n.service"
        )
        assert supervisor[service]["unitConfig"]["OnFailure"] == [expected_failure]
    assert any(
        "desktop-ready" in command
        for command in supervisor["autostart"]["serviceConfig"]["ExecStartPre"]
    )
    assert supervisor["setup"]["serviceConfig"]["ExecCondition"][0].endswith(
        "test ! -e /run/airlock/boot-supervisor/recovery-required.json"
    )

    assert report["domains"] == [
        "sys-ctrl",
        "sys-dev",
        "sys-firewall",
        "sys-gui",
        "sys-net",
        "usr-browser",
        "usr-home",
    ]
    assert report["bootRole"] == "normal\n"
    assert report["default_netvm"] == "sys-firewall"
    assert report["browserNetvm"] == "sys-firewall"
    assert report["devNetvm"] == "sys-firewall"
    assert report["ctrlNetvm"] == "sys-firewall"
    assert report["firewallNetvm"] == "sys-net"
    assert report["netNetvm"] is None
    assert report["homeNetvm"] is None
    assert not report["devProvider"]
    assert report["netProvider"] and report["firewallProvider"]
    assert report["devPci"] == report["netPci"] == []
    assert report["devAutoPci"] == "0"
    assert report["netAutoPci"] == "1"
    assert report["devMemory"] == 8192
    assert report["devMaxmem"] == 8192
    assert report["ctrlMemory"] == 8192
    assert report["ctrlMaxmem"] == 8192
    assert report["ctrlBuildJobs"] == 1
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
    assert "120s tailscale up --ssh" in report["tailscaleHandler"]
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
        "caller": "sys-ctrl",
        "targets": [
            "sys-gui",
            "sys-dev",
            "sys-ctrl",
            "sys-net",
            "sys-firewall",
            "usr-home",
            "usr-browser",
        ],
    }
    assert report["poolAdmissionMode"] == "0555"
    for target in report["poolAdmission"]["targets"]:
        assert (
            f"airlock.Pool.Admit +{target} sys-ctrl dom0 allow"
            in report["poolAdmissionPolicy"]
        )
    assert "airlock.Pool.Admit * @anyvm @anyvm deny" in report["poolAdmissionPolicy"]
    for source, target, expected_error in [
        ("usr-home", "usr-home", "restricted to sys-ctrl"),
        ("sys-dev", "usr-home", "restricted to sys-ctrl"),
        ("sys-ctrl", "dom0", "target is not permitted"),
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
        "networkManager": False,
        "userSelection": True,
        "providesNetwork": False,
    }
    assert "jq" in report["dev"]["packages"]


@pytest.mark.nix
@pytest.mark.parametrize('repo, configuration', [
    (ROOT, 'usb-dev'), (ROOT / 'airlock-machine', 'machine'),
])
def test_image_and_release_embed_their_own_committed_boot_source(repo, configuration):
    expression = f'''
      let
        f = builtins.getFlake "{repo}";
        config = f.nixosConfigurations.{configuration}.config;
      in {{
        expected = {{ revision = f.rev or "dirty"; timestamp = f.lastModified or 0; }};
        normal = builtins.fromJSON config.specialisation.usb-lab.configuration.environment.etc."airlock/boot-source.json".text;
        recovery = builtins.fromJSON config.specialisation.usb-text-recovery.configuration.environment.etc."airlock/boot-source.json".text;
        consoleMode = config.specialisation.usb-lab.configuration.boot.loader.systemd-boot.consoleMode;
      }}
    '''
    result = subprocess.run([
        str(ROOT / 'lifecycle/misc/tools/scripts/airlock-nix.sh'),
        'eval', '--impure', '--json', '--expr', expression,
    ], cwd=ROOT, capture_output=True, text=True, check=True)
    report = json.loads(result.stdout)
    assert report['normal'] == report['recovery'] == report['expected']
    assert report['consoleMode'] == '0'
