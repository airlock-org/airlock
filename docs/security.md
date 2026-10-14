# Security

AirlockOS uses the Qubes architecture to separate applications and system roles
into domains. Xen provides virtualization; Qubes controls VM lifecycle, device
ownership, inter-domain communication, and policy.

## Domain boundaries

Applications run in their own domains with separate private storage. Qubes
policy controls inter-VM RPC through qrexec. A shared global Nix store provides
read-only system closures to guests; it does not expose other guests' private
files.

Dom0 administers the machine. Its permissions and the domains trusted to request
administrative operations are central to the system's security.

## Current development configuration

AirlockOS is under active development and has not established security
equivalence with Qubes OS. The current development configuration permits a
privileged dom0 shell from `usr-assistant`. An assistant task using that access
can affect the entire machine.

Take this access into account when deciding what data and tasks to place on an
AirlockOS system.

## Report a security issue

Request a private reporting channel from the maintainer before sharing sensitive
details. Include the affected component, system version, and steps to reproduce.
Keep credentials and private machine data out of public reports.
