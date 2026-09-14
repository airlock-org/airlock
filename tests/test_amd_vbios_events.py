"""Exercise guard ordering with Qubes' real dispatcher and inert fake VMs."""

import os
import shlex
import subprocess

import pytest

pytestmark = pytest.mark.qubes

SCRIPT = r"""
import asyncio
from pathlib import Path
import runpy
import sys
from types import SimpleNamespace

asyncio.set_event_loop(asyncio.new_event_loop())
wrapper = Path('/run/current-system/sw/bin/qubesd').resolve().with_name('.qubesd-wrapped')
runpy.run_path(str(wrapper), run_name='airlock_event_test')
sys.path.insert(0, str(Path(sys.argv[1]) / f'lib/python{sys.version_info.major}.{sys.version_info.minor}/site-packages'))
import qubes.events
import qubes.exc
from airlock_amd_vbios.extension import AMDVBIOS

class VM(qubes.events.Emitter):
    def __init__(self, name, running):
        super().__init__()
        self.events_enabled = True
        self.name = name
        self.running = running
        self.app = SimpleNamespace(env=SimpleNamespace(filters={}))
        self.virt_mode = 'hvm'
        self.features = SimpleNamespace(check_with_template=lambda key, default=None: 'none' if key == 'video-model' else default)
        self.devices = {'pci': SimpleNamespace(get_assigned_devices=lambda **kwargs: [])}
        self.reached_device_handler = False

    def is_running(self):
        return self.running

    @qubes.events.handler('domain-pre-start', 'device-pre-attach:pci', 'device-pre-detach:pci')
    def device_handler(self, event, **kwargs):
        self.reached_device_handler = True

extension = AMDVBIOS()
extension.settings = lambda vm: {'guestSlot': 16} if vm.name == 'graphics' else None
for event in ('domain-pre-start', 'device-pre-attach:pci', 'device-pre-detach:pci'):
    vm = VM('graphics', event != 'domain-pre-start')
    vm._airlock_amd_vbios = {'stale': True}
    extension.on_load(vm, 'domain-load')
    try:
        asyncio.get_event_loop().run_until_complete(vm.fire_event_async(event, pre_event=True))
    except qubes.exc.QubesException as error:
        print(event + ': ' + str(error))
    else:
        raise AssertionError('guard did not reject ' + event)
    assert not vm.reached_device_handler, 'guard ran after device side effect'
    if event == 'domain-pre-start':
        assert vm._airlock_amd_vbios is None
    ordinary = VM('ordinary', True)
    extension.on_load(ordinary, 'domain-load')
    asyncio.get_event_loop().run_until_complete(ordinary.fire_event_async(event, pre_event=True))
    assert ordinary.reached_device_handler, 'non-opted-in VM was affected'
print('ordering and stale-state checks passed')
"""


def test_guards_run_before_device_side_effects():
    host = os.environ.get("AIRLOCK_VBIOS_LIBVIRT_SSH")
    package = os.environ.get("AIRLOCK_VBIOS_PACKAGE")
    if not host or not package:
        pytest.skip("set AIRLOCK_VBIOS_LIBVIRT_SSH and AIRLOCK_VBIOS_PACKAGE")
    result = subprocess.run(
        [
            "ssh",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            host,
            "python3 - " + shlex.quote(package),
        ],
        input=SCRIPT,
        text=True,
        capture_output=True,
        timeout=30,
        check=False,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    assert "ordering and stale-state checks passed" in result.stdout
