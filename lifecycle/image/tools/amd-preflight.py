#!/usr/bin/env python3
"""Read-only GPU assignment gate. Bus-only reset is a stop, not an override."""

import json
from pathlib import Path
import sys


def read(path):
    try:
        return path.read_text().strip()
    except OSError:
        return None


def inspect(sysfs=Path("/sys"), proc=Path("/proc")):
    bdf = "0000:04:00.0"
    device = sysfs / "bus/pci/devices" / bdf
    group = device / "iommu_group/devices"
    members = sorted(p.name for p in group.iterdir()) if group.is_dir() else []
    methods = (read(device / "reset_method") or "").replace("[", "").replace("]", "").split()
    driver = (device / "driver").resolve().name if (device / "driver").is_symlink() else None
    capabilities = read(proc / "xen/capabilities") or ""
    failures = []
    if "control_d" not in capabilities:
        failures.append("not-xen-dom0")
    if read(device / "vendor") != "0x1002" or read(device / "device") != "0x1681":
        failures.append("unexpected-gpu")
    if members != [bdf]:
        failures.append("missing-or-shared-iommu-group")
    if driver != "pciback":
        failures.append("gpu-not-reserved-by-xen-pciback")
    # A separate group is not proof that a bus reset leaves siblings alone.
    # Deliberately do not provide a bypass; this laptop was bus-only in Ubuntu.
    if not methods or methods[0] != "flr":
        failures.append("function-level-reset-not-selected-review-required")
    dom0_drm = sorted(str(p) for p in (sysfs / "class/drm").glob("card*/device")
                      if p.resolve() == device.resolve())
    if dom0_drm:
        failures.append("dom0-still-has-gpu-drm-device")
    return {"schema": 1, "bdf": bdf, "vendor": read(device / "vendor"),
            "device": read(device / "device"), "driver": driver,
            "iommu_members": members, "reset_methods": methods,
            "dom0_drm_devices": dom0_drm, "failures": failures,
            "assignment_gate_passed": not failures,
            "graphics_verified": False, "input_verified": False,
            "note": "Read-only preflight, not proof of passthrough or reset safety."}


if __name__ == "__main__":
    report = inspect()
    print(json.dumps(report, indent=2))
    sys.exit(0 if report["assignment_gate_passed"] else 2)
