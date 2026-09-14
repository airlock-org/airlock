"""Read-only preconditions for the existing Qubes topology assignment path."""

import argparse
import json
from pathlib import Path
import sys


def validate(
    selection, topology, app, *, phase="bootstrap", sysfs=Path("/sys/bus/pci/devices")
):
    if not selection.get("enable", True):
        if phase == "start":
            raise ValueError(
                "physical sys-gui GPU ownership is disabled in this profile"
            )
        if "sys-gui" in app.domains:
            pci = app.domains["sys-gui"].devices["pci"]
            if list(pci.get_assigned_devices()) or list(pci.get_attached_devices()):
                raise ValueError(
                    "physical sys-gui PCI state remains from another profile; boot the physical-GPU profile or remove the assignment while sys-gui is halted in text recovery"
                )
        return {"status": "passed", "enabled": False}
    bdf = selection["bdf"]
    port = selection["port"]
    path = sysfs / bdf
    if not path.is_dir():
        raise ValueError(f"selected GPU {bdf} is missing; review machine configuration")
    for field in ("vendor", "device"):
        actual = int((path / field).read_text(), 16)
        if actual != int(selection[field], 16):
            raise ValueError(
                f"GPU {bdf} {field} mismatch: expected {selection[field]}, got {actual:04x}"
            )
    if int((path / "class").read_text(), 16) >> 16 != 0x03:
        raise ValueError(f"selected PCI device {bdf} is not a display controller")
    # Xen, not Linux dom0, owns the IOMMU. Linux iommu_group links need not
    # exist here. The Nix profile requires iommu=required on Xen's command line;
    # leave assignment/reset/isolation enforcement to Xen and Qubes.
    if (path / "driver").resolve().name != "pciback":
        raise ValueError(
            f"GPU {bdf} is not reserved to pciback; boot the matching Xen profile"
        )
    device = app.domains["dom0"].devices["pci"][port]
    # Qubes supplies the resolved host address. Do not duplicate its PCI-path
    # algorithm or assume that a topology port is a bus/device/function name.
    if device.data.get("sbdf") != bdf:
        raise ValueError(
            f"Qubes port dom0:{port} does not resolve to selected GPU {bdf}"
        )
    expected = {
        "backend": "dom0",
        "id": port,
        "mode": "required",
        "match": "port",
        "options": {},
    }
    domain = topology.get("domains", {}).get("sys-gui", {})
    if domain.get("devices", {}).get("pci") != [expected]:
        raise ValueError(
            "sys-gui topology does not contain exactly the selected required GPU assignment"
        )
    prefs = domain.get("prefs", {})
    if (
        prefs.get("autostart", False)
        or prefs.get("virt_mode") != "hvm"
        or domain.get("features", {}).get("video-model") != "none"
    ):
        raise ValueError(
            "sys-gui topology must use HVM, video-model=none and autostart=false"
        )

    assigned = False
    running = False
    for vm in app.domains:
        assignments = list(vm.devices["pci"].get_assigned_devices())
        attached = list(vm.devices["pci"].get_attached_devices())
        for item in assignments + attached:
            if (
                vm.name != "sys-gui"
                and item.backend_domain.name == "dom0"
                and item.port_id in (port, "*")
            ):
                raise ValueError(
                    f"selected GPU already assigned/attached to {vm.name}; resolve ownership manually"
                )
        if vm.name != "sys-gui":
            continue
        running = vm.is_running()
        if any(
            item.backend_domain.name != "dom0" or item.port_id != port or item.options
            for item in attached
        ):
            raise ValueError("sys-gui has conflicting attached PCI devices/options")
        for item in assignments:
            if (
                item.backend_domain.name != "dom0"
                or item.port_id != port
                or item.mode.value != "required"
                or item.device_id != "*"
                or item.options
            ):
                raise ValueError(
                    "sys-gui has a conflicting PCI assignment; refusing to replace it automatically"
                )
        assigned = len(assignments) == 1
        if len(assignments) > 1:
            raise ValueError("sys-gui has multiple PCI assignments")
        if running and not assigned:
            raise ValueError(
                "sys-gui is running without the declared persistent assignment; leave it unchanged and test on a fresh boot"
            )
    if phase == "start" and not assigned:
        raise ValueError(
            "bootstrap has not persisted the required sys-gui GPU assignment"
        )
    return {
        "status": "passed",
        "bdf": bdf,
        "port": port,
        "persistent_assignment": assigned,
        "running": running,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--config", type=Path, default=Path("/etc/airlock/gui-gpu.json")
    )
    parser.add_argument(
        "--topology", type=Path, default=Path("/etc/airlock/qubes-topology.json")
    )
    parser.add_argument("--phase", choices=("bootstrap", "start"), default="bootstrap")
    args = parser.parse_args()
    try:
        import qubesadmin

        result = validate(
            json.loads(args.config.read_text()),
            json.loads(args.topology.read_text()),
            qubesadmin.Qubes(),
            phase=args.phase,
        )
    except Exception as error:
        result = {"status": "failed", "error": str(error)}
    print(
        json.dumps(
            {"schema": "airlock.gui-gpu-check.v1", "phase": args.phase, **result},
            sort_keys=True,
        ),
        flush=True,
    )
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
