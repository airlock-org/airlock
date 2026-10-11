# Earlier USB first-boot notes

Historical profile-specific notes moved on 2026-10-11. They include direct
dom0 networking and older GUI assumptions. Follow [USB development](../usb-dev.md)
and [the normal lab topology](../usb-lab.md) for current setup.

## Earlier first boot

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
Dom0's local console uses `user` and the password supplied during
personalization (`password` with `--insecure-dev-password`). The development USB
also supplies **dom0 Terminal (Development)** in sys-gui's applications menu
(or run `airlock-dom0-terminal` from a guest terminal). It opens a shell as
dom0's `user` account; `sudo` uses the dom0 password. In that terminal:

```sh
sudo nmcli --ask device wifi connect YOUR_SSID
sudo tailscale up --ssh --hostname=airlock-usb
cd /home/user/dev/airlock
codex
```

Complete the Tailscale authentication link on another connected device if
needed. This works from the physical AMD lab without a recovery boot once its
desktop/input works. The ordinary guest terminal is still a sys-gui shell.

The boot menu also provides `Airlock (recovery)`: a Xen text console that skips
topology setup, all guest startup and GPU handoff. It starts offline. Log in as
`user`, then run `sudo airlock-recovery-network` to start NetworkManager and
Tailscale SSH in dom0. With no saved Wi-Fi connection, use the `nmcli --ask`
command printed by the launcher and invoke it again. `Airlock` remains the
default latest generation; older normal generations remain directly selectable.

The launcher uses a byte stream over [Qubes qrexec](https://github.com/QubesOS/qubes-doc/blob/main/developer/services/qrexec.rst),
not a dom0 X server, SSH listener or general `qubes.VMShell` permission. The
dedicated `airlock.DevDom0Shell` RPC allows only sys-gui with no argument. Its
handler verifies the source and requires Qubes to invoke it as the declared
dom0 `user` account before opening a PTY. No passwordless sudo is added. **This grants any process in sys-gui
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
the attempt to start the dom0 user session, and entry into the remote PTY.
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

### Direct dom0-user execution — 2026-09-14

The first diagnostic image showed the handler ran as UID 1000 and then failed:
`runuser: may not be used by non-root user`. On that image, UID 1000 belonged
to dom0's `airlock` account, so the extra `runuser` was unnecessary and failed.
The service now expects the configured dom0 user and opens `script`'s PTY
directly. The policy no longer requests `user=root`. `sudo` continues to
prompt for the dom0 password. This is still a development-only capability.

35 focused checks passed, with one optional check skipped, including the
ordinary-user startup path and error forwarding. The launcher package build
and laptop retry remain pending.

The XFCE panel's **Shut Down** action requests a clean dom0 power-off through
a single qrexec service restricted to `sys-gui`; it does not merely halt the
GUI VM. Log out and reboot remain guest-session actions for now.

Wired Ethernet can simplify first boot. Join Tailscale interactively; no auth
key is in the image. This is a **new Tailscale node**, not the laptop's Ubuntu
identity. Use its reported address from the host:
`ssh user@NEW_USB_TAILSCALE_ADDRESS`. Verify the new SSH host key at the
laptop console. SSH password/root login and non-Tailscale port 22 are disabled.
Do not disable host-key verification to reuse the Ubuntu identity.

The desktop's Tailscale launcher checks the route, DNS and HTTPS connectivity
from sys-agent before requesting login. Failed checks print a specific error;
interactive login has a two-minute timeout and can be retried. Offline use does
not require passing this check to reach GNOME. Network-provider startup checks
the firewall and forwarding services as well as qrexec readiness.

Following an observed sys-net out-of-memory failure, sys-net starts with 1 GiB
RAM and sys-firewall with 512 MiB (maximum 1 GiB). Both restart their Qubes
firewall updater after failure. Qubes owns DNS configuration in guests with a
Qubes uplink; sys-net retains NetworkManager/resolvconf and includes
redistributable hardware firmware. These changes still require physical
verification on a rebuilt image.

Keep physical custody of this development USB. Tailscale's machine identity,
Wi-Fi settings, logs, repository and VM state persist on the **unencrypted** root.

For builds on the USB, keep the custom store off the host store, for example:
`AIRLOCK_DATA_ROOT=/home/user/airlock-data make usb-dev-image`.
The image has approximately 31 GiB of root filesystem capacity. The prepared
guest store eliminates initial admission's temporary copies, but local builds
and VM updates can still fill the disk. Use the guarded `make gc-auto` with
that same data root. No full-store garbage collection is done automatically.

## Evidence and next experiment

Store notes/logs under the repo's `.airlock-logs/usb-dev/`. The image contains the
[normal Airlock configuration](../usb-lab.md) and text recovery. Its prepared pool
includes `sys-gui` (GNOME), `sys-agent`, `sys-ctrl` routed through sys-firewall,
`usr-home`, and `usr-browser`.
Standalone GUI and hybrid GUI configurations remain available as source builders
but are excluded from the USB image, including their guest stores.
See [the historical GPU experiment](usb-gui-lab.md) for earlier validation work.
