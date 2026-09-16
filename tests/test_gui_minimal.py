from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.nix
def test_gui_minimal_topology_and_desktop():
    expression = f"""
      let
        flake = builtins.getFlake "{ROOT}";
        image = flake.nixosConfigurations.vm-gui-minimal.config;
        guest = flake.nixosConfigurations.airlock-domain-sys-gui-minimal.config;
        normal = flake.nixosConfigurations.airlock-domain-sys-gui.config;
        gnome = flake.nixosConfigurations.airlock-domain-sys-gui-usb-lab.config;
        sysGuiPolicy = builtins.head (builtins.filter (package:
          (package.pname or package.name) == "airlock-sys-gui-policy"
        ) image.virtualisation.qubes.packages);
        gnomeHostPoweroffExtension = builtins.head (builtins.filter (package:
          (package.pname or package.name) == "airlock-gnome-host-poweroff-1"
          || (package.pname or "") == "airlock-gnome-host-poweroff"
        ) gnome.environment.systemPackages);
      in {{
        topology = image.airlock.qubes.topology;
        setup = image.airlock.qubes.setupConfigOnBoot;
        setupCommand = image.systemd.services.airlock-setup-config.serviceConfig.ExecStart;
        networkStartup = image.airlock.qubes.startNetworkProvidersOnBoot.enable;
        deferredStartup = image.airlock.qubes.deferredSetupConfigOnBoot.enable;
        configBootstrap = image.airlock.qubes.configCurrentBootstrapOnBoot.enable;
        configPolicy = image.airlock.qubes.configAccessPolicy.enable;
        devMode = image.airlock.image.devMode.enable;
        proxy = image.airlock.dom0.devModeNetworkProxy.enable;
        session = image.services.displayManager.defaultSession;
        guestSession = guest.services.displayManager.defaultSession;
        xfce = guest.services.xserver.desktopManager.xfce.enable;
        personalLaunchers = guest.airlock.sysGuiGuest.personalLaunchers.enable;
        normalPersonalLaunchers = normal.airlock.sysGuiGuest.personalLaunchers.enable;
        normalPackages = map (p: p.pname or p.name) normal.environment.systemPackages;
        gnomeOverrides = gnome.services.desktopManager.gnome.extraGSettingsOverrides;
        gnomeHostPoweroffExtensionSource = builtins.readFile
          "${{gnomeHostPoweroffExtension}}/share/gnome-shell/extensions/airlock-host-poweroff@airlock.local/extension.js";
        gnomeHostPoweroffHelper = builtins.readFile gnomeHostPoweroffExtension.hostPoweroff;
        etc = builtins.attrNames guest.environment.etc;
        hostEtc = builtins.attrNames image.environment.etc;
        hostPoweroffService = builtins.readFile image.environment.etc."qubes-rpc/airlock.Dom0Poweroff".source;
        hostPoweroffPolicy = builtins.readFile "${{sysGuiPolicy}}/etc/qubes/policy.d/50-airlock-sys-gui.policy";
        mimeDefaults = guest.xdg.mime.defaultApplications;
        packages = map (p: p.pname or p.name) guest.environment.systemPackages;
        assertionsPass = builtins.all (a: a.assertion) (image.assertions ++ guest.assertions);
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
    log_dir = Path(os.environ["AIRLOCK_LOG_DIR"]) / "gui-minimal-topology"
    log_dir.mkdir(parents=True, exist_ok=True)
    (log_dir / "report.json").write_text(result.stdout)
    report = json.loads(result.stdout)
    assert report["assertionsPass"] is True
    topology = report["topology"]
    assert set(topology["domains"]) == {"sys-gui"}
    assert topology["templates"] == {}
    assert topology["prefs"]["default_guivm"] == "sys-gui"
    assert topology["prefs"]["default_netvm"] is None
    gui = topology["domains"]["sys-gui"]
    assert gui["netvm"] is None
    assert gui["prefs"]["guivm"] == "dom0"
    assert gui["prefs"]["autostart"] is True
    assert gui["devices"]["pci"] == []
    assert report["setup"]["enable"] is True
    assert report["setup"]["scope"] == "all"
    assert report["setupCommand"].endswith(" --apply")
    for key in (
        "networkStartup",
        "deferredStartup",
        "configBootstrap",
        "configPolicy",
        "devMode",
        "proxy",
        "personalLaunchers",
    ):
        assert report[key] is False, key
    assert report["normalPersonalLaunchers"] is True
    assert "airlock-launch-terminal" in report["normalPackages"]
    assert "airlock-terminal.desktop" in report["normalPackages"]
    assert "airlock-host-poweroff@airlock.local" in report["gnomeOverrides"]
    assert "airlock-gnome-host-poweroff" in report["gnomeHostPoweroffExtensionSource"]
    assert "activatePowerOff" in report["gnomeHostPoweroffExtensionSource"]
    assert (
        "import GObject from 'gi://GObject'"
        in report["gnomeHostPoweroffExtensionSource"]
    )
    assert "GObject.registerClass(" in report["gnomeHostPoweroffExtensionSource"]
    assert "_init()" in report["gnomeHostPoweroffExtensionSource"]
    assert (
        "super._init({destroyOnClose: false})"
        in report["gnomeHostPoweroffExtensionSource"]
    )
    assert "constructor()" not in report["gnomeHostPoweroffExtensionSource"]
    assert "airlock.Dom0Poweroff" in report["gnomeHostPoweroffHelper"]
    assert report["session"] == "sys-gui"
    assert report["guestSession"] == "xfce"
    assert report["xfce"] is True
    assert (
        "xdg/autostart/airlock-bootstrap-desktop-launchers.desktop" not in report["etc"]
    )
    assert (
        "airlock.Dom0Poweroff is restricted to sys-gui" in report["hostPoweroffService"]
    )
    assert "systemctl poweroff" in report["hostPoweroffService"]
    assert "airlock.Dom0Poweroff * sys-gui dom0 allow" in report["hostPoweroffPolicy"]
    assert "admin.vm.Stats * sys-gui dom0 allow" in report["hostPoweroffPolicy"]
    assert not any(
        name.startswith("airlock/config-inputs/") for name in report["hostEtc"]
    )
    assert "airlock-launch-files" not in report["packages"]
    assert "airlock-launch-terminal" not in report["packages"]
    assert "airlock-launch-browser" not in report["packages"]
    assert "xfce4-session-logout" in report["packages"]
    assert "firefox.desktop" not in str(report["mimeDefaults"])
    assert "org.gnome.Nautilus.desktop" not in str(report["mimeDefaults"])
