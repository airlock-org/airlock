# airlock-ctrl

Airlock Control is the user-facing interface for sys-ctrl's configuration and
deployment responsibilities. This first version is a local Rust/egui app with
source review, deployment-plan inspection and local approval/rejection records.
It uses `similar` for line and inline diffs.

The native theme follows the sibling landing page's `app/globals.css`: deep
navy background (`#071017`), blue-grey cards (`#0a141d`), pale text (`#d5e4f6`),
muted text (`#a4b8cd`), borders (`#253b4d`) and orange accents (`#ff5832`).
Interface text uses bundled Inter regular/medium/semibold instances from the
website's font; code remains monospace, with green/red diff colours.
Theme tokens live in `src/theme.rs`; font provenance and regeneration are
documented in [assets/fonts/](assets/fonts/README.md).

Window decorations are separate from the app theme: the desktop supplies the
title bar and window controls where supported. On Wayland desktops that require
client decorations, the build enables winit's standard Adwaita title bar in its
automatic light/dark mode. Airlock does not draw or colour the window controls.

## Local preview

From the repository root, `make ctrl-preview` opens a built-in example using
the Nix package's native libraries. The window is labelled **LOCAL PREVIEW**;
its example decisions stay in memory. No VM, machine service or deployment
client is contacted. You can also run the built binary with `--demo` where
the GUI libraries are available.

## Review committed source

```sh
apps/target/debug/airlock-ctrl \
  --repo airlock-machine \
  --base <baseline-commit> \
  --candidate <candidate-commit> \
  --plan /path/to/saved-plan.json \
  --decisions-dir .airlock-logs/airlock-ctrl/decisions
```

On NixOS, launch through the package wrapper instead:

```sh
./lifecycle/misc/tools/scripts/airlock-nix.sh run ./apps#airlock-ctrl --no-write-lock-file -- \
  --repo airlock-machine --base <baseline-commit> --candidate <candidate-commit>
```

The baseline is explicit: a parent Git commit is not necessarily the deployed
configuration. Refs are resolved once to immutable commits. The viewer reads
Git trees and blobs directly, includes every changed path (Nix, lock files,
scripts, additions, deletions, submodule identities and file modes), and never
checks out source, invokes Git diff filters, or follows candidate symlinks.
Uncommitted working-tree changes are excluded. Non-UTF-8 paths fail the review
rather than being silently omitted. Binary/non-UTF-8 blobs and files above
1 MiB are listed with their object identities and disable approval.

Changed files appear in a collapsible folder tree, with compact directory
chains, single-line filenames, change markers and aligned addition/deletion
counts. Full paths are available on hover. Search matches path terms without
case sensitivity and keeps matching files in their folder context; clearing
the filter restores the saved expansion state. The up/down icon buttons and Up/Down when
the tree has focus navigate matching files and reveal the selected path.
Tree expansion, collapse, navigation, filter clearing and column menus use
vector icons with descriptive tooltips and keyboard-accessible buttons.

The change title appears above the main tabs. Beside Approve/Reject, the effects
summary names the VMs that will restart and states whether a system reboot is
required. VMs present in both stop/start lists are restarts; start-only and
stop-only VMs appear separately, without assuming they are newly created or
permanently stopped. Names are deduplicated and sorted for display. A missing
plan or unreported reboot flag remains unknown. Right-click the reboot status
to copy the complete effects summary. Commit hashes are in Review record; the
sidebar is dedicated to finding and navigating changed files.
The footer keeps the effects summary and decision controls together. The header's
information icon opens About Airlock Control, explaining plan provenance, the
local-only review mode and receipt storage. The dialog has a centred app identity
and version, with separate sections explaining the current plan and decision
storage rather than describing configuration options. Close it with its Close
button, Escape or a click outside it.

Review record groups source revisions and content identities into cards.
Each available value has a copy icon that copies the complete value
and briefly changes to a green checkmark. Missing values have disabled copy
controls. Cards stack on smaller windows.

Source diffs use continuous 22-pixel rows with centred line numbers and change
markers. Syntax highlighting uses syntect with bat's syntax definitions (via
two-face), including Nix and JSON for `flake.lock`. Each side is highlighted
independently, preserving multiline string/comment state through diff alignment
gaps. Unknown file types use plain text. Red/green row backgrounds and stronger
inline-change backgrounds remain visible alongside syntax colours.

Drag across source text to select within either diff column; Shift-click extends
the selection, double-click selects a word and triple-click selects a line.
Ctrl+C copies the selection and Ctrl+A selects that side's complete file.
Right-click opens copy controls for the selection, line, whole file or path.
The Copy review icon beside the tabs is available in every tab. It copies a
Markdown report with the change title, review/commit/lock identities, effects
summary, all changed-file metadata, full supplied deployment JSON (including any
response wrapper) and the complete source patch. Missing plans and unknown effects
are explicit. Selection and search filters do not limit the report. The report is
captured when the window opens; later branch, worktree or plan-file edits do not
change it. JSON and patch sections use code fences that adapt to embedded fences.
Extract the Source patch block to use with `git apply`; it preserves modes,
additions/deletions, submodule revisions and binary patches. External diff and
text-conversion helpers are disabled. Copying writes no exported file, and the
icon briefly becomes a green checkmark. `--print-review` prints the same report
to stdout; `--print-patch` remains available for patch-only offline tooling.
Non-UTF-8 text patches cannot be copied losslessly; the disabled button's tooltip
explains the failure rather than replacing bytes or copying a partial review.
Each column's `⋯` menu offers selection, file-content and path copy actions.
Copied source preserves the original tabs, Unicode and newlines and excludes
line numbers, the other column and empty diff alignment rows. The source stays
read-only. File-tree rows have path/filename and whole-file copy menus; plan
JSON and review details support text selection and right-click copy controls.
The editable file search also provides cut and paste in its context menu.

`--plan` accepts a saved plan object or a receiver response containing `plan`.
The plan must contain `changed`, `stop` and `start` arrays of domain names.
Known optional domain arrays are also checked. Supplied revision and lock
identities must match the resolved candidate. The UI shows restart order,
retirement, restoration, `host_reboot_required` and the complete supplied JSON,
including policy/device details that are not covered by summary fields.
The plan view leads with named restarts and system reboot status, then other VM
effects and configuration changes, using compact label/value rows without fixed
heights or card borders. A small reboot badge sits beside the restart summary;
on narrow windows it moves above the rows. VM names wrap and remain selectable
with copy menus. Empty categories show None; missing optional lists show Not
reported. Expand
"Technical details" to inspect stop/start order and copy the complete supplied
JSON. The displayed summary does not modify the supplied plan or its fingerprint.
An absent plan allows source review and rejection but disables approval.

Review the source and effects, then click Approve or Reject to open the decision
confirmation. Without `--decisions-dir`, a local decision stays in memory.
With it, the app atomically writes a private receipt named after the review's
SHA-256 fingerprint; existing decisions cannot be overwritten. The fingerprint
binds source commits, blob identities, file modes, displayed content, lock hash
and the supplied plan. The record uses the distinct
`airlock.ctrl-local-decision.v1` schema and `local-review-only` authority.

**These receipts are not deployment approvals.** Plans are imported local
files, not authenticated observations of machine state. There is no qrexec
connection, deployment action, machine module installation or policy change
in this version. Live integration must require approval of the exact prepared
candidate, check staleness in dom0, and prevent submission clients from granting
human approval. Existing typed recovery operations must remain available when
sys-ctrl is stopped.

## Inspect and check

```sh
make ctrl-build
make ctrl-check
apps/target/debug/airlock-ctrl --demo --inspect
apps/target/debug/airlock-ctrl --repo airlock-machine --base HEAD~1 --inspect
AIRLOCK_CTRL_BIN="$PWD/apps/target/debug/airlock-ctrl" make test-quick
```

`--inspect` prints the review JSON without a display server. For offline tools,
`--record-decision approve|reject` records the same local-only receipt without
opening a window; it requires `--decisions-dir` and committed repository input.
It cannot write demo decisions or initiate deployment.

`--screenshot /absolute/path/preview.png` renders a PNG and exits. Use a separate
display such as Xvfb for visual checks that should not open a desktop window.
The screenshot command does not create review decisions.

The standalone `apps/flake.nix` exports `packages.x86_64-linux.airlock-ctrl`.
It packages the workspace with its committed Cargo lock and wraps the native
libraries and Git. Use `./apps#airlock-ctrl` for local builds/previews; the app
flake explicitly disables submodule fetching and needs only its own pinned
nixpkgs input. The parent flake
retains its app package export for integration; that entry point also loads
machine inputs and can require fetching the pinned machine Git submodule.
Future Rust apps can join the workspace; the diff view stays an internal module
until another app has a concrete need for a shared crate.
