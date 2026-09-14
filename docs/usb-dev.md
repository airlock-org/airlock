# Persistent USB developer system

Physical USB boot is still being validated. This is a writable disk image, not
a live ISO or an installer. XFCE remains the desktop. Normal VM targets are unchanged.

The USB developer profile disables USBGuard so the inherited dom0 default-block
policy cannot disconnect its USB root disk. This development-only exception also
applies to its recovery modes, not normal installed-system profiles. Persistent
journals synchronize every second to reduce lost diagnostics after forced shutdown.
Before a physical GUI test, validate the image with a virtual USB boot disk;
virtio-only testing does not exercise USB root access. Use the default hybrid GUI
entry for this checkpoint, not the experimental AMD passthrough entry.

## Build and provision on the development host

Run `make usb-dev-image`. Its result link identifies the current image without
exposing its changing Nix store path. The current image is a fixed 32 GiB
image, so it fits a nominal 64 GB USB. The remaining USB space is left unused
for this POC. 128 GB is still recommended for longer-lived builds.

The build now prepares the shared guest package store for both sys-gui boot
variants and places it at its final location in the image. Guest-only source
packages are omitted from dom0's ordinary store. First setup checks the prepared
store's integrity and attaches it; it no longer repackages gigabytes of packages
on the USB. The integrity scan still reads the backing data once. The speedup
needs to be measured on a rebuilt image; see [storage details](config-system.md#admission-and-prepared-usb-stores).

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

The AMD lab now prepares its GPU's VFCT automatically from the booted host's
firmware during normal Qubes VM startup. No local firmware artifact or
`AMD_VFCT_FILE` is needed for provisioning. See [automatic AMD VBIOS](amd-vbios-automatic.md)
for supported configurations and failure handling. The lower-level
`usb-dev.py --amd-vfct` option remains available only for reproducing older
manual experiments; the automatic startup path never reads that file.

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

This asks for a new local password for dom0 `airlock`, installs one public SSH
key and clones the clean committed repository onto a writable
`usb-dev` branch. By default, `--auth-file` creates an encrypted Codex vault,
imports only that cache, then locks the vault. No private keys, untracked files,
hooks, build artifacts, remote credentials or Codex settings are copied. The
clone has no origin; configure a suitable remote later. The password hash is
outside Nix, readable only by root. USB dom0 uses declarative users so boot
activation applies this hash even though the image already contains a locked
account. This also applies to the text-recovery and AMD-lab boot entries.
Changes made with `passwd` are reset at activation; the provisioned hash file
is the source of truth for dom0's password. Re-personalization refuses to overwrite an
existing setup.

For a throwaway, physically controlled developer USB only, add
`--insecure-dev-password` to set dom0's literal password `password` without a
prompt. sys-gui uses that same fixed developer password, set declaratively in
its immutable NixOS image; it is not mutated over qrexec during boot. This is
intentionally opt-in and must not be used for a shared or network-exposed system.

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
On the supported ThinkPad, AMD graphics is loaded in the initrd so the panel
stays active while Xen hands display control to dom0.
Do not manually start sys-gui until `airlock-setup-config.service` completes:
its kernel preference is applied after storage preparation. Starting it early
can misleadingly report a missing `pvgrub2-pvh/vmlinuz` even though the intended
Qubes kernel is already installed. The detailed setup log is
`/var/log/airlock/setup-config.commands.log` and the result is
`/var/log/airlock/setup-config.json`.

Both USB graphics modes configure the guest login as `user` / `password`.
Dom0's local console uses `airlock` and the password supplied during
personalization (`password` with `--insecure-dev-password`). The development USB
also supplies **dom0 Terminal (Development)** in sys-gui's applications menu
(or run `airlock-dom0-terminal` from a guest terminal). It opens a shell as
dom0's `airlock` account; `sudo` uses the dom0 password. In that terminal:

```sh
sudo nmcli --ask device wifi connect YOUR_SSID
sudo tailscale up --ssh --hostname=airlock-usb
cd /home/airlock/dev/nixome
codex
```

Complete the Tailscale authentication link on another connected device if
needed. This works from the physical AMD lab without a recovery boot once its
desktop/input works. The ordinary guest terminal is still a sys-gui shell.

The launcher uses a byte stream over [Qubes qrexec](https://github.com/QubesOS/qubes-doc/blob/main/developer/services/qrexec.rst),
not a dom0 X server, SSH listener or general `qubes.VMShell` permission. The
dedicated `airlock.DevDom0Shell` RPC allows only sys-gui with no argument. Its
fixed root handler verifies the source then drops to `airlock` before opening
a PTY. No passwordless sudo is added. **This grants any process in sys-gui
access to the dom0 user account and its files, including development secrets.**
It is an intentional development exception, not a secure administrative
boundary against a compromised sys-gui; both policy and launcher are absent
from normal images. A host assertion rejects enabling it outside dev mode.

The terminal starts at 100 columns by 30 rows. Automatic remote resize is not
implemented; use `stty rows N cols M` in dom0 if you resize the window. Exit
disconnects the shell; the window stays open so its final messages remain
readable and can be closed manually. This is not a fallback for failed guest/display startup:
retain text recovery and provisioned remote access for those failures.

Launcher verification (2026-09-14): 33 focused pytest checks passed, with one
optional SSH check skipped; the handler, policy and menu launcher built.
Local PTY transport checks cover an interactive shell, hidden password input,
Ctrl-C and terminal restoration. They substitute a local byte-stream relay
for qrexec and do not establish end-to-end Qubes policy/PAM behavior on the
laptop. That fresh-image check remains pending. Evidence is under
`.airlock-logs/dev-dom0-terminal/`.

### Diagnosing a blank dom0 terminal

The first laptop attempt opened a blank window and returned exit code 1.
That did not establish whether the failure was in policy, the handler, PAM,
or shell startup. The launcher now holds the window open and prints the
connection exit code after restoring terminal settings. Dom0's handler merges
its stderr into the forwarded stdout stream: Qubes' dom0 service execution
path otherwise leaves that stderr on the host, unlike its VM-side transport
([upstream implementation](https://github.com/QubesOS/qubes-core-qrexec/blob/v4.4.1/daemon/qrexec-daemon-common.c)).

The window reports handler entry, caller/argument rejection when applicable,
the attempt to start the unprivileged session, and entry into the remote PTY.
Numeric UIDs help detect an execution-user mismatch. These fixed stage messages
also go to dom0's journal with tag `airlock-dev-dom0-terminal`. Commands,
passwords, environment contents and the interactive stream are **not** logged.
Permissions and authentication are unchanged.

Rebuild/flash, open **dom0 Terminal (Development)**, and record the visible
messages. If no `handler reached` message appears, inspect dom0's qrexec/policy
logs. With dom0 access, use:

```sh
sudo journalctl -b -t airlock-dev-dom0-terminal --no-pager
sudo journalctl -b -u qubes-qrexec-policy-daemon --no-pager
```

Diagnostic verification: seven focused pytest checks passed, including a
simulated startup stderr failure and terminal restoration on exit code 1;
launcher package built. Actual laptop shell startup is still unverified.
Evidence is under `.airlock-logs/dev-dom0-terminal-diagnostics/`.

The XFCE panel's **Shut Down** action requests a clean dom0 power-off through
a single qrexec service restricted to `sys-gui`; it does not merely halt the
GUI VM. Log out and reboot remain guest-session actions for now.

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
The image has approximately 31 GiB of root filesystem capacity. The prepared
guest store eliminates initial admission's temporary copies, but local builds
and VM updates can still fill the disk. Use the guarded `make gc-auto` with
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
