"""Declarative selection, read-only guards, and boot wiring (no GPU resets)."""

import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
from types import SimpleNamespace as NS

import pytest

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "lifecycle/misc/tools/pkgs/airlock-gui-gpu-check/src/check.py"
spec = importlib.util.spec_from_file_location("gui_gpu_check", SOURCE)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class Domains(dict):
    def __iter__(self):
        return iter(self.values())


def vm(name, *, assignments=(), attached=(), running=False):
    return NS(
        name=name,
        is_running=lambda: running,
        devices={
            "pci": NS(
                get_assigned_devices=lambda: assignments,
                get_attached_devices=lambda: attached,
            )
        },
    )


def assignment(port="00_03.1-00_00.0", **kwargs):
    values = dict(
        backend_domain=NS(name="dom0"),
        port_id=port,
        mode=NS(value="required"),
        device_id="*",
        options={},
    )
    values.update(kwargs)
    return NS(**values)


@pytest.fixture
def fixture(tmp_path):
    # Different GPU, vendor, bus and Qubes port from the ThinkPad fixture.
    selection = dict(
        bdf="0000:09:03.0",
        port="00_03.1-00_00.0",
        vendor="8086",
        device="1234",
        driver="i915",
    )
    path = tmp_path / selection["bdf"]
    path.mkdir()
    for field, value in dict(
        vendor="0x8086", device="0x1234", **{"class": "0x030000"}
    ).items():
        (path / field).write_text(value)
    (path / "driver").symlink_to("/sys/bus/pci/drivers/pciback")
    topology = {
        "domains": {
            "sys-gui": {
                "prefs": {"virt_mode": "hvm", "autostart": False},
                "features": {"video-model": "none"},
                "devices": {
                    "pci": [
                        {
                            "backend": "dom0",
                            "id": selection["port"],
                            "mode": "required",
                            "match": "port",
                            "options": {},
                        }
                    ]
                },
            }
        }
    }
    dom0 = vm("dom0")

    class PCI:
        def __getitem__(self, port):
            return NS(data={"sbdf": selection["bdf"]})

        def get_assigned_devices(self):
            return []

        def get_attached_devices(self):
            return []

    dom0.devices["pci"] = PCI()
    app = NS(domains=Domains(dom0=dom0))
    return selection, topology, app, tmp_path


def check(fixture, **kwargs):
    selection, topology, app, sysfs = fixture
    return checker.validate(selection, topology, app, sysfs=sysfs, **kwargs)


def test_fresh_boot_before_vm_creation(fixture):
    assert check(fixture)["persistent_assignment"] is False
    with pytest.raises(ValueError, match="bootstrap has not persisted"):
        check(fixture, phase="start")


@pytest.mark.parametrize("running", [False, True])
def test_matching_persistent_assignment_is_idempotent(fixture, running):
    fixture[2].domains["sys-gui"] = vm(
        "sys-gui", assignments=[assignment()], running=running
    )
    assert check(fixture, phase="start")["persistent_assignment"] is True


@pytest.mark.parametrize(
    "field,value,match",
    [
        ("vendor", "0x1002", "vendor mismatch"),
        ("device", "0x9999", "device mismatch"),
        ("class", "0x020000", "not a display"),
    ],
)
def test_hardware_mismatch(fixture, field, value, match):
    (fixture[3] / fixture[0]["bdf"] / field).write_text(value)
    with pytest.raises(ValueError, match=match):
        check(fixture)


def test_missing_gpu(fixture):
    fixture[0]["bdf"] = "0000:08:03.0"
    with pytest.raises(ValueError, match="is missing"):
        check(fixture)


def test_gpu_still_owned_by_dom0(fixture):
    driver = fixture[3] / fixture[0]["bdf"] / "driver"
    driver.unlink()
    driver.symlink_to("/sys/bus/pci/drivers/i915")
    with pytest.raises(ValueError, match="not reserved"):
        check(fixture)


def test_qubes_port_resolves_to_different_gpu(fixture):
    fixture[2].domains["dom0"].devices["pci"] = {
        fixture[0]["port"]: NS(data={"sbdf": "0000:08:00.0"})
    }
    with pytest.raises(ValueError, match="does not resolve"):
        check(fixture)


@pytest.mark.parametrize(
    "field,value",
    [
        ("id", "00_01.0"),
        ("mode", "auto-attach"),
        ("options", {"no-strict-reset": True}),
    ],
)
def test_topology_cannot_disagree_with_selection(fixture, field, value):
    fixture[1]["domains"]["sys-gui"]["devices"]["pci"][0][field] = value
    with pytest.raises(ValueError, match="topology does not contain"):
        check(fixture)


@pytest.mark.parametrize(
    "change",
    [
        {"port_id": "00_02.0"},
        {"device_id": "specific-device"},
        {"mode": NS(value="auto-attach")},
        {"options": {"permissive": "true"}},
        {"backend_domain": NS(name="other")},
    ],
)
def test_stale_assignment_is_not_replaced(fixture, change):
    fixture[2].domains["sys-gui"] = vm("sys-gui", assignments=[assignment(**change)])
    with pytest.raises(ValueError, match="conflicting PCI assignment"):
        check(fixture)


@pytest.mark.parametrize("kind", ["assignments", "attached"])
def test_other_vm_ownership_is_not_taken(fixture, kind):
    fixture[2].domains["other"] = vm("other", **{kind: [assignment()]})
    with pytest.raises(ValueError, match="already assigned/attached to other"):
        check(fixture)


def test_live_manual_experiment_remains_untouched(fixture):
    fixture[2].domains["sys-gui"] = vm("sys-gui", attached=[assignment()], running=True)
    with pytest.raises(
        ValueError, match="running without the declared persistent assignment"
    ):
        check(fixture)


def test_extra_attached_device_rejected(fixture):
    fixture[2].domains["sys-gui"] = vm(
        "sys-gui",
        assignments=[assignment()],
        attached=[assignment(port="00_01.0")],
        running=True,
    )
    with pytest.raises(ValueError, match="conflicting attached"):
        check(fixture)


def test_validation_does_not_mutate_topology(fixture):
    before = copy.deepcopy(fixture[1])
    check(fixture)
    assert before == fixture[1]


def test_disabled_profile_needs_no_gpu(fixture):
    fixture[0].update(enable=False, bdf="0000:ff:00.0")
    assert check(fixture)["enabled"] is False
    with pytest.raises(ValueError, match="ownership is disabled"):
        check(fixture, phase="start")


@pytest.mark.parametrize("kind", ["assignments", "attached"])
def test_switch_to_hybrid_does_not_reuse_stale_physical_gpu(fixture, kind):
    fixture[0]["enable"] = False
    fixture[2].domains["sys-gui"] = vm("sys-gui", **{kind: [assignment()]})
    with pytest.raises(ValueError, match="PCI state remains from another profile"):
        check(fixture)


@pytest.mark.nix
def test_nix_selection_drives_reservation_assignment_and_start():
    expression = f"""
      let
        f = builtins.getFlake "{ROOT}";
        base = f.nixosConfigurations.usb-dev;
        normal = base.config;
        lab = normal.specialisation.usb-gui-lab.configuration;
        guestBase = f.nixosConfigurations.airlock-domain-sys-gui-usb-gui-lab;
        guestReport = c: {{
          defaultUnit = c.systemd.defaultUnit;
          displayWantedBy = c.systemd.services.display-manager.wantedBy;
          session = c.services.displayManager.defaultSession;
          gdm = c.services.displayManager.gdm.enable;
          lightdm = c.services.xserver.displayManager.lightdm.enable;
          receiver = c.airlock.inputProxy.receiver.enable;
          guiAgent = c.systemd.services.qubes-gui-agent.enable;
          failed = map (a: a.message) (builtins.filter (a: !a.assertion) c.assertions);
        }};
        alternate = (base.extendModules {{ modules = [ ({{ lib, ... }}: {{
          specialisation.usb-gui-lab.configuration.airlock.guiGpu.device = lib.mkForce {{
            bdf = "0000:09:03.0"; port = "00_03.1-00_00.0";
            vendor = "1002"; device = "9999"; driver = "amdgpu";
          }};
        }}) ]; }}).config.specialisation.usb-gui-lab.configuration;
        report = c: {{
          enabled = c.airlock.guiGpu.enable;
          device = c.airlock.guiGpu.device;
          pci = c.airlock.qubes.topology.domains.sys-gui.devices.pci;
          params = c.boot.kernelParams;
          early = c.boot.initrd.kernelModules;
          xenParams = c.virtualisation.xen.boot.params;
          failed = map (a: a.message) (builtins.filter (a: !a.assertion) c.assertions);
        }};
      in {{
        normal = report normal; lab = report lab; alternate = report alternate;
        service = {{ inherit (lab.systemd.services.airlock-start-gui-gpu) requires wants after serviceConfig; }};
        noBypass = !lab.systemd.services."qubes-vm@sys-gui".enable;
        normalGuard = builtins.fromJSON normal.environment.etc."airlock/gui-gpu.json".text;
        framebuffer = {{
          enabled = lab.airlock.guiGpu.bootFramebuffer.enable;
          statusDevice = lab.airlock.qubes.setupConfigOnBoot.statusDevice;
          getty = lab.systemd.services."getty@tty1".enable;
          displayManager = lab.systemd.services.display-manager.enable;
          prepare = lab.systemd.services.airlock-setup-config.serviceConfig.ExecStartPre;
        }};
        dark = report (base.extendModules {{ modules = [ ({{ lib, ... }}: {{
          specialisation.usb-gui-lab.configuration.airlock.guiGpu.bootFramebuffer.enable = lib.mkForce false;
        }}) ]; }}).config.specialisation.usb-gui-lab.configuration;
        guest = guestReport guestBase.config;
        diagnosticGuest = guestReport (guestBase.extendModules {{ modules = [
          ({{ lib, ... }}: {{ airlock.sysGuiGuest.physicalDesktopOnBoot = lib.mkForce false; }})
        ]; }}).config;
        hybridGuest = guestReport f.nixosConfigurations.airlock-domain-sys-gui-usb.config;
        normalSender = normal.airlock.inputProxy.sender.enable;
        recoverySender = normal.specialisation.usb-text-recovery.configuration.airlock.inputProxy.sender.enable;
        inputTarget = lab.systemd.targets.airlock-gui-input.wantedBy;
        inputServices = lab.systemd.targets.airlock-gui-input.wants;
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
    report = json.loads(result.stdout)
    logs = Path(os.environ["AIRLOCK_LOG_DIR"]) / "gui-gpu-assignment"
    logs.mkdir(parents=True, exist_ok=True)
    (logs / "nix.json").write_text(result.stdout)
    assert report["normal"]["enabled"] is False
    assert report["normalGuard"]["enable"] is False
    assert report["normal"]["pci"] == []
    assert "amdgpu" in report["normal"]["early"]
    for key in ("lab", "alternate"):
        cfg = report[key]
        assert cfg["failed"] == []
        assert f'xen-pciback.hide=({cfg["device"]["bdf"]})' in cfg["params"]
        assert cfg["pci"] == [
            dict(
                backend="dom0",
                id=cfg["device"]["port"],
                match="port",
                mode="required",
                options={},
            )
        ]
        assert "amdgpu" not in cfg["early"]
        assert "xen-pciback" in cfg["early"]
        assert "iommu=required" in cfg["xenParams"]
    assert "xen-pciback.hide=(0000:04:00.0)" not in report["alternate"]["params"]
    service = report["service"]
    assert "airlock-setup-config.service" in service["requires"]
    assert "qubesd.service" not in service["requires"]
    assert "qubesd.service" in service["wants"]
    assert "qubesd.service" in service["after"]
    assert "airlock-setup-config.service" in service["after"]
    assert service["serviceConfig"]["Restart"] == "no"
    assert (
        "is-active --quiet qubesd.service"
        in service["serviceConfig"]["ExecStartPre"][0]
    )
    assert "--phase start" in service["serviceConfig"]["ExecStartPre"][1]
    assert report["noBypass"] is True
    assert report["framebuffer"]["enabled"] is True
    assert report["framebuffer"]["statusDevice"] == "/dev/tty1"
    assert report["framebuffer"]["getty"] is False
    assert report["framebuffer"]["displayManager"] is False
    assert report["framebuffer"]["prepare"][0].endswith("/bin/chvt 1")
    assert "initcall_blacklist=sysfb_init" not in report["lab"]["params"]
    assert "video=efifb:off" not in report["lab"]["params"]
    assert "fbcon=nodefer" in report["lab"]["params"]
    assert "vga=current" in report["lab"]["xenParams"]
    assert not any(
        p.startswith("vga=") and ",keep" in p for p in report["lab"]["xenParams"]
    )
    assert "initcall_blacklist=sysfb_init" in report["dark"]["params"]
    assert "video=efifb:off" in report["dark"]["params"]
    assert (
        "boot-framebuffer.py --gpu 0000:04:00.0"
        in service["serviceConfig"]["ExecStartPre"][2]
    )
    assert service["serviceConfig"]["ExecStartPost"].endswith(
        "/bin/systemctl start airlock-gui-input.target"
    )
    assert report["inputTarget"] == []  # Only pulled in after successful VM start.
    assert report["inputServices"]
    assert report["normalSender"] is False
    assert report["recoverySender"] is False
    guest = report["guest"]
    assert guest["failed"] == []
    assert guest["defaultUnit"] == "graphical.target"
    assert guest["displayWantedBy"] == ["graphical.target"]
    assert guest["session"] == "gnome"
    assert guest["gdm"] is True
    assert guest["lightdm"] is False
    assert guest["receiver"] is True
    assert guest["guiAgent"] is False
    assert report["diagnosticGuest"]["defaultUnit"] == "multi-user.target"
    assert report["diagnosticGuest"]["displayWantedBy"] == []
    assert report["diagnosticGuest"]["failed"] == []
    assert report["hybridGuest"]["displayWantedBy"] == []
    assert report["hybridGuest"]["receiver"] is False
    assert report["hybridGuest"]["gdm"] is False
    assert report["hybridGuest"]["lightdm"] is True


def test_bootstrap_guard_precedes_mutation():
    script = (ROOT / "lifecycle/misc/tools/scripts/setup-config.sh").read_text()
    assert script.index("airlock-gui-gpu-check --topology") < script.index(
        'for template in "${template_names[@]}"'
    )


@pytest.mark.qubes
def test_laptop_read_only_preflight():
    host = os.environ.get("AIRLOCK_GPU_ASSIGNMENT_SSH")
    if not host:
        pytest.skip("set AIRLOCK_GPU_ASSIGNMENT_SSH for read-only laptop validation")
    evaluated = subprocess.run(
        [
            "nix",
            "eval",
            "--json",
            ".#nixosConfigurations.usb-dev.config.specialisation.usb-gui-lab.configuration",
            "--apply",
            'c: { selection = builtins.fromJSON c.environment.etc."airlock/gui-gpu.json".text; topology = c.airlock.qubes.topology; }',
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert evaluated.returncode == 0, evaluated.stderr
    payload = json.loads(evaluated.stdout)
    script = f"""
import hashlib, json, pathlib, runpy
from types import SimpleNamespace
wrapper = pathlib.Path('/run/current-system/sw/bin/qvm-device').resolve().with_name('.qvm-device-wrapped')
runpy.run_path(str(wrapper), run_name='read_only_test')
import qubesadmin
scope = {{'__name__': 'gpu_check_test'}}
exec({SOURCE.read_text()!r}, scope)
validate = scope['validate']
payload = json.loads({json.dumps(payload)!r})
selection, topology = payload['selection'], payload['topology']
state = pathlib.Path('/var/lib/qubes/qubes.xml')
before = hashlib.sha256(state.read_bytes()).hexdigest()
app = qubesadmin.Qubes()
try:
    actual = validate(selection, topology, app)
except ValueError as error:
    assert 'running without the declared persistent assignment' in str(error), str(error)
    actual = {{'status': 'refused-live-mutation', 'error': str(error)}}
# Project only sys-gui away to model a fresh bootstrap, retaining real host
# sysfs, Qubes port resolution and every other VM's ownership. No VM is changed.
class Domains(dict):
    def __iter__(self):
        return iter(self.values())
fresh = SimpleNamespace(domains=Domains({{vm.name: vm for vm in app.domains if vm.name != 'sys-gui'}}))
projected = validate(selection, topology, fresh)
assert projected['status'] == 'passed'
try:
    validate(selection | {{'vendor': 'ffff'}}, topology, fresh)
except ValueError as error:
    assert 'vendor mismatch' in str(error)
else:
    raise AssertionError('wrong vendor was accepted')
assert hashlib.sha256(state.read_bytes()).hexdigest() == before, 'Qubes state changed during read-only test'
print(json.dumps({{'schema': 'airlock.gui-gpu-live-check.v1', 'actual': actual, 'projected_fresh_boot': projected, 'wrong_vendor_rejected': True, 'qubes_xml_unchanged': True}}))
"""
    result = subprocess.run(
        ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", host, "python3 -"],
        input=script,
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    logs = Path(os.environ["AIRLOCK_LOG_DIR"]) / "gui-gpu-assignment"
    logs.mkdir(parents=True, exist_ok=True)
    (logs / "live.stdout").write_text(result.stdout)
    (logs / "live.stderr").write_text(result.stderr)
    assert result.returncode == 0, result.stdout + result.stderr
    report = json.loads(result.stdout)
    (logs / "live.json").write_text(json.dumps(report, indent=2) + "\n")
    assert report["qubes_xml_unchanged"] is True
