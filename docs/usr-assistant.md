# Computer assistant

The computer assistant helps you inspect, configure, and maintain your computer.
It runs in `usr-assistant`, a dedicated VM with its own persistent workspace,
credentials, and tools.

## Open the assistant

Open **usr-assistant: Console** from GNOME and start Codex in the terminal.
The current interface is a command-line conversation. Complete authentication
inside this VM; credentials belong in its private storage.

## Things to ask

Start with a question or describe what you want to accomplish:

- “Explain which domains are running and what each one does.”
- “Why can this application not reach the network?”
- “Help me set up a workspace for this project.”
- “What changed in the last machine update?”
- “Show me the rollback options before changing anything.”

You can ask for an explanation or a proposed plan before authorizing changes.
The assistant can also help with scripts, development, and everyday automation.

## How changes happen

System configuration lives in Nix. The assistant can help prepare that
configuration and submit it through the machine's build and deployment tools.
Sys-ctrl builds releases; dom0 manages activation and the Qubes lifecycle.

See [Configuration](configuration.md) and [Updates and rollback](updates.md)
for the steps involved in changing the system.

## Workspace and access

The assistant's workspace persists across system updates. Your everyday
applications and files live separately in `usr-home`, with both VMs' windows
shown by the same desktop.

Running the assistant in its own VM does not grant it access to every domain's
files or credentials. Access follows the configured Qubes permissions. The
current development configuration includes a privileged dom0 shell, so tasks
using that access can affect the whole machine. See [Security](security.md).
