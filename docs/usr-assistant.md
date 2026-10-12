# Computer assistant

`usr-assistant` is the user's workspace for assistance with their computer.
Its purpose covers inspection, configuration, maintenance, everyday automation,
and development. Coding is one use case, rather than the definition of the VM.

## Current interface

The current interface is Codex in **usr-assistant: Console**, displayed in
sys-gui. The VM includes a persistent workspace, the machine checkout, and
command-line tools. There is no separate graphical assistant application yet.

Ask the assistant to inspect a problem, explain the relevant state, propose a
configuration change, or carry out an authorized task. For example:

- “Why can this application not reach the network?”
- “Help me create a VM for this project.”
- “What changed in the last machine update?”
- “Inspect the rollback options before changing anything.”

The longer-term direction is conversation as the main entry point for operating
the computer, with selectors, previews, and progress views where useful.
Replacing every settings interface is not implemented by this rename.

## Responsibilities and access

The assistant's conversation, credentials, tools, and workspace live in
usr-assistant. Usr-home owns everyday applications and personal files. Sys-gui
owns the desktop and displays windows from both VMs.

Sys-ctrl validates and builds committed machine source; dom0 retains Qubes
policy, VM lifecycle, storage, and activation authority. The assistant requests
changes through those existing interfaces. Nix remains the source of truth,
and deployment records distinguish a submitted change from a verified machine.

The name does not give the assistant unrestricted access to other VMs' files
or credentials. Access still follows the configured Qubes mechanisms and
permissions. The current development configuration also explicitly enables
remote access and a privileged dom0 shell; see [development access](usb-lab.md).

## Start and maintain the workspace

Open **usr-assistant: Console** from GNOME, enter the persistent machine
checkout, and start Codex. Authentication belongs in the assistant VM's private
state and must not be included in Nix configuration or source archives.

The [installed workspace guide](../airlock-machine/docs/usr-assistant.md)
contains login, source, tool, and deployment instructions. The
[assistant build store](usr-assistant-build-store.md) documents its persistent
builder. Read [machine deployment](machine-deployment.md) before changing a
running machine.

## Existing machines

The previous VM name was `sys-agent`. Current declarations and commands use
`usr-assistant`; dated reports retain the names observed during their trials.
Changing the source does not rename an already running VM.

Use the [rename migration](usr-assistant-rename.md) to copy the stopped private
volume through Qubes and retain the old VM for recovery. The migration is an
explicit transition release, separate from ordinary releases and fresh images.
