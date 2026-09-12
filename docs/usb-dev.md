# Persistent USB developer system

Implementation is unbuilt and untested. This is a writable disk image, not a
live ISO or an installer. XFCE remains the desktop. Normal VM targets are unchanged.

## Build and provision on the development host

Run `make usb-dev-image`. The result link is under
`$AIRLOCK_RESULT_LINK_DIR/usb-dev-image` (or `.airlock/usb-dev-image` without a
configured data root). Find its `.img` file; resolve it to its physical path
in the custom store before using the writer. The current image is a 20 GiB
seed, so it fits a nominal 64 GB USB. On its first boot, NixOS grows the root
partition and filesystem to use the remaining USB capacity. 128 GB is still
recommended for longer-lived builds.

Inspect `lsblk -o NAME,TRAN,SIZE,MODEL,SERIAL,MOUNTPOINTS` and select an expendable
whole-disk `/dev/disk/by-id/usb-…` path. Do not use the internal NVMe. Unmount
every partition on the selected USB. The writer rejects mounted disks,
non-USB disks, partitions and active device-mapper holders.

```sh
python3 lifecycle/image/tools/usb-dev.py write --image /absolute/airlock-usb-dev.img --device /dev/disk/by-id/usb-EXACT_ID
sha256sum /absolute/airlock-usb-dev.img
```

Only after reviewing the printed model/serial/size, repeat with sudo and add
`--write --confirm-serial EXACT_SERIAL --sha256 EXACT_HASH`. This **erases the
selected USB**, flushes the device, and does not touch firmware boot variables.
It checks the image hash before writing, then copies only allocated image
extents by default; this makes a sparse seed image much quicker to flash. Add
`--full-write` to materialize zero-filled holes, or `--verify` for the slower
full write and full hash read-back. Sparse writing leaves any prior USB bytes
in filesystem blocks marked free by the image; use `--full-write` for an
untrusted/reused USB where that data-remanence trade-off is unacceptable.
An interrupted write requires re-imaging that USB. The host needs Python,
util-linux, Git, OpenSSH, OpenSSL, cryptsetup and e2fsprogs for these tools.

Mount the new USB's `airlock-usb` root partition at a fresh mountpoint, using its
**USB by-id partition path** (normally `…-part2`). Inspect `lsblk -f` first.
Do not mount by a potentially duplicated generic label. Then:

```sh
sudo python3 lifecycle/image/tools/usb-dev.py personalize \
  --root /mnt/airlock-usb --repo "$PWD" --public-key /absolute/id_ed25519.pub \
  --auth-file /absolute/path/to/auth.json
```

This asks for a new local password for dom0 `airlock` and guest `user`, installs
one public SSH key and clones the clean committed repository onto a writable
`usb-dev` branch. When `--auth-file` is supplied, it also creates an encrypted
Codex vault, imports only that cache, then locks the vault. No private keys,
untracked files, hooks, build artifacts, remote credentials or Codex settings
are copied. The clone has no origin; configure a suitable remote later. The
password hash is outside Nix, readable only by root. Re-personalization refuses
to overwrite an existing setup.

For a throwaway, physically controlled developer USB only, add
`--insecure-dev-password` to set the literal password `password` without a
prompt. It is intentionally opt-in and must not be used for a shared or
network-exposed system.

## Codex authentication: separate encrypted vault

The raw OS itself is **not encrypted**. Codex's state lives in a 512 MiB LUKS2
file vault. Its passphrase is separate from the login password and is never
saved by the tool. Supplying `--auth-file` to `personalize` is the preferred
one-step setup. To add credentials later, offline before unmounting the USB:

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

Log into XFCE as `user` with the password you chose. Dom0's local console uses
`airlock` and the same password. From the dom0 console:

```sh
nmcli --ask device wifi connect YOUR_SSID
sudo tailscale up --hostname=airlock-usb
sudo airlock-usb vault-unlock
cd /home/airlock/dev/nixome
codex
```

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
The first boot expands the root filesystem to the USB's available capacity.
It can fill quickly; use the guarded `make gc-auto` with that same data root.
No full-store garbage collection is done automatically.

## Evidence and next experiment

Store notes/logs under the repo's `.airlock-logs/usb-dev/`. Confirm networking,
Xen, sys-gui lifecycle, input and persistence before trying the GPU experiment.
See [the plan](usb-dev-plan.md) for the acceptance gates. Do not infer AMD
passthrough from a working hybrid desktop: in default mode dom0 still drives
the physical AMD GPU.

The image also includes manual [text-recovery and AMD-lab boot entries](usb-amd-lab.md).
The AMD entry is scaffolding for remote diagnostics, not a working physical
desktop yet; input forwarding and reset safety remain open.
