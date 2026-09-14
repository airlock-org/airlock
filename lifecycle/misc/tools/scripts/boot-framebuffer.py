#!/usr/bin/env python3
"""Release only a sole-GPU firmware framebuffer before the reviewed guest start."""

import argparse
import json
from pathlib import Path
import re
import sys

DRIVERS = ("efi-framebuffer", "simple-framebuffer")


def inventory(root, bdf):
    displays = sorted(
        p.name
        for p in (root / "bus/pci/devices").iterdir()
        if (int((p / "class").read_text().strip(), 16) >> 16) == 3
    )
    if displays != [bdf]:
        raise ValueError(
            f"boot framebuffer requires the selected sole display GPU; found {displays}"
        )
    devices = []
    for entry in sorted((root / "bus/platform/devices").glob("*")):
        if not re.fullmatch(r"(?:efi|simple)-framebuffer\.\d+", entry.name):
            continue
        device = entry.resolve(strict=True)
        # Platform bus membership does not imply a /devices/platform parent:
        # sysfb may parent the firmware device beneath its PCI display device.
        # Validate the bus backlink, not a particular hardware hierarchy.
        if (
            not device.is_relative_to((root / "devices").resolve(strict=True))
            or device.name != entry.name
            or (device / "subsystem").resolve(strict=True)
            != (root / "bus/platform").resolve(strict=True)
        ):
            raise ValueError(f"not a firmware platform device: {entry} -> {device}")
        driver = None
        if (device / "driver").exists():
            driver = (device / "driver").resolve(strict=True)
            allowed = [
                (root / "bus/platform/drivers" / name).resolve() for name in DRIVERS
            ]
            if driver not in allowed:
                raise ValueError(f"firmware driver mismatch: {entry}")
            if (driver / entry.name).resolve(strict=True) != device:
                raise ValueError(f"firmware driver registration mismatch: {entry}")
        if not (device / "driver_override").exists() or (
            driver and not (driver / "unbind").exists()
        ):
            raise ValueError(f"firmware device cannot be detached safely: {entry}")
        devices.append((device, driver))
    for fb in (root / "class/graphics").glob("fb[0-9]*"):
        owner = (fb / "device").resolve(strict=True)
        if not any(
            driver is not None and (owner == device or owner.is_relative_to(device))
            for device, driver in devices
        ):
            raise ValueError(f"unexpected framebuffer owner: {fb.name}: {owner}")
    if len(devices) > 1:
        raise ValueError(
            "multiple firmware framebuffers: refusing an ambiguous handoff"
        )
    return devices


def release(root, bdf, write=None):
    if write is None:
        write = Path.write_text
    # Complete validation before touching any binding. No PCI driver or reset
    # interface is written, and no attempt is made to restore a legacy VGA mode.
    devices = inventory(root, bdf)
    consoles = [
        p
        for p in sorted((root / "class/vtconsole").glob("vtcon*"))
        if "frame buffer" in (p / "name").read_text()
        and (p / "bind").read_text().strip() == "1"
    ]
    if not devices and consoles:
        raise ValueError(
            "fbcon remains bound without a recognized firmware framebuffer"
        )
    # Also inhibit late probing of firmware devices that have no driver yet.
    for device, _ in devices:
        if (device / "driver_override").read_text().strip() != "airlock-released":
            write(device / "driver_override", "airlock-released\n")
    for console in consoles:
        write(console / "bind", "0\n")
        if (console / "bind").read_text().strip() != "0":
            raise ValueError(f"framebuffer console did not detach: {console.name}")
    for device, driver in devices:
        if driver:
            write(driver / "unbind", device.name + "\n")
        if (device / "driver").exists():
            raise ValueError(f"firmware driver still bound: {device.name}")
    if any(driver is not None for _, driver in inventory(root, bdf)):
        raise ValueError("firmware framebuffer remained bound after handoff")
    return {
        "schema": "airlock.boot-framebuffer.v1",
        "status": (
            "released"
            if any(driver is not None for _, driver in devices)
            else "no-framebuffer"
        ),
        "gpu": bdf,
        "devices": [device.name for device, _ in devices],
        "consoles": [p.name for p in consoles],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gpu", required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    try:
        if not re.fullmatch(r"0000:[0-9a-f]{2}:[0-1][0-9a-f]\.[0-7]", args.gpu):
            raise ValueError("invalid selected GPU address")
        result = release(Path("/sys"), args.gpu)
        code = 0
    except (ValueError, OSError) as error:
        result = {
            "schema": "airlock.boot-framebuffer.v1",
            "status": "failed",
            "error": str(error),
        }
        code = 1
    output = json.dumps(result, sort_keys=True) + "\n"
    print(output, end="", flush=True)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(output)
    return code


if __name__ == "__main__":
    sys.exit(main())
