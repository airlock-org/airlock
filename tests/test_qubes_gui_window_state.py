"""Qubes GUI window-state forwarding is patched consistently on both sides."""

import json
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
PATCH_ROOT = ROOT / "os/lifecycle/misc/patches"


def patch_text(package: str, name: str) -> str:
    return (PATCH_ROOT / package / name).read_text(encoding="utf-8")


def test_window_state_protocol_patches_cover_both_protocol_peers():
    common = patch_text(
        "qubes-gui-common", "0001-protocol-add-maximize-window-flags.patch"
    )
    agent = patch_text(
        "qubes-gui-agent-linux",
        "0001-gui-agent-forward-minimize-and-maximize-requests.patch",
    )
    daemon = patch_text(
        "qubes-gui-daemon", "0001-gui-daemon-apply-maximize-window-flags.patch"
    )

    for flag in ("WINDOW_FLAG_MAXIMIZE_VERT", "WINDOW_FLAG_MAXIMIZE_HORZ"):
        assert flag in common
        assert flag in agent
        assert flag in daemon

    assert "ev->message_type == g->wm_change_state" in agent
    assert ".flags_set = WINDOW_FLAG_MINIMIZE" in agent
    assert "toggle_flags & ~current_flags" in agent
    assert '"_NET_WM_STATE_MAXIMIZED_VERT"' in agent
    assert '"_NET_WM_STATE_MAXIMIZED_HORZ"' in agent
    assert "#define SUPPORTED_ATOMS 9" in agent

    assert "Keep the two-dimensional maximize transition atomic" in daemon
    assert "flags &= ~(WINDOW_FLAG_MAXIMIZE_VERT" in daemon
    assert "msg.flags_unset & maximize_flags" in daemon


def test_window_move_protocol_uses_trusted_host_pointer_coordinates():
    common = patch_text(
        "qubes-gui-common",
        "0002-protocol-add-interactive-window-move-message.patch",
    )
    agent = patch_text(
        "qubes-gui-agent-linux",
        "0002-gui-agent-forward-interactive-window-move-requests.patch",
    )
    daemon = patch_text(
        "qubes-gui-daemon",
        "0002-gui-daemon-apply-interactive-window-move-requests.patch",
    )

    assert "QUBES_GUID_PROTOCOL_VERSION_MINOR 9" in common
    assert "QUBES_GUID_MIN_WINDOW_MOVERESIZE 0x00010009" in common
    assert "MSG_WINDOW_MOVERESIZE" in common
    assert "struct msg_window_moveresize" in common
    assert "int32_t x" not in common

    assert "ev->message_type == g->wm_moveresize" in agent
    assert "g->protocol_version >= QUBES_GUID_MIN_WINDOW_MOVERESIZE" in agent
    assert "hdr.type = MSG_WINDOW_MOVERESIZE" in agent

    assert '"_NET_WM_MOVERESIZE"' in daemon
    assert "XQueryPointer(g->display, g->root_win" in daemon
    assert "Button1Mask << (msg.button - 1)" in daemon
    assert "ev.data.l[0] = root_x" in daemon
    assert "ev.data.l[1] = root_y" in daemon
    assert "vm_window->override_redirect" in daemon


@pytest.mark.nix
def test_window_state_overlay_patches_common_agent_and_daemon():
    expression = f"""
      let
        f = builtins.getFlake "{ROOT}";
        pkgs = import f.inputs.nixpkgs {{
          system = "x86_64-linux";
          overlays = [
            f.inputs.nixos-qubes.overlays.default
            f.overlays.airlockQubesGuiWindowState
          ];
        }};
        patchNames = package: map builtins.baseNameOf package.patches;
      in {{
        common = patchNames pkgs.qubes-gui-common;
        agent = patchNames pkgs.qubes-gui-agent-linux;
        daemon = patchNames pkgs.qubes-gui-daemon;
        preparedGuestAgent = patchNames
          f.nixosConfigurations.airlock-domain-usr-home.config.nixpkgs.pkgs.qubes-gui-agent-linux;
        preparedGuiDaemon = patchNames
          f.nixosConfigurations.airlock-domain-sys-gui-usb-gui-lab.config.nixpkgs.pkgs.qubes-gui-daemon;
      }}
    """
    result = subprocess.run(
        ["nix", "eval", "--impure", "--json", "--expr", expression],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    report = json.loads(result.stdout)

    assert report == {
        "common": [
            "0001-protocol-add-maximize-window-flags.patch",
            "0002-protocol-add-interactive-window-move-message.patch",
        ],
        "agent": [
            "0001-gui-agent-forward-minimize-and-maximize-requests.patch",
            "0002-gui-agent-forward-interactive-window-move-requests.patch",
        ],
        "daemon": [
            "0001-gui-daemon-apply-maximize-window-flags.patch",
            "0002-gui-daemon-apply-interactive-window-move-requests.patch",
        ],
        "preparedGuestAgent": [
            "0001-gui-agent-forward-minimize-and-maximize-requests.patch",
            "0002-gui-agent-forward-interactive-window-move-requests.patch",
        ],
        "preparedGuiDaemon": [
            "0001-gui-daemon-apply-maximize-window-flags.patch",
            "0002-gui-daemon-apply-interactive-window-move-requests.patch",
        ],
    }
