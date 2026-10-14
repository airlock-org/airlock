# usr-assistant repository Git identity, 2026-10-14

The assistant repository now receives local defaults of `Airlock User` and
`user@airlock.invalid` when its effective identity is missing. The oneshot runs
as the guest user after workspace relocation and optional source seeding, before
qrexec access. Existing local, global, or included identity settings take
precedence. Missing checkouts are left alone and handled on a subsequent boot.

The implementation is machine commit `0bf5e3731665f5f68880b52dcaadeb0138e1e879`.
Quick tests passed (971 passed, 64 skipped after deployment fixes); the focused
Nix check of service ordering and user identity passed. Tests cover real Git
commit authorship, local-only settings, repeated setup, existing and partial
personal identities, a checkout created later, and malformed configuration.

## Deployment

The live `~/airlock-machine` checkout had three uncommitted usr-home edits.
They were preserved: deployment used an isolated committed checkout under
`~/.local/share/airlock/deploy-sources/`. The original HEAD, dirty-file list, and
SHA-256 of the complete binary diff matched after the guest restart.

The controller built the assistant and admitted generation 2, transferring 16
objects (3,567,104 payload bytes). Its first plan correctly refused inherited
one-time retirement instructions from the completed assistant rename.
Machine commit `c68ccf1` clears retirement and private-volume-copy intent when
deriving a guest-only candidate; a regression reproduces the refusal and
verifies that subsequent guest activation leaves the migration record intact.
The parent transport now chunks large admission receipts within the existing
PTY control limit and verifies their checksum before publication.

The committed standalone coordinator was built and transferred through the
existing development transport. Parent `make machine-guest-plan` and
`make machine-guest-apply` used `UPDATE_COORDINATOR=1`. The reviewed plan changed
and stopped only usr-assistant, with no retirements, volume copies, or host
reboot. The coordinator includes the earlier sealed-boot profile filtering fix;
current host kernel/initrd cache files were reused without quarantine or resizing.

Candidate:
`/var/lib/airlock/machine/candidates/145148b1f1853590e8194e9cf46dfd61d9128899f507899acf11b6643d75f5b8`

Transaction: `1791973358736544432`.
Coordinator: `/nix/store/42nl9cxvdjk7wd3p833211wfilqbw93m-airlock-machine-deploy/bin/airlock-machine-deploy`.
Assistant: `/nix/store/6kp4fbs4kd6n7miffxrxkbz5qcz74rrz-nixos-system-usr-assistant-26.11`.
Host: `/nix/store/3j39bazrs08pdxhazd7srs0j6s1jglk6-nixos-system-dom0-26.11.20260904.801bef6`.

## Result

The guest restarted and reconnected. Its identity service reported
`active (exited)`, `Result=success`, `User=user`. Repository-local identity and
`git var GIT_AUTHOR_IDENT` both reported the requested defaults. The deployed
generation matched the admitted system. GNOME remained running, and the
sys-gui-originated readiness calls for usr-assistant and usr-home passed.

Guarded generation maintenance passed, pruning two eligible old records and
reclaiming 53,948,416 bytes. The normal maintenance path was temporarily paused
so the committed coordinator could handle boot refreshes while the running host
still contains the earlier installer. The path was restored after cleanup;
its normal service is inactive with `Result=success`, and no pending work or
temporary runtime override remains.

After restoring normal maintenance, `make machine-verify` reported `complete`:
the host and all six guests matched their selected generations. Old retired VMs,
including sys-agent, remain halted and retained. No host reboot was performed.
The ordinary host update remains separate because EFI staging for a new host
closure still needs sufficient space.

Local evidence is under `.airlock-logs/usr-assistant-git-identity/`.
