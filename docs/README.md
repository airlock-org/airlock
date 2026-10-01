# Documentation map

Updated 2026-10-01: bootstrap/normal-startup separation passed its first physical
reboot and operator checks; repeated unchanged-boot reuse and broader recovery
trials remain unverified. Boot-menu presentation, controller deployment, direct
pool admission and unified host/guest storage also passed their recorded
acceptance. Detailed summaries retain their boundaries.

## Placement rules

- Root: current operating guides, implemented mechanisms and design policy.
- `todo/`: outstanding implementation, investigations and acceptance, including
  implemented changes awaiting verification. Use the stated status to distinguish
  planned, deferred and awaiting-acceptance work.
- `completed/`: accepted plans, with their scope and evidence boundaries.
- `experiments/`: opt-in implementations and diagnostic instructions.
- `history/`: dated findings and superseded procedures. Old paths forward here.

Keep current acceptance work in a TODO and link it from the relevant reference
or historical report. A dated report's old pending checks are not automatically
current backlog items.

## Current operating guides

- [Repository boundaries](repositories.md): source ownership and submodule workflow.
- [USB development](usb-dev.md) and [USB lab](usb-lab.md): current image,
  provisioning, recovery, and physical topology. The normal Airlock entry uses
  physical-GPU GNOME; the deployed USB lab release routes sys-dev through
  sys-firewall and sys-net.
- [Machine deployment](machine-deployment.md): host and sys-dev submission,
  whole-machine activation, verification, rollback, and maintenance.
- [Pool-backed builder](sys-dev-build-store.md): implemented build-store behavior
  and its physical/automated validation boundaries.
- [Physical input](sys-gui-input.md): declared dom0-to-sys-gui forwarding;
  its dated experiment section is historical evidence, not generic USB setup.
- [Testing](testing.md): test layers and execution guidance.
- [Hardware usability policy](hardware-usability-policy.md): agreed requirements
  for usable, recoverable hardware isolation and future wizard trials.

## Work plans and completed acceptance

Assign each new numbered plan one greater than the highest number across both
`docs/todo/` and `docs/completed/`. Its number is permanent: preserve it when
moving the document to `completed`, and never reuse or renumber it. Existing
plan filenames retain their numbers. Forwarding pages preserve moved
historical paths.

| Plan | Status and boundary |
| --- | --- |
| [First-run wizard](todo/first-run-setup-wizard.md) | Welcome/completion shell implemented; physical GUI and reboot verification pending. Timed trials and rollback remain before disruptive pages. |
| [USB input setup](todo/usb-input-setup.md) | Planned. First hardware trial consumer; preserve storage and existing connectivity. Ownership depends on hardware. |
| [USB lab network follow-ups](todo/usb-lab-network-followups.md) | Focused tests, updates proxy, PCI Wi-Fi traffic, and an exact persistence sentinel remain. The wired topology passed physical acceptance. |
| [Deployment metadata and validation reuse](todo/08-deployment-metadata-validation-reuse.md) | Planned. Reduce repeated preparation work with operation-scoped caches; RAM staging is out of scope. |
| [Deployment code health](todo/09-deployment-code-health.md) | Planned. Preserve cleanup errors, use durable record writes, and simplify host publication. |
| [Internal compatibility cleanup](todo/11-simplify-internal-compatibility.md) | Local cleanup implemented; worktree retired and quick tests passed on main (681 passed, 59 skipped). Nix/EROFS checks and persisted-state acceptance remain pending; the development deployment route is retained. |
| [Live VM additions](completed/12-live-vm-additions.md) | Completed. Physical no-reboot addition, application launch, VM stop/start, later host boot and recoverable rollback/reapply passed with exact persistent data and unchanged existing VMs. Declaration simplification remains in TODO 13. |
| [Minimal VM declarations](todo/13-minimal-vm-declarations.md) | Planned. Derive builds, topology, deployment targets and live desktop integration from one small declaration per ordinary VM. |
| [Separate bootstrap from normal startup](completed/14-separate-bootstrap-from-normal-startup.md) | Completed for implemented scope. First reboot verified and login/input/application launch accepted; GUI configuration precedes background work. Repeated unchanged-boot reuse, fresh initialization and broader recovery checks remain unverified. |
| [GNOME Files launcher](todo/sys-gui-files-launcher.md) | Planned. Hide local Files in sys-gui, retain Nautilus for DING, and expose the usr-home launcher. |
| [Display consistency across VMs](todo/cross-vm-display-consistency.md) | Planned. Align cursors, scaling, fonts and appearance; use GNOME Console for utility terminals and verify display layout changes. |
| [Hyprland physical trial](todo/sys-gui-hyprland-trial.md) | Deferred. Configuration exists; physical Wayland desktop and cross-VM transport are unverified. |
| [sys-gpu hardware](todo/sys-gpu-hardware.md) | Deferred. Physical assignment, rendering, lifecycle and later inference work for an additional GPU. |
| [AMD GPU lifecycle](todo/amd-gpu-lifecycle.md) | Open investigation. Reset effectiveness, state isolation and supported lifecycle operations need evidence. |
| [Qubes restart race](todo/qubesd-restart-race.md) | Open investigation. Socket readiness and client recovery; the cause of the historical black-screen incident is unconfirmed. |
| [GNOME focused VM indicator](completed/gnome-focused-vm-indicator.md) | Completed. Deployed and operator accepted, including Calculator focus and window identity caching. Broader GUI checks and host-generation verification after reboot remain pending. |
| [sys-ctrl](completed/05-sys-ctrl.md) | Completed controller deployment and fault-handling acceptance. |
| [Direct pool deployment](completed/06-direct-pool-machine-deployment.md) | Completed package admission, sparse root reuse and physical update/rollback acceptance. |
| [Unified host and guest store](completed/07-unified-host-guest-store.md) | Completed shared storage, host boot/recovery, builder reuse and maintenance acceptance. |
| [Cached offline builder check](completed/07-cached-offline-builder-check.md) | Completed image prerequisite that reuses passing offline acceptance for unchanged inputs. |
| [Boot menu presentation](completed/10-boot-menu-presentation.md) | Completed. Commit labels, newest-first/default ordering, pruning and recovery placement; operator accepted menu text after reboot. Physical manual older-generation/default restoration was not separately exercised. |

Wizard and input work do not depend on implementing sys-ctrl. Network isolation
and sys-ctrl are distinct changes to the current lab. USB Wi-Fi routing has no
implementation plan yet; preserving existing USB connectivity is required,
but that does not promise support for new adapters.

The [completed network isolation trial](completed/usb-lab-network-isolation.md)
records wired provider recovery and whole-machine apply, rollback, and reapply.
The [completed trash purge and release pruning trial](completed/vm-trash-purge-release-pruning.md)
records the physical early-purge result and its shared-pool GC limit. The
[completed offline builder baseline](completed/06-builder-input-baseline.md)
records disconnected guest builds, pin retention through pool cleanup, and
the reboot trial for two routine configuration changes. The
[completed plans](completed/) record acceptance evidence for earlier work.
Their shared controller, direct-pool and unified-store trial timeline is in the
[2026-09-25 to 2026-09-29 acceptance report](history/machine-acceptance-2026-09-25-to-29.md).
Completion is limited to each document's stated scope, not universal hardware
support. A successful build is not physical acceptance.

## Component and experimental documentation

[Config system](config-system.md) describes the separate sys-config-* workflow,
not current physical-lab deployment authority. [sys-gui](sys-gui.md) describes
the nested/hybrid desktop path, not the current physical desktop. Their names
alone do not imply that they describe every profile.

[Automatic AMD VBIOS](amd-vbios-automatic.md) and
[declarative GPU assignment](gui-gpu-assignment.md) describe implemented
mechanisms with hardware-specific evidence boundaries.

[GNOME](sys-gui-gnome.md) records verified desktop integration and deferred
appearance/window work. [Hyprland](experiments/sys-gui-hyprland.md) remains an unverified
physical desktop experiment under `experiments/`; its old menu instructions
are historical.
[sys-gpu](experiments/sys-gpu.md) is a separate optional hardware direction, not a desktop
prerequisite.

Deployable implementation belongs to `airlock-machine`; image/lab integration
belongs here. Older component notes are present in both repositories, with
different relative links. Use this index and this repository's runbooks for
lab operations; check pinned component source and its documentation for an
implementation change. These inherited copies are not independent authorities
for the current deployment topology. Consolidation should preserve links and
profile/evidence scope rather than blindly synchronizing text.

## Historical evidence and unresolved checks

- [Original USB plan](history/usb-dev-plan.md): superseded; original unbuilt/untested
  status and session restrictions do not apply to current work.
- [USB GUI lab](history/usb-gui-lab.md): earlier AMD experiment, including old boot,
  credentials, and networking procedures. Use current runbooks instead.
- [AMD investigation](history/amd-gpu-investigation.md), [first assignment](history/amd-gpu-first-assignment.md),
  [VBIOS trial](history/amd-gpu-vbios.md), and [reset findings](history/amd-gpu-assignment-reset-findings.md):
  dated hardware evidence. Later successful desktops do not prove reset safety,
  suspend/resume, or general hardware compatibility.
- [Machine acceptance history](history/machine-acceptance-2026-09-25-to-29.md):
  shared deployment trials, failures, cleanup and final acceptance for plans 05–07.
- [Pool builder proof](history/pool-builder-proof.md): historical precursor to the
  implemented pool-backed builder, not an active implementation checklist.

Historical reports live under `history/`; forwarding pages preserve their old
paths and section links. Read their dates and follow-up links before treating
a "remaining" or "next" item as
current work. Preserve unresolved checks unless later evidence explicitly
verifies the same behavior; do not close them merely because a later boot worked.
