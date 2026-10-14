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
the filter restores the saved expansion state. Previous/Next and Up/Down when
the tree has focus navigate matching files and reveal the selected path.

Drag across source text to select within either diff column; Shift-click extends
the selection, double-click selects a word and triple-click selects a line.
Ctrl+C copies the selection and Ctrl+A selects that side's complete file.
Right-click opens copy controls for the selection, line, whole file or path.
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
