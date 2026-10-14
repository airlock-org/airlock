# Desktop and applications

AirlockOS uses GNOME as a shared desktop for applications running in separate
virtual machines. You can work with their windows together while the processes
and files remain in their own domains.

## Open an application

Use GNOME's application overview to find and launch applications. Guest
launchers identify their VM, such as **usr-assistant: Console**. Opening an
application starts its domain when needed.

Everyday applications, including Firefox, run in `usr-home`. Files you create
there belong to that VM's private storage and persist across system updates.

## Know which domain you are using

The panel's focused-VM indicator identifies the domain of the active application
window. Check it when switching between personal work and the assistant.

| Domain | What you use it for |
| --- | --- |
| `usr-home` | Everyday applications and personal files. |
| `usr-assistant` | The computer assistant, its tools, and its workspace. |
| `sys-gui` | The desktop that displays application windows. |

Sys-gui handles the desktop and display. Application processes and their data
stay in their originating VMs. See the [GUI topology](architecture.md#gui-and-input).

## Work with the assistant

Open **usr-assistant: Console** to start a conversation about your computer.
The [computer assistant guide](usr-assistant.md) includes suggested tasks and
explains how its workspace relates to your personal applications.

## Desktop settings

Display and desktop settings belong to sys-gui. Applications in other VMs have
their own settings, so appearance and scaling changes may need configuration in
more than one domain.

For system-level changes, use the [Nix configuration](configuration.md) so the
settings are retained in future releases.
