"""Qubes extension: prepare host firmware before normal libvirt VM creation."""

import json
from pathlib import Path

from markupsafe import Markup
import qubes.ext
import qubes.exc

from .firmware import digest
from .runtime import (
    inject_xml,
    prepare_start,
    read_vfct,
    selected_gpu,
    validate_layout,
    verify_native,
)

CONFIG = Path("/etc/airlock/amd-vbios.json")


class AMDVBIOS(qubes.ext.Extension):
    def settings(self, vm):
        if not CONFIG.exists():
            return None
        return json.loads(CONFIG.read_text()).get("domains", {}).get(vm.name)

    @qubes.ext.handler("domain-init", "domain-load")
    def on_load(self, vm, _event, **_kwargs):
        vm.app.env.filters["airlock_amd_vbios"] = self.render
        # Qubes runs instance handlers before class/extension handlers for
        # pre-events. The PCI extension itself acts in pre-events, so a
        # class-level guard would have undefined ordering relative to it.
        vm.add_handler("domain-pre-start", self.on_pre_start)
        vm.add_handler("device-pre-attach:pci", self.on_hotplug)
        vm.add_handler("device-pre-detach:pci", self.on_hotplug)

    def render(self, xml, vm):
        settings = self.settings(vm)
        if settings is None:
            return Markup(xml)
        report = getattr(vm, "_airlock_amd_vbios", None)
        if report is None:
            raise qubes.exc.QubesException(
                "AMD VFCT is not prepared; start the VM through Qubes"
            )
        try:
            validate_layout(vm, settings["guestSlot"])
            gpu = selected_gpu(vm)
            if (
                gpu.host_bdf != report["host_bdf"]
                or gpu.vendor != int(report["vendor"], 16)
                or gpu.device != int(report["device"], 16)
                or gpu.port_id != report["port_id"]
                or settings["guestSlot"] != report["guest_slot"]
            ):
                raise ValueError("GPU assignment changed after VFCT preparation")
            if (
                digest(read_vfct(Path(report["table_path"])))
                != report["guest_vfct_sha256"]
            ):
                raise ValueError("Prepared VFCT changed before domain creation")
            return Markup(inject_xml(xml, report))
        except (OSError, ValueError) as error:
            raise qubes.exc.QubesException(f"AMD VFCT: {error}") from error

    def on_pre_start(self, vm, _event, **_kwargs):
        vm._airlock_amd_vbios = None
        settings = self.settings(vm)
        if settings is not None:
            try:
                validate_layout(vm, settings["guestSlot"])
                selected_gpu(vm)
            except (OSError, ValueError) as error:
                raise qubes.exc.QubesException(f"AMD VFCT: {error}") from error

    @qubes.ext.handler("domain-pre-spawn")
    def on_pre_spawn(self, vm, _event, **_kwargs):
        settings = self.settings(vm)
        if settings is None:
            return
        try:
            report = prepare_start(vm, settings["guestSlot"])
            vm._airlock_amd_vbios = report
            xml = vm.create_config_file()
            native = vm.app.vmm.libvirt_conn.domainXMLToNative("xen-xl", xml, 0)
            verify_native(native, report)
            vm.log.info(
                "airlock-amd-vbios prepared %s", json.dumps(report, sort_keys=True)
            )
        except Exception as error:
            vm._airlock_amd_vbios = None
            raise qubes.exc.QubesException(
                f"AMD VFCT preparation failed: {error}"
            ) from error

    @qubes.ext.handler("domain-start-failed", "domain-stopped")
    def on_stop(self, vm, _event, **_kwargs):
        vm._airlock_amd_vbios = None

    def on_hotplug(self, vm, _event, **_kwargs):
        if self.settings(vm) is not None and vm.is_running():
            raise qubes.exc.QubesException(
                "AMD VFCT GPU hotplug is unsupported; firmware is prepared at VM startup"
            )
