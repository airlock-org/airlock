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
- Before editing deployable system configuration for a live target, inspect its
  recorded deployed source and read-only status/verification. Compare that source
  with the checkout, including committed and uncommitted changes; a clean Git
  tree does not prove deployment. Record the baseline and starting revision.
- If older deployable changes, an unfinished/failed current deployment or a
  required reboot remain, stop new work and report them. Ask the user whether to
  deploy, discard or isolate the older work; do not do so automatically. If the
  deployed baseline cannot be established, stop rather than assume a match.
  Documentation/instruction-only changes do not require deployment; inspect mixed
  commits by content. Explicitly requested work to resolve the backlog may proceed.
- Recheck the baseline, deployment state and checkout immediately before
  submission. Stop if they changed beyond the user's authorized change; do not
  bundle intervening work into its review. Never bypass source approval or build
  a candidate just to discover whether older changes are pending.
- Match verification to the change. For routine package-list additions/removals,
  skip repository tests: review and commit the exact diff, obtain required source
  approval before evaluation/build, then apply and verify the requested package
  state. Keep normal Nix build checks enabled.
- For implementation, service, policy or deployment changes, run relevant tests;
  use `make test-quick` when shared behavior could be affected. For documentation
  changes, review the diff and check formatting; executable tests are unnecessary.
  Preserve clear logs and machine-readable output under `.airlock-logs/`.
- Do not add tests that merely repeat the configured package list.
- Routine package-list additions/removals do not require documentation updates
  unless they change documented behavior, instructions or supported capabilities.
  Keep package selections in Nix; link to that configuration rather than maintain
  package inventories or installation/removal logs in docs. Git and deployment
  records provide the change history. Preserve dated validation evidence.
- Keep `test-quick` free of Nix evaluations, builds and automatic tool provisioning.
  `make test-nix` is opt-in; do not run it as a routine pre-deployment gate.
- New tests should be written in the pytest framework. Do not add new Bash test
  scripts; migrate or extend pytest coverage instead.
- Do not make full GUI support block the first qrexec/VM lifecycle milestone.
- Commit changes after each coherent conceptual change when practical. Use
  judgment about granularity, but avoid bundling unrelated work into one commit.
- Long Airlock image/template/VM iterations can grow the custom store quickly.
  Run `make gc-auto` periodically and after large failed build loops. Use only
  the guarded Airlock GC targets; do not garbage collect the host `/nix/store`.
