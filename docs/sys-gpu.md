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
