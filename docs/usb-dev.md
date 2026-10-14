# Persistent USB developer system

Physical USB boot is still being validated. This is a writable disk image, not
a live ISO or an installer. The normal `Airlock` entry uses the internal
`usb-lab` GNOME topology. Normal VM targets are unchanged.

The USB developer profile disables USBGuard so the inherited dom0 default-block
policy cannot disconnect its USB root disk. This development-only exception also
applies to its recovery modes, not normal installed-system profiles. Persistent
journals synchronize every second to reduce lost diagnostics after forced shutdown.
Topology setup, physical-GPU handoff and declared-VM autostart also force a journal
sync at their successful boundaries; the GPU path syncs immediately before the
visible bootstrap console is released as well. A sudden power cut can still lose
the final in-flight messages, but completed boot milestones are explicitly durable.
Before a physical GUI test, validate the image with a virtual USB boot disk;
virtio-only testing does not exercise USB root access.

## Build and provision on the development host

Run `make usb-dev-image`. It prepares the committed machine source and pinned
flake inputs from the initialized local submodule, including unpublished commits;
no temporary Git environment overrides are needed. The checkout must be clean
and match the committed parent pin. It realizes the declared builder closure,
then checks two isolated
offline guest builds before assembling the image. A matching passing receipt
skips the two trials on later builds with the same inputs. The receipt and
machine-readable results live under the configured Airlock cache and log
directories. Run `make usb-builder-offline-check` to force both trials and
refresh the receipt.
Failures stop image assembly. The examples add an `/etc` marker and change an
existing usr-home service setting using the image's pinned software. Each trial
imports the unchanged committed guest module in a temporary checkout and adds
its setting in a separate module, so application-list changes do not break the
examples. The declared build tools use the guest's package set, including the
Composefs tools needed to rebuild its `/etc` image. They do
not establish offline support for arbitrary new packages or versions.

The v2 USB image marker requires the matching provisioner. Older v1 images
retain their `airlock` dom0 account; this change does not migrate them.

The image result link identifies the current image without
exposing its changing Nix store path. The current image is a fixed 32 GiB
seed image with a 3 GiB EFI partition, so it fits a nominal 64 GB USB. EFI space
holds retained Xen, kernel and initrd files as well as temporary installer copies.
This layout applies to newly built images; normal deployments do not resize an
existing USB's partitions. `usb-dev-provision` expands its
root partition and ext4 filesystem to the USB's capacity after writing, before
personalization. The build/flash does not need to write a larger empty image.
Larger media provides more headroom for longer-lived development.

The build prepares the shared guest package store and places it at its final
location in the image. It also creates all seven VMs' initial root and private
disks, kernel files, Qubes configuration and shared-store registrations using
Qubes' offline API. Sparse disks are installed directly into `/var/lib/qubes`;
guest-only source packages and root-image copies are omitted from dom0's ordinary
store. First setup reconciles this existing state and performs hardware-dependent
assignments. It does not import the seven root disks again or register unchanged
store volumes. Hardware discovery, VM startup and readiness checks still happen
on the machine. The prepared configuration uses the normal `usb-lab` topology;
no hardware assignments or native Qubes autostart links are baked into the image.
Full checksums run directly before each image assembly, even if the separate
validation receipt is cached. Startup timing needs
to be measured on a rebuilt image; see [storage details](config-system.md#admission-and-prepared-usb-stores).

Inspect `lsblk -o NAME,TRAN,SIZE,MODEL,SERIAL,MOUNTPOINTS` and select an expendable
whole-disk `/dev/disk/by-id/usb-…` path. Do not use the internal NVMe. Unmount
every partition on the selected USB. The writer rejects mounted disks,
non-USB disks, partitions and active device-mapper holders.

Provision it with the serial printed by `lsblk`; the command resolves the
current result link, calculates the image SHA-256 itself, unmounts the selected
USB, writes it, expands root into the remaining space, copies the committed repository and SSH public key, syncs it, then unmounts it again:

```sh
make usb-dev-provision USB_SERIAL=YOUR_USB_SERIAL
```

Expansion is part of this command; no extra sudo command or first-boot resize
is needed. It validates the USB serial and the expected two-partition GPT layout,
checks the Airlock image marker and ext4 label, backs up the **newly written**
partition table, checks the filesystem offline, relocates the backup GPT header,
and grows only partition 2. The EFI partition, root start, partition UUIDs and
filesystem UUID remain unchanged. It checks the grown filesystem again before
personalization. Successful low-level diagnostics, including the expected GPT
relocation notices, are reported on stdout; stderr is reserved for command
failures that stop provisioning. Unexpected layouts or filesystem errors stop it.
Raw table backups, `before.json`, and a successful `result.json` are stored in
`$AIRLOCK_LOG_DIR/usb-root-expansion/grow-*/` on the development host (root-owned),
or `/var/log/airlock/usb-root-expansion/grow-*/` without a configured log directory.
Keep these outside the repository: root-only backup directories prevent Nix from
reading the source tree. Backups from older tooling under
`.airlock-logs/usb-root-expansion` should be moved outside the repo, not deleted.
These are recovery metadata, **not backups of data erased by flashing**. Do not
restore a smaller old partition boundary over an already enlarged filesystem.

The roughly 58.2-GiB USB with serial `YOUR_USB_SERIAL` should get a roughly
55.2-GiB root partition instead of 29 GiB. Dom0 needs that space because it hosts
the shared VM package store, writable VM disks and retained rollback generations.
The deployment workflow also imports/stages new Nix closures before changing
running VMs; the first transfer can temporarily duplicate packages already packed
in the image. Actual free space is lower than partition capacity and the deployment
space check still applies. This is disk storage, not dom0 RAM allocation.

Low-level `write` and `write-latest` remain image-only operations and do not grow
partitions. `boot.growPartition` and filesystem auto-resize remain disabled:
partition mutation occurs only in this explicit host-side provisioning command.

In particular, `blockdev --rereadpt: Device or resource busy` after sfdisk's
successful partition update could come from a redundant second kernel refresh
racing device probing. Provisioning now settles udev and checks the actual
kernel partition size instead of repeating that refresh. If the size is still
stale, it stops before filesystem resizing and requests unplug/replug. Rerun the
normal provisioning command to start cleanly from a fresh image write; there is
no partial-provisioning resume path. This is deliberate: resuming requires the
tool to recognize and safely continue several partially mutated GPT, filesystem
and personalization states. Rewriting the known image first is slower but keeps
provisioning reproducible and avoids treating an ambiguous disk state as valid.

The AMD lab now prepares its GPU's VFCT automatically from the booted host's
firmware during normal Qubes VM startup. No local firmware artifact or
`AMD_VFCT_FILE` is needed for provisioning. See [automatic AMD VBIOS](amd-vbios-automatic.md)
for supported configurations and failure handling. The lower-level
`usb-dev.py --amd-vfct` option remains available only for reproducing older
manual experiments; the automatic startup path never reads that file.

Provisioning prompts for a local dom0 password. Use
`USB_INSECURE_DEV_PASSWORD=1` only to select the literal development password
`password` on a physically controlled USB. It requires exactly one attached whole-disk USB with that serial, **erases
it**, flushes the device, and does not touch firmware boot variables. It copies
only allocated image extents by default; this makes a sparse seed image much
quicker to flash. To write without personalization or use `--full-write` or
`--verify`, run `python3 lifecycle/image/tools/usb-dev.py write --help`.
Sparse writing leaves any prior USB bytes in filesystem blocks marked free by
the image; use `--full-write` for an untrusted/reused USB where that
data-remanence trade-off is unacceptable. An interrupted write requires
re-imaging that USB. The host needs Python, util-linux, Git, OpenSSH, OpenSSL,
and e2fsprogs for these tools.

During device flushing, filesystem flushing, and unmounting, the provisioner
updates a spinner and elapsed time every five seconds. When Linux device counters
are available, it also shows completed device writes since that step began,
the current write rate, and I/O requests in flight. These show activity, not a
flush percentage or completion estimate; the step finishes only when the real
flush or unmount returns successfully. Terminals update one line, while captured
logs retain periodic status lines. Progress changes in the repository apply to
the next host-side provisioning command without rebuilding the image.

Mount the new USB's `airlock-usb` root partition at a fresh mountpoint, using its
**USB by-id partition path** (normally `…-part2`). Inspect `lsblk -f` first.
Do not mount by a potentially duplicated generic label. Then:

```sh
sudo python3 lifecycle/image/tools/usb-dev.py personalize \
  --root /mnt/airlock-usb --repo "$PWD" --public-key /absolute/id_ed25519.pub
```

This asks for a new local password for dom0 `user`, installs one public SSH
key and clones the clean committed repository onto a writable
`usb-dev` branch. No credentials, local Git hooks, untracked files, build
artifacts, or user application settings are copied. The SSH public key is the
only authentication input. Codex remains an ordinary application in usr-assistant;
Airlock does not import or manage its credentials.

The OS filesystem is unencrypted. This tool no longer supports the old Codex
vault commands. Existing USB credential files and vaults are not erased by a
source update; remove or migrate them separately if you have used that feature.

Unmount the root and use the laptop's firmware USB boot menu. No installation
onto an internal disk is needed. Only one Airlock USB should be attached.

## First boot

Use the firmware USB boot menu with one Airlock USB attached. The normal
`Airlock` entry starts the sys-gui GNOME topology. Dom0 uses the password set
during personalization; the current development guest login is `user` / `password`.
Wait for bootstrap to complete before manually starting guests.

For networking and interactive Tailscale enrollment in usr-assistant, follow
[the development access guide](usb-lab.md#first-boot). Normal dom0 has no direct network
or SSH listener. The enrolled identity persists on usr-assistant's private volume;
no Tailscale auth key or host identity is copied during provisioning.

The text recovery entry starts offline and skips normal guest startup. Follow
[machine recovery](machine-deployment.md#failures-recovery-and-limits) before
changing a failed machine. Earlier direct-dom0/GUI diagnostic instructions are
retained in [historical first-boot notes](history/usb-dev-first-boot-2026-09.md).

## Further reading

Use [machine deployment](machine-deployment.md) for updates and recovery and
[status](status.md) for current evidence boundaries. Older terminal diagnostics
and early image trials remain in the historical first-boot report.
