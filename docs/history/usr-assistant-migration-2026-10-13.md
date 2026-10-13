# Assistant migration trial — 2026-10-13

The physical `sys-agent` to `usr-assistant` migration completed through the
incremental controller route. The machine rebooted into the selected host,
explicit whole-machine verification passed, and all six selected/running guest
generations matched. Credentials, session history and workspace state survived
the independent private-volume copy. The old VM is stopped in recoverable trash.

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
before shutdown; all recorded fingerprints matched after the copy. The source checkout
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
The service and its temporary script disappeared after the successful reboot;
no policy grant or runtime configuration override was added.

## Verified running generation and persistence

Resume transaction `1791852186369516557` reached `complete`. The running host
equals the expected host above. The explicit typed verification command passed;
its first attempt returned an error during concurrent maintenance, so the
coordinator was also checked directly before the successful retry.

| VM | Selected and running system |
| --- | --- |
| sys-ctrl | `/nix/store/w3nyy7lwixf8zwaa7k961yw0zz562fbw-nixos-system-sys-ctrl-26.11` |
| sys-firewall | `/nix/store/38411k15h38g2sc94947lcahw3pnylxi-nixos-system-sys-firewall-26.11` |
| sys-gui | `/nix/store/2ly7rvrvsnbvr60rgxh1ik7x6b2jwwz5-nixos-system-sys-gui-26.11` |
| sys-net | `/nix/store/ydlaf3w0fcrxlzrrfii2afr0l000clcy-nixos-system-sys-net-26.11` |
| usr-assistant | `/nix/store/xyx1l4fjkj454z4v0lj8paaq0s0d1a03-nixos-system-usr-assistant-26.11` |
| usr-home | `/nix/store/4r3j73y7p91vylwqrmzhqn13860h232s-nixos-system-usr-home-26.11` |

The private-copy receipt is `complete`, with different source/target VM UUIDs
and volume IDs. Sys-agent is halted and retained as
`1791852186369516557-sys-agent`; usr-assistant is running. Exact comparisons
passed for Codex authentication, configuration and history files, the session
file count, source revision, and preserved `airlock-before-assistant-rename`
branch. Managed guidance links point to `/etc/airlock/usr-assistant/`. The
archived old builder exists under `/rw/airlock-builder-sys-agent-builder`.

Usr-assistant reconnected with the existing Tailscale address `100.104.92.101`
and the new DNS name `usr-assistant.tail6750b0.ts.net`. GNOME Shell is running,
and AppSessionReady calls from the actual sys-gui caller succeeded for both
usr-assistant and usr-home. Usr-home reports Firefox 155.0. These checks do not
replace physical window, colour, input, or broader GPU recovery acceptance.

## Follow-up and recovery records

The old release's automatic maintenance encountered the same upstream cache
duplication bug after reboot. Its retry was temporarily paused for recorded
recovery using the committed coordinator fix. A second guarded cache move
verified all 20 sealed boot entries and quarantined 202,955,737 bytes at
`/var/lib/airlock/boot-cache-quarantine/1791854117430608139/`.
The corrected coordinator completed the interrupted pruning and protected pool
cleanup with status `passed`, reclaiming 589,664,256 bytes. The normal maintenance
path was restored, its pending request was cleared, and temporary operation
files were absent. Final typed whole-machine verification passed again after
these operational changes were removed. Both assistant and home GUI sessions
were ready, and the post-reboot persistence comparison matched the original.

At final inspection, the EFI partition had 73,457,664 bytes free and the root
filesystem about 5.2 GiB free. EFI capacity remains a constraint for another
host release: review its space requirements before activation. Protected boot
generations were not removed to force the migration to fit.

An ordinary release containing the boot-installation fix still needs deployment
to clear the one-shot copy/retirement declarations and include that fix in the
selected host. The committed standalone fix was used for migration recovery;
the physically verified whole-machine release remains source `95a3ad3`.

On the physical dom0 console, inspect the recorded state before attempting
recovery or reboot:

```sh
sudo /var/lib/airlock/machine/controller status
sudo journalctl -u airlock-machine-deploy-d8e273b55e8b --no-pager -n 100
```

Rediscover the assistant's current address when reconnecting. The
[migration guide](../usr-assistant-rename.md) describes the verification and
ordinary-release follow-up. Retain the old VM and rollback records until their
recovery purpose has been reviewed.

Detailed preparation, plans, receipts, worker logs and fingerprint snapshots
are saved on the development computer under
`.airlock-logs/assistant-live-rename/`.
