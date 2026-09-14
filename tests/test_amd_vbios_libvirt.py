"""Read-only conversion checks against an explicitly selected Xen/libvirt host."""

import os
import subprocess
import xml.etree.ElementTree as ET

import pytest

pytestmark = pytest.mark.qubes


@pytest.fixture
def convert():
    host = os.environ.get("AIRLOCK_VBIOS_LIBVIRT_SSH")
    if not host:
        pytest.skip(
            "set AIRLOCK_VBIOS_LIBVIRT_SSH for read-only libvirt conversion checks"
        )

    def run(direction, text):
        return subprocess.run(
            [
                "ssh",
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=10",
                host,
                "virsh",
                "-c",
                "xen:///",
                f"domxml-{direction}-native",
                "xen-xl",
                "/dev/stdin",
            ],
            input=text,
            text=True,
            capture_output=True,
            timeout=30,
            check=False,
        )

    return run


def domain(address=""):
    return f"""<domain type="xen"><name>airlock-vbios-conversion-only</name>
    <uuid>a7e27839-b68f-4c21-8b91-6861db3ec112</uuid>
    <memory unit="MiB">1024</memory><vcpu>1</vcpu>
    <os><type arch="x86_64" machine="xenfv">hvm</type><loader type="rom">hvmloader</loader></os>
    <devices><hostdev mode="subsystem" type="pci" managed="yes">
    <source><address domain="0x0000" bus="0x09" slot="0x03" function="0x0"/></source>
    {address}</hostdev></devices></domain>"""


@pytest.mark.parametrize("slot", [8, 16, 18, 31])
def test_explicit_guest_slot_survives_native_roundtrip(convert, slot):
    address = f'<address type="pci" domain="0x0000" bus="0x00" slot="0x{slot:02x}" function="0x0"/>'
    native = convert("to", domain(address))
    assert native.returncode == 0, native.stderr
    assert f"0000:09:03.0@{slot:02x}" in native.stdout
    xml = convert("from", native.stdout)
    assert xml.returncode == 0, xml.stderr
    actual = ET.fromstring(xml.stdout).find("devices/hostdev/address")
    assert int(actual.get("slot"), 0) == slot


def test_unaddressed_device_keeps_automatic_placement(convert):
    native = convert("to", domain())
    assert native.returncode == 0, native.stderr
    assert '"0000:09:03.0"' in native.stdout
    assert "@" not in native.stdout


@pytest.mark.parametrize(
    "attribute,value",
    [("bus", "0x01"), ("function", "0x1"), ("domain", "0x0001"), ("slot", "0x00")],
)
def test_unrepresentable_guest_address_rejected(convert, attribute, value):
    fields = {
        "type": "pci",
        "domain": "0x0000",
        "bus": "0x00",
        "slot": "0x10",
        "function": "0x0",
    }
    fields[attribute] = value
    address = ET.tostring(ET.Element("address", fields), encoding="unicode")
    native = convert("to", domain(address))
    assert native.returncode != 0


@pytest.mark.parametrize("slot", ["00", "20", "-1", "10.1", "10junk"])
def test_native_invalid_slot_rejected(convert, slot):
    native = convert("to", domain())
    assert native.returncode == 0, native.stderr
    broken = native.stdout.replace("0000:09:03.0", f"0000:09:03.0@{slot}")
    result = convert("from", broken)
    assert result.returncode != 0
