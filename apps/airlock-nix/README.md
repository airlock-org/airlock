# airlock-nix

The Rust home for Airlock's Nix subsystem: configuration sources, builds, store
contents, generations, activation and recovery. This first version provides
typed foundations and a local, read-only record inspector. It does not implement
Nix execution, qrexec transport, worker scheduling or deployment yet.

The existing Python backend in `airlock-machine` remains authoritative and
unchanged. No services, installed packages, VM permissions or deployment routes
are switched to this crate. `airlock-ctrl` remains the separate review GUI.

## Build and inspect

From the monorepo root:

```sh
make airlock-nix-build
make airlock-nix-check
apps/target/release/airlock-nix --help
apps/target/release/airlock-nix inspect-job apps/airlock-nix/examples/job.json
apps/target/release/airlock-nix inspect-review apps/airlock-nix/examples/review.json
apps/target/release/airlock-nix inspect-response apps/airlock-nix/examples/response.json
apps/target/release/airlock-nix inspect-plan apps/airlock-nix/examples/plan.json
```

The example job has an approved decision followed by a failed build. Inspection
shows both, demonstrating why submission feedback must remain separate from the
worker's latest status. The other examples describe one guest-source review and
a matching deployment plan. These files are sample data, not authorizations.

`inspect-job FILE --request-id ID` also verifies the expected 32-character job
identity. All record inspection commands read only the explicit local file, print JSON
and exit; they never contact a VM, modify a record or execute its contents.
Only regular files are accepted; final-component symlinks, FIFOs, devices and
directories are rejected. Job/plan inputs are bounded to 4 MiB, review inputs
to 64 KiB. Duplicate JSON keys are rejected throughout the record, including
nested metadata. Unknown fields in
job observations and plan metadata are retained. Approval request/reply schemas
reject unknown fields and versions. `inspect-plan` accepts the bare plan, not a
transport response containing one. Output is a normalized inspection, not a
byte-for-byte export of the source record.

Successful output uses `airlock.nix-inspection.v1` and explicitly labels its
authority `local-inspection-only`. Invalid records produce
`airlock.nix-error.v1`, a diagnostic and exit status 1. CLI argument errors use
Clap's normal diagnostics and exit status 2. No record is skipped or repaired.
Failures retain typed `details`: record paths and JSON locations, I/O operation
and OS error number, or scope/publication context as applicable. Diagnostic paths
have a `display` string and include `raw_bytes` for non-UTF-8 names, preserving
the filename's identity without making error serialization fail.
Inspection's exit status describes whether the record is valid. Job inspection
also reports `submission_exit_code`: approval returns 0 even after a later work
failure, pending decisions return null, and stopped requests retain their
rejection/cancellation/failure code.

The standalone apps flake exports `./apps#airlock-nix`. It builds only this
executable in release mode and requires no GUI libraries. It does not install
the application into any machine configuration.

## Types and boundaries

- `identity`: distinct, validated `JobId` (32 hex), `ReviewId` (64 hex),
  `GitRevision` (40 hex), `LockHash`, `ReviewFingerprint`,
  `BaselineFingerprint`, `VmName` and `GuestName`. VM names follow dom0's guest
  admission grammar: 1–31 ASCII characters, starting with a letter, followed by
  letters, digits, underscores, dots or hyphens. `VmName` can represent dom0;
  `GuestName` excludes it, so a guest deployment cannot select the host. JSON
  decoding uses the same validation as Rust constructors. Git revisions follow
  the existing SHA-1 protocol; moving
  to another Git object format will need an explicit protocol change.
- `review`: existing `airlock.ctrl-approval-{request,response}.v1` wire formats,
  a source-and-target `CandidateIdentity`, a machine-or-guest target enum,
  a normalized nonempty VM scope, typed human
  decisions, and consuming workflow transitions. A machine request cannot carry
  a guest target; a guest request must name one.
- `job`: exhaustive job statuses, distinct job/authorization IDs, and a durable
  approval decision separate from work progress. Internal states carry the
  authorization ID wherever required. Feedback uses separate `PendingStatus`,
  `ApprovedStatus` and `StopReason` types, making contradictory categories
  unconstructible; `continuing` is computed instead of supplied by a caller.
  The existing JSON status/decision/feedback format remains unchanged.
  Building/planned/built/handoff
  snapshots require recorded approval and its authorization ID. These are
  current reviewed-controller records, not arbitrary legacy deployment logs.
  Build completion and deployment handoff never claim successful activation.
  Additional request/result metadata is retained as untrusted data; inspection
  does not promote it into a validated submission or deployment capability.
- `state`: bounded JSON reads; a trusted state store with separate record and
  lock namespaces; resource-bound writers with snapshot conflict detection; and
  nonblocking kernel locks with verified owner observations. Publication uses
  explicit permissions, unique temporary files, file fsync, rename and directory
  fsync. There is no public publisher accepting an arbitrary path.
- `error`: typed failures for source/baseline mismatches, scope violations,
  invalid records, lock contention, serialization and I/O, converted to JSON at
  the CLI boundary without losing paths, OS errors or affected VM names. Scope
  failures retain a typed list of names rather than a comma-separated string.
  Publication followed by failed directory fsync has a distinct
  `PublishedButNotDurable` error; callers must not assume the old record remains.

The review workflow is:

```text
ReviewRequest + observed source/target/deployed baseline
    → PendingReview
    → Reject / Cancelled / ApprovedChange
    → PreparedPlan (complete build-source/target identity + matching plan)
    → recheck candidate and baseline; check the resulting plan's scope
    → ApprovedDeployment
```

`ApprovedChange` and `ApprovedDeployment` have private fields and cannot be
deserialized. Only an approving, identity-matched response can produce the
first; only it can produce the second. `PreparedPlan` requires an explicit
candidate identity from the plan producer. Binding compares the entire source
and target against approval, requires the matching plan revision and checks any
supplied plan lock hash. Plan data and metadata are exposed through read-only
accessors, so callers cannot inject metadata that shadows checked fields during
serialization. Scope checks include all
known VM effect lists and both private-volume copy endpoints. An unknown reboot
requirement is not treated as “not required”; legacy scopes that disallow reboot
still fail closed. This adds no second reboot permission step.

These types prove local consistency checks ran. They do **not** authenticate
input, grant runtime authority or prove a build/activation succeeded. Future
runtime adapters must obtain decisions through the trusted GUI route, enforce
dom0's protected approval records, bind actual build outputs to source/target,
and recheck baseline/source under the appropriate lock immediately before work.
Qubes qrexec and policy remain the authority boundaries. Byte-oriented adapters
must use `state::parse_json` (after bounding their input) to reject duplicate
keys before Serde or metadata maps can discard them. `state::read_json` already
uses this parser.

`StateStore::initialize(root)` explicitly creates and syncs `records` and `.locks`
inside an existing trusted root. `StateStore::open(root)` only opens that layout;
it never initializes or repairs it. Root and child directories reject symlinks
and group/other write permission. Their identities are retained and checked.
The caller must trust the directory owner and all ancestors, keep the namespace
stable, and use one schema/limit per resource. These primitives do not defend
against an equally privileged process deliberately modifying their files.

A validated `ResourceKey` selects `records/KEY.json`. Record writer locks live
at `.locks/record-KEY.lock`; long-running operations use
`.locks/operation-KEY.lock`. Neither keys nor public lock APIs accept arbitrary
paths. A record writer cannot select a lock file as its output. Acquiring an
operation lock does not block unrelated record writes or observations.

`Record::read` returns a lock-free `RecordSnapshot<T>`, including absence for a
new record. To publish, acquire `Record::try_write(owner)` and call
`writer.commit(&snapshot, &value, permissions)`. The writer checks its matching
lock, resource and exact current file version before serialization and again
immediately before rename. Snapshots retain their file descriptor to prevent
inode reuse from making an old snapshot current. A stale snapshot returns
`ConcurrentUpdate`; a snapshot from another resource returns `BindingMismatch`.
After conflict, reread and reconsider the update rather than blindly replaying
old data against a new snapshot. Snapshots are observations, not evidence of
source approval; domain workflow checks are still required.

Temporary files stay private until fully serialized and validated for unique
JSON keys. Serialization stops before output would exceed the byte limit,
including the final newline. Oversized records, serialization failures and failed
version checks leave the destination unchanged. Write failures preserve OS error context.
After rename, directory fsync failure returns `PublishedButNotDurable`: the new
record may already be visible, so reread before deciding what to do next.

Each lock requires a name, reason and operation kind, with optional phase,
job ID and review ID. The descriptor is private, non-clonable through this API,
released on drop and closed across exec. Lock files are never renamed or
unlinked by the crate. `check_path` detects replacement and hard-link aliases;
every record publication checks it automatically. A runtime adapter must also
check it immediately before protected external effects. Preserving a lock
across intentional exec handoff remains a Python runtime capability.

Owner diagnostics are written in place through the held lock descriptor,
without moving its acquisition-specific seek position. Readers verify the
kernel flock, PID/start time, boot ID, holder descriptor/inode and acquisition
position. An old record cannot identify a later acquisition just because the
PID or descriptor was reused. The random position distinguishes acquisitions;
this diagnostic verification is not an authentication boundary. Missing,
incomplete, stale or inaccessible metadata reports an unknown owner. Metadata
publication is best effort: failures are exposed by `diagnostic_error()` and do
not fail an otherwise valid operation. `update_phase` refreshes diagnostics.
After release, old diagnostic bytes have no authority over kernel lock state.

Operation/record lock observations read Linux procfs without acquiring a lock,
creating files or writing records. They distinguish `unlocked`, `held` with a
verified/unknown owner, and `unknown` when kernel visibility is unavailable.
Procfs must expose the holder in the same PID namespace; these observations are
momentary diagnostics, never a permission to proceed. Busy acquisition errors
include the verified holder when available in both text and JSON. For inspection:

```sh
apps/target/release/airlock-nix inspect-lock /path/to/state deployment
apps/target/release/airlock-nix inspect-lock /path/to/state job-id --record
```

Future controller and dom0 executables can be packaged separately under this
subsystem, with only the dependencies and authority each VM needs. This crate
currently has a library plus one inspection binary; it installs no workers.
