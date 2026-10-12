# Local public-readiness preparation

Local preparation recorded on 2026-10-11. At that point, changes were local.
Neither repository visibility nor
hosting settings changed; no commits, images, artifacts, or releases were
uploaded, and no live machine was deployed.

## Implemented

- Original Airlock code in both repositories has an MIT license; upstream
  source/patch terms and the unresolved vendored-source question are documented.
- Codex credential copying, vaults, wrappers, sync targets, and import RPC/policy
  are removed. Codex remains an application. Existing runtime credentials are
  not erased by these source changes. Tailscale setup/enrollment is unchanged.
- USB provisioning prompts for a dom0 password by default and accepts an explicit
  development-password opt-in. The SSH public-key path is configurable.
- The component URL and remote clone examples use SSH:
  `git@github.com:airlock-org/machine.git` and
  `git@github.com:airlock-org/airlock.git`. Local-source overrides remain documented.
- The default lab hardware remains intact; `hardwareModules` can replace it,
  and physical GUI assignment derives the PCI port from that hardware declaration.
- READMEs, contributor setup, security reporting, status, and lab-configuration
  guides are in place. Dated deployment/first-boot notes moved into history;
  stale network topology and broken source links were corrected.
- Known personal lab addresses, USB serials, and developer checkout paths were
  replaced with placeholders in current documentation. Hardware inventories
  and evidence boundaries remain concrete. Git history was not rewritten.
- Quick tests explicitly use prepared dependencies (`uv run --no-sync`). Both
  repos have CI workflows with pinned actions and retained JUnit/JSON reports.
- Deployment cleanup preserves startup errors while attempting every unmount;
  saved deployment records are durable; host publication finishing is shared.
  The local test runner also no longer duplicates its pytest arguments.

## Validation

The parent quick suite passed **878 tests, 59 skipped**; the component quick suite
passed **785 tests, 34 skipped**. The focused deployment suite passed **70 tests**
with 5 Nix tests deselected. A focused Nix test proved replacement hardware drives
the machine GPU assignment. Parsing changed Nix modules and evaluating the parent
library succeeded. No full Nix/USB build or hardware acceptance was performed.

A fresh `git clone --no-local` of parent commit `5b6070d`, initialized from the
local component source at `20bedc2`, installed locked dependencies from the local
cache and passed **878 tests, 59 skipped**. Its working tree stayed clean. A Nix
tripwire recorded only the host-capability `--version` probe; there were **no Nix
evaluations or builds**. This exercised the documented local-source override
while keeping the tracked SSH URLs intact. The clone was removed after checking.

Both CI workflows passed actionlint v1.7.12. A local documentation check
resolved 580 relative link targets with no missing files. Hosted CI is not claimed
as run; remote availability and live deployment remain unverified.

## Source/history audit

Checksum-verified Gitleaks **v8.30.1** scanned all available local refs after the
implementation commits: **929 parent commits** and **764 component commits**,
with no detected secrets. This covers available local history, not unknown remote
refs or ignored runtime files. Gitleaks is a pattern-based check, not proof that
all sensitive material is absent. Both repositories retain two author-email
identities in their history. Historical paths/identifiers remain in old commits.
No tracked binary captures were found in the inspected source snapshots.

Redacted reports and raw check logs are retained under
`.airlock-logs/public-readiness/`; they are intentionally ignored by Git.
You can repeat an audit with an installed Gitleaks:

```sh
gitleaks git . --log-opts='--all' --redact --report-format=json \
  --report-path=.airlock-logs/parent-history-secrets.json
gitleaks git airlock-machine --log-opts='--all' --redact --report-format=json \
  --report-path=.airlock-logs/machine-history-secrets.json
```

## Remaining publication prerequisites

1. Clarify licensing permission for the vendored `CertainLach/nixos-qubes` modules
   and packaging. No license grant was established from the local snapshot or
   upstream license endpoint. Airlock's MIT license does not resolve this.
2. Make both intended SSH remotes and the pinned component commit accessible
   when publication is explicitly authorized. A local-source clone only proves
   local reproducibility; it does not establish remote availability.
3. Enable GitHub private vulnerability reporting, or provide a working private
   contact, before inviting security reports. The local policy documents this
   prerequisite; no external reporting settings were changed.

Further hardware acceptance and existing feature plans remain separate from this
source-preparation work. See [status](status.md) and [the docs index](README.md).

## Authorized commit transfer (2026-10-12)

The user subsequently authorized pushing commits to the existing repositories.
The component's actual SSH remote is `git@github.com:airlock-org/machine.git`;
the local source directory remains `airlock-machine`. The parent uses
`git@github.com:airlock-org/airlock.git`. The submodule URL, instructions, and
component security-reporting link now match these existing destinations.
Repository visibility and hosting settings remain outside this authorization.
