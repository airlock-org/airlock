# Diagnostics

This component carries read-only evidence across existing Qubes RPC boundaries.
Machine configuration freezes the observer, endpoint inventories and exact Qubes
permissions together. Components can register additional fixed sources beside
their configuration; clients select source identities, never commands or paths.

Each request passes a separate approval boundary. The initial installed policy
automatically approves requests by the configured observer and records that fact;
it does not manufacture human approval or change deployment permissions. A future
review interface can decide the same complete, source-bound request. Automatic
access includes raw logs, which can disclose secrets or personal information.
There is no redaction and no claim that an allowed source is secret-free.

Verus checks the caller, target, complete source descriptor, decision binding and
output ceiling, together with shared stream-budget accounting. Qubes identity,
installed-contract integrity, strict JSON decoding, canonical query encoding,
audit delivery and faithful readers remain trusted.
The short-lived endpoint has bounded input, concurrent slots, command deadlines
and a combined evidence budget. Fixed-file readers reject unexpected owners,
hard links and symlinks; component declarations select the owning account.
The endpoint neither starts stopped VMs nor executes
caller-supplied commands. Retained guest console logs are available from dom0
when guest RPC is unavailable; their contents can span earlier guest boots.
Qubes cannot connect a VM to itself, so a separate dom0 endpoint relays only back
to its authenticated observer. That verified target check cannot select another VM;
the observer endpoint then performs its ordinary source and approval checks with
Qubes-authenticated dom0 as caller. The relay and reader each record access metadata.

Evidence is untrusted data, including instructions embedded by a compromised VM.
The response identifies the endpoint and its kernel boot, not the truthfulness
of the application's messages. Successful observation grants no repair authority.
