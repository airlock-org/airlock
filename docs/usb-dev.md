# Persistent USB developer system

Physical USB boot is still being validated. This is a writable disk image, not
a live ISO or an installer. XFCE remains the desktop. Normal VM targets are unchanged.

## Build and provision on the development host

Run `make usb-dev-image`. Its result link identifies the current image without
exposing its changing Nix store path. The current image is a fixed 20 GiB
image, so it fits a nominal 64 GB USB. The remaining USB space is left unused
for this POC. 128 GB is still recommended for longer-lived builds.

Inspect `lsblk -o NAME,TRAN,SIZE,MODEL,SERIAL,MOUNTPOINTS` and select an expendable
whole-disk `/dev/disk/by-id/usb-…` path. Do not use the internal NVMe. Unmount
every partition on the selected USB. The writer rejects mounted disks,
non-USB disks, partitions and active device-mapper holders.

Provision it with the serial printed by `lsblk`; the command resolves the
current result link, calculates the image SHA-256 itself, unmounts the selected
USB, writes it, copies the committed repository, SSH public key and local
Codex auth cache, syncs it, then unmounts it again:

```sh
make usb-dev-provision USB_SERIAL=25110814640207
```

This development shortcut deliberately selects the literal local password
`password` and plaintext Codex auth; it is only for a physically controlled
USB. It requires exactly one attached whole-disk USB with that serial, **erases
it**, flushes the device, and does not touch firmware boot variables. It copies
only allocated image extents by default; this makes a sparse seed image much
quicker to flash. To write without personalization or use `--full-write` or
`--verify`, run `python3 lifecycle/image/tools/usb-dev.py write --help`.
Sparse writing leaves any prior USB bytes in filesystem blocks marked free by
the image; use `--full-write` for an untrusted/reused USB where that
data-remanence trade-off is unacceptable. An interrupted write requires
re-imaging that USB. The host needs Python, util-linux, Git, OpenSSH, OpenSSL,
cryptsetup and e2fsprogs for these tools.

Mount the new USB's `airlock-usb` root partition at a fresh mountpoint, using its
**USB by-id partition path** (normally `…-part2`). Inspect `lsblk -f` first.
Do not mount by a potentially duplicated generic label. Then:

```sh
sudo python3 lifecycle/image/tools/usb-dev.py personalize \
  --root /mnt/airlock-usb --repo "$PWD" --public-key /absolute/id_ed25519.pub \
  --auth-file /absolute/path/to/auth.json \
  --insecure-dev-password --insecure-plain-codex-auth
```

This asks for a new local password for dom0 `airlock` and guest `user`, installs
one public SSH key and clones the clean committed repository onto a writable
`usb-dev` branch. By default, `--auth-file` creates an encrypted Codex vault,
imports only that cache, then locks the vault. No private keys, untracked files,
hooks, build artifacts, remote credentials or Codex settings are copied. The
clone has no origin; configure a suitable remote later. The password hash is
outside Nix, readable only by root. Re-personalization refuses to overwrite an
existing setup.

For a throwaway, physically controlled developer USB only, add
`--insecure-dev-password` to set the literal password `password` without a
prompt. It is intentionally opt-in and must not be used for a shared or
network-exposed system.

Add `--insecure-plain-codex-auth` with `--auth-file` to skip LUKS and store the
Codex cache directly in the USB's writable root. This is also intentionally
opt-in: anyone who obtains the USB can use those access tokens.

## Codex authentication: separate encrypted vault

The raw OS itself is **not encrypted**. Codex's state lives in a 512 MiB LUKS2
file vault. Its passphrase is separate from the login password and is never
saved by the tool. Supplying `--auth-file` to `personalize` is the preferred
one-step setup unless you deliberately select `--insecure-plain-codex-auth`.
To add credentials later, offline before unmounting the USB:

```sh
sudo python3 lifecycle/image/tools/usb-dev.py vault-create --root /mnt/airlock-usb
sudo python3 lifecycle/image/tools/usb-dev.py auth-import --root /mnt/airlock-usb --auth-file /absolute/auth.json
sudo python3 lifecycle/image/tools/usb-dev.py vault-lock --root /mnt/airlock-usb
```

Creation prompts for the passphrase more than once. Import refuses to overwrite
an existing auth cache. If your local login uses an OS keyring there may be no
`auth.json`; authenticate inside Codex on the USB instead. No full `config.toml`
is copied. OpenAI documents copying the cache for headless login and treating
it as a password: [official OpenAI authentication documentation](https://learn.chatgpt.com/docs/auth#fallback-authenticate-locally-and-copy-your-auth-cache).

Unmount the root and use the laptop's firmware USB boot menu. No installation
onto an internal disk is needed. Only one Airlock USB should be attached.

## First boot

Initial setup prepares sys-gui's storage before launching the login screen.
The bootstrap screen shows the current phase and elapsed time; preparation
can take several minutes, and substantially longer on a slow USB drive.
Do not manually start sys-gui until `airlock-setup-config.service` completes:
its kernel preference is applied after storage preparation. Starting it early
can misleadingly report a missing `pvgrub2-pvh/vmlinuz` even though the intended
Qubes kernel is already installed. The detailed setup log is
`/var/log/airlock/setup-config.commands.log` and the result is
`/var/log/airlock/setup-config.json`.

Log into XFCE as `user` with the password you chose. Dom0's local console uses
`airlock` and the same password. From the dom0 console:

```sh
nmcli --ask device wifi connect YOUR_SSID
sudo tailscale up --hostname=airlock-usb
cd /home/airlock/dev/nixome
codex
```

When using the encrypted-vault path, run `sudo airlock-usb vault-unlock` before
starting Codex. It is not needed for `--insecure-plain-codex-auth`.

Wired Ethernet can simplify first boot. Join Tailscale interactively; no auth
key is in the image. This is a **new Tailscale node**, not the laptop's Ubuntu
identity. Use its reported address from the host:
`ssh airlock@NEW_USB_TAILSCALE_ADDRESS`. Verify the new SSH host key at the
laptop console. SSH password/root login and non-Tailscale port 22 are disabled.
Do not disable host-key verification to reuse the Ubuntu identity.

The vault must be unlocked after each reboot; the `codex` wrapper refuses to
run while it is locked. Tokens can refresh inside the mounted vault normally.
Close Codex before `sudo airlock-usb vault-lock`. Losing the passphrase loses
access to the vault (you can authenticate afresh). The vault protects stored
credentials, not a modified bootloader/OS or credentials while it is unlocked.
Keep physical custody of this development USB. Tailscale's machine identity,
Wi-Fi settings, logs, repository and VM state persist on the **unencrypted** root.

For builds on the USB, keep the custom store off the host store, for example:
`AIRLOCK_DATA_ROOT=/home/airlock/airlock-data make usb-dev-image`.
The fixed 20 GiB root can fill quickly; use the guarded `make gc-auto` with
that same data root. No full-store garbage collection is done automatically.

## Evidence and next experiment

Store notes/logs under the repo's `.airlock-logs/usb-dev/`. Confirm networking,
Xen, sys-gui lifecycle, input and persistence before trying the GPU experiment.
See [the plan](usb-dev-plan.md) for the acceptance gates. Do not infer AMD
passthrough from a working hybrid desktop: in default mode dom0 still drives
the physical AMD GPU.

The image also includes manual [text-recovery and AMD-lab boot entries](usb-amd-lab.md).
The AMD entry is scaffolding for remote diagnostics, not a working physical
desktop yet; input forwarding and reset safety remain open.
