# Generation retention deployment, 2026-10-12

Component revision: `40a730d46971a49a6e67b1ddc15aff9a7a925e51`.
`make test-quick` passed with 891 passed and 59 skipped; a separate focused
retention run passed all 10 tests. Normal whole-machine preparation succeeded.

Activation transaction `1791800447980401342` failed in host boot preparation,
before stopping guests: the separate 1 GiB EFI partition was full during
`airlock-host-store seal-boot --keep-default`. The ordinary root partition's
available space did not cover this requirement.

Recovery moved an unreferenced native initrd, verified byte-identical to its
retained EFI/airlock copy, and the failed partial `.pending` initrd into
`/var/log/airlock/efi-space-recovery-2026-10-12/`. No retained generation or
boot-menu entry was removed. This made 147 MiB available on the EFI partition.

Normal resume transaction `1791800628499467903` then failed with a retained EFI
filename collision. The interrupted seal had left a Xen binary without an
artifact descriptor; boot installation regenerated different bytes under that
same filename. The leftover was confirmed absent from both boot-menu references
and retained artifact descriptors, moved into the same recovery directory, and
normal coordinator resume was submitted again.

Follow-up work:

- Include EFI capacity and sealing's temporary duplicate space in admission
  checks before publishing or changing boot profiles.
- Reclaim only boot files proven unreferenced by all native Xen entries and
  retained artifact descriptors; preserve selected, running and recovery boots.
- Make interrupted sealing resumable when boot installation regenerates a Xen
  EFI file before its retained descriptor is committed. Never overwrite a
  descriptor-protected file on a hash mismatch.
- Add regression coverage for both exhaustion and interrupted-seal recovery.

Another resume regenerated the unused native initrd, consuming the freed space
before sealing could complete. The recreated duplicate and partial copy were
moved to a separate `attempt-3` recovery subdirectory. With the coordinator
stopped, the existing `airlock-host-store seal-boot --keep-default` maintenance
command completed successfully; normal coordinator resume was then submitted.
This is an operational boot-artifact repair, not a configuration overlay.

Resume still exhausted space in upstream boot installation. A read-only policy
plan retained the newest two, weekly anchor and aging successor, and identified
34 verified records eligible for pruning. Two obsolete candidates,
`872cae54eddb1835858b18a441c38cdbe74e2ed8cf06f2a954a0ab61caa0b393` and
`ddc933c8ccd074231719e80990e1d0d32d3954ebab5b98df57c43f7786149324`,
had no prune blockers and were the only retained descriptors sharing the old
`7niaps0v3rimqahjzqi03pns476q7q5d` initrd. Pruning also attempted menu
regeneration before retiring artifacts, initially failing for space. Their exact
host artifacts were retired using the existing backend command; unreferenced,
hash-identical native copies and failed `.tmp` files were quarantined under
`attempt-4`. Both coordinator prune journals then completed successfully,
preserving all four policy-selected rollback choices. Deployment was resumed.

The prune ordering must also tolerate a full EFI partition: authenticated
obsolete artifact retirement should make space before fallible menu refresh.
Automatic maintenance must not repeatedly hit this dependency cycle.

Deployment transaction `1791801231221221547` subsequently passed boot preparation,
restarted sys-agent, sys-ctrl and sys-gui, selected candidate
`52e5cd6520d6961f7215ccdf6ae6a37cb25bfc92dd44cb7330f64e7f0693b1f1`,
and reached `gui-reconnecting`. It is waiting for physical GNOME login before
GUI reconnection and final guest verification. The host is still running the
previous generation. A verification request at this intermediate phase did not
succeed; this is not a completed or post-reboot verified deployment. Do not
reboot until reconnection finishes and status reports `reboot-required`.

The first login arrived after the reconnection timeout. Normal resume replayed
the affected VM restarts; after a second physical sign-in, transaction
`1791811335379818663` reached `reboot-required` with no error. All six guests
reported their expected running systems, and usr-home reconnected to sys-gui
without restarting usr-home. `make machine-verify` passed at the pre-reboot
boundary. The selected host is
`/nix/store/gsi4qfc2gb60aqmzvnvijnai8g3mglcz-nixos-system-dom0-26.11.20260904.801bef6`.
Host boot and automatic retention/GC activation still require reboot and
post-reboot verification; the user has been told it is now ready to reboot.
