"""GNOME desktop-work applications run in usr-home, not sys-gui."""

import json
import os
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.nix
def test_gnome_desktop_work_applications_are_routed_to_usr_home():
    expression = f"""
      let
        f = builtins.getFlake "{ROOT}";
        pkgs = import f.inputs.nixpkgs {{ system = "x86_64-linux"; }};
        applications = import {ROOT}/lifecycle/misc/lib/usr-home-applications.nix {{ inherit pkgs; }};
        workspace = (f.nixosConfigurations.airlock-domain-sys-gui-usb-gui-lab.extendModules {{
          modules = [ ({{ lib, ... }}: {{
            airlock.sysGuiGuest.personalLaunchers.enable = lib.mkForce true;
          }}) ];
        }}).config;
        minimal = f.nixosConfigurations.airlock-domain-sys-gui-usb-gui-lab.config;
        appGuest = f.nixosConfigurations.airlock-domain-usr-browser.config;
        homeGuest = f.nixosConfigurations.airlock-domain-usr-home.config;
      in {{
        applications = map (application: {{
          inherit (application) launcher application desktopFile;
          package = application.package.pname or application.package.name;
        }}) applications;
        workspaceCoreApps = workspace.services.gnome.core-apps.enable;
        workspacePackages = map (package: package.name or "") workspace.environment.systemPackages;
        minimalCoreApps = minimal.services.gnome.core-apps.enable;
        minimalPackages = map (package: package.name or "") minimal.environment.systemPackages;
        workspaceEtc = builtins.attrNames workspace.environment.etc;
        workspaceStartApp = workspace.environment.etc."qubes-rpc/qubes.StartApp".source;
        workspaceStartAppConfig = workspace.environment.etc."qubes/rpc-config/qubes.StartApp".text;
        workspaceGSettings = workspace.services.desktopManager.gnome.extraGSettingsOverrides;
        guiLogRules = workspace.systemd.tmpfiles.rules;
        mutterFlags = workspace.nixpkgs.pkgs.mutter.mesonFlags;
        appGuestDhcp = appGuest.networking.useDHCP;
        appGuestUplink = {{ inherit (appGuest.systemd.services."qubes-network-uplink@") requires after bindsTo; }};
        appGuestForwarding = builtins.hasAttr "qubes-network" appGuest.systemd.services;
        appGuestUdev = appGuest.services.udev.extraRules;
        appGui = map (cfg: {{
          enabled = cfg.systemd.services.qubes-gui-agent.enable;
          envPath = cfg.systemd.services.qubes-gui-agent.environment.ENV_PATH;
          scale = cfg.airlock.appGui.scale;
          forkAutostart = cfg.environment.etc."xdg/autostart/qubes-qrexec-fork-server.desktop".text;
          wantedBy = cfg.systemd.services.qubes-gui-agent.wantedBy;
          pam = cfg.security.pam.services.qubes-gui-agent.startSession;
          session = cfg.environment.etc."X11/xinit/xinitrc".source;
          xorg = cfg.environment.etc."X11/xorg-qubes.conf.template".source;
          networkBoot = cfg.systemd.services.qubes-network-uplink.wantedBy;
          networkReconcile = cfg.systemd.services.qubes-network-uplink.script;
        }}) [ appGuest homeGuest ];
      }}
    """
    result = subprocess.run(
        ["nix", "eval", "--impure", "--json", "--expr", expression],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    log_dir = Path(os.environ["AIRLOCK_LOG_DIR"]) / "usr-home-gnome"
    log_dir.mkdir(parents=True, exist_ok=True)
    (log_dir / "report.json").write_text(result.stdout)
    report = json.loads(result.stdout)
    assert "d /var/log/qubes 2775 root qubes - -" in report["guiLogRules"]
    assert any(
        flag.startswith("-Dxwayland_path=")
        and "xwayland-qubes" in flag
        and flag.endswith("/bin/Xwayland")
        for flag in report["mutterFlags"]
    )
    for guest in report["appGui"]:
        assert guest["envPath"] == "/run/current-system/sw/bin:/run/wrappers/bin"
        assert len(guest["envPath"]) < 250
        assert guest["scale"] == 3
        assert "/bin/qrexec-fork-server" in guest["forkAutostart"]
        assert "Exec=/usr/bin/" not in guest["forkAutostart"]
        assert guest["enabled"] and guest["pam"]
        assert "multi-user.target" in guest["wantedBy"]
        assert guest["session"] and guest["xorg"]
        assert "multi-user.target" in guest["networkBoot"]
        assert "/sys/class/net/*" in guest["networkReconcile"]
    assert report["appGuestDhcp"] is False
    assert report["appGuestForwarding"] is False
    assert "qubes-db.service" in report["appGuestUplink"]["requires"]
    assert "qubes-sysinit.service" in report["appGuestUplink"]["after"]
    assert report["appGuestUplink"]["bindsTo"] == [
        "sys-subsystem-net-devices-%i.device"
    ]
    assert 'DRIVERS=="vif"' in report["appGuestUdev"]
    assert "qubes-network-uplink@$name.service" in report["appGuestUdev"]
    assert 'KERNEL=="eth*"' not in report["appGuestUdev"]

    expected = {
        "files": ("org.gnome.Nautilus", "nautilus"),
        "terminal": ("org.gnome.Console", "gnome-console"),
        "disk-usage": ("org.gnome.baobab", "baobab"),
        "audio": ("org.gnome.Decibels", "decibels"),
        "text-editor": ("org.gnome.TextEditor", "gnome-text-editor"),
        "calculator": ("org.gnome.Calculator", "gnome-calculator"),
        "calendar": ("org.gnome.Calendar", "gnome-calendar"),
        "characters": ("org.gnome.Characters", "gnome-characters"),
        "clocks": ("org.gnome.clocks", "gnome-clocks"),
        "contacts": ("org.gnome.Contacts", "gnome-contacts"),
        "font-viewer": ("org.gnome.font-viewer", "gnome-font-viewer"),
        "logs": ("org.gnome.Logs", "gnome-logs"),
        "music": ("org.gnome.Music", "gnome-music"),
        "system-monitor": ("org.gnome.SystemMonitor", "gnome-system-monitor"),
        "images": ("org.gnome.Loupe", "loupe"),
        "documents": ("org.gnome.Papers", "papers"),
        "videos": ("org.gnome.Showtime", "showtime"),
        "help": ("org.gnome.Yelp", "yelp"),
    }
    assert {
        application["launcher"]: (application["application"], application["package"])
        for application in report["applications"]
    } == expected
    assert report["workspaceCoreApps"] is False
    assert report["minimalCoreApps"] is True
    assert (
        "xdg/autostart/airlock-gnome-desktop-shortcuts.desktop"
        in report["workspaceEtc"]
    )
    assert "airlock-qubes-start-app" in report["workspaceStartApp"]
    assert "force-user=" in report["workspaceStartAppConfig"]
    assert "ding@rastersoft.com" in report["workspaceGSettings"]

    packages = set(report["workspacePackages"])
    for launcher in expected:
        assert f"airlock-launch-{launcher}" in packages
    for package in ("gnome-calculator", "gnome-text-editor"):
        assert not any(name.startswith(f"{package}-") for name in packages)
    # DING requires the local Nautilus binary for desktop file operations. The
    # visible Files launcher still calls usr-home through qrexec.
    assert any(name.startswith("nautilus-") for name in packages)
    assert any("airlock-usr-home-application-icons" in name for name in packages)
    assert any("desktop-icons-ng-ding" in name for name in packages)
    assert any(
        package.startswith("gnome-console-") for package in report["minimalPackages"]
    )
