# Deployment authorization and execution

This library checks whether a deployment may request its proposed effects. It binds
approval to source, candidate, baseline and operation identity, enforces publication
barriers and constructs permitted VM lifecycle and configuration requests. It performs no I/O.
[Machine planning](../machine-planning/README.md) independently calculates required
effects; a required effect acquires no permission merely by being planned.
Ordinary Cargo executes the same `verus!` function bodies that Verus checks;
specifications and proofs are erased from normal builds.

The coordinator's [approval adapter](../../airlock-dom0/service/src/coordinator/approval.rs)
authenticates and decodes records. It carries owned authority through this chain:

```
review + candidate identity + classified plan → AuthorizedPlan → AuthorizedOperation
    → DurableOperation → StartRequest / ShutdownRequest / configuration requests → Qubes adapter
```

Each arrow before the Qubes adapter executes Verus-checked Rust. The authority
types have private fields and cannot be cloned or deserialized. The adapter
must consume `AuthorizedOperation` to obtain a publication gate. Constructing a
wire journal or an ordinary `Publication` does not produce that authority.

## Guarantees and their scope

`review::check` has no preconditions. Its result is successful **if and only if**
`authorized(review, binding, candidate)` holds. That predicate requires matching
request, candidate location, source commit and dependency lock; an approved or
started status with review material; the guest restriction; a fresh baseline
when supplied; the same candidate for a started review; the same previous
operation for a consumed review; every proposed VM effect in the allowed set;
and permission for required or unknown reboot effects. Loop invariants and
termination measures prove membership checking for arbitrary lists, including
duplicates and empty lists.

`AuthorizedPlan::reviewed` computes its own complete effect projection; its
`Identity` input has no effect list or reboot flag. The independent
[`affects` relation](../machine-planning/src/effects.rs) names every source of VM
impact: configuration reconciliation (including unchanged declarations), configuration
changes, staging, shutdown, startup, dependency-expanded
impact, retirement, restoration, both private-copy endpoints, GUI providers and
clients, and both PCI-handoff endpoints. `PlanEffects::project` proves exact set
equivalence to that relation, including overlaps and duplicate names, and rejects
unclassified fields. Unknown reboot requirements consume reboot permission.
The checked constructor proves every such VM belongs to the original approval,
checks source/candidate/baseline/consumed-operation identity, and derives its own
startup and shutdown lists without changing their contents or order.

The opaque plan retains the binding of the candidate, baseline, revision and
complete serialized plan. Its fresh-operation and recovery methods consume that
binding internally, accepting no replacement. Publication and dispatch compare
it again. Recovery can replan against new observations, but must freshly check
**all** effects against the original protected decision and preserve its stable
operation/authority through `RecoveryDecision`. This does not require identical
operation sets across attempts, or grant permission merely because an effect is
necessary.

VM software, properties, services, storage and device configuration are scoped
through their VM targets. Machine policy, global preferences and host software
are approved as part of the exact source/candidate, with reboot permission checked
separately; there is no finer per-policy or per-device human capability in this
approval model. `retained_undeclared` reports VMs left untouched and grants no
operation. Source/lock fields and storage-copy receipts are identity or progress
metadata, not extra VM effects. Individual adapter arguments and effects absent
from the plan are not covered by the projection theorem.

The ordinary wire adapter destructures every `DeploymentPlan` field explicitly;
new fields require classification. Unknown wire extensions fail at projection.
The coordinator additionally allows only the known `private_copies` extension
in `Plan.extra`, preventing flattened metadata from shadowing typed plan fields.
Malformed PCI transfers fail decoding. Explicit administrative/review-disabled
policies bypass human scope checking, but still require classified plans and
coherent GUI shutdown modes; they carry no human-approval theorem.

`AuthorizedOperation::new` consumes checked startup and shutdown scopes and performs mandatory
identity selection. It returns a value only when saved and current authority
encodings match. Recovery retains the saved operation ID; legacy recovery adopts
the previous attempt ID; a new operation uses its attempt ID. The internal
`operation_id` comparison is not a callable alternative authorization API.
`AttemptId`, `OperationId`, `AuthorityBinding`, `PlanBinding` and `IntentBinding`
distinguish roles at the verified boundary; string comparisons remain internal.
These wrappers classify already decoded inputs, rather than authenticate them.
The coordinator encodes the `OperationAuthority` enum, including
the protected decision digest and candidate fingerprint. The decision digest
includes the original baseline and scope. Recovery rechecks review status and
the observed plan as well.

`DurableOperation` owns a non-cloneable, volatile `Publication` gate:

```
Unpublished --IntentSaved--> Intent --ApprovalSaved--> Ready
Ready --begin(exact attempt, exact intent)--> Executing --Verified--> Complete
any invalid event or interruption --> Blocked
```

`advance` is proved to implement `next`. The invariant requires both publication
acknowledgments before readiness, execution or completion. `preserves` proves
one-step preservation; `history_preserves` proves preservation and unchanged
identities for every finite event history by induction. `begin` succeeds exactly
when both barriers are acknowledged and both identities match, and consumes
readiness. Invalid order, interruption and uncertain completion cannot reopen a
gate. This proves one begin per gate, **not exactly-once external execution**.

`AuthorizedOperation::publication` refuses a substituted attempt, operation,
authority or candidate/baseline/plan binding before returning an unready gate.
Production supplies `IntentSaved` only after authoritative intent and history
publication succeeds, then `ApprovalSaved` after the required approval update
succeeds. For administrative/review-disabled policies no approval
projection is required. Publication errors emit interruption and return no token.
`activate` calls `begin` before external execution and its caller reports verified
completion or uncertainty. The intent binding hashes the entire preflight record,
including attempt, candidate, baseline, operation authority and effect plan.

`Targets::reviewed` calls the full review predicate and checks every lifecycle
target against both the plan's affected VMs and the original approved scope.
Its postconditions establish `effects::valid`: each retained target belongs to
the approved scope. Binding and publication preserve that scope; every gate
transition preserves the operation authority. `DurableOperation::request`
requires matching attempt, operation, authority and plan at runtime, and an
executing gate with both acknowledgments. Its postconditions bind the returned
target to the planned set and, using the constructor invariant, to the original
approval. No caller supplies an `authorised` boolean.

The immutable plan binding covers candidate and baseline paths, both content
fingerprints, source revision and the complete plan. It excludes changing phases
and outcome observations. The full preflight digest is retained separately.
`StartRequest` has no public constructor or target setter, and borrows the gate
until dispatch; Rust prevents interruption/completion while it is outstanding.
The Qubes adapter accepts only that request and uses its checked target as the
sole VM argument. Explicit policy exemptions use the `policy_exempt` constructors: their
requests remain bounded by the plan and durable intent, but carry no human-review
theorem. Choosing these exemptions remains part of the trusted coordinator policy.

`Shutdowns::reviewed` checks the explicit stop list with the same target checker.
It also checks every expected preserved GUI client against the candidate's effect
set and the original approval. A GUI preservation entry must name a stopped
`sys-gui`, contain clients, and exclude clients explicitly scheduled to stop.
Clients are GUI-interruption effects; their presence never grants permission to
shut them down. The shared review-plan extractor includes both GUI providers and
clients, even when other effect lists omit them.

`DurableOperation::shutdown` checks the same identity bindings and executing gate
as startup. `ShutdownRequest` borrows that gate and carries an immutable target
and GUI mode. `arguments` is verified executable Rust: it emits only the fixed
wait/timeout arguments, the plan-selected `--force` flag when appropriate, and
the sole checked target. The external adapter converts those strings to process
arguments without accepting an independent VM name or force flag.

Before dispatch, the coordinator retains every decoded Qubes inventory row in a
target-bound `ShutdownObservation`. The verified `inventory` module classifies
known non-halted states, including paused and transitional states, as active.
The opaque `PowerState` type requires that classifier for construction.
It selects every active peer for a live GuiVM query. The request gate checks all
rows: an uncertain state or missing active-peer query prevents execution, every
active GUI client must belong to the saved expected set, and hard dependents
block shutdown. Undeclared active peers also block force. Halted domains, dom0
and the explicit target need no peer query; uncertain states still block them.
The inventory decision is proved equivalent to its predicate, and
`approved_clients` composes it with the original checked scope: every active GUI
client in the supplied inventory is approved when a reviewed request succeeds.
Normal shutdown cannot acquire `--force` from a caller or ignore active GUI
clients. Recovery cannot replace the saved expected client set without changing
the checked plan binding.

Crashes destroy this gate. Recovery consumes a checked `RecoveryDecision`, reuses
the original operation, and starts a fresh unready gate.
The finite-history theorem covers the volatile gate's interruption transitions;
it does **not** model disk contents across a crash. Missing, corrupt, conflicting
and stale records are refused by the reader and coordinator adapter. Verus
proves the checks on decoded relationships; JSON parsing, coordinator error
propagation and orchestration remain outside the proof boundary.

## VM configuration requests

The planner records `configure` explicitly because setup reconciles every desired VM,
including declarations unchanged since the baseline. The complete approval projection
includes these names. Missing legacy fields decode to an empty list; a nonempty desired
configuration cannot acquire configuration authority from that empty list.

`AuthorizedPlan::configuration` constructs an opaque `ConfigurationPlan` before
publication. Its verified constructor accepts exactly the checked VM namespace,
rejecting missing, extra and duplicate definitions and substituted helper paths.
The authenticated release adapter decodes classes, labels, templates and declared
settings. The plan retains those values immutably with the complete plan binding.
Every create, property, feature and service request checks the actual executing
`DurableOperation`, both publication acknowledgments and the journal's current attempt,
operation, authority and plan binding. Requests borrow that gate through dispatch.
Their dispatch methods accept no independent mutating VM name or key/value.

The executable Verus contracts prove exact Qubes argv, not just target membership.
Setting requests also require a power observation naming the retained VM and an
explicitly halted classification before a mutation. An already-current value can be
left alone while running. Matching NetVM/GuiVM values still require an explicit
Qubes override. Observation truth/completeness, XML interpretation, decoding, iteration
and absence of a concurrent administrative restart remain adapter obligations.

The setup protocol separates global preparation, Rust VM reconciliation and completion.
Boot/manual setup uses the same Rust reconciliation with its existing local policy;
it cannot reuse a deployment runner to dispatch raw changes. Older admitted setup
programs use an explicit legacy batch request after their recognized unsupported-option
response; unknown versions and uncertain protocol outcomes are refused. Fixed per-VM
backend/device requests bind the target, program and topology; they do **not** verify the native helper's storage,
derived boot metadata or device behaviour. Global policy, template import, label and
kernel preparation are likewise still trusted batch adapters. Template import can
create/start VMs outside the ordinary domain namespace. These are not covered by
the common VM configuration request theorem.

Setup reports aggregate preparation, per-VM helper and Rust change receipts, preserving
restart/unsupported evidence. Run identity, missing/failed child evidence and repeated
finalization are checked by ordinary Rust tests. These informational reports are
separate from the protected durable journal and grant no authority.

## Recovery evidence and decisions

The [reconciler](src/recovery.rs) separates intended state from completion evidence.
These records answer different questions:

| Fact | Authoritative evidence |
| --- | --- |
| Current attempt, immutable release pair, plan and operation | `latest.json`; a retained history copy may lag in progress but must agree on immutable intent. |
| Original source approval and permitted effect scope | Protected review and candidate binding, rechecked before every new execution attempt. A started-status projection alone grants nothing. |
| Intended machine/boot selection | `selection.json` and its referenced journal. Selection is published before VM shutdown/configuration and does not attest that either completed. |
| Actual selected and running generations | Fresh Qubes, pool and guest observations plus host inspection; desired selection is not a substitute. |
| Private-copy completion | Identity-bound copy receipt, checked by the migration adapter; not inferred from the deployment phase. |

`RecoveryDecision::reconcile` executes in production. Its success is proved
equivalent to `compatible`, which permits a current selection only when its
transaction, release pair, fingerprints, plan and operation agree. A lagging
selection must name one of the two fingerprint-bound endpoints and have a
matching journal: either another attempt of the same operation with the same
authority and pair, or a completed earlier deployment of that endpoint. Missing
selection is accepted only for an interrupted pair containing the image baseline.
Other missing records, undecodable native identities, collapsed release/predecessor
pairs and unsupported legacy journals yield no recovery decision. A collapsed
pair cannot identify the possibly applied candidate, so it needs operator repair.
For reviewed V2 authority the original approved candidate and fingerprint must
also be an endpoint; recovery cannot substitute an unrelated pair.
A verified outcome requires selection of
the current attempt; `Complete` cannot roll back, while `RebootRequired` can.

Every incomplete phase selects reconciliation, regardless of how far its external
effects may have progressed. Resume uses the recorded release and predecessor;
rollback exchanges them. Selection never supplies the planning baseline. This
preserves knowledge of the attempted candidate when selection is absent or lags.
`releases` proves this mapping; changing the rollback baseline or dropping a
fingerprint comparison violates its executable contracts.

The [coordinator adapter](../../airlock-dom0/service/src/coordinator/recovery.rs)
loads and checks those files, replans from fresh stable power observations and
rechecks the complete proposed effects against the original approval. It must
consume the accepted decision through `authorize` to create the new
`AuthorizedOperation`. That method rejects a substituted prior journal, loaded
release, baseline or V2 authority, preserves the stable operation identity and
carries the freshly checked scopes and new plan binding into publication. V1
adopts its attempt ID after checking the protected approval; it cannot compare an
original decision digest that its format did not retain. A prior plan cannot be silently replaced in the evidence;
the new plan may differ, but must be independently reauthorized. Verification-only
decisions cannot create deployment authority. Neither action restores volatile
publication acknowledgments from the previous attempt.

`history_preserves` proves by induction that finite endpoint exchanges retain
the original pair, operation and recorded authority. This composes with the
executable endpoint and operation contracts; it is **not** a model of filesystem
crash consistency or every coordinator instruction.

Before retirement and again before configuration, the coordinator obtains a
fresh stable power snapshot. Verified `quiescent` accepts exactly when no required
VM is active. The tested adapter supplies changed, affected, retired and explicit
stop targets, including VMs halted when the plan was constructed. Thus an observed
restart after shutdown blocks configuration. The check cannot prevent a later
independent restart: Qubes observation and mutation are not atomic.

Replaying reconciles uncertain external outcomes rather than assuming a failed
call had no effects. Host/storage/setup helper replay safety, generation
verification, copy receipts and rollback's private-data preservation remain
adapter obligations. No exactly-once or eventual-recovery theorem is claimed.
Invalid authority, unsupported rollback migrations and ambiguous evidence can
require operator intervention. The same separation of durable intent, checked
reconciliation and fresh publication can serve retirement, artifact admission
and generation publication; each needs its own external reconciliation contract.

## Production execution paths

The production [planner adapter](../machine/src/planning.rs) uses
`airlock-machine-planning` to interpret topology, derive deployment seeds and order
VM effects. That crate has no dependency on deployment approval or publication.
This crate depends on its public contracts to compose planning with authorization.

[`name_targets`](../machine-planning/src/graph.rs) verifies the exact string contents copied from computed vertex
indices. [`approved_schedule`](src/planning.rs) composes that mapping and exact selection with the
existing checked scope: every selected operation needs original approval.
The coordinator checks the complete proposed effect set before creating owned
lifecycle authority; graph reachability cannot grant execution permission.
Recovery and rollback use the same seed selector and kernel with reconstructed
inputs and then reauthorize. Recovery adds saved startup targets to its running
input and filters stops against a fresh live snapshot; those reconstruction and
filtering steps remain unverified coordinator code. Existing operation, attempt
and plan bindings prevent substitution
within an executing attempt. Recovery endpoint selection and its handoff to the
owned operation are verified; observation reconstruction and external progress
remain outside the proof. Different attempts may replan within the original scope.

| Path | Enforcement and proof boundary |
| --- | --- |
| `relationships.rs` → `machine-planning/src/topology.rs`: reference interpretation and canonical indexing | Verified effective defaults/overrides, reference grammar, exact internal edge relation, canonical universe, unique identities, name/mask correspondence and named planner calls. JSON/typed-field decoding and conversion to/from ordinary `GuestName` collections remain tested adapters. External relationships remain visible to admission. |
| `machine-planning/src/graph.rs`: closure and lifecycle operation lists | Verified least forward/reverse closures, exact startup effects, affected/running shutdown selection, dependency/reverse ordering and cycle rejection. Startup uses candidate edges; impact uses baseline edges with eligible soft GUI edges removed. Change/removal/autostart roots, baseline GUI eligibility and their named graph handoffs are verified in `machine-planning/src/deployment.rs`. The adapter still chooses baseline/candidate inputs and decodes configuration and observations. |
| `machine-planning/src/deployment.rs`: seed selection and GUI preservation | Verified exact change/removal/startup roots, configuration identity coverage, GUI opt-in/provider/hard-edge conditions, exact reconnect selection, cached impact closure and input-order-independent root sets. Production deployment planning rejects unsupported power states; inventory parsing/completeness and canonical configuration encoding remain tested adapters. |
| Complete plan effect projection and authorization | Verified exact VM-impact union, unknown-field rejection, required/unknown reboot permission, full scope and identity check, exact lifecycle-list derivation and retained plan binding through fresh execution/recovery. Wire decoding/classification, staging/retirement/copy/PCI selection and complete-plan serialization remain adapter obligations; no whole-plan correctness/determinism theorem. |
| `recovery.rs`: durable evidence → `RecoveryDecision` → `AuthorizedOperation` | Verified evidence compatibility, exact direction/endpoint selection, unchanged retained intent and V2 authority, stable operation identity, fresh checked scopes and new publication. JSON/record authentication, snapshot projection, runtime reconstruction and actual effect completion remain adapter obligations. |
| Pre-retirement/configuration power check | Verified disjointness decision for required targets and fresh active observations. Hazard-set extraction, inventory completeness and absence of changes after observation remain trusted. |
| `activation.rs`: explicit starts after configuration, including resume and rollback | Verified target selection and identity/scope checks; opaque request passed to `Runner::start_deployment`. The narrow process adapter remains trusted. |
| `activation.rs`: explicit shutdowns, including resume and rollback | Verified target, GUI mode, expected-client scope, observed-client bounds, identity and gate checks; verified argv passed to `Runner::shutdown_deployment`. |
| Forced GUI shutdown | Only a checked GUI-preservation entry supplies force. Verified power-state classification and complete row checking require all active GUI clients to fit the approved expected set. Missing probes, uncertain states and hard/undeclared active dependents block execution. Inventory decoding, truthful Qubes observations and baseline reference extraction remain trusted. |
| Direct kill | Deployment issues no separate kill request. Raw `qvm-kill` is rejected; Qubes' shutdown helper can kill the same checked target on timeout. |
| Generic `Runner::run`, `capture`, `text`, `stream` during deployment | Reject direct `qvm-start`, `qvm-shutdown`, `qvm-kill` and `qvm-create`, including absolute paths. This tested adapter guard prevents accidental raw-call bypass; it does not confine arbitrary helpers, aliases or shell commands. |
| VM creation and declared properties/features/services | Verified `ConfigurationPlan` namespace, retained definitions, durable context, exact target/key/value/tool and argv; typed requests reach thin Qubes adapters. JSON decoding, reconciliation orchestration, observations and external CLI semantics are trusted. |
| `activation.rs`: host roots/controller, boot preparation and selection | Verified `HostPlan`/`HostRequest` preserve exact admitted host/transport/controller, prepared-view policy and fixed native/Nix argv under the executing gate. Local retention paths, controller symlink publication and external filesystem/storage/boot effects remain trusted. Selection/status files are transaction metadata. |
| Retained setup sections | Checked helper modes/paths and complete configuration namespace; global setup, backend-derived properties, storage and device dispatch remain trusted adapters. |
| `migration.rs`: private copies | Verified original ordered endpoints, preflight configuration/resource bindings, monotonic receipt progress, stopped/capacity checks, durable context and exact clone request names. Completed and uncertain receipts never request an overwrite. Qubes and copy-publication acknowledgments remain trusted. |
| Reversible retirement/restoration | Verified checked namespaces/configuration, canonical trash identity, named UUID/storage/power observations, retire-only selected generation/system/manifest checks, no purge states and fixed tag/autostart/retention argv. Baseline pin requests are target/attempt-bound. Observation decoding, storage identity hashing, persistence and faithful execution remain trusted. |
| PCI handoff and explicit purge | Handoff retains runtime scope/ownership checks; purge has independent maintenance authority and runtime reference/UUID/volume/expiry safeguards. Neither has fine-grained verified request dispatch. |
| `admission.rs` and storage adapters: artifact admission and generation publication | Existing storage contracts and checks, outside this effect proof. |
| `boot.rs::boot_start`, trash enforcement, development startup | Separate boot/maintenance/development policy; no deployment-review theorem. |
| Qubes Manager Admin API | Existing `sys-gui` policy separately permits `admin.vm.Shutdown` and `admin.vm.Kill`. Those user administration requests do not pass through deployment authority or its machine lock. |
| RPC prepared apply and CLI recovery | Route into the coordinator activation path. Routing and prepared-record interpretation are tested code, not Verus proofs. |

Qubes can start dependencies implicitly. The planner expands declared startup
dependencies; regression coverage checks that such expansion still needs approval.
References outside the declared topology, live dependency drift and Qubes/Xen's
actual effects are outside the explicit-request theorem. No claim covers all
privileged operations in dom0 or all effects of starting a VM.

The pinned [Qubes client v4.3.30 shutdown helper](https://github.com/QubesOS/qubes-core-admin-client/blob/v4.3.30/qubesadmin/tools/qvm_shutdown.py)
uses `--force` to bypass dependent-VM protection. With the existing
`--wait --timeout 120` arguments it can kill the same explicit target after timeout.
The [Qubes GUI handler v4.3.44](https://github.com/QubesOS/qubes-core-admin/blob/v4.3.44/qubes/ext/gui.py)
normally prevents stopping a provider with running GUI clients; force bypasses
that protection without explicitly shutting those clients down. Airlock checks
their expected GUI interruption and subsequently verifies preserved boot IDs and
GUI reconnection. Verus does not prove client survival or faithful Xen/Qubes effects.

## Private-volume replacement

The [copy boundary](src/copy.rs) keeps the original pair order and resource definitions
under the complete plan binding. Names, registration UUIDs, pool/volume identities and
the destination configuration must agree across preflight, the protected receipt and
current named observations. A clone requires explicitly halted endpoints and enough
capacity. The request owns neither an arbitrary VM name nor a new approval flag.

`CopyIntent` is not accepted by the Qubes adapter. The coordinator first pins its
validated destination and durably saves `Copying`, then consumes the intent into a
clone request. This publication acknowledgment relies on truthful filesystem success.
A `Copying` receipt after an error or restart represents uncertainty; it blocks automatic
copying and needs explicit reconciliation. A complete receipt never requests a clone,
including when the destination has since been used. Recovery preflight also executes
verified comparison against the prior authorised journal’s copy definition, preserving
source identity, destination configuration, any pinned target and monotonic progress.
An incomplete copy missing that original intent is refused. Journal selection and
faithful decoding remain adapter duties. This is a safety guarantee, not
exactly-once execution or guaranteed automatic recovery. Qubes must preserve the observed
registrations and disks through its name-based clone calls; independent administrators
can race the machine lock. Neither disk contents nor that external atomicity are proved.

## Host preparation and selection

The [host boundary](src/host.rs) retains the admitted candidate artifacts and native
storage adapter before durable publication. The approved complete plan supplies its
binding; the release adapter is trusted to decode those artifacts faithfully. Host
scope comes from the approved source and reboot permission, rather than a fabricated
`dom0` VM entry. This does not prove Rusix evaluation, build provenance or admission.

Each request checks the original operation, authority, attempt and complete plan against
the executing durable gate. Host and transport pinning, controller pin/link publication,
profile selection and boot/storage operations have distinct fixed actions. Exhaustive
matches require new actions to be classified. Prepared host views allow validation;
transport-supplied views allow publication. CLI arguments and the retained executable
are selected by verified Rust, not reconstructed from current environment or raw journal
fields at dispatch. The recovery path retains the existing running-controller policy.

The Nix/storage process adapter executes those parameters; the controller filesystem
adapter consumes the immutable selected target. Retention directories, atomic symlink
publication, store effects and bootloader/firmware behavior remain trusted. Protect/seal/
checkpoint commands intentionally observe existing host state inside the storage adapter;
the proof covers their requests, not the truth or atomicity of those observations.
Recovery reissues only requests admitted under its newly checked, original authority.

## Network PCI handoff

The [PCI boundary](src/pci.rs) retains the specific checked source and destination
fields, rather than accepting a pair from the caller at dispatch. Only the existing
assistant/network-provider transfer is supported. Snapshot validation checks every
physical network function and both persistent assignments and live attachments,
including undeclared frontends. Foreign or conflicting owners, broad port wildcards,
missing network functions and unsupported endpoints yield no executable handoff.
The complete snapshot is checked before the first mutation, so a later conflict
cannot follow an earlier unassignment.

Selection is proved exact: a request exists precisely for a persistent source-owned
row matching a current dom0 network function. Network class is specified numerically;
matching preserves backend, port and hardware identity. Future device-specific rules
which do not match current hardware, other PCI classes and current attachments never
become unassignment requests. Recovery may accept an already unassigned no-op; it
cannot relax ownership, change endpoints or bypass the operation/attempt/authority/
complete-plan binding and executing publication gate.

The [Qubes adapter](../../airlock-dom0/service/src/qubes/pci.rs) queries all registered
frontends, validates identity consistency and encodes the selected virtual assignment
for the upstream [device API](https://github.com/QubesOS/qubes-core-admin-client/blob/v4.3.30/qubesadmin/devices.py).
It preserves wildcard device identity rather than resolving it to the current device.
Wire decoding, sysfs/Qubes completeness, transport encoding and faithful external
execution remain trusted and tested Rust. The snapshot is not atomic with external
administrators or hotplug. Unassignment changes persistent policy, not proof of physical
detachment; quiescence and subsequent device reconciliation retain their existing
coordinator rules. Raw deployment `qvm-device`/`qvm-pci` mutations are blocked by the
process adapter; boot/manual setup keeps its separate policy. The separate assignment boundary below checks native deployment configuration; boot/manual setup keeps its compatibility path.

## PCI assignment configuration

The [device boundary](src/devices.rs) retains the complete, ordered configuration
namespace and each VM's exact assignment declarations before publication. Selection
checks the executing operation/attempt/authority/plan binding and the named physical
hardware snapshot. Declared backend, port, explicit device restriction, matching mode,
attachment mode and complete options cannot be substituted at dispatch. Physical PCI
requests require dom0; unknown backends and identities fail closed.

Automatic discovery follows the effective root-provider rules: no explicit assignments,
`provides_network`, the conventional provider or literal opt-in feature, and no NetVM.
Only observed network-class functions outside both exclusion sets are selectable.
Assignment and attachment ownership is checked across all relevant frontends. Device-ID
wildcards check every matching physical port and refuse incomplete identities. These
rules do not grant VM scope: the retained configuration target must independently belong
to the original approval.

An opaque selection allows checking whether an assignment already matches, including
all options. It is not accepted by the privileged adapter. Mutation requires consuming
that selection after a named halted-state and complete ownership check. Qubes PCI only
supports required assignments; unsupported new modes are refused before replacement.
An option/mode replacement may unassign only the exact same frontend and virtual device
selected by the approved new request. Interrupted replacement is reconciled from fresh
structured observations; matching completed assignments are skipped even on running VMs.
No exactly-once or atomic replacement guarantee is claimed.

The direct [local API adapter](../../airlock-dom0/service/src/qubes/pci.rs) receives the
opaque request and serializes its retained identity, mode and options. Topology decoding,
legacy option normalization, truthful complete snapshots and faithful Qubes execution
remain trusted. All selections for one VM are checked before its first mutation. The
machine lock does not exclude independent administrators or hotplug. A declaration
without a device ID authorises resolution of the current hardware at its declared port;
automatic discovery likewise authorises a rule rather than a fixed hardware inventory.
Retries reapply those original rules to fresh observations. They do not promise that
hotplug preserves the first attempt's physical device. An explicit device ID restricts
resolution on every attempt. Ordinary deployment rejects changed device declarations
on an existing VM; those require migration. This boundary reconciles approved declarations
and configures new VMs. It does not infer removal of a different virtual resource from
an absent declaration. USB/block configuration
is not implemented by this PCI boundary. Boot/manual setup and older full-batch admitted
helpers retain explicit compatibility policies outside these fine-grained guarantees.

## Native VM storage requests

The [storage boundary](src/storage.rs) retains each native backend in the exact checked
configuration order before intent publication. Selecting a backend requires the complete
executing operation context. Root import requests require a named halted observation and
a resolution naming that VM and its original root source. The import argv fixes the root
volume and resolved image. Private-volume growth requires the same named halted evidence,
an original declared capacity, and an observed capacity below it. Missing or unknown power
and capacity observations fail closed. Kernel requests retain the original effective kernel
name and source. Store requests retain the native executable, VM, system, root, closure,
descriptor and supported immutable layout; dispatch accepts no independent overrides.
A store request retains the selected generation explicitly, rejecting a different or
missing selection when the original release names one. The storage adapter passes that
number to the native service; decimal rendering is trusted. The service rejects a
conflicting staged ledger and does not reuse a different generation merely because its
boot sources match. The verified argv also requires retained/prepared data: the service
refuses fresh package admission during deployment reconciliation, including repair of
an unchanged VM whose runtime storage has drifted. Boot/manual admission retains its
separate policy.

Native deployment calls the shared Rust backend reconciliation directly. Boot/manual setup
retains its separate compatibility policy. Immutable root import may repeat after a lost
reply; private growth reconciles current capacity. These are reconciliation guarantees,
not exactly-once execution. Qubes administrators can still change registration or state
between observation and dispatch.

Derived kernel and boot settings have fixed typed keys and original backend values.
Mutation requires fresh named halted evidence; already matching values need no mutation.
Generation-dependent features and daemon restoration require a storage outcome matching
the VM, system, root, closure, kernel and the request's selected generation. A receipt
cannot select arbitrary feature keys or a different system service. Root-artifact features
must attest the original VM/root source. Boot-option interpolation, scalar decoding and
truthfulness of generation metadata remain trusted; the verified boundary confines the
retained values and rejects unclassified setting requests.

Source decoding, physical Nix-path translation and source-to-root-image authentication
remain trusted. Staged generation selection and the storage service's admission, pool
locking, volume isolation, offline snapshot updates, restart journal and attachment checks
remain ordinary Rust. In particular, the store request theorem confines its source and boot
parameters; it does not prove the inner storage algorithm or that a running VM experiences
no implicit effect. The independent observation-to-dispatch race remains outside the
theorem. Legacy full-batch
helpers and global setup retain their separately documented trusted boundaries.

## Reversible retirement and restoration

The [retirement boundary](src/retirement.rs) retains separate checked namespaces and
operation kinds. Retirement requires the complete baseline configuration, registered
UUID, complete volume set and currently selected generation/system/pool manifest to
agree. Restoration checks class/template/private-state compatibility and the original
private-volume identity while permitting an approved new root. Both require explicitly
halted named observations. Purging, purged and already restored records yield no request.
The trash identity must be the recorded transaction plus the same checked VM name.

A baseline pin request retains the exact rollback release before disruption and uses
the protected proposal’s canonical transaction/VM root identity across retries. After the trash journal records retirement,
the process adapter consumes fixed autostart, tag and retention arguments. Restoration
emits only tag removal; neither family can construct a remove/purge command. Metadata
publication and removal of the validated trash retention root remain filesystem adapters.
The adapter retains the first proposal under the stable operation ID and reuses it
after a crash, refusing a replacement registration or volume set. Proposal lookup,
first-writer persistence, VM/volume observations and cryptographic encoding of
configuration/storage identities remain trusted; the machine lock does not exclude independent Qubes administrators.
Purge remains a separate, explicitly confirmed maintenance protocol, never a consequence
of possessing these deployment requests. The [purge boundary](src/purge.rs) captures the
canonical confirmed trash record and requires the original UUID, complete volume identity,
generation, system, manifest, retired tag and named halted observation before producing a
remove intent. Durable `Purging` publication turns that intent into the sole request accepted
by the maintenance remove adapter. Successful removal permits cleanup of the same generation
and canonical trash root. An absent VM permits cleanup only when the protected record already
says `Purging`; it cannot produce a remove request. The deployment runner refuses this
maintenance adapter.

Retention expiry/waiver, release and policy reference discovery, storage alias checks,
protected-record authentication and actual publication remain runtime/I/O boundaries. Their
existing checks precede removal; Verus proves the request identity, observed resource checks,
halted condition and publication-token handoff. It does not prove those reference algorithms,
wall-clock correctness, faithful deletion, cleanup durability or atomicity with independent
Qubes administrators. A crash after removal is reconciled from registration observations.

## Privileged deployment routes

Reviewed deployment is distinct from `config apply`/`config host`, boot setup, and explicit storage/maintenance commands; those administrative compatibility paths do not carry deployment-review authority. The setup executable is Rust; its thin shell launcher and the upstream Qubes API adapters still sit outside Verus.

All new, resume and rollback attempts converge in `Coordinator::activate` after
intent publication and `DurableAuthorization::begin`. A phase records uncertainty,
not completion. The machine lock does not serialize external Qubes administrators.
The following inventory distinguishes deployment requests from other policies.

| Route and plan evidence | Final boundary and current enforcement |
| --- | --- |
| VM creation/properties/features/services; explicit `configure` | Rust retains every desired domain in an opaque checked configuration plan, then issues verified create/setting requests. Native backend-derived boot settings use typed requests from original definitions or identity-checked storage outcomes. Template imports and admitted legacy helper behaviour remain trusted exceptions to per-setting coverage. Native deployment device assignments use separate typed requests, rather than the per-VM device helper. |
| Global preferences, labels, kernels, templates and qrexec policy | Fixed prepare/finish batch requests invoke the admitted native setup executable, which reconciles global configuration and writes managed policy/kernel files; Qubes CLI/local admin API performs mutation. Exact approved source/candidate is the authority, not an invented dom0 VM scope. No verified fine-grained dispatch. Template import may start a template through upstream postprocessing when configured to do so. |
| Private-copy preparation and execution; `private_copies` | `AuthorizedPlan::copies` retains the exact ordered source/destination pairs, preflight identities and target configuration. Verified `CopyPlan::prepare` checks the durable execution context, receipt and observed UUID/pool/VID, distinct disks, halted power classifications and capacity. `CopyIntent` becomes `CopyRequest` only after the adapter saves `Copying`; the private Qubes clone adapter accepts that request alone. Complete receipts skip overwrite; uncertain `Copying` receipts refuse automatic replay. Observation, durable acknowledgment and faithful Qubes execution remain trusted. Both endpoints are original approval effects. |
| Automatic network PCI handoff; `pci_handoff` | Verified Rust checks the complete structured assignment/attachment snapshot, network class, exact supported endpoints and ownership before dispatch. An opaque request retains the exact source-owned virtual assignment, including wildcard device identity, under the checked plan and executing durable context. The thin local Qubes Unassign adapter accepts that request alone. Native deployment subsequently reconciles PCI assignments through separate verified assignment requests; boot/manual setup retains its compatibility helper policy. |
| Declared and automatic PCI assignments; `configure` and admitted device definitions | `DevicePlan` retains the exact configuration name/index mapping and assignment parameters. Verified selections check original scope, complete context, physical identity and full ownership; device-ID wildcards cover all matching ports. Mutation requires a named halted observation. The thin Qubes API adapter accepts only `AssignmentRequest`; same-resource replacements require an additional verified removal request. Boot/manual device helpers remain separate. Existing VM device-declaration changes require migration and are rejected by normal planning. |
| Guest root/private/kernel/store requests; `configure` and admitted native backend | `StoragePlan` retains exact configure coordinates and original sources. Verified requests bind root import, private growth, kernel installation, store installation, derived boot settings and outcome-driven daemon restoration to the executing context. Native deployment consumes those requests directly; raw deployment volume mutation and VM removal are refused. Image resolution and storage's internal admission/attachment implementation remain trusted/runtime-checked boundaries. |
| Guest storage generation selection; `stage`, desired domain backend and admitted manifest generations | `stage_images` validates retained identities and writes `staged.json`. The backend checks explicitly declared generations; the storage service authenticates the staged identity before its reuse path and retains its offline/restart safeguards. These generation-selection and persistence algorithms remain ordinary Rust. |
| Retirement preparation/retire; `retire` and baseline domain | `RetirementPlan` binds exact baseline definitions to the checked retire field. Verified requests require canonical trash ID, matching configuration/UUID/volume set, halted power, and matching selected generation/system/manifest. A separate verified baseline pin precedes disruption. The reversible adapter consumes fixed autostart/tag/retention requests after the trash journal save. No VM deletion is selectable. |
| Restoration; `restore` and candidate domain | A separately checked restore namespace binds compatible class/template/private-state configuration. Verified requests require the named UUID/private volume and halted state, reject purge states and select only removal of the retired tag. Approved new root generations remain permitted. Metadata and root unlink use the validated canonical trash ID; filesystem execution remains trusted. |
| Host roots/view/controller; candidate host, transport and coordinator manifest | `HostPlan` retains the admitted host/transport/controller and the current native storage service before publication. Verified `HostRequest` selection checks the complete executing context and emits exact Nix/storage argv. A prepared view permits validation instead of publishing. Controller link publication is a trusted filesystem adapter consuming the selected request; local retention paths and artifact decoding remain adapter duties. |
| Host boot preparation/selection; candidate host and transaction | Verified host requests select the fixed protect/install/seal/checkpoint/bind/select operations and exact system-profile argv. The process adapter receives the request with its retained native program; raw state-changing host wrappers reject deployment use. `selection.json`, staged ledgers and boot-receipt removal remain transaction writes. Filesystem/device-mapper/bootloader actions and their implicit running-host observations remain trusted external execution. Selection does not reboot. |
| Completion metadata | Phase/history/`latest.json`, observed VM receipts and maintenance scheduling are transaction protocol writes. They publish authority/progress and therefore cannot all require an already executing deployment. Atomic publication/fsync remains a trusted I/O boundary. |
| Verification and GUI observation | `qvm-ls`/prefs/features and fixed read-only qrexec probes observe runtime generation, boot IDs and reconnect status. No explicit GUI restart command is issued here. Qubes may start dependencies or terminate a shutdown target implicitly; those external effects are not proved. |
| Boot/post-boot trash quarantine | Separate boot/verification policy, outside `activate` and its durable gate. The protected trash record must match the registered UUID and complete volume set before normal shutdown, autostart suppression or tag repair. Unknown power refuses enforcement; known non-halted states require shutdown. These are ordinary runtime checks, not verified deployment requests. |
| Trash purge | Separate explicit maintenance confirmation and lock, not a call from `activate`. Verified intent selection checks canonical confirmation, original UUID/volumes/generation/system/manifest, retired tag and named halted state. The remove adapter consumes the publication-gated request; cleanup preserves the same VM/generation/trash root. Expiry/waiver, retained references and alias discovery remain ordinary runtime checks. This independent authority cannot be derived from a deployment request. |
| Generation/host pruning and scheduled maintenance | Separate maintenance policies and locks, not calls from `activate`. Completion only writes a maintenance request; the maintenance service applies retention and active-root protections before deletion. Those selection and filesystem algorithms remain unverified. |
| Admission, candidate preparation, baseline capture | Separate pre-execution protocols write protected artifacts, provenance, roots and baseline snapshots. They use their existing review/admission/boot policies; possession of a deployment request does not grant admission or arbitrary filesystem authority. |
| Boot setup/start, local development and administrative repair | Deliberate distinct policies. `boot.rs` invokes setup outside deployment; reviewed deployment proofs do not apply. Raw lifecycle commands are rejected by the deployment runner. Raw non-lifecycle mutations, the native setup executable, older admitted compatibility helpers and storage subcommands remain trusted escape boundaries. |

## Trusted boundaries

- The adapter must decode and classify every security field faithfully, including
  copy endpoints, grouped GUI entries and PCI transfers. The projection and its
  complete approval check execute verified Rust; converting typed wire fields
  into `PlanEffects` remains tested ordinary Rust, not a decoding theorem.
  Unknown top-level extensions are refused, and storage identities/receipts are
  interpreted by existing storage adapters. Duplicate JSON keys may be collapsed
  by decoding before this boundary. Adapter tests cover each effect field. The [planning boundary](../machine-planning/README.md)
  documents trusted decoding, configuration encoding and source/observation projection.
  Effect selection outside the verified lifecycle planner, canonical authority
  encoding, SHA-256 collision resistance and release fingerprint computation
  remain trusted.
  The adapter chooses fresh versus recovery baseline checking. During rollback it
  checks the original reviewed candidate here. Recovery's target/baseline selection
  and their equality at authority construction are verified; loading, fingerprint
  calculation, immutable-plan projection and calling the gates remain adapter duties.
- The process adapter must execute exactly the permitted target with the fixed
  start invocation or verified shutdown argv. Command resolution, argument encoding,
  subprocess creation, timeout escalation and Qubes behavior
  are trusted. The proof establishes an authorized request, not successful execution.
- The I/O adapter must decode the complete Qubes inventory without dropping rows,
  supply raw state fields to the verified classifier and record truthful GuiVM
  queries. Missing probes are checked, rather than assumed successful. Live hard
  references must still match the protected baseline. The machine lock
  serializes Airlock operations; it does not make Qubes observations and dispatch
  atomic against external administrators. Changes after observation, guest shutdown
  hooks, implicit VM deletion and other external effects are outside this proof.
- The machine lock, protected file ownership, approval origin, administrative
  authorization and review-required policy determine which inputs carry authority.
  This library cannot authenticate arbitrary data supplied by another Rust caller.
- Filesystem success must mean atomic same-filesystem publication with file and parent
  directory fsync. An error may have committed data; it never supplies an acknowledgment.
  Power-loss durability and agreement of current/history records are outside the
  proof boundary. Process-crash tests exercise the real adapter.
- Qubes effects depend on the coordinator's observation and reconciliation adapters.
  Direct helpers inherit the machine flock, preventing a new Airlock recovery
  while they retain its descriptor after coordinator death. Helpers that close
  descriptors or hand work to a daemon, and independent Qubes administrators,
  need separate fencing/reconciliation guarantees. The verified gate does not
  prove safe external replay, completion observations, VM isolation or storage integrity.
- The verified request constructors and argument encoders are production Rust. The
  coordinator's call graph, wire decoding and external dispatch adapters are audited
  ordinary Rust. Raw deployment lifecycle/device/volume guards protect the existing
  process entry points; they are not a proof that arbitrary new coordinator code cannot
  invoke a new subprocess or filesystem mutation. New routes must be classified and
  connected to a request boundary during review.
- Verus, Z3, Rust compilation, proof erasure and the standard-library specifications
  in pinned vstd form the proof toolchain's trusted base. No Airlock proof uses
  `assume`, `external_body` or an unchecked contract to omit executable obligations.
- Liveness is unproved. Eventual recovery requires retained valid records, successful
  I/O, exclusive scheduling, and terminating/reconcilable helpers. Revocation or
  damaged authority intentionally blocks execution.

The largest unverified policy boundary is the storage service's interpretation of admitted
generations into concrete volume definitions and offline Qubes snapshots. Next are global
setup's policy/kernel/template/default reconciliation and the admitted legacy batch executor.
Their outer request identities are checked, but their internal effect selection is not proved.
Boot quarantine, retention/reference discovery and other maintenance policies have separate
authority and incomplete proof coverage. Below those policy boundaries, truthful observations,
durable acknowledgments, process resolution and faithful filesystem/Qubes execution remain
necessary environmental assumptions.

## Verification tooling

`make verify-prepare` explicitly installs the checksum-pinned Linux release and
its exact Rust toolchain, then verifies both domain libraries and vstd. `make verify` uses
prepared tools without provisioning them. The dedicated CI job runs the same
command, with the exact `vstd` version and transitive dependencies in Cargo.lock.
Ordinary Cargo and `test-quick` never install or run the prover. Tool preparation
uses Rust orchestration in `airlock-dev`; the [workspace pin](../../verus.json)
is shared by both crates and the Nix development shell.

Cargo tests exercise the erased executable code; compile-fail doctests enforce
identity distinctions, evidence-versus-authority separation and request lifetimes.
The dom0 `durable_handoff` namespace scenario also exercises the real persistence
and process adapters, using the existing Rust command peer. It needs bubblewrap
and the explicitly built native test fixtures; CI prepares both.

On NixOS, use the Rust-authored prepared applications shell:
`nix develop "$(make --silent prepare)/crates#verification" -c make verify-prepare`.
It packages the same checksum-pinned release with Nix library/interpreter fixes;
rustup supplies the pinned compiler in the user's toolchain directory. With the
tools prepared, verification uses `make verify` in that shell. This workstation
tooling is separate from deployed configuration. The pin targets x86_64 Linux.

Upstream guidance: [Cargo integration](https://verus-lang.github.io/verus/guide/cargo_verus.html)
and [pinned Verus release](https://github.com/verus-lang/verus/releases/tag/release/0.2026.10.18.7d82994).
