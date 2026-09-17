from __future__ import annotations

import configparser
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def test_os_is_a_real_pinned_submodule():
    entry = subprocess.check_output(
        ["git", "ls-files", "--stage", "os"], cwd=ROOT, text=True
    ).split()
    assert entry[0] == "160000"
    assert (
        entry[1]
        == subprocess.check_output(
            ["git", "-C", "os", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip()
    )

    modules = configparser.ConfigParser()
    modules.read(ROOT / ".gitmodules")
    assert modules['submodule "os"']["path"] == "os"
    assert modules['submodule "os"']["url"] == "/home/user/dev/airlock/airlock-os"


def test_release_and_usb_assembly_have_separate_dependency_edges():
    os_release = (ROOT / "os/lifecycle/misc/modules/machine-release.nix").read_text()
    usb_adapter = (ROOT / "lifecycle/image/profiles/usb-prepared-store.nix").read_text()
    dev_flake = (ROOT / "flake.nix").read_text()

    assert "prepared" not in os_release.lower()
    assert "airlockMachineRelease" not in usb_adapter
    assert "airlock-os.lib.mkMachineConfiguration" in dev_flake
    assert "preparedStore.descriptors" in dev_flake


def test_both_nix_wrappers_select_the_external_airlock_store():
    for repository in (ROOT, ROOT / "os"):
        output = subprocess.check_output(
            [
                "bash",
                "-c",
                'source "$1"; printf "%s\n%s\n" "$AIRLOCK_NIX_ROOT" "$NIX_CONFIG"',
                "bash",
                str(repository / "lifecycle/misc/tools/scripts/lib/airlock-env.sh"),
            ],
            text=True,
        ).splitlines()
        assert output == [
            "/run/media/user/data/airlock/nix-root",
            "store = /run/media/user/data/airlock/nix-root",
        ]
