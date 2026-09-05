# Nixome Agent Notes

- Preserve the Qubes architecture for the POC. Do not replace Xen, qrexec,
  QubesDB, qubesd, libvirt, or Qubes policy with new designs.
- Prefer upstream code, especially `CertainLach/nixos-qubes`, NixOS Xen support,
  and Qubes' own tests.
- Keep Nix as the source of truth wherever practical, but accept existing Qubes
  runtime state such as `qubes.xml` for the first milestone.
- Keep development observation separate from production behavior. Observation
  tools should report state without expanding Qubes control authority.
- `make test` is the primary feedback loop. Preserve clear logs and
  machine-readable output under `.nixome-logs/`.
- Do not make full GUI support block the first qrexec/VM lifecycle milestone.
