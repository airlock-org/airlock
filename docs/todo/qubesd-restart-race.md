# Qubes communication race during deployment

Status: investigation needed; not a confirmed explanation for the black-screen
incident on 2026-09-23.

## Evidence

Deployment of machine revision `a9ac77b`, transaction `1790168009761922058`,
updated sys-dev's shared-store generation. The storage helper stopped
`qubesd`, saved Qubes storage metadata, then started it again.

At 13:54:30–31 BST, concurrent GUI/qrexec requests encountered missing sockets
and permission errors. Requests subsequently succeeded, and input forwarding's
reconnection check completed at 13:54:33. This does not prove physical keyboard
input worked afterward.

The journal ended later, at 13:55:18, during sys-dev startup and physical-device
setup. The transaction remained in `starting`, without a recorded timeout or
verification result. Do not treat the earlier Qubes errors as proof of the
freeze's cause.

## Investigate

- Trace socket creation, permissions, and readiness across the `qubesd` restart.
- Check how GUI/qrexec clients and input forwarding recover interrupted calls.
- Determine whether the storage update can use supported Qubes APIs without
  restarting `qubesd`; otherwise define and test safe restart/readiness handling.
- Add pytest regression coverage and a controlled live recovery test before
  claiming a fix. Keep the later sys-dev/device-start stall a separate lead.

Evidence sources: the dom0 journal, `qrexec.sys-gui.log`, and the transaction's
`status.json` and `setup/setup-config.commands.log`. Preserve the existing Qubes
architecture; do not weaken socket permissions to hide the race.
