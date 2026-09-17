"""Airlock's systemd-boot menu exposes current, recovery and old generations."""

import importlib.util
from pathlib import Path

import pytest


@pytest.fixture
def boot_menu():
    source = (
        Path(__file__).resolve().parents[1]
        / "lifecycle/misc/tools/scripts/boot-menu.py"
    )
    spec = importlib.util.spec_from_file_location("airlock_boot_menu", source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def add_entry(esp, store, name, generation, role=None, title="NixOS"):
    system = store / name
    if role:
        marker = system / "etc/airlock/boot-role"
        marker.parent.mkdir(parents=True)
        marker.write_text(role + "\n")
    ordinary = esp / "loader/entries" / f"nixos-{name}.conf"
    ordinary.write_text(
        f"title {title}\n"
        f"version Generation {generation} test\n"
        f"options init=/nix/store/{name}/init console=tty0\n"
        "sort-key nixos\n"
    )
    xen = esp / "loader/entries" / f"xen-{name}.conf"
    xen.write_text(
        f"title {title} (with Xen Hypervisor)\n"
        f"version Generation {generation} test\n"
        f"efi /EFI/nixos/xen-{name}.efi\n"
        "sort-key nixos\n"
    )
    return xen


def menu_fixture(tmp_path):
    esp = tmp_path / "esp"
    store = tmp_path / "store"
    entries = esp / "loader/entries"
    entries.mkdir(parents=True)
    store.mkdir()
    loader = esp / "loader/loader.conf"
    loader.write_text("timeout 10\npreferred xen-base12\ndefault xen-base12\n")
    add_entry(esp, store, "base12", 12)
    current = add_entry(esp, store, "lab12", 12, "normal", "NixOS (usb-lab)")
    recovery = add_entry(
        esp, store, "recovery12", 12, "recovery", "NixOS (usb-text-recovery)"
    )
    previous = add_entry(esp, store, "lab11", 11, "normal")
    add_entry(esp, store, "lab10a", 10, "normal", "NixOS (usb-lab)")
    add_entry(esp, store, "lab10b", 10, "normal")
    add_entry(esp, store, "recovery11", 11, "recovery")
    add_entry(esp, store, "gui12", 12, None, "NixOS (usb-gui-lab)")
    other = entries / "other.conf"
    other.write_text("title Other OS\nefi /EFI/other.efi\n")
    return esp, store, current, recovery, previous, other


def test_menu_keeps_current_recovery_and_numbered_normal_generations(
    boot_menu, tmp_path
):
    esp, store, current, recovery, previous, other = menu_fixture(tmp_path)

    boot_menu.configure(esp, store)

    entries = list((esp / "loader/entries").glob("*.conf"))
    titles = {boot_menu.fields(entry).get("title"): entry for entry in entries}
    assert set(titles) == {
        "Airlock",
        "Airlock (recovery)",
        "Airlock (generation 11)",
        "Airlock (generation 10)",
        "Other OS",
    }
    assert titles["Airlock"] == current
    assert titles["Airlock (recovery)"] == recovery
    assert titles["Airlock (generation 11)"] == previous
    assert other.is_file()
    loader = boot_menu.fields(esp / "loader/loader.conf")
    assert loader["default"] == current.stem
    assert loader["preferred"] == current.stem
    assert boot_menu.fields(current)["sort-key"] == "00-airlock"
    assert boot_menu.fields(recovery)["sort-key"] == "01-airlock-recovery"


def test_menu_refuses_to_mutate_without_recovery(boot_menu, tmp_path):
    esp, store, current, recovery, _, _ = menu_fixture(tmp_path)
    for marker in store.glob("recovery*/etc/airlock/boot-role"):
        marker.unlink()
    before = {path: path.read_text() for path in esp.rglob("*.conf")}

    with pytest.raises(ValueError, match="no Xen Airlock recovery"):
        boot_menu.configure(esp, store)

    assert {path: path.read_text() for path in esp.rglob("*.conf")} == before


def test_menu_pins_recovery_as_a_system_profile(boot_menu, tmp_path, monkeypatch):
    esp, store, _, _, _, _ = menu_fixture(tmp_path)
    profile = tmp_path / "profiles/system-profiles/airlock-recovery"
    calls = []
    monkeypatch.setattr(
        boot_menu.subprocess,
        "run",
        lambda command, check: calls.append((command, check)),
    )

    boot_menu.configure(
        esp, store, recovery_profile=profile, nix_env="/nix/store/nix/bin/nix-env"
    )

    assert profile.parent.is_dir()
    assert calls == [
        (
            [
                "/nix/store/nix/bin/nix-env",
                "--profile",
                str(profile),
                "--set",
                str(store / "recovery12"),
            ],
            True,
        )
    ]


def test_selected_rollback_generation_remains_the_airlock_default(boot_menu, tmp_path):
    esp, store, current, _, previous, _ = menu_fixture(tmp_path)
    loader = esp / "loader/loader.conf"
    loader.write_text(f"timeout 10\ndefault {previous.stem}\n")

    boot_menu.configure(esp, store)

    assert boot_menu.fields(previous)["title"] == "Airlock"
    assert boot_menu.fields(current)["title"] == "Airlock (generation 12)"
    assert boot_menu.fields(loader)["default"] == previous.stem
