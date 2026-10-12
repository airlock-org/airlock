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

Activation and post-reboot verification remain pending at the time of this note.
