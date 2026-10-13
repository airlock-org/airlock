# Assistant migration trial — 2026-10-13

The physical `sys-agent` to `usr-assistant` migration was submitted through the
incremental controller route. Whole-machine verification and persistence
acceptance are pending. The last remotely observed phase was `stopping`;
the old assistant disconnected during the declared shutdown sequence.

## Committed source and preparation

- Migration source: `airlock-machine` commit
  `95a3ad3c8eba7116a9ad82acac32b1ecff058316`.
- Candidate:
  `/var/lib/airlock/machine/candidates/50571af160ca6d337abae291e1507d51c4040189667adc891174f827a3a0503c`.
- Expected host:
  `/nix/store/3j39bazrs08pdxhazd7srs0j6s1jglk6-nixos-system-dom0-26.11.20260904.801bef6`.
- Expected assistant:
  `/nix/store/xyx1l4fjkj454z4v0lj8paaq0s0d1a03-nixos-system-usr-assistant-26.11`.

The controller was updated first and its selected/running guest systems matched.
The migration build then completed in 484 seconds. Admission reused every guest
root image and transferred 49 guest package objects, totalling 48,005,632 bytes.
Host/transport staging transferred 59 paths with 117,262,944 compressed bytes.
Apply reused the reviewed prepared plan without another build or transfer.

The plan copies only `sys-agent`'s private volume into `usr-assistant` and retires
only `sys-agent`, retaining the stopped VM for recovery. It restarts sys-ctrl,
sys-gui and usr-home. Credentials and session file fingerprints were recorded
before shutdown; post-copy comparison is still pending. The source checkout
preserves the live Firefox addition and its original local branch.

## EFI space failure and committed recovery

Initial transaction `1791851490044789561` failed at `host-boot-preparation`.
The upstream EFI installer encountered `ENOSPC` on the separate 1 GiB EFI
partition. This happened before guest shutdown or private-volume copying;
all existing VMs remained available. The conditional reboot did not run.

The repository maintenance command verified all 19 independently sealed boot
entries and their payloads, then quarantined 322,047,828 bytes of unreferenced
upstream cache, including a partial installer file. It changed no retained boot
entry, recovery artifact, profile history, or Nix store object. The complete
receipt and copied cache are retained at:

`/var/lib/airlock/boot-cache-quarantine/1791852074346813362/`.

Machine commit `289803335127ae2696a421b3ac9b592ce3eef65a` corrects boot
installation to reuse verified sealed generations instead of regenerating
their upstream payload copies. Parent deployment tooling can build and transfer
this committed standalone coordinator for recovery. Tests passed: 961 quick
tests, 63 skipped, with targeted EFI integrity, failure, and recovery coverage.

The standalone coordinator built successfully and recovery transferred three
missing Nix paths. Resume transaction `1791852186369516557`, worker
`airlock-machine-deploy-d8e273b55e8b.service`, passed boot preparation and reached
guest shutdown. Its recovery coordinator is:

`/nix/store/40waiggklykwpq1p21qyla3f3lbyppcl-airlock-machine-deploy/bin/airlock-machine-deploy`.

Temporary maintenance code under `/run` was removed after the cache move.
An operational transient service, `airlock-assistant-rename-reboot.service`,
waits at most 30 minutes after submission. It reboots only if this exact
candidate reaches `reboot-required`, the independent private-copy receipt is
complete, usr-assistant is observed in guest verification, and the selected
host matches the candidate. A failed resumed transaction exits without reboot.
The service and its temporary script disappear after reboot; no policy grant
or runtime configuration override was added.

## Resume observation and acceptance

On the physical dom0 console, inspect the recorded state before attempting
recovery or reboot:

```sh
sudo /var/lib/airlock/machine/controller status
sudo journalctl -u airlock-machine-deploy-d8e273b55e8b --no-pager -n 100
```

The last known assistant Tailscale identity used address `100.104.92.101`.
Rediscover its current name/address rather than assuming that enrollment changed.
After reconnection, follow [the migration guide](../usr-assistant-rename.md):
verify the running host and every guest, compare credentials/session history and
workspace state, confirm the old VM is retained as stopped trash, and check GUI
behavior separately. Deploy an ordinary release containing the boot-installation
fix to clear the one-shot copy/retirement declarations after acceptance.

Detailed preparation, plans, receipts, worker logs and fingerprint snapshots
are saved on the development computer under
`.airlock-logs/assistant-live-rename/`. No successful physical migration or GUI
acceptance is claimed by this pending report.
