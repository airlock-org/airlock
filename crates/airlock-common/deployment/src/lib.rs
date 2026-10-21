//! Bind deployment effects to approval, durable publication and execution requests.
//! Machine planning determines required effects; this crate independently checks
//! their permission. Callers authenticate records and perform filesystem/Qubes I/O.
//! Cargo executes the same Rust bodies that Verus checks.
//!
//! ## Guarantees and their scope
//!
//! `review::check` has no preconditions. Its result is successful **if and only if**
//! `authorized(review, binding, candidate)` holds. That predicate requires matching
//! request, candidate location, source commit and dependency lock; an approved or
//! started status with review material; the guest restriction; a fresh baseline
//! when supplied; the same candidate for a started review; the same previous
//! operation for a consumed review; every proposed VM effect in the allowed set;
//! and permission for required or unknown reboot effects. Loop invariants and
//! termination measures prove membership checking for arbitrary lists, including
//! duplicates and empty lists.
//!
//! `AuthorizedPlan::reviewed` computes its own complete effect projection; its
//! `Identity` input has no effect list or reboot flag. The independent
//! [`affects` relation](../../machine-planning/src/effects.rs) names every source of VM
//! impact: configuration reconciliation (including unchanged declarations), configuration
//! changes, staging, shutdown, startup, dependency-expanded
//! impact, retirement, restoration, both private-copy endpoints, GUI providers and
//! clients, and both PCI-handoff endpoints. `PlanEffects::project` proves exact set
//! equivalence to that relation, including overlaps and duplicate names, and rejects
//! unclassified fields. Unknown reboot requirements consume reboot permission.
//! The checked constructor proves every such VM belongs to the original approval,
//! checks source/candidate/baseline/consumed-operation identity, and derives its own
//! startup and shutdown lists without changing their contents or order.
//!
//! The opaque plan retains the binding of the candidate, baseline, revision and
//! complete serialized plan. Its fresh-operation and recovery methods consume that
//! binding internally, accepting no replacement. Publication and dispatch compare
//! it again. Recovery can replan against new observations, but must freshly check
//! **all** effects against the original protected decision and preserve its stable
//! operation/authority through `RecoveryDecision`. This does not require identical
//! operation sets across attempts, or grant permission merely because an effect is
//! necessary.
//!
//! VM software, properties, services, storage and device configuration are scoped
//! through their VM targets. Machine policy, global preferences and host software
//! are approved as part of the exact source/candidate, with reboot permission checked
//! separately; there is no finer per-policy or per-device human capability in this
//! approval model. `retained_undeclared` reports VMs left untouched and grants no
//! operation. Source/lock fields and storage-copy receipts are identity or progress
//! metadata, not extra VM effects. Individual adapter arguments and effects absent
//! from the plan are not covered by the projection theorem.
//!
//! The ordinary wire adapter destructures every `DeploymentPlan` field explicitly;
//! new fields require classification. Unknown wire extensions fail at projection.
//! The coordinator additionally allows only the known `private_copies` extension
//! in `Plan.extra`, preventing flattened metadata from shadowing typed plan fields.
//! Malformed PCI transfers fail decoding. Explicit administrative/review-disabled
//! policies bypass human scope checking, but still require classified plans and
//! coherent GUI shutdown modes; they carry no human-approval theorem.
//!
//! `AuthorizedOperation::new` consumes checked startup and shutdown scopes and performs mandatory
//! identity selection. It returns a value only when saved and current authority
//! encodings match. Recovery retains the saved operation ID; legacy recovery adopts
//! the previous attempt ID; a new operation uses its attempt ID. The internal
//! `operation_id` comparison is not a callable alternative authorization API.
//! `AttemptId`, `OperationId`, `AuthorityBinding`, `PlanBinding` and `IntentBinding`
//! distinguish roles at the verified boundary; string comparisons remain internal.
//! These wrappers classify already decoded inputs, rather than authenticate them.
//! The coordinator encodes the `OperationAuthority` enum, including
//! the protected decision digest and candidate fingerprint. The decision digest
//! includes the original baseline and scope. Recovery rechecks review status and
//! the observed plan as well.
//!
//! `DurableOperation` owns a non-cloneable, volatile `Publication` gate:
//!
//! ```text
//! Unpublished --IntentSaved--> Intent --ApprovalSaved--> Ready
//! Ready --begin(exact attempt, exact intent)--> Executing --Verified--> Complete
//! any invalid event or interruption --> Blocked
//! ```
//!
//! `advance` is proved to implement `next`. The invariant requires both publication
//! acknowledgments before readiness, execution or completion. `preserves` proves
//! one-step preservation; `history_preserves` proves preservation and unchanged
//! identities for every finite event history by induction. `begin` succeeds exactly
//! when both barriers are acknowledged and both identities match, and consumes
//! readiness. Invalid order, interruption and uncertain completion cannot reopen a
//! gate. This proves one begin per gate, **not exactly-once external execution**.
//!
//! `AuthorizedOperation::publication` refuses a substituted attempt, operation,
//! authority or candidate/baseline/plan binding before returning an unready gate.
//! Production supplies `IntentSaved` only after authoritative intent and history
//! publication succeeds, then `ApprovalSaved` after the required approval update
//! succeeds. For administrative/review-disabled policies no approval
//! projection is required. Publication errors emit interruption and return no token.
//! `activate` calls `begin` before external execution and its caller reports verified
//! completion or uncertainty. The intent binding hashes the entire preflight record,
//! including attempt, candidate, baseline, operation authority and effect plan.
//!
//! `Targets::reviewed` calls the full review predicate and checks every lifecycle
//! target against both the plan's affected VMs and the original approved scope.
//! Its postconditions establish `effects::valid`: each retained target belongs to
//! the approved scope. Binding and publication preserve that scope; every gate
//! transition preserves the operation authority. `DurableOperation::request`
//! requires matching attempt, operation, authority and plan at runtime, and an
//! executing gate with both acknowledgments. Its postconditions bind the returned
//! target to the planned set and, using the constructor invariant, to the original
//! approval. No caller supplies an `authorised` boolean.
//!
//! The immutable plan binding covers candidate and baseline paths, both content
//! fingerprints, source revision and the complete plan. It excludes changing phases
//! and outcome observations. The full preflight digest is retained separately.
//! `StartRequest` has no public constructor or target setter, and borrows the gate
//! until dispatch; Rust prevents interruption/completion while it is outstanding.
//! The Qubes adapter accepts only that request and uses its checked target as the
//! sole VM argument. Explicit policy exemptions use the `policy_exempt` constructors: their
//! requests remain bounded by the plan and durable intent, but carry no human-review
//! theorem. Choosing these exemptions remains part of the trusted coordinator policy.
//!
//! `Shutdowns::reviewed` checks the explicit stop list with the same target checker.
//! It also checks every expected preserved GUI client against the candidate's effect
//! set and the original approval. A GUI preservation entry must name a stopped
//! `sys-gui`, contain clients, and exclude clients explicitly scheduled to stop.
//! Clients are GUI-interruption effects; their presence never grants permission to
//! shut them down. The shared review-plan extractor includes both GUI providers and
//! clients, even when other effect lists omit them.
//!
//! `DurableOperation::shutdown` checks the same identity bindings and executing gate
//! as startup. `ShutdownRequest` borrows that gate and carries an immutable target
//! and GUI mode. `arguments` is verified executable Rust: it emits only the fixed
//! wait/timeout arguments, the plan-selected `--force` flag when appropriate, and
//! the sole checked target. The external adapter converts those strings to process
//! arguments without accepting an independent VM name or force flag.
//!
//! Before dispatch, the coordinator retains every decoded Qubes inventory row in a
//! target-bound `ShutdownObservation`. The verified `inventory` module classifies
//! known non-halted states, including paused and transitional states, as active.
//! The opaque `PowerState` type requires that classifier for construction.
//! It selects every active peer for a live GuiVM query. The request gate checks all
//! rows: an uncertain state or missing active-peer query prevents execution, every
//! active GUI client must belong to the saved expected set, and hard dependents
//! block shutdown. Undeclared active peers also block force. Halted domains, dom0
//! and the explicit target need no peer query; uncertain states still block them.
//! The inventory decision is proved equivalent to its predicate, and
//! `approved_clients` composes it with the original checked scope: every active GUI
//! client in the supplied inventory is approved when a reviewed request succeeds.
//! Normal shutdown cannot acquire `--force` from a caller or ignore active GUI
//! clients. Recovery cannot replace the saved expected client set without changing
//! the checked plan binding.
//!
//! Crashes destroy this gate. Recovery consumes a checked `RecoveryDecision`, reuses
//! the original operation, and starts a fresh unready gate.
//! The finite-history theorem covers the volatile gate's interruption transitions;
//! it does **not** model disk contents across a crash. Missing, corrupt, conflicting
//! and stale records are refused by the reader and coordinator adapter. Verus
//! proves the checks on decoded relationships; JSON parsing, coordinator error
//! propagation and orchestration remain outside the proof boundary.
//! ## Privileged deployment routes
//!
//! Reviewed deployment is distinct from `config apply`/`config host`, boot setup, and explicit storage/maintenance commands; those administrative compatibility paths do not carry deployment-review authority. The setup executable is Rust; its thin shell launcher and the upstream Qubes API adapters still sit outside Verus.
//!
//! All new, resume and rollback attempts converge in `Coordinator::activate` after
//! intent publication and `DurableAuthorization::begin`. A phase records uncertainty,
//! not completion. The machine lock does not serialize external Qubes administrators.
//! The following inventory distinguishes deployment requests from other policies.
//!
//! | Route and plan evidence | Final boundary and current enforcement |
//! | --- | --- |
//! | VM creation/properties/features/services; explicit `configure` | Rust retains every desired domain in an opaque checked configuration plan, then issues verified create/setting requests. Native backend-derived boot settings use typed requests from original definitions or identity-checked storage outcomes. Template imports and admitted legacy helper behaviour remain trusted exceptions to per-setting coverage. Native deployment device assignments use separate typed requests, rather than the per-VM device helper. |
//! | Global preferences, labels, kernels, templates and qrexec policy | Fixed prepare/finish batch requests invoke the admitted native setup executable, which reconciles global configuration and writes managed policy/kernel files; Qubes CLI/local admin API performs mutation. Exact approved source/candidate is the authority, not an invented dom0 VM scope. No verified fine-grained dispatch. Template import may start a template through upstream postprocessing when configured to do so. |
//! | Private-copy preparation and execution; `private_copies` | `AuthorizedPlan::copies` retains the exact ordered source/destination pairs, preflight identities and target configuration. Verified `CopyPlan::prepare` checks the durable execution context, receipt and observed UUID/pool/VID, distinct disks, halted power classifications and capacity. `CopyIntent` becomes `CopyRequest` only after the adapter saves `Copying`; the private Qubes clone adapter accepts that request alone. Complete receipts skip overwrite; uncertain `Copying` receipts refuse automatic replay. Observation, durable acknowledgment and faithful Qubes execution remain trusted. Both endpoints are original approval effects. |
//! | Automatic network PCI handoff; `pci_handoff` | Verified Rust checks the complete structured assignment/attachment snapshot, network class, exact supported endpoints and ownership before dispatch. An opaque request retains the exact source-owned virtual assignment, including wildcard device identity, under the checked plan and executing durable context. The thin local Qubes Unassign adapter accepts that request alone. Native deployment subsequently reconciles PCI assignments through separate verified assignment requests; boot/manual setup retains its compatibility helper policy. |
//! | Declared and automatic PCI assignments; `configure` and admitted device definitions | `DevicePlan` retains the exact configuration name/index mapping and assignment parameters. Verified selections check original scope, complete context, physical identity and full ownership; device-ID wildcards cover all matching ports. Mutation requires a named halted observation. The thin Qubes API adapter accepts only `AssignmentRequest`; same-resource replacements require an additional verified removal request. Boot/manual device helpers remain separate. Existing VM device-declaration changes require migration and are rejected by normal planning. |
//! | Guest root/private/kernel/store requests; `configure` and admitted native backend | `StoragePlan` retains exact configure coordinates and original sources. Verified requests bind root import, private growth, kernel installation, store installation, derived boot settings and outcome-driven daemon restoration to the executing context. Native deployment consumes those requests directly; raw deployment volume mutation and VM removal are refused. Image resolution and storage's internal admission/attachment implementation remain trusted/runtime-checked boundaries. |
//! | Guest storage generation selection; `stage`, desired domain backend and admitted manifest generations | `stage_images` validates retained identities and writes `staged.json`. The backend checks explicitly declared generations; the storage service authenticates the staged identity before its reuse path and retains its offline/restart safeguards. These generation-selection and persistence algorithms remain ordinary Rust. |
//! | Retirement preparation/retire; `retire` and baseline domain | `RetirementPlan` binds exact baseline definitions to the checked retire field. Verified requests require canonical trash ID, matching configuration/UUID/volume set, halted power, and matching selected generation/system/manifest. A separate verified baseline pin precedes disruption. The reversible adapter consumes fixed autostart/tag/retention requests after the trash journal save. No VM deletion is selectable. |
//! | Restoration; `restore` and candidate domain | A separately checked restore namespace binds compatible class/template/private-state configuration. Verified requests require the named UUID/private volume and halted state, reject purge states and select only removal of the retired tag. Approved new root generations remain permitted. Metadata and root unlink use the validated canonical trash ID; filesystem execution remains trusted. |
//! | Host roots/view/controller; candidate host, transport and coordinator manifest | `HostPlan` retains the admitted host/transport/controller and the current native storage service before publication. Verified `HostRequest` selection checks the complete executing context and emits exact Nix/storage argv. A prepared view permits validation instead of publishing. Controller link publication is a trusted filesystem adapter consuming the selected request; local retention paths and artifact decoding remain adapter duties. |
//! | Host boot preparation/selection; candidate host and transaction | Verified host requests select the fixed protect/install/seal/checkpoint/bind/select operations and exact system-profile argv. The process adapter receives the request with its retained native program; raw state-changing host wrappers reject deployment use. `selection.json`, staged ledgers and boot-receipt removal remain transaction writes. Filesystem/device-mapper/bootloader actions and their implicit running-host observations remain trusted external execution. Selection does not reboot. |
//! | Completion metadata | Phase/history/`latest.json`, observed VM receipts and maintenance scheduling are transaction protocol writes. They publish authority/progress and therefore cannot all require an already executing deployment. Atomic publication/fsync remains a trusted I/O boundary. |
//! | Verification and GUI observation | `qvm-ls`/prefs/features and fixed read-only qrexec probes observe runtime generation, boot IDs and reconnect status. No explicit GUI restart command is issued here. Qubes may start dependencies or terminate a shutdown target implicitly; those external effects are not proved. |
//! | Boot/post-boot trash quarantine | Separate boot/verification policy, outside `activate` and its durable gate. The protected trash record must match the registered UUID and complete volume set before normal shutdown, autostart suppression or tag repair. Unknown power refuses enforcement; known non-halted states require shutdown. These are ordinary runtime checks, not verified deployment requests. |
//! | Trash purge | Separate explicit maintenance confirmation and lock, not a call from `activate`. Verified intent selection checks canonical confirmation, original UUID/volumes/generation/system/manifest, retired tag and named halted state. The remove adapter consumes the publication-gated request; cleanup preserves the same VM/generation/trash root. Expiry/waiver, retained references and alias discovery remain ordinary runtime checks. This independent authority cannot be derived from a deployment request. |
//! | Generation/host pruning and scheduled maintenance | Separate maintenance policies and locks, not calls from `activate`. Completion only writes a maintenance request; the maintenance service applies retention and active-root protections before deletion. Those selection and filesystem algorithms remain unverified. |
//! | Admission, candidate preparation, baseline capture | Separate pre-execution protocols write protected artifacts, provenance, roots and baseline snapshots. They use their existing review/admission/boot policies; possession of a deployment request does not grant admission or arbitrary filesystem authority. |
//! | Boot setup/start, local development and administrative repair | Deliberate distinct policies. `boot.rs` invokes setup outside deployment; reviewed deployment proofs do not apply. Raw lifecycle commands are rejected by the deployment runner. Raw non-lifecycle mutations, the native setup executable, older admitted compatibility helpers and storage subcommands remain trusted escape boundaries. |
//! ## Trusted boundaries
//!
//! - The adapter must decode and classify every security field faithfully, including
//!   copy endpoints, grouped GUI entries and PCI transfers. The projection and its
//!   complete approval check execute verified Rust; converting typed wire fields
//!   into `PlanEffects` remains tested ordinary Rust, not a decoding theorem.
//!   Unknown top-level extensions are refused, and storage identities/receipts are
//!   interpreted by existing storage adapters. Duplicate JSON keys may be collapsed
//!   by decoding before this boundary. Adapter tests cover each effect field. The [planning boundary](../../machine-planning/README.md)
//!   documents trusted decoding, configuration encoding and source/observation projection.
//!   Effect selection outside the verified lifecycle planner, canonical authority
//!   encoding, SHA-256 collision resistance and release fingerprint computation
//!   remain trusted.
//!   The adapter chooses fresh versus recovery baseline checking. During rollback it
//!   checks the original reviewed candidate here. Recovery's target/baseline selection
//!   and their equality at authority construction are verified; loading, fingerprint
//!   calculation, immutable-plan projection and calling the gates remain adapter duties.
//! - The process adapter must execute exactly the permitted target with the fixed
//!   start invocation or verified shutdown argv. Command resolution, argument encoding,
//!   subprocess creation, timeout escalation and Qubes behavior
//!   are trusted. The proof establishes an authorized request, not successful execution.
//! - The I/O adapter must decode the complete Qubes inventory without dropping rows,
//!   supply raw state fields to the verified classifier and record truthful GuiVM
//!   queries. Missing probes are checked, rather than assumed successful. Live hard
//!   references must still match the protected baseline. The machine lock
//!   serializes Airlock operations; it does not make Qubes observations and dispatch
//!   atomic against external administrators. Changes after observation, guest shutdown
//!   hooks, implicit VM deletion and other external effects are outside this proof.
//! - The machine lock, protected file ownership, approval origin, administrative
//!   authorization and review-required policy determine which inputs carry authority.
//!   This library cannot authenticate arbitrary data supplied by another Rust caller.
//! - Filesystem success must mean atomic same-filesystem publication with file and parent
//!   directory fsync. An error may have committed data; it never supplies an acknowledgment.
//!   Power-loss durability and agreement of current/history records are outside the
//!   proof boundary. Process-crash tests exercise the real adapter.
//! - Qubes effects depend on the coordinator's observation and reconciliation adapters.
//!   Direct helpers inherit the machine flock, preventing a new Airlock recovery
//!   while they retain its descriptor after coordinator death. Helpers that close
//!   descriptors or hand work to a daemon, and independent Qubes administrators,
//!   need separate fencing/reconciliation guarantees. The verified gate does not
//!   prove safe external replay, completion observations, VM isolation or storage integrity.
//! - The verified request constructors and argument encoders are production Rust. The
//!   coordinator's call graph, wire decoding and external dispatch adapters are audited
//!   ordinary Rust. Raw deployment lifecycle/device/volume guards protect the existing
//!   process entry points; they are not a proof that arbitrary new coordinator code cannot
//!   invoke a new subprocess or filesystem mutation. New routes must be classified and
//!   connected to a request boundary during review.
//! - Verus, Z3, Rust compilation, proof erasure and the standard-library specifications
//!   in pinned vstd form the proof toolchain's trusted base. No Airlock proof uses
//!   `assume`, `external_body` or an unchecked contract to omit executable obligations.
//! - Liveness is unproved. Eventual recovery requires retained valid records, successful
//!   I/O, exclusive scheduling, and terminating/reconcilable helpers. Revocation or
//!   damaged authority intentionally blocks execution.
//!
//! The largest unverified policy boundary is the storage service's interpretation of admitted
//! generations into concrete volume definitions and offline Qubes snapshots. Next are global
//! setup's policy/kernel/template/default reconciliation and the admitted legacy batch executor.
//! Their outer request identities are checked, but their internal effect selection is not proved.
//! Boot quarantine, retention/reference discovery and other maintenance policies have separate
//! authority and incomplete proof coverage. Below those policy boundaries, truthful observations,
//! durable acknowledgments, process resolution and faithful filesystem/Qubes execution remain
//! necessary environmental assumptions.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod review;

pub mod publication;

pub mod effects;

pub mod inventory;

pub mod operation;

pub mod planning;

pub mod recovery;

pub mod plan;

pub mod configuration;

pub mod copy;

pub mod host;

pub mod retirement;

pub mod pci;

pub mod devices;

pub mod storage;

pub mod purge;
