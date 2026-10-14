# Validation status

Airlock is a proof of concept with nested and hardware-specific physical evidence.
It has not demonstrated the full Qubes OS security model or general hardware support.
A successful build, a passing pytest suite, a selected release, and a verified
running generation are distinct results.

Current source and the physical machine name the assistant VM `usr-assistant`.
The [2026-10-13 migration trial](history/usr-assistant-migration-2026-10-13.md)
completed apply, reboot, whole-machine generation verification and persistence
checks. An ordinary-release follow-up remains to include the committed
boot-installation fix and clear the one-shot migration declarations.

## Demonstrated paths

Nested Qubes trials on Xen cover control services, inter-VM RPC through qrexec,
closure-backed NixOS VM startup, read-only guest stores, config staging/validation/promotion, and
host generation switching. See [testing](testing.md) and [config system](config-system.md).

Physical lab trials include wired networking through sys-net and
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

The 2026-10-13 migration verified a newer running host and all six guest
generations. Physical colour acceptance from the earlier khaki-label trial
remains pending. See [deployment lab evidence](history/machine-deployment-lab-notes-2026-10-09.md)
and the [migration result](history/usr-assistant-migration-2026-10-13.md).

Use the [docs index](README.md) to find current acceptance work. Historical
pending statements are not automatically current backlog or a support promise.
