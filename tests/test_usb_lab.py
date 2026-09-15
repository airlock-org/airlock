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
      in {{
        domains = builtins.attrNames host.airlock.qubes.domains;
        inherit (host.airlock.qubes.prefs) default_netvm;
        browserNetvm = host.airlock.qubes.domains.usr-browser.netvm;
        guiPci = host.airlock.qubes.domains.sys-gui.devices.pci;
        guiHostname = gui.networking.hostName;
        labGuiHasDom0Terminal = labGui.airlock.devDom0Terminal.guest.enable;
        labGuiPackages = map (package: package.pname or package.name) labGui.environment.systemPackages;
        labGuiGsettingsOverridePackages = map (package: package.pname or package.name)
          labGui.services.desktopManager.gnome.extraGSettingsOverridePackages;
        tailscaleLauncher = builtins.readFile "${{tailscaleLauncher}}/bin/airlock-start-sys-dev-tailscale";
        tailscaleDesktop = builtins.readFile "${{tailscaleDesktop}}/share/applications/airlock-start-sys-dev-tailscale.desktop";
        hostNetworkManager = host.networking.networkmanager.enable;
        hostTailscale = host.services.tailscale.enable;
        hostSsh = host.services.openssh.enable;
        callers = host.airlock.devDom0Terminal.callers;
        policy = builtins.readFile "${{policy}}/etc/qubes/policy.d/40-airlock-dev-dom0-terminal.policy";
        dev = {{
          role = dev.airlock.qubeGuest.role;
          tailscale = dev.services.tailscale.enable;
          tailscaleSsh = dev.services.tailscale.extraUpFlags;
          dom0Shell = dev.airlock.devDom0Terminal.guest.enable;
          networkManager = dev.networking.networkmanager.enable;
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

    assert report["domains"] == ["sys-dev", "sys-gui", "usr-browser", "usr-home"]
    assert report["default_netvm"] == "sys-dev"
    assert report["browserNetvm"] == "sys-dev"
    assert report["guiHostname"] == "sys-gui"
    assert not report["labGuiHasDom0Terminal"]
    assert "airlock-start-sys-dev-tailscale" in report["labGuiPackages"]
    assert "airlock-start-sys-dev-tailscale.desktop" in report["labGuiPackages"]
    assert "nautilus" in report["labGuiPackages"]
    assert report["labGuiGsettingsOverridePackages"] == ["gnome-shell"]
    assert "/bin/xterm -hold " in report["tailscaleLauncher"]
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
    assert report["dev"] == {
        "role": "sys-dev",
        "tailscale": True,
        "tailscaleSsh": ["--ssh"],
        "dom0Shell": True,
        "networkManager": True,
        "userSelection": True,
        "providesNetwork": True,
    }
