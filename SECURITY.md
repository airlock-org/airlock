# Security reporting

Airlock is experimental. No supported production release or Qubes OS security
equivalence is claimed. Security fixes currently target the main development
branch; older lab snapshots are retained as evidence, not supported releases.

## Sensitive reports

When GitHub private vulnerability reporting is enabled, use
[Report a vulnerability](https://github.com/airlockos-org/airlockos/security/advisories/new).
Repository preparation does not enable this service. Before it is available,
request a private reporting channel from the maintainer without posting exploit
details, credentials, or sensitive machine information in a public issue.
A working private channel must be established before opening the repository to
security reports. No response-time commitment is made at this stage.

Include the source revision, affected component, prerequisites, reproduction,
impact, and whether the result came from nested tests or physical hardware.
Use sanitized logs. Ordinary non-sensitive bugs can use the issue tracker once
available. For upstream Qubes, Xen, or other dependency vulnerabilities, follow
the relevant upstream's reporting process as well.
