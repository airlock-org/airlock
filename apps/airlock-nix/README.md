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
identity. All inspection commands read only the explicit local file, print JSON
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
  `BaselineFingerprint` and `VmName`. JSON decoding uses the same validation as
  Rust constructors. Git revisions follow the existing SHA-1 protocol; moving
  to another Git object format will need an explicit protocol change.
- `review`: existing `airlock.ctrl-approval-{request,response}.v1` wire formats,
  a source-and-target `CandidateIdentity`, a machine-or-guest target enum,
  a normalized nonempty VM scope, typed human
  decisions, and consuming workflow transitions. A machine request cannot carry
  a guest target; a guest request must name one.
- `job`: exhaustive job statuses, distinct job/authorization IDs, and a durable
  approval decision separate from work progress. Building/planned/built/handoff
  snapshots require recorded approval and its authorization ID. These are
  current reviewed-controller records, not arbitrary legacy deployment logs.
  Build completion and deployment handoff never claim successful activation.
- `state`: bounded JSON reads; atomic JSON publication with explicit permission
  policy, unique temporary files, file fsync, rename and directory fsync; and a
  nonblocking kernel lock guard with a required named owner and reason.
- `error`: typed failures for source/baseline mismatches, scope violations,
  invalid records, lock contention, serialization and I/O, converted to JSON at
  the CLI boundary. Publication followed by failed directory fsync has a distinct
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

The lock's `File` is private, non-clonable through this API, and released when
the guard is dropped. Owner/reason and typed operation IDs are available to the
holder. This initial primitive does not publish Python-compatible owner
sidecars, inspect another process's owner or preserve a lock across exec;
those runtime requirements remain implemented by Python. Lock acquisition rejects
symlinks, nonregular files and multiple hard links, and verifies that the name
still refers to the held inode. `check_path` can detect later replacement; the
trusted directory owner must prevent unlink/rename while the operation runs.
State writes require an existing trusted parent directory with a stable namespace;
directory creation/durability belongs to explicit state initialization. Callers
must serialize logical read/modify/write updates. Temporary files stay private
until fully serialized and validated for unique JSON keys, and nonregular
destinations are rejected. Serialization/validation failures leave the old
record in place; write I/O failures retain their I/O error classification.
Atomic publication prevents partial files, not lost updates between writers.

Future controller and dom0 executables can be packaged separately under this
subsystem, with only the dependencies and authority each VM needs. This crate
currently has a library plus one inspection binary; it installs no workers.
