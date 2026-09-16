# Airlock Agent Notes

- Preserve the Qubes architecture for the POC. Do not replace Xen, qrexec,
  QubesDB, qubesd, libvirt, or Qubes policy with new designs.
- Prefer upstream code, especially `CertainLach/nixos-qubes`, NixOS Xen support,
  and Qubes' own tests.
- Keep Nix as the source of truth wherever practical, but accept existing Qubes
  runtime state such as `qubes.xml` for the first milestone.
- For physical lab development, read `docs/machine-deployment.md` first. Change
  and commit this repo, then use `make machine-plan` / `make machine-apply`
  with `DEPLOY_TARGET=...`; verify the resulting whole-machine generation.
  Never leave successful fixes only in a live VM, `/run`, or an ad-hoc overlay.
  Record any unavoidable diagnostic mutation and remove it before verification.
  A successful build is not a successful live deployment; report both separately.
- Keep development observation separate from production behavior. Observation
  tools should report state without expanding Qubes control authority.
- `make test-quick` is the primary feedback loop. Preserve clear logs and
  machine-readable output under `.airlock-logs/`.
- New tests should be written in the pytest framework. Do not add new Bash test
  scripts; migrate or extend pytest coverage instead.
- Do not make full GUI support block the first qrexec/VM lifecycle milestone.
- Commit changes after each coherent conceptual change when practical. Use
  judgment about granularity, but avoid bundling unrelated work into one commit.
- Long Airlock image/template/VM iterations can grow the custom store quickly.
  Run `make gc-auto` periodically and after large failed build loops. Use only
  the guarded Airlock GC targets; do not garbage collect the host `/nix/store`.
