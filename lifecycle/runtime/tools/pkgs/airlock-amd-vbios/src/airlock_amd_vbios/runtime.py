"""Bounded firmware preparation and XML transformation, without PCI writes."""

from dataclasses import dataclass
import json
import os
from pathlib import Path
import re
import stat
import tempfile
import xml.etree.ElementTree as ET

from .firmware import MAX_TABLE_SIZE, bdf, digest, prepare


@dataclass(frozen=True)
class GPU:
    host_bdf: str
    vendor: int
    device: int
    port_id: str


def selected_gpu(vm, sysfs=Path("/sys/bus/pci/devices")):
    if vm.virt_mode != "hvm":
        raise ValueError("AMD VFCT requires an HVM guest")
    assignments = list(vm.devices["pci"].get_assigned_devices())
    required = list(vm.devices["pci"].get_assigned_devices(required_only=True))
    if len(assignments) != 1 or len(required) != 1:
        raise ValueError("AMD VFCT requires exactly one required PCI assignment")
    assignment = required[0]
    if assignment.backend_domain.qid != 0:
        raise ValueError("AMD VFCT requires a GPU supplied by dom0")
    devices = list(assignment.devices)
    if len(devices) != 1:
        raise ValueError("AMD VFCT assignment must resolve to exactly one GPU")
    device = devices[0]
    try:
        host = f"{device.segment}:{device.bus}:{device.device}.{device.function}"
        bdf(host)
    except (AttributeError, ValueError) as error:
        raise ValueError(
            "AMD VFCT requires an available segment-zero PCI device"
        ) from error
    path = sysfs / host
    vendor = int((path / "vendor").read_text(), 16)
    device_id = int((path / "device").read_text(), 16)
    pci_class = int((path / "class").read_text(), 16)
    if vendor != 0x1002 or pci_class >> 16 != 0x03:
        raise ValueError("Assigned device is not an AMD display controller")
    if (path / "driver").resolve().name != "pciback":
        raise ValueError("AMD GPU must already be reserved to pciback at host boot")
    return GPU(host.lower(), vendor, device_id, device.port_id)


def validate_layout(vm, slot):
    if type(slot) is not int or not 8 <= slot <= 31:
        raise ValueError("AMD VFCT guestSlot must be an integer from 8 to 31")
    feature = vm.features.check_with_template
    if feature("video-model", "") != "none":
        raise ValueError("AMD VFCT requires video-model=none")
    if feature("uefi", False) or not feature("linux-stubdom", True):
        raise ValueError("AMD VFCT currently supports SeaBIOS with Linux stubdom only")
    if feature("qemu-extra-args", ""):
        raise ValueError("AMD VFCT does not support custom QEMU device arguments")


def read_vfct(path):
    with path.open("rb") as stream:
        return stream.read(MAX_TABLE_SIZE + 1)


def secure_directory(path):
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o077:
        raise ValueError(
            f"AMD VFCT runtime directory must be root-owned mode 0700: {path}"
        )


def prepare_start(
    vm,
    slot,
    *,
    sysfs=Path("/sys/bus/pci/devices"),
    source=Path("/sys/firmware/acpi/tables/VFCT"),
    output=Path("/run/airlock-amd-vbios"),
):
    validate_layout(vm, slot)
    gpu = selected_gpu(vm, sysfs)
    data = read_vfct(source)
    # The source is this host's sysfs table. Its hash records provenance; it
    # is not a signature or a claim that platform firmware is trustworthy.
    table, _, report = prepare(
        data, bdf(gpu.host_bdf), (0, slot, 0), gpu.vendor, gpu.device, digest(data)
    )
    secure_directory(output)
    # A fresh path per attempt prevents a previous host/assignment's artifact
    # from being reused, and never overwrites a table a running VM may need.
    directory = Path(tempfile.mkdtemp(prefix=f"{vm.uuid}-", dir=output))
    report.update(domain=vm.name, domain_uuid=str(vm.uuid), port_id=gpu.port_id)
    report["guest_slot"] = slot
    report["source"] = str(source)
    report["table_path"] = str(directory / "vfct.bin")
    for name, payload in (
        ("vfct.bin", table),
        (
            "manifest.json",
            (json.dumps(report, sort_keys=True, indent=2) + "\n").encode(),
        ),
    ):
        with (directory / name).open("xb") as stream:
            stream.write(payload)
            stream.flush()
            os.fchmod(stream.fileno(), 0o400)
    return report


def address_tuple(element):
    return tuple(
        int(element.get(key, "0"), 0) for key in ("domain", "bus", "slot", "function")
    )


def inject_xml(xml, report):
    """Transform upstream XML while preserving its devices and security flags."""
    root = ET.fromstring(xml)
    if root.tag != "domain" or root.get("type") != "xen":
        raise ValueError("AMD VFCT requires a Xen domain definition")
    if root.findtext("uuid") != report["domain_uuid"]:
        raise ValueError("AMD VFCT domain identity changed during preparation")
    os_element = root.find("os")
    if os_element is None or os_element.findtext("type") != "hvm":
        raise ValueError("AMD VFCT requires HVM XML")
    if os_element.find("acpi") is not None:
        raise ValueError("AMD VFCT refuses to replace existing custom ACPI tables")
    hostdevs = root.findall("devices/hostdev")
    if len(hostdevs) != 1 or hostdevs[0].get("type") != "pci":
        raise ValueError("AMD VFCT XML must contain exactly one PCI host device")
    hostdev = hostdevs[0]
    source_address = hostdev.find("source/address")
    host = (0, *bdf(report["host_bdf"]))
    if source_address is None or address_tuple(source_address) != host:
        raise ValueError("AMD VFCT XML does not match the prepared GPU")
    guest = (0, 0, report["guest_slot"], 0)
    if hostdev.find("address") is not None:
        raise ValueError("AMD VFCT refuses to overwrite an existing guest PCI address")
    for address in root.findall("devices/*/address[@type='pci']"):
        if address_tuple(address)[:3] == guest[:3]:
            raise ValueError("AMD VFCT guest PCI slot is already occupied")
    ET.SubElement(
        hostdev,
        "address",
        dict(
            type="pci",
            domain="0x0000",
            bus="0x00",
            slot=f"0x{guest[2]:02x}",
            function="0x0",
        ),
    )
    acpi = ET.SubElement(os_element, "acpi")
    # Pinned libvirt 10.5 Xen's name for the raw ACPI-table transport.
    ET.SubElement(acpi, "table", type="slic").text = report["table_path"]
    return ET.tostring(root, encoding="unicode")


def verify_native(native, report):
    """Refuse startup with libvirt versions that silently discard guest slots."""
    pci_line = re.search(r"^pci\s*=\s*\[(.*)\]\s*$", native, re.MULTILINE)
    expected = re.escape(f'{report["host_bdf"]}@{report["guest_slot"]:02x}')
    if not pci_line or not re.search(r'"' + expected + r"(?:,|\")", pci_line[1]):
        raise ValueError(
            "libvirt did not preserve the explicit GPU guest slot; install Airlock's libxl slot patch"
        )
    acpi_line = re.search(r'^acpi_firmware\s*=\s*"([^"]+)"\s*$', native, re.MULTILINE)
    if not acpi_line or acpi_line[1] != report["table_path"]:
        raise ValueError("libvirt did not preserve the prepared VFCT path")
