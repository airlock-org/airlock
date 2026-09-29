# Documentation map

Updated 2026-09-25 with physical network-isolation deployment and rollback
results. Other document summaries retain their stated validation boundaries.

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
  for usable, recoverable hardware isolation; not yet an implemented wizard.

## Active work

| Plan | Status and boundary |
| --- | --- |
| [First-run wizard](todo/first-run-setup-wizard.md) | Planned. Welcome/completion shell first; timed trials and rollback before disruptive pages. |
| [USB input setup](todo/usb-input-setup.md) | Planned. First hardware trial consumer; preserve storage and existing connectivity. Ownership depends on hardware. |
| [USB lab network follow-ups](todo/usb-lab-network-followups.md) | Focused tests, updates proxy, PCI Wi-Fi traffic, and an exact persistence sentinel remain. The wired topology passed physical acceptance. |
| [sys-ctrl](completed/05-sys-ctrl.md) | Completed controller deployment and fault-handling acceptance. |
| [Direct pool deployment](todo/06-direct-pool-machine-deployment.md) | Package admission and sparse root reuse; physical acceptance remains open. |
| [Unified host and guest store](todo/07-unified-host-guest-store.md) | Proposed shared object storage, early-boot host view and builder reuse of host objects. |
| [Cached offline builder check](completed/07-cached-offline-builder-check.md) | Completed image prerequisite that reuses passing offline acceptance for unchanged inputs. |

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
Completion is limited to each document's stated scope, not universal hardware
support. A successful build is not physical acceptance.

## Component and experimental documentation

[Config system](config-system.md) describes the separate sys-config-* workflow,
not current physical-lab deployment authority. [sys-gui](sys-gui.md) describes
the nested/hybrid desktop path, not the current physical desktop. Their names
alone do not imply that they describe every profile.

[GNOME](sys-gui-gnome.md) records verified desktop integration and deferred
appearance/window work. [Hyprland](sys-gui-hyprland.md) remains an unverified
physical desktop experiment; its old menu instructions are historical.
[sys-gpu](sys-gpu.md) is a separate optional hardware direction, not a desktop
prerequisite.

Deployable implementation belongs to `airlock-machine`; image/lab integration
belongs here. Older component notes are present in both repositories, with
different relative links. Use this index and this repository's runbooks for
lab operations; check pinned component source and its documentation for an
implementation change. These inherited copies are not independent authorities
for the current deployment topology. Consolidation should preserve links and
profile/evidence scope rather than blindly synchronizing text.

## Historical evidence and unresolved checks

- [Original USB plan](usb-dev-plan.md): superseded; original unbuilt/untested
  status and session restrictions do not apply to current work.
- [USB GUI lab](usb-gui-lab.md): earlier AMD experiment, including old boot,
  credentials, and networking procedures. Use current runbooks instead.
- [AMD investigation](amd-gpu-investigation.md), [first assignment](amd-gpu-first-assignment.md),
  [VBIOS trial](amd-gpu-vbios.md), and [reset findings](amd-gpu-assignment-reset-findings.md):
  dated hardware evidence. Later successful desktops do not prove reset safety,
  suspend/resume, or general hardware compatibility.
- [Pool builder proof](pool-builder-proof.md): historical precursor to the
  implemented pool-backed builder, not an active implementation checklist.

Historical files retain their paths to preserve existing links. Read their
dates and follow-up links before treating a "remaining" or "next" item as
current work. Preserve unresolved checks unless later evidence explicitly
verifies the same behavior; do not close them merely because a later boot worked.
