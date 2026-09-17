from __future__ import annotations

import configparser
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def test_machine_is_a_real_pinned_submodule():
    entry = subprocess.check_output(
        ["git", "ls-files", "--stage", "airlock-machine"], cwd=ROOT, text=True
    ).split()
    assert entry[0] == "160000"
    assert (
        entry[1]
        == subprocess.check_output(
            ["git", "-C", "airlock-machine", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip()
    )

    modules = configparser.ConfigParser()
    modules.read(ROOT / ".gitmodules")
    assert modules['submodule "airlock-machine"']["path"] == "airlock-machine"
    assert (
        modules['submodule "airlock-machine"']["url"]
        == "/home/user/dev/airlock-org/airlock-machine"
    )


def test_release_and_usb_assembly_have_separate_dependency_edges():
    machine_release = (
        ROOT / "airlock-machine/lifecycle/misc/modules/machine-release.nix"
    ).read_text()
    usb_adapter = (ROOT / "lifecycle/image/profiles/usb-prepared-store.nix").read_text()
    dev_flake = (ROOT / "flake.nix").read_text()

    assert "prepared" not in machine_release.lower()
    assert "airlockMachineRelease" not in usb_adapter
    assert "airlock-machine.lib.mkMachineConfiguration" in dev_flake
    assert "preparedStore.descriptors" in dev_flake


def test_both_nix_wrappers_select_the_external_airlock_store():
    for repository in (ROOT, ROOT / "airlock-machine"):
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
