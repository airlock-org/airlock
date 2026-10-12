# sys-gpu experiments and hardware direction

## Agreed hardware split

`sys-gui` is the standard desktop domain and gets the only GPU on a single-GPU
machine. `sys-gpu` is an optional, customized service for additional internal
or external GPUs; it must not borrow the desktop GPU when its own device is
absent. The nested baseline is described in the [minimal sys-gui image](../sys-gui.md).
The GPU lab below is retained as earlier opt-in groundwork, not the default
profile or a verified physical GPU setup.

The intended deployment assigns **AMD integrated graphics to `sys-gui`** and
the **NVIDIA RTX 3090 to `sys-gpu`**. The current physical USB desktop uses GNOME;
AI inference remains a deferred experiment. When added, the
inference engine and model storage may live inside `sys-gpu`.

This supersedes the earlier proposal to make the desktop depend on a virtual GPU
provided by `sys-gpu`. With separate physical GPUs, the desktop need not share the
3090 with inference. Keep Xen, qrexec, QubesDB, qubesd, libvirt, and Qubes policy.

A read-only inventory of the development host found:

| Device | PCI address | Current host driver |
| --- | --- | --- |
| RTX 3090 | `0000:01:00.0` | `nvidia` |
| RTX 3090 audio function | `0000:01:00.1` | `snd_hda_intel` |
| AMD integrated graphics | `0000:7a:00.0` | `amdgpu` |

These are observations, not assignments. The lab does not unbind host drivers,
modify the host boot configuration, or change physical display ownership.
The target deployment still needs IOMMU/reset checks, companion-function
selection, and matching guest kernel modules and userspace drivers. The
current Nix-native builder boots a shared Qubes kernel; installing an NVIDIA
NixOS package alone would not establish kernel/module compatibility.

## Current experiment

The separate `vm-gpu-lab` image adds an on-demand NixOS StandaloneVM named
`sys-gpu`. Its configuration is in
[`sys-gpu.nix`](../../airlock-machine/lifecycle/misc/experiments/sys-gpu.nix), and the guest module
is [`sys-gpu-guest.nix`](../../airlock-machine/lifecycle/misc/modules/sys-gpu-guest.nix).

- No physical PCI devices are assigned yet.
- HVM mode with fixed 2048 MiB RAM prepares the domain for PCI experiments.
- No NetVM, GUI VM, autostart, custom administrative RPC, or inference service.
- Mesa/EGL/Vulkan diagnostic tools and `airlock-gpu-probe` are installed.
- The normal image's domains, desktop session, and default GUI VM are unchanged.
- This is a bootstrap lab image. It does not add `sys-gpu` to the normal
  initial runtime configuration; a later normal configuration deployment must
  explicitly declare the domain before managing it through that workflow.

Build and test from the GPU worktree:

```sh
make vm-gpu-lab-image
AIRLOCK_LOG_DIR="$PWD/.airlock-logs" make vm-gpu-lab-test
```

The pytest experiment creates a fresh, isolated nested-Xen VM directory and
SSH port. It verifies that `sys-gpu` starts on demand, answers through qrexec,
emits JSON diagnostics, rejects a hardware-required check without a physical
GPU, and shuts down/restarts. The test stops only the lab QEMU process it
created. It retains its disks and logs for inspection. Reports are written
under `$AIRLOCK_LOG_DIR/gpu-lab-vm/`, with the retained VM path in
`environment.json`. The normal running development VM is not reused.

For interactive inspection, run `make vm-gpu-lab-run` in one terminal. It uses
the dedicated `vm-gpu-lab` directory and SSH port 2322. Wait for
`airlock-setup-config-deferred.service` to finish provisioning before starting
`sys-gpu`; the domain can exist before its kernel and storage are ready.
In another terminal:

```sh
AIRLOCK_VM_SSH_PORT=2322 ./lifecycle/misc/tools/scripts/ssh-nested-vm.sh
sudo qvm-start sys-gpu
sudo qvm-run --pass-io sys-gpu airlock-gpu-probe
sudo qvm-run --pass-io sys-gpu 'airlock-gpu-probe --require-hardware'
sudo qvm-shutdown --wait sys-gpu
```

The probe reports PCI display functions, bound drivers, DRM nodes, kernel
version, and raw EGL/Vulkan output. A successful default invocation means the
diagnostic report was collected. `--require-hardware` additionally requires
successful Vulkan initialization of an integrated or discrete GPU. CPU
renderers such as llvmpipe and virtual GPUs do not meet that requirement.
Neither mode proves actual rendering, exclusive ownership, or cross-VM
graphics. Those unverified properties are explicit in the JSON report.

<a id="later-experiments"></a>

## Outstanding hardware work

The [sys-gpu hardware plan](../todo/sys-gpu-hardware.md) records the deferred
physical assignment, rendering, lifecycle and inference work.
