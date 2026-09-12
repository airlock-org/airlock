# sys-gpu direction

Explore a standalone NixOS `sys-gpu` that owns the physical GPU and drivers,
similar to how `sys-net` owns network hardware. Its primary purpose is AI
compute, with inference applications running in separate NixOS qubes.

- AI qubes own model files, inference logic, conversations, and application APIs.
- `sys-gpu` provides a backend for GPU operations. Model weights and working
  tensors still reach its GPU memory; they are not hidden from the backend.
- Keep `sys-gui` independent. Compute offloading does not imply support for
  desktop graphics or simultaneous display use of the same GPU.

First prove one AI qube offloading computation to `sys-gpu`, then assess
performance, client isolation, resource limits, and recovery before sharing it
between qubes. Prefer existing upstream implementations and preserve Xen and
Qubes policy. Qrexec can gate access but does not establish backend isolation.

[llama.cpp RPC](https://github.com/ggml-org/llama.cpp/blob/master/tools/rpc/README.md)
is a possible experiment, not a chosen implementation: it is engine-specific
and upstream labels it experimental and insecure. General CUDA access would
require a broader GPU virtualization or API forwarding solution.

## Graphics and inference proposal

This is a design proposal, not an implemented VM or a verified hardware setup.

The discrete GPU should be persistently assigned to `sys-gpu` through Qubes
PCI device management. Reserve it for Xen's PCI backend before a dom0 graphics
driver binds it. Account for companion functions, IOMMU isolation, reset
behaviour, and firmware/framebuffer use when selecting the device. Dom0 still
administers assignment through Xen and Qubes; removing its GPU driver does
not remove dom0's control authority.

Ordinary qubes receive only explicitly allowed services. Keep `sys-gpu`
without a NetVM or Qubes administrative privileges by default. A qrexec service
can authorize a named client and carry a stream to a backend, but adapting an
existing TCP protocol requires a bridge and measurement. Authorization does
not make the backend parser safe or isolate its GPU memory between clients.

| Use | Possible service boundary | Main limitation |
| --- | --- | --- |
| AI application in another qube | AI qube runs inference orchestration; `sys-gpu` runs an engine-specific compute backend | llama.cpp RPC is experimental and explicitly insecure upstream; it does not provide general remote CUDA |
| Complete inference service | `sys-gpu` runs an inference server; other qubes submit requests | Simpler interface, but moves model execution and model storage into the GPU VM, changing the original division of responsibilities |
| Rendering or video jobs | A fixed worker in `sys-gpu` accepts jobs and returns results | The worker processes client data in the shared GPU trust domain; it does not accelerate arbitrary client applications |
| Accelerated browsers, games, or 3D tools that remain in their own qubes | A virtual GPU frontend and a renderer/backend in `sys-gpu` | Substantial Xen/Qubes integration, buffer transport, presentation, and security work |
| Physical desktop output | GPU-owning display domain, integrated with the Qubes GUI protocol | Changes the display boundary and does not itself accelerate applications in other qubes |

Qubes explicitly distinguishes GPU ownership in a GUI domain from acceleration
inside application qubes in its [GUI domain documentation](https://doc.qubes-os.org/en/latest/user/advanced-topics/gui-domain.html).
Its [GPU acceleration tracking issue](https://github.com/QubesOS/qubes-issues/issues/8552)
remains open when checked for this proposal. Mesa's [Venus documentation](https://docs.mesa3d.org/drivers/venus.html)
describes an existing Vulkan virtualization building block, but does not
establish a working Xen/Qubes service-domain integration. Treat that integration
as a separate development milestone, preserving Xen and the Qubes GUI boundary.

The [llama.cpp server](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
provides a higher-level inference API if model execution inside `sys-gpu` is
acceptable. The RPC alternative above preserves the existing AI-qube direction
more closely, but should initially be evaluated with disposable data and one
client, following upstream's warning against sensitive environments.

Sharing one GPU couples workloads: the backend can observe submitted data,
clients compete for memory and execution time, and a GPU hang can interrupt
every service. Plan admission limits and recovery before allowing multiple
clients. Do not assume process separation provides Qubes-strength isolation
inside `sys-gpu`, or that graphics and inference will meet latency targets
when run concurrently.

## First implementation milestone

1. Identify the GPU model, VRAM, PCI functions, isolation/reset constraints,
   and display wiring. Decide whether graphics means physical output,
   submitted jobs, or acceleration inside other qubes.
2. Add the NixOS guest and domain declarations using the existing
   `mkNixNativeDomain` and `devices.pci` interfaces, with HVM mode and fixed
   memory suitable for the selected hardware. Configure explicit device IDs
   and early dom0 device reservation. Keep hardware activation opt-in until
   the assignment and recovery path are validated.
3. Package matching guest kernel modules and userspace GPU libraries. The
   current builder selects a shared Qubes kernel; installing a NixOS GPU
   package alone does not establish kernel/module compatibility.
4. Prove the physical GPU initializes in `sys-gpu`, remains unavailable to
   dom0's graphics stack and application qubes, and survives the intended
   shutdown/restart sequence. Keep strict reset and PCI configuration defaults
   unless the selected hardware requires an explicitly assessed exception;
   Qubes documents the implications of [additional PCI attach options](https://doc.qubes-os.org/en/latest/user/how-to-guides/how-to-use-pci-devices.html).
5. Prove one explicitly allowed AI client can use the backend and an
   unauthorized client is denied. Measure model loading, throughput, latency,
   memory exhaustion, cancellation, and backend failure. Keep results under
   `.airlock-logs/` and extend pytest coverage alongside `make test-quick`.
6. Select a graphics experiment from the table, then evaluate concurrent use.

Keep `sys-gui` independent for the initial compute milestone. The current
hybrid GUI still relies on dom0 for display, so it needs another display device
or a separately designed display path when the discrete GPU is reserved for
`sys-gpu`. Nested VM tests can cover configuration and service policy, but do
not prove physical GPU passthrough or performance.
