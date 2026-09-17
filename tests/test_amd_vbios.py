"""Synthetic firmware only; never touch a PCI device or a real machine's ROM."""

import importlib.util
from pathlib import Path
import struct

import pytest


@pytest.fixture
def vbios():
    path = Path(__file__).resolve().parents[1] / (
        "airlock-machine/lifecycle/runtime/tools/pkgs/airlock-amd-vbios/src/airlock_amd_vbios/firmware.py"
    )
    spec = importlib.util.spec_from_file_location("amd_vbios", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def checksum(data, position):
    data[position] = 0
    data[position] = -sum(data) % 256
    return bytes(data)


def firmware(vbios, duplicate=False):
    rom = bytearray(512)
    rom[:3] = b"\x55\xaa\x01"
    struct.pack_into("<H", rom, 0x48, 0x80)
    rom[0x84:0x88] = b"ATOM"
    struct.pack_into("<H", rom, 0x18, 0xA0)
    rom[0xA0:0xA4] = b"PCIR"
    struct.pack_into("<HH", rom, 0xA4, 0x1002, 0x1681)
    struct.pack_into("<H", rom, 0xB0, 1)
    rom = checksum(rom, 511)
    entry = vbios.IMAGE.pack(4, 0, 0, 0x1002, 0x1681, 0, 0, 1, len(rom)) + rom
    body = bytes(16) + struct.pack("<II", 76, 0) + bytes(16) + entry
    body += entry if duplicate else bytes(28)
    table = bytearray(
        vbios.ACPI.pack(
            b"VFCT", 36 + len(body), 1, 0, b"LENOVO", b"TEST    ", 1, b"TEST", 1
        )
        + body
    )
    return checksum(table, 9), rom


def prepare(vbios, source):
    return vbios.prepare(
        source, (4, 0, 0), (0, 5, 0), 0x1002, 0x1681, vbios.digest(source)
    )


def test_filtered_table_preserves_rom_and_remaps_only_wrapper(vbios):
    source, expected_rom = firmware(vbios)
    table, rom, report = prepare(vbios, source)
    assert rom == expected_rom
    assert table[104:] == rom
    assert vbios.IMAGE.unpack_from(table, 76)[:3] == (0, 5, 0)
    assert len(table) == 104 + len(rom)
    assert sum(table) % 256 == 0
    assert report["rom_modified"] is False
    assert report["source_image_count"] == 1


def test_wrong_source_hash_rejected(vbios):
    source, _ = firmware(vbios)
    with pytest.raises(ValueError, match="SHA-256"):
        vbios.prepare(source, (4, 0, 0), (0, 5, 0), 0x1002, 0x1681, "0" * 64)


@pytest.mark.parametrize(
    "field,value",
    [
        (0, b"SSDT"),
        (4, struct.pack("<I", 12)),
        (52, struct.pack("<I", 0)),
        (56, struct.pack("<I", 76)),
        (100, struct.pack("<I", 999999)),
    ],
)
def test_malformed_or_unrelated_tables_rejected(vbios, field, value):
    source, _ = firmware(vbios)
    broken = bytearray(source)
    broken[field : field + len(value)] = value
    with pytest.raises(ValueError):
        prepare(vbios, checksum(broken, 9))


def test_invalid_table_checksum_rejected(vbios):
    source, _ = firmware(vbios)
    broken = bytearray(source)
    broken[9] ^= 1
    with pytest.raises(ValueError, match="checksum"):
        prepare(vbios, bytes(broken))


def test_invalid_rom_checksum_rejected(vbios):
    source, _ = firmware(vbios)
    broken = bytearray(source)
    broken[104 + 300] ^= 1
    with pytest.raises(ValueError, match="checksum"):
        prepare(vbios, checksum(broken, 9))


def test_multiple_matching_images_rejected(vbios):
    source, _ = firmware(vbios, duplicate=True)
    with pytest.raises(ValueError, match="exactly one"):
        prepare(vbios, source)


def test_wrong_gpu_rejected(vbios):
    source, _ = firmware(vbios)
    with pytest.raises(ValueError, match="exactly one"):
        vbios.prepare(
            source, (3, 0, 0), (0, 5, 0), 0x1002, 0x1681, vbios.digest(source)
        )


@pytest.mark.parametrize(
    "value", ["0001:04:00.0", "0000:04:20.0", "04:00.0", "0000:04:00.8"]
)
def test_invalid_bdf_rejected(vbios, value):
    with pytest.raises(ValueError):
        vbios.bdf(value)
