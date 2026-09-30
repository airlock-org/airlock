# Hide local Files from GNOME

Status: planned. Created 2026-09-30. Live launcher origin is unverified.

The user reports that GNOME exposes a Files launcher that may start Nautilus
in `sys-gui`. Confirm its origin on the live desktop. Where `usr-home` is
available, the visible Files launcher should open the file browser in
`usr-home` through the existing Qubes RPC path.

Keep Nautilus installed in `sys-gui`: the GNOME configuration explicitly
retains it for DING's desktop file operations. The existing
`airlock-files.desktop` launcher targets `usr-home` and is the declared
`inode/directory` default, but its distinct desktop ID does not hide the
local Nautilus entry.

- [ ] Identify the visible Files entries and confirm which VM each starts.
- [ ] Hide the local Nautilus application entry declaratively in
  `airlock-machine`, scoped to GNOME profiles with personal VM launchers
  enabled. Evaluate `NoDisplay=true` first; it suppresses menu visibility
  while retaining the entry, as described in the
  [Desktop Entry Specification](https://specifications.freedesktop.org/desktop-entry/latest/recognized-keys.html).
- [ ] Check GNOME's application grid, app search, Files search provider,
  desktop shortcuts, and existing dock favourites for exposure of local Files.
  Ensure the ordinary visible Files launch routes to `usr-home`.
- [ ] Preserve the local Nautilus executable and DING's required operations.
  Inspect desktop-folder actions separately: hiding a launcher does not
  prevent direct invocation, and a `sys-gui` path cannot simply be treated as
  the same path in `usr-home`.
- [ ] Preserve local Files visibility in minimal profiles without `usr-home`.
- [ ] Extend pytest coverage in
  `airlock-machine/tests/test_usr_home_gnome.py` to verify the effective desktop
  entry, retained Nautilus dependency, existing RPC launcher and MIME default,
  and minimal-profile behavior. Run `make test-quick`.
- [ ] Follow the [whole-machine deployment workflow](../machine-deployment.md)
  and verify the resulting live generation: only the `usr-home` Files launcher
  is exposed, its process runs there, and DING's desktop operations still work.
  Record build and live verification separately.

This task changes desktop visibility while preserving the existing Qubes
architecture and policy. It does not remove local Nautilus or prevent its
intentional use by desktop integration.
