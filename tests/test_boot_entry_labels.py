"""Exercise the actual post-install shell hook against retained boot entries."""

from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def test_boot_labels_repair_retained_entries_and_are_idempotent(tmp_path):
    source = (ROOT / "os/lifecycle/misc/modules/dom0.nix").read_text()
    hook = source.split(
        "boot.loader.systemd-boot.extraInstallCommands = lib.mkAfter ''", 1
    )[1].split("\n    '';", 1)[0]
    hook = (
        hook.replace("${config.boot.loader.efi.efiSysMountPoint}", str(tmp_path))
        .replace("${pkgs.gnused}/bin/sed", shutil.which("sed"))
        .replace("${lib.optionalString cfg.bootMenu.enable bootMenu}", "")
    )
    entries = tmp_path / "loader/entries"
    entries.mkdir(parents=True)
    recovery = " (Recovery: without Xen)"
    contents = {
        "xen-old.conf": "title NixOS (usb-lab)"
        + recovery * 2
        + " (with Xen Hypervisor)\nversion Generation 1\nefi /EFI/nixos/old.efi\n",
        "xen-new.conf": "title NixOS (with Xen Hypervisor)\nversion Generation 3\n",
        "nixos-old.conf": "title NixOS (usb-lab)"
        + recovery * 2
        + "\nlinux /EFI/kernel\noptions init=/nix/store/system/init\n",
        "nixos-new.conf": "title NixOS\nversion Generation 3\n",
        "other.conf": "title Other OS\nefi /EFI/other.efi\n",
    }
    for name, content in contents.items():
        (entries / name).write_text(content)
    for _ in range(2):
        subprocess.run(["bash", "-eu", "-c", hook], check=True)
        for name, original in contents.items():
            title, rest = original.split("\n", 1)
            if name.startswith(("xen-", "nixos-")):
                title = title.replace(recovery, "")
            if name.startswith("nixos-"):
                title += recovery
            assert (entries / name).read_text() == title + "\n" + rest

    # Empty directories must also succeed (unmatched globs are not filenames).
    for entry in entries.iterdir():
        entry.unlink()
    subprocess.run(["bash", "-eu", "-c", hook], check=True)
