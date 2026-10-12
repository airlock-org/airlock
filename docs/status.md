# Validation status

Airlock is a proof of concept with nested and hardware-specific physical evidence.
It has not demonstrated the full Qubes OS security model or general hardware support.
A successful build, a passing pytest suite, a selected release, and a verified
running generation are distinct results.

Current source names the assistant VM `usr-assistant`. Earlier physical trials
used `sys-agent` or its predecessor; the
[rename transition](usr-assistant-rename.md) has not been physically verified.

## Demonstrated paths

Nested Qubes trials on Xen cover control services, inter-VM RPC through qrexec,
closure-backed NixOS VM startup, read-only guest stores, config staging/validation/promotion, and
host generation switching. See [testing](testing.md) and [config system](config-system.md).

Physical ThinkPad lab trials include wired networking through sys-net and
sys-firewall, GNOME/input integration, whole-machine apply/reboot/rollback,
persistent guest data, direct-pool deployment, and shared host/guest storage.
Evidence is limited to the configuration and trial recorded in each
[completed plan](completed/) and [dated report](history/).

## Open limits

- AMD GPU reset and restart failures remain under investigation.
- GUI watcher startup/reconnection can require operational recovery.
- First-run wizard acceptance and broader recovery trials remain pending.
- Additional-GPU work remains experimental/deferred.
- Compatibility cleanup has remaining persisted-state and Nix/EROFS acceptance.

The latest recorded khaki-label deployment (2026-10-09) selected a host requiring
reboot; its post-reboot verification and physical colour acceptance were pending.
See [deployment lab evidence](history/machine-deployment-lab-notes-2026-10-09.md).
This local public-readiness cleanup does not change that live status.

Use the [docs index](README.md) to find current acceptance work. Historical
pending statements are not automatically current backlog or a support promise.
