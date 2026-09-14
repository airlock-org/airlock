"""Firmware discovery and placement with synthetic sysfs, XML, and VFCT."""

import importlib
from pathlib import Path
from types import SimpleNamespace
import uuid
import xml.etree.ElementTree as ET

import pytest

from test_amd_vbios import firmware


@pytest.fixture
def runtime(monkeypatch):
    source = Path(__file__).resolve().parents[1] / (
        "lifecycle/runtime/tools/pkgs/airlock-amd-vbios/src"
    )
    monkeypatch.syspath_prepend(str(source))
    return importlib.import_module("airlock_amd_vbios.runtime")


class Features(dict):
    def check_with_template(self, name, default=None):
        return self.get(name, default)


class Devices:
    def __init__(self, assignments):
        self.assignments = assignments

    def get_assigned_devices(self, required_only=False):
        return [a for a in self.assignments if not required_only or a.required]


@pytest.fixture
def setup_gpu(tmp_path):
    # Deliberately use a different PCI location from the hardware lab.
    pci = tmp_path / "pci"
    device = pci / "0000:09:03.0"
    device.mkdir(parents=True)
    for name, value in {
        "vendor": "0x1002",
        "device": "0x1681",
        "class": "0x030000",
    }.items():
        (device / name).write_text(value)
    (device / "driver").symlink_to(tmp_path / "drivers/pciback")
    gpu = SimpleNamespace(
        segment="0000",
        bus="09",
        device="03",
        function="0",
        port_id="arbitrary-topology",
    )
    assignment = SimpleNamespace(
        backend_domain=SimpleNamespace(qid=0), devices=[gpu], required=True
    )
    vm = SimpleNamespace(
        name="graphics",
        uuid=uuid.uuid4(),
        virt_mode="hvm",
        devices={"pci": Devices([assignment])},
        features=Features({"video-model": "none"}),
    )
    return vm, pci, device


def test_discovers_actual_assignment_instead_of_laptop_addresses(runtime, setup_gpu):
    vm, pci, _ = setup_gpu
    gpu = runtime.selected_gpu(vm, pci)
    assert gpu.host_bdf == "0000:09:03.0"
    assert gpu.port_id == "arbitrary-topology"


@pytest.mark.parametrize(
    "case",
    ["missing", "optional", "multiple", "backend", "unavailable", "segment", "pvh"],
)
def test_unsupported_assignment_is_rejected(runtime, setup_gpu, case):
    vm, pci, _ = setup_gpu
    assignments = vm.devices["pci"].assignments
    if case == "missing":
        assignments.clear()
    elif case == "optional":
        assignments[0].required = False
    elif case == "multiple":
        assignments.append(assignments[0])
    elif case == "backend":
        assignments[0].backend_domain.qid = 4
    elif case == "unavailable":
        assignments[0].devices = [SimpleNamespace()]
    elif case == "segment":
        assignments[0].devices[0].segment = "0001"
    else:
        vm.virt_mode = "pvh"
    with pytest.raises(ValueError):
        runtime.selected_gpu(vm, pci)


@pytest.mark.parametrize("field,value", [("vendor", "0x10de"), ("class", "0x040300")])
def test_other_hardware_is_not_selected(runtime, setup_gpu, field, value):
    vm, pci, device = setup_gpu
    (device / field).write_text(value)
    with pytest.raises(ValueError, match="AMD display"):
        runtime.selected_gpu(vm, pci)


def test_host_driver_is_not_unbound(runtime, setup_gpu):
    vm, pci, device = setup_gpu
    (device / "driver").unlink()
    (device / "driver").symlink_to(device.parent / "amdgpu")
    with pytest.raises(ValueError, match="pciback"):
        runtime.selected_gpu(vm, pci)
    assert (device / "driver").resolve().name == "amdgpu"


@pytest.mark.parametrize(
    "feature,value",
    [
        ("uefi", True),
        ("linux-stubdom", False),
        ("video-model", "vga"),
        ("qemu-extra-args", "-device something"),
    ],
)
def test_unsupported_layout_is_rejected(runtime, setup_gpu, feature, value):
    vm, _, _ = setup_gpu
    vm.features[feature] = value
    with pytest.raises(ValueError):
        runtime.validate_layout(vm, 16)


@pytest.mark.parametrize("slot", [0, 7, 32, True, "16"])
def test_invalid_guest_slot_rejected(runtime, setup_gpu, slot):
    with pytest.raises(ValueError):
        runtime.validate_layout(setup_gpu[0], slot)


@pytest.fixture
def prepared(runtime, setup_gpu, tmp_path, monkeypatch):
    fw = importlib.import_module("airlock_amd_vbios.firmware")
    vm, pci, _ = setup_gpu
    source, _ = firmware(fw)
    # Move the source entry to the synthetic host BDF and fix the ACPI checksum.
    source = bytearray(source)
    import struct

    struct.pack_into("<III", source, 76, 9, 3, 0)
    source[9] = 0
    source[9] = -sum(source) % 256
    table = tmp_path / "VFCT"
    table.write_bytes(source)
    # Tests are unprivileged; production enforces root ownership separately.
    monkeypatch.setattr(
        runtime, "secure_directory", lambda path: path.mkdir(exist_ok=True)
    )
    report = runtime.prepare_start(
        vm, 18, sysfs=pci, source=table, output=tmp_path / "output"
    )
    return report


def test_prepares_fresh_source_for_explicit_guest_slot(runtime, prepared):
    fw = importlib.import_module("airlock_amd_vbios.firmware")
    table = Path(prepared["table_path"]).read_bytes()
    assert fw.IMAGE.unpack_from(table, 76)[:3] == (0, 18, 0)
    assert prepared["host_bdf"] == "0000:09:03.0"
    assert prepared["guest_bdf"] == "0000:00:12.0"
    assert prepared["rom_modified"] is False
    assert Path(prepared["table_path"]).stat().st_mode & 0o777 == 0o400


def xml(report):
    return f"""<domain type="xen"><uuid>{report["domain_uuid"]}</uuid>
    <os><type>hvm</type><loader type="rom">hvmloader</loader></os>
    <devices><disk type="block"><readonly/></disk>
    <hostdev type="pci" managed="yes" writeFiltering="yes">
    <source><address domain="0x0000" bus="0x09" slot="0x03" function="0x0"/></source>
    </hostdev></devices></domain>"""


def test_xml_keeps_upstream_devices_and_security_flags(runtime, prepared):
    root = ET.fromstring(runtime.inject_xml(xml(prepared), prepared))
    hostdev = root.find("devices/hostdev")
    assert hostdev.get("managed") == "yes"
    assert hostdev.get("writeFiltering") == "yes"
    assert hostdev.find("source/address").get("bus") == "0x09"
    assert hostdev.find("address").get("slot") == "0x12"
    assert root.find("devices/disk/readonly") is not None
    assert root.findtext("os/acpi/table") == prepared["table_path"]


@pytest.mark.parametrize("case", ["source", "uuid", "acpi", "address", "collision"])
def test_xml_conflicts_fail_closed(runtime, prepared, case):
    root = ET.fromstring(xml(prepared))
    if case == "source":
        root.find("devices/hostdev/source/address").set("bus", "0x04")
    elif case == "uuid":
        root.find("uuid").text = str(uuid.uuid4())
    elif case == "acpi":
        ET.SubElement(root.find("os"), "acpi")
    elif case == "address":
        ET.SubElement(root.find("devices/hostdev"), "address", type="pci")
    else:
        controller = ET.SubElement(root.find("devices"), "controller")
        ET.SubElement(controller, "address", type="pci", slot="0x12")
    with pytest.raises(ValueError):
        runtime.inject_xml(ET.tostring(root, encoding="unicode"), prepared)


def test_unpatched_libvirt_cannot_silently_start_with_wrong_firmware(runtime, prepared):
    native = f'pci = [ "0000:09:03.0" ]\nacpi_firmware = "{prepared["table_path"]}"\n'
    with pytest.raises(ValueError, match="guest slot"):
        runtime.verify_native(native, prepared)
    runtime.verify_native(native.replace('03.0"', '03.0@12,permissive=0"'), prepared)
    with pytest.raises(ValueError, match="VFCT path"):
        runtime.verify_native(
            native.replace('03.0"', '03.0@12"').replace("vfct.bin", "old.bin"), prepared
        )
