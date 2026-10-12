"""Provisioning safety contracts; no actual disks, mounts or credentials used."""

import importlib.util
import json
from pathlib import Path
import subprocess
from copy import deepcopy
from types import SimpleNamespace
import shutil

import pytest


@pytest.fixture
def usb():
    source = Path(__file__).resolve().parents[1] / "lifecycle/image/tools/usb-dev.py"
    spec = importlib.util.spec_from_file_location("usb_dev", source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.mark.parametrize(
    "device", ["/dev/sda", "/dev/nvme0n1", "/", "/dev/disk/by-id/ata-disk"]
)
def test_writer_requires_explicit_usb_identity(usb, device):
    with pytest.raises(ValueError, match="whole-disk"):
        usb.usb_disk(device)


def test_mutable_destination_rejects_symlinks(usb, tmp_path):
    (tmp_path / "home").symlink_to("/tmp")
    with pytest.raises(ValueError, match="symlink"):
        usb.beneath(tmp_path, "home/user/.ssh")


def test_provisioner_rejects_prior_dom0_username_image(usb, tmp_path, monkeypatch):
    device = tmp_path / "usb-root-device"
    device.touch()
    marker = tmp_path / "airlock-usb-image"
    marker.write_text("airlock-usb-dev-v1\n")
    monkeypatch.setattr(usb, "root_only", lambda: None)
    monkeypatch.setattr(
        usb,
        "output",
        lambda *args: json.dumps(
            {"filesystems": [{"fstype": "ext4", "source": str(device)}]}
        ),
    )
    monkeypatch.setattr(
        usb,
        "disks",
        lambda: [
            {
                "tran": "usb",
                "name": "/dev/test-usb",
                "children": [{"name": str(device), "label": "airlock-usb"}],
            }
        ],
    )
    with pytest.raises(ValueError, match="Missing USB image marker"):
        usb.checked_root(tmp_path)
    marker.write_text(usb.MARKER)
    assert usb.checked_root(tmp_path) == tmp_path


def test_private_file_never_overwrites(usb, tmp_path):
    target = tmp_path / "existing"
    target.write_text("keep")
    with pytest.raises(FileExistsError):
        usb.private_file(target, b"replacement")
    assert target.read_text() == "keep"


def test_sparse_copy_writes_allocated_extents_only(usb, tmp_path):
    image = tmp_path / "seed.img"
    with image.open("wb") as stream:
        stream.write(b"first")
        stream.seek(1024 * 1024)
        stream.write(b"last")
    target = tmp_path / "target"
    target.write_bytes(b"x" * (1024 * 1024 + 4))
    with target.open("r+b", buffering=0) as destination:
        copied = usb.copy_sparse_image(image, destination)
    assert copied < image.stat().st_size
    with target.open("rb") as stream:
        assert stream.read(5) == b"first"
        stream.seek(1024 * 1024)
        assert stream.read(4) == b"last"


def test_flatten_includes_descendants(usb):
    nodes = [
        {
            "name": "disk",
            "children": [{"name": "partition", "children": [{"name": "crypt"}]}],
        }
    ]
    assert [node["name"] for node in usb.flatten(nodes)] == [
        "disk",
        "partition",
        "crypt",
    ]


@pytest.mark.parametrize("action", ["vault-create", "vault-unlock", "vault-lock", "auth-import"])
def test_removed_credential_commands_fail_before_disk_access(usb, monkeypatch, action):
    monkeypatch.setattr(usb.sys, "argv", ["airlock-usb", action])
    monkeypatch.setattr(usb, "root_only", lambda: pytest.fail("must not access disks"))
    with pytest.raises(SystemExit) as error:
        usb.main()
    assert error.value.code == 2


@pytest.fixture
def amd():
    source = (
        Path(__file__).resolve().parents[1] / "lifecycle/image/tools/amd-preflight.py"
    )
    spec = importlib.util.spec_from_file_location("amd_preflight", source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_amd_missing_evidence_is_a_failure(amd, tmp_path):
    report = amd.inspect(tmp_path / "sys", tmp_path / "proc")
    assert not report["assignment_gate_passed"]
    assert "not-xen-dom0" in report["failures"]
    assert "missing-or-shared-iommu-group" in report["failures"]
    assert not report["graphics_verified"]


@pytest.mark.nix
def test_usb_dom0_display_does_not_wait_for_guest_password_mutation():
    root = Path(__file__).resolve().parents[1]
    expression = f"""
      let
        configs = (builtins.getFlake "{root}").nixosConfigurations;
        config = configs.usb-dev.config;
        labels = (configs.usb-dev.extendModules {{ modules = [
          ({{ airlockUsbGuestEntries, ... }}: {{
            system.build.testGuestLabels = builtins.attrNames airlockUsbGuestEntries;
          }})
        ]; }}).config.system.build.testGuestLabels;
        login = guest: {{
          password = guest.users.users.user.initialPassword;
          initialHash = guest.users.users.user.initialHashedPassword;
          mutable = guest.users.mutableUsers;
        }};
        dom0Login = host: {{
          mutable = host.users.mutableUsers;
          passwordFile = host.users.users.user.hashedPasswordFile;
          account = host.airlock.dom0.user;
          terminalAccount = host.airlock.devDom0Terminal.user;
          uid = host.users.users.user.uid;
          home = host.users.users.user.home;
          rootHash = host.users.users.root.hashedPassword;
        }};
      in {{
        specialisations = builtins.attrNames config.specialisation;
        guestLabels = labels;
        baseSetup = config.airlock.qubes.setupConfigOnBoot.enable;
        normalSetup = config.specialisation.usb-lab.configuration.airlock.qubes.setupConfigOnBoot.enable;
        journal = config.services.journald.settings.Journal;
        conciseBootMenu = config.airlock.dom0.bootMenu.enable;
        imageMarker = config.environment.etc."airlock/usb-dev".text;
        bootHook = config.boot.loader.systemd-boot.extraInstallCommands;
        dom0Logins = [ (dom0Login config) ] ++ builtins.map
          (name: dom0Login config.specialisation.${{name}}.configuration)
          (builtins.filter (name: name != "usb-text-recovery") (builtins.attrNames config.specialisation));
        hasPasswordService = builtins.hasAttr "airlock-usb-gui-password" config.systemd.services;
        displayRequires = config.systemd.services.display-manager.requires;
        displayAfter = config.systemd.services.display-manager.after;
        hybridLogin = login configs.airlock-domain-sys-gui-usb.config;
        labLogin = login configs.airlock-domain-sys-gui-usb-gui-lab.config;
        normalPassword = configs.airlock-domain-sys-gui.config.users.users.user.initialPassword;
        labHasPasswordService = builtins.hasAttr "airlock-usb-gui-password"
          config.specialisation.usb-lab.configuration.systemd.services;
        recovery = let
          recoveryConfig = config.specialisation.usb-text-recovery.configuration;
        in {{
          role = recoveryConfig.environment.etc."airlock/boot-role".text;
          xen = recoveryConfig.virtualisation.xen.enable;
          display = recoveryConfig.services.xserver.enable;
          setup = recoveryConfig.airlock.qubes.setupConfigOnBoot.enable or false;
          networkManager = recoveryConfig.networking.networkmanager.enable;
          tailscale = recoveryConfig.services.tailscale.enable;
          ssh = recoveryConfig.services.openssh.enable;
          poolRecovery = recoveryConfig.environment.etc."airlock/pool-recovery".text;
          store = recoveryConfig.fileSystems."/nix/store".device;
          passwordFile = recoveryConfig.users.users.user.hashedPasswordFile;
        }};
      }}
    """
    result = subprocess.run(
        ["nix", "eval", "--impure", "--json", "--expr", expression],
        cwd=root,
        capture_output=True,
        text=True,
        check=True,
    )
    configuration = json.loads(result.stdout)
    assert configuration["specialisations"] == [
        "usb-lab",
        "usb-text-recovery",
    ]
    assert configuration["guestLabels"] == [
        "sys-ctrl",
        "usr-assistant",
        "sys-firewall",
        "sys-gui",
        "sys-net",
        "usr-browser",
        "usr-home",
    ]
    assert configuration["baseSetup"] is False
    assert configuration["normalSetup"] is True
    assert configuration["journal"]["Storage"] == "persistent"
    assert configuration["journal"]["SyncIntervalSec"] == "1s"
    assert configuration["conciseBootMenu"] is True
    assert configuration["imageMarker"] == "airlock-usb-dev-v2\n"
    assert "-airlock-boot-menu" in configuration["bootHook"]
    recovery = configuration["recovery"]
    assert recovery["role"] == "recovery\n"
    assert recovery["xen"] is True
    assert recovery["display"] is False
    assert recovery["setup"] is False
    assert recovery["networkManager"] is False
    assert recovery["tailscale"] is False
    assert recovery["ssh"] is False
    assert "Independent pool recovery" in recovery["poolRecovery"]
    assert recovery["store"] == "/var/lib/airlock/recovery-store"
    assert recovery["passwordFile"] == "/var/lib/airlock/usb/local-password.hash"
    for login in configuration["dom0Logins"]:
        assert login == {
            "mutable": False,
            "account": "user",
            "terminalAccount": "user",
            "uid": 1000,
            "home": "/home/user",
            "passwordFile": "/var/lib/airlock/usb/local-password.hash",
            "rootHash": "!",
        }
    assert configuration["hasPasswordService"] is False
    assert "airlock-usb-gui-password.service" not in configuration["displayRequires"]
    assert "airlock-usb-gui-password.service" not in configuration["displayAfter"]
    assert (
        configuration["hybridLogin"]
        == configuration["labLogin"]
        == {"password": "password", "initialHash": None, "mutable": False}
    )
    assert configuration["normalPassword"] == "airlock"
    assert configuration["labHasPasswordService"] is False


def test_amd_bus_reset_is_not_accepted(amd, tmp_path):
    device = tmp_path / "sys/bus/pci/devices/0000:04:00.0"
    device.mkdir(parents=True)
    (device / "vendor").write_text("0x1002\n")
    (device / "device").write_text("0x1681\n")
    (device / "reset_method").write_text("bus\n")
    report = amd.inspect(tmp_path / "sys", tmp_path / "proc")
    assert "function-level-reset-not-selected-review-required" in report["failures"]
    assert not report["assignment_gate_passed"]


@pytest.fixture
def growth_table():
    return {
        "label": "gpt",
        "device": "/dev/sdc",
        "id": "disk-id",
        "sectorsize": 512,
        "lastlba": 67108830,
        "partitions": [
            {
                "node": "/dev/sdc1",
                "start": 16384,
                "size": 2082816,
                "type": "C12A7328-F81F-11D2-BA4B-00A0C93EC93B",
                "uuid": "esp-id",
            },
            {
                "node": "/dev/sdc2",
                "start": 2099200,
                "size": 65007616,
                "type": "0FC63DAF-8483-4772-8E79-3D69D8477DE4",
                "uuid": "root-id",
            },
        ],
    }


def test_growth_accepts_actual_seed_layout(usb, growth_table):
    root = usb.checked_growth_layout(
        growth_table, Path("/dev/sdc"), Path("/dev/sdc2"), 62495129600
    )
    assert root["start"] == 2099200


@pytest.mark.parametrize(
    "damage", ["mbr", "extra", "overlap", "sector", "identity", "beyond", "type"]
)
def test_growth_rejects_unexpected_layout(usb, growth_table, damage):
    if damage == "mbr":
        growth_table["label"] = "dos"
    elif damage == "extra":
        growth_table["partitions"].append({})
    elif damage == "overlap":
        growth_table["partitions"][1]["start"] = 40
    elif damage == "sector":
        growth_table["sectorsize"] = 4096
    elif damage == "identity":
        growth_table["partitions"][1]["uuid"] = ""
    elif damage == "beyond":
        growth_table["partitions"][1]["size"] = 10**12
    else:
        growth_table["partitions"][1]["type"] = "different"
    with pytest.raises(ValueError):
        usb.checked_growth_layout(
            growth_table, Path("/dev/sdc"), Path("/dev/sdc2"), 62495129600
        )


@pytest.mark.parametrize(
    "damage", ["start", "uuid", "esp", "disk", "shrink", "not-full"]
)
def test_growth_refuses_bad_result_before_filesystem_resize(usb, growth_table, damage):
    after = deepcopy(growth_table)
    after["lastlba"] = 122060766
    after["partitions"][1]["size"] = (
        after["lastlba"] + 1 - after["partitions"][1]["start"]
    )
    if damage in ("start", "uuid"):
        after["partitions"][1][damage] = 20
    elif damage == "esp":
        after["partitions"][0]["size"] += 1
    elif damage == "disk":
        after["id"] = "other"
    elif damage == "shrink":
        after["partitions"][1]["size"] = 1
    else:
        after["partitions"][1]["size"] -= 2048
    with pytest.raises(ValueError):
        usb.checked_growth_result(growth_table, after)


def test_provision_stops_before_personalization_if_expansion_fails(usb, monkeypatch):
    calls = []
    monkeypatch.setattr(usb, "write_latest_image", lambda args: calls.append("write"))
    monkeypatch.setattr(usb, "run", lambda *args: calls.append(args))

    def fail(*args, **kwargs):
        calls.append("expand")
        raise ValueError("expansion failed")

    monkeypatch.setattr(usb, "expand_provisioned_root", fail)
    monkeypatch.setattr(
        usb, "personalize", lambda args: pytest.fail("must not personalize")
    )
    with pytest.raises(ValueError, match="expansion failed"):
        usb.provision_latest_image(
            SimpleNamespace(
                device="device", serial="serial", resize_backup_dir="backups"
            )
        )
    assert calls == ["write", ("udevadm", "settle"), "expand"]


@pytest.mark.parametrize(
    "code,ok", [(0, True), (1, True), (2, False), (4, False), (8, False)]
)
def test_filesystem_check_exit_statuses(usb, monkeypatch, code, ok):
    monkeypatch.setattr(
        usb.subprocess,
        "run",
        lambda *a, **k: SimpleNamespace(returncode=code, stdout="", stderr=""),
    )
    if ok:
        usb.check_ext4("partition")
    else:
        with pytest.raises(ValueError, match="Filesystem check failed"):
            usb.check_ext4("partition")


def test_successful_command_stderr_is_reported_as_normal_output(
    usb, monkeypatch, capsys
):
    monkeypatch.setattr(
        usb.subprocess,
        "run",
        lambda *a, **k: SimpleNamespace(
            returncode=0, stdout="normal output\n", stderr="expected warning\n"
        ),
    )
    usb.report_command("example")
    captured = capsys.readouterr()
    assert captured.out == "normal output\nexpected warning\n"
    assert captured.err == ""


def test_reported_command_accepts_explicit_text_input(usb, monkeypatch):
    observed = {}

    def run(*args, **kwargs):
        observed.update(kwargs)
        return SimpleNamespace(returncode=0, stdout="", stderr="")

    monkeypatch.setattr(usb.subprocess, "run", run)
    usb.report_command("example", input=", +\n", text=True)
    assert observed["input"] == ", +\n"
    assert observed["text"] is True


def test_failed_command_diagnostics_remain_on_stderr(usb, monkeypatch, capsys):
    monkeypatch.setattr(
        usb.subprocess,
        "run",
        lambda *a, **k: SimpleNamespace(
            returncode=2, stdout="failure context\n", stderr="action required\n"
        ),
    )
    with pytest.raises(subprocess.CalledProcessError):
        usb.report_command("example")
    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "failure context\naction required\n"


def test_gpt_expansion_on_regular_file_preserves_partition_identities(usb, tmp_path):
    """Real util-linux, no root, loop devices, mounts, or physical disks."""
    if not shutil.which("sfdisk"):
        pytest.skip("sfdisk unavailable")
    image = tmp_path / "disk.img"
    with image.open("wb") as stream:
        stream.truncate(64 * 1024**2)
    usb.run(
        "sfdisk", str(image), input="label: gpt\nsize=8M, type=U\ntype=L\n", text=True
    )
    before = json.loads(usb.output("sfdisk", "--json", str(image)))["partitiontable"]
    usb.run(
        "sfdisk",
        "--backup-file",
        str(tmp_path / "backup"),
        "--backup-pt-sectors",
        str(image),
    )
    assert list(tmp_path.glob("backup*"))
    with image.open("r+b") as stream:
        stream.truncate(128 * 1024**2)
    usb.run("sfdisk", "--lock=yes", "--relocate", "gpt-bak-std", str(image))
    usb.run(
        "sfdisk",
        "--lock=yes",
        "--wipe=never",
        "--wipe-partitions=never",
        "-N",
        "2",
        str(image),
        input=", +\n",
        text=True,
    )
    after = json.loads(usb.output("sfdisk", "--json", str(image)))["partitiontable"]
    usb.checked_growth_layout(
        after, image, Path(str(image) + "2"), image.stat().st_size
    )
    usb.checked_growth_result(before, after)
    assert after["partitions"][1]["size"] > before["partitions"][1]["size"]


def test_ext4_grows_to_container_size_without_changing_uuid(usb, tmp_path):
    if not all(
        shutil.which(tool)
        for tool in ("mkfs.ext4", "e2fsck", "resize2fs", "blkid", "dumpe2fs")
    ):
        pytest.skip("e2fsprogs unavailable")
    image = tmp_path / "root.ext4"
    with image.open("wb") as stream:
        stream.truncate(32 * 1024**2)
    usb.run("mkfs.ext4", "-q", "-L", "airlock-usb", str(image))
    old_uuid = usb.output("blkid", "-s", "UUID", "-o", "value", str(image))
    usb.check_ext4(image)
    with image.open("r+b") as stream:
        stream.truncate(64 * 1024**2)
    usb.run("resize2fs", str(image))
    usb.check_ext4(image)
    assert usb.output("blkid", "-s", "UUID", "-o", "value", str(image)) == old_uuid
    fields = dict(
        (key.strip(), value.strip())
        for key, value in (
            line.split(":", 1)
            for line in usb.output("dumpe2fs", "-h", str(image)).splitlines()
            if ":" in line
        )
    )
    assert (
        int(fields["Block count"]) * int(fields["Block size"]) == image.stat().st_size
    )


def test_kernel_size_verification_does_not_repeat_partition_reread(usb, monkeypatch):
    calls = []
    monkeypatch.setattr(usb, "run", lambda *args: calls.append(args))
    monkeypatch.setattr(
        usb, "output", lambda *args: calls.append(args) or "61420322304"
    )
    usb.verify_kernel_partition_size(Path("/dev/sdc2"), 61420322304)
    assert calls == [
        ("udevadm", "settle"),
        ("blockdev", "--getsize64", "/dev/sdc2"),
    ]


def test_stale_kernel_size_still_fails_closed(usb, monkeypatch):
    monkeypatch.setattr(usb, "run", lambda *args: None)
    monkeypatch.setattr(usb, "output", lambda *args: "33283899392")
    with pytest.raises(ValueError, match="Unplug/replug"):
        usb.verify_kernel_partition_size(Path("/dev/sdc2"), 61420322304)


def test_growth_detects_already_expanded_partition(usb, growth_table):
    disk_bytes = 62495129600
    assert not usb.root_fills_disk(growth_table, disk_bytes)
    growth_table["lastlba"] = disk_bytes // 512 - 34
    growth_table["partitions"][1]["size"] = 61420322304 // 512
    assert usb.root_fills_disk(growth_table, disk_bytes)


def test_provision_make_target_keeps_root_only_backups_outside_repo():
    result = subprocess.run(
        [
            "make",
            "-n",
            "usb-dev-provision",
            "USB_SERIAL=test-serial",
            "AIRLOCK_DATA_ROOT=",
            "AIRLOCK_LOG_DIR=",
        ],
        cwd=Path(__file__).resolve().parents[1],
        capture_output=True,
        text=True,
        check=True,
    )
    assert '--resize-backup-dir "/var/log/airlock/usb-root-expansion"' in result.stdout
