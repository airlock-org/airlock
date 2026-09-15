"""USB general-lab topology and its explicit development-only privilege."""

import json
import os
from pathlib import Path
import subprocess

import pytest

ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.nix
def test_usb_lab_uses_sys_dev_as_the_tailscale_network_and_dom0_shell_entrypoint():
    expression = f"""
      let
        f = builtins.getFlake "{ROOT}";
        host = f.nixosConfigurations.usb-dev.config.specialisation.usb-lab.configuration;
        dev = f.nixosConfigurations.airlock-domain-sys-dev.config;
        policy = builtins.head (builtins.filter (p:
          (p.name or "") == "airlock-dev-dom0-terminal-policy"
        ) host.virtualisation.qubes.packages);
      in {{
        domains = builtins.attrNames host.airlock.qubes.domains;
        inherit (host.airlock.qubes.prefs) default_netvm;
        browserNetvm = host.airlock.qubes.domains.usr-browser.netvm;
        guiPci = host.airlock.qubes.domains.sys-gui.devices.pci;
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
