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
      in {{
        applications = map (application: {{
          inherit (application) launcher application desktopFile;
          package = application.package.pname or application.package.name;
        }}) applications;
        workspaceCoreApps = workspace.services.gnome.core-apps.enable;
        workspacePackages = map (package: package.name or "") workspace.environment.systemPackages;
        minimalCoreApps = minimal.services.gnome.core-apps.enable;
        minimalPackages = map (package: package.name or "") minimal.environment.systemPackages;
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

    packages = set(report["workspacePackages"])
    for launcher in expected:
        assert f"airlock-launch-{launcher}" in packages
    for package in ("gnome-calculator", "gnome-text-editor", "nautilus"):
        assert not any(name.startswith(f"{package}-") for name in packages)
    assert any(
        package.startswith("gnome-console-") for package in report["minimalPackages"]
    )
