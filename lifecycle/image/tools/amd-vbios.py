#!/usr/bin/env python3
"""Prepare one validated AMD VBIOS as guest VFCT data; never access PCI controls.

Layout: Linux drivers/gpu/drm/amd/include/atombios.h, UEFI_ACPI_VFCT and
VFCT_IMAGE_HEADER. Outputs are machine-local firmware, not redistributable assets.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re
import struct

ACPI = struct.Struct("<4sIBB6s8sI4sI")
IMAGE = struct.Struct("<IIIHHHHII")
VFCT_HEADER_SIZE = 76
MAX_TABLE_SIZE = 8 * 1024 * 1024


def bdf(value):
    match = re.fullmatch(r"0000:([0-9a-fA-F]{2}):([0-9a-fA-F]{2})\.([0-7])", value)
    if not match:
        raise ValueError("BDF must be a segment-zero address such as 0000:04:00.0")
    bus, device, function = (int(part, 16) for part in match.groups())
    if device > 31:
        raise ValueError("PCI device number exceeds 31")
    return bus, device, function


def digest(data):
    return hashlib.sha256(data).hexdigest()


def validate_rom(rom, vendor, device):
    if len(rom) < 0x4A or rom[:2] != b"\x55\xaa":
        raise ValueError("Missing VBIOS ROM signature")
    if rom[2] * 512 != len(rom) or sum(rom) % 256:
        raise ValueError("VBIOS length or checksum mismatch")
    atom = struct.unpack_from("<H", rom, 0x48)[0]
    if (
        atom < 0x4A
        or atom + 8 > len(rom)
        or rom[atom + 4 : atom + 8] not in (b"ATOM", b"MOTA")
    ):
        raise ValueError("Missing or out-of-bounds ATOM header")
    pcir = struct.unpack_from("<H", rom, 0x18)[0]
    if pcir < 0x1A or pcir + 0x18 > len(rom) or rom[pcir : pcir + 4] != b"PCIR":
        raise ValueError("Missing or out-of-bounds PCI ROM header")
    if struct.unpack_from("<HH", rom, pcir + 4) != (vendor, device):
        raise ValueError("PCI ROM vendor/device mismatch")
    if struct.unpack_from("<H", rom, pcir + 0x10)[0] * 512 != len(rom):
        raise ValueError("PCI ROM image length mismatch")


def prepare(data, host, guest, vendor, device, expected_sha256):
    if digest(data) != expected_sha256.lower():
        raise ValueError("Source VFCT SHA-256 mismatch")
    if not VFCT_HEADER_SIZE <= len(data) <= MAX_TABLE_SIZE:
        raise ValueError("VFCT size outside supported bounds")
    signature, length, revision, *_ = ACPI.unpack_from(data)
    if signature != b"VFCT" or length != len(data) or revision != 1 or sum(data) % 256:
        raise ValueError("Invalid VFCT header, revision, length or checksum")
    offset, lib_offset = struct.unpack_from("<II", data, 52)
    if offset < VFCT_HEADER_SIZE or offset > len(data) - IMAGE.size or lib_offset:
        raise ValueError("Unsupported VFCT image/library offsets")
    selected = []
    image_count = 0
    while offset < len(data):
        if offset + IMAGE.size > len(data):
            raise ValueError("Truncated VFCT image header")
        header = IMAGE.unpack_from(data, offset)
        offset += IMAGE.size
        image_end = offset + header[-1]
        if image_end > len(data):
            raise ValueError("Truncated VFCT image")
        if header[-1]:
            image_count += 1
            if header[:3] == host and header[3:5] == (vendor, device):
                selected.append((header, data[offset:image_end]))
        elif any(header):
            raise ValueError("Unsupported nonempty VFCT entry without image")
        offset = image_end
    if len(selected) != 1:
        raise ValueError("Expected exactly one matching host GPU VBIOS")
    header, rom = selected[0]
    validate_rom(rom, vendor, device)
    # Keep only the chosen image, without host UUID, library data or other ACPI
    # tables. ROM bytes remain unchanged; the surrounding VFCT location changes.
    body = bytes(16) + struct.pack("<II", VFCT_HEADER_SIZE, 0) + bytes(16)
    body += IMAGE.pack(*guest, *header[3:]) + rom
    table = bytearray(
        ACPI.pack(
            b"VFCT", ACPI.size + len(body), 1, 0, b"ALLOCK", b"GPUVBIOS", 1, b"ALCK", 1
        )
        + body
    )
    table[9] = -sum(table) % 256
    manifest = {
        "schema": 1,
        "source_sha256": digest(data),
        "source_image_count": image_count,
        "host_bdf": "0000:%02x:%02x.%x" % host,
        "guest_bdf": "0000:%02x:%02x.%x" % guest,
        "vendor": f"{vendor:04x}",
        "device": f"{device:04x}",
        "rom_bytes": len(rom),
        "rom_sha256": digest(rom),
        "guest_vfct_bytes": len(table),
        "guest_vfct_sha256": digest(table),
        "rom_modified": False,
        "guest_vfct_checksum_valid": sum(table) % 256 == 0,
    }
    return bytes(table), rom, manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument(
        "--sha256", required=True, help="Previously recorded source-table hash"
    )
    parser.add_argument("--host-bdf", required=True)
    parser.add_argument(
        "--guest-bdf", required=True, help="Observed guest GPU BDF; do not guess"
    )
    parser.add_argument("--vendor", type=lambda value: int(value, 16), default=0x1002)
    parser.add_argument("--device", type=lambda value: int(value, 16), required=True)
    parser.add_argument(
        "--out-dir", type=Path, help="New directory only; omit for read-only validation"
    )
    args = parser.parse_args()
    try:
        # Bound the read before parsing potentially malformed firmware.
        with args.input.open("rb") as stream:
            data = stream.read(MAX_TABLE_SIZE + 1)
        table, rom, manifest = prepare(
            data,
            bdf(args.host_bdf),
            bdf(args.guest_bdf),
            args.vendor,
            args.device,
            args.sha256,
        )
        if args.out_dir:
            args.out_dir.mkdir(mode=0o700, exist_ok=False)
            for name, contents in (
                ("guest-vfct.bin", table),
                ("gpu.rom", rom),
                ("manifest.json", (json.dumps(manifest, indent=2) + "\n").encode()),
            ):
                target = args.out_dir / name
                with target.open("xb") as stream:
                    stream.write(contents)
                target.chmod(0o400)
        print(json.dumps(manifest, indent=2))
    except (OSError, ValueError, struct.error) as error:
        parser.exit(1, f"airlock-amd-vbios: {error}\n")


if __name__ == "__main__":
    main()
