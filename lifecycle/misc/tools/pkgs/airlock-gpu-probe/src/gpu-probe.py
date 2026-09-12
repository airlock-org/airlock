"""Observe guest GPU inventory and API initialization; never attach devices."""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import subprocess
from pathlib import Path


def read_text(path: Path) -> str | None:
    try:
        return path.read_text().strip()
    except OSError:
        return None


def pci_inventory(root: Path = Path("/sys/bus/pci/devices")) -> list[dict]:
    devices = []
    for device in sorted(root.glob("*")):
        pci_class = read_text(device / "class") or ""
        if not pci_class.startswith("0x03"):
            continue
        driver = device / "driver"
        devices.append(
            {
                "address": device.name,
                "vendor": read_text(device / "vendor"),
                "device": read_text(device / "device"),
                "class": pci_class,
                "driver": driver.resolve().name if driver.is_symlink() else None,
            }
        )
    return devices


def run_probe(command: list[str]) -> dict:
    env = os.environ.copy()
    env["LC_ALL"] = "C"
    try:
        result = subprocess.run(
            command,
            env=env,
            capture_output=True,
            text=True,
            errors="replace",
            timeout=30,
            check=False,
        )
        return {
            "command": command,
            "returncode": result.returncode,
            "stdout": result.stdout,
            "stderr": result.stderr,
        }
    except (OSError, subprocess.TimeoutExpired) as exc:
        return {"command": command, "returncode": None, "error": str(exc)}


def vulkan_device_types(probe: dict) -> list[str]:
    if probe.get("returncode") != 0:
        return []
    return sorted(
        set(
            re.findall(
                r"deviceType\s*=\s*PHYSICAL_DEVICE_TYPE_([A-Z_]+)",
                probe.get("stdout", ""),
            )
        )
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--require-hardware",
        action="store_true",
        help="fail unless Vulkan initializes an integrated or discrete GPU",
    )
    args = parser.parse_args()
    vulkan = run_probe(["vulkaninfo", "--summary"])
    device_types = vulkan_device_types(vulkan)
    hardware_initialized = bool(
        {"INTEGRATED_GPU", "DISCRETE_GPU"}.intersection(device_types)
    )
    report = {
        "schema": "airlock.gpu-probe.v1",
        "hostname": platform.node(),
        "kernel": platform.release(),
        "pci_display_devices": pci_inventory(),
        "drm_nodes": sorted(str(path) for path in Path("/dev/dri").glob("*")),
        "vulkan_device_types": device_types,
        "hardware_api_initialized": hardware_initialized,
        # API enumeration is not a rendering, isolation, or presentation test.
        "rendering_verified": False,
        "exclusive_ownership_verified": False,
        "cross_vm_graphics_verified": False,
        "probes": {
            "vulkan": vulkan,
            "egl": run_probe(["eglinfo", "-B", "-p", "surfaceless"]),
        },
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if args.require_hardware and not hardware_initialized else 0


if __name__ == "__main__":
    raise SystemExit(main())
