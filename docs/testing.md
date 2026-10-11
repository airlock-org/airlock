# Testing

Airlock exposes four test-suite targets. They are intentionally not aliases:

- `make test-quick`
- `make test-nix` (explicit Nix evaluations and builds)
- `make test-warm`
- `make test-slow`

## Quick

```bash
uv sync --locked
make test-quick
```

Component checks run directly from `airlock-machine/tests/` against that
checkout's runtime sources. Parent tests cover image assembly,
deployment transport, repository boundaries, and nested-VM orchestration.
The `TESTS="..."` names below still select component checks in the submodule;
`run-nested-gui-smoke-static` selects the parent orchestration check.

The checks include:

- Python unit and regression tests,
- setup/apply/status helper tests,
- staged candidate validation tests,
- host capability detection,
- shell syntax and static runner checks.

Prepare the locked Python dependencies explicitly before testing. Quick tests
use `uv run --no-sync`, so the test command does not install dependencies.
CI runs this layer on Linux without Nix or Xen and retains JUnit/JSON reports.

The quick layer should pass on ordinary Linux development hosts. It does not
prove that Xen boots or that Qubes services run.

By default, local tests and Nix builds use the shared Airlock store under
`${AIRLOCK_DATA_ROOT:-/run/media/user/data/airlock}/nix-root`, not the host
`/nix/store`. That shared store is expected and acts as the test/build cache.
Only set `AIRLOCK_USE_HOST_NIX_STORE=1` when you intentionally want test Nix
commands to use your host store.

`make test-quick` runs parallel-safe local tests with 4 pytest-xdist workers by
default. Tune or disable that worker count with:

```bash
make test-quick TEST_JOBS=4
make test-quick TEST_JOBS=auto
make test-quick TEST_JOBS=
```

Run selected tests by exact name, or pass normal pytest arguments through
`PYTEST_ARGS`:

```bash
make test-quick TESTS="host-capabilities setup-config-static"
make test-quick PYTEST_ARGS="-k topology"
make test-quick PYTEST_ARGS=--collect-only
```

## Nix checks (opt-in)

```bash
make test-nix
make test-nix TESTS="topology-schema"
```

This target runs checks marked `nix`, including both flake checks, topology and
system schema evaluation, evaluated module contracts and package builds. It is
separate from the regular feedback loop and is not a prerequisite of deployment.
`make test-quick` excludes these checks, including when selecting a Nix check by
name. Release builds still evaluate the configuration they need to build.

## Warm

```bash
make test-warm
```

Runs the reusable warm nested-VM suite. It keeps one nested dom0 alive, executes
ordered pytest-visible stages, and includes both config-authority coverage and
the standalone nix-closure domain lifecycle check.

Warm stages intentionally do not run in parallel: they share one mutable running
VM and depend on state from earlier stages. Select a single stage by exact name
when debugging:

```bash
make test-warm TESTS=nix-closure-domain-smoke
make test-warm TESTS=config-apply-resolve
```

## Slow

```bash
make test-slow
```

Runs cold nested-VM confidence coverage. By default it prepares one shared
baseline, starts isolated VM lanes in parallel for the config-authority flow
and nix-closure lifecycle flow, then runs the serial boot-activation flow. The
serial flow stages and verifies its own boot generation, so the default slow
suite does not run a separate boot-only flow first.

Tune the number of cold VM lanes with:

```bash
make test-slow TEST_VM_JOBS=2
```

Each lane gets its own VM directory, result-link directory, log directory, SSH
port, and qcow2 overlay while sharing the Nix store/cache. The lane-local VM
image result link is prepared from the shared prebuild and workers run with
`AIRLOCK_VM_SKIP_BASE_BUILD=1`, so they do not each re-resolve the same base
image with Nix. Slow parallel lanes use ports from
`VM_PARALLEL_SSH_BASE_PORT`, defaulting to `2300`, and the serial
boot-activation flow uses `VM_SLOW_SERIAL_VM_DIR` plus
`VM_SLOW_SERIAL_SSH_PORT`, defaulting to `2310`. That keeps slow runs separate
from the warm VM's default port and disk.

Multi-lane VM runs also set `AIRLOCK_SKIP_GC_AUTO=1` inside each lane to avoid
concurrent GC lock contention; run `make gc-auto` before or after a slow batch
when needed.

Selected slow tests run by exact `tests/test_vm.py` flow name. Parallel-safe
selections use the same shared-baseline lane runner by default; serial
selections run in the isolated serial VM path:

```bash
make test-slow TESTS=vm-config-flow
make test-slow TESTS="vm-config-flow vm-nix-closure-flow"
make test-slow TESTS=vm-config-boot-activate-flow
```

Full GUI smoke remains a manual VM target so GUI readiness does not block the
core qrexec/VM-lifecycle suites.

VM pytest targets stream each nested flow's stdout/stderr live by default and
also save the raw command output to that flow's `pytest-command.log`. The
default pytest capture mode is controlled by `PYTEST_VM_OUTPUT_ARGS`, which is
set to `--capture=tee-sys`; set `PYTEST_VM_OUTPUT_ARGS=` for quieter runs.

Direct pytest runs without `AIRLOCK_RUN_VM_TESTS=1` collect the VM tests but skip
them.

## Reports

All test-suite targets print the slowest tests with `--durations=20` and write
pytest reports under the log directory:

- `pytest-report.json`
- `pytest-junit.xml`
- `test-runtimes-latest.json`
- `test-runtimes-latest.tsv`
- `test-runtimes-history.jsonl`

Each shell check receives its own `AIRLOCK_LOG_DIR` subdirectory below the report
directory so parallel workers do not overwrite same-named artifacts.

Pytest initializes the shared Airlock environment without evaluating or building
Nix inputs. Storage format tests use `mkfs.erofs` and `fsck.erofs` from `PATH`;
tests that require unavailable tools report a skip. Test setup does not build
tools or launch Nix shells. The optional legacy dependency warm-up requires
`AIRLOCK_PYTEST_WARM_STORE=1`; `test-quick` always disables it.

Show the slowest tests from the latest run:

```bash
column -t -s $'\t' "${AIRLOCK_LOG_DIR:-.airlock-logs}/test-runtimes-latest.tsv"
```

## Build Layer

```bash
make build
```

Builds:

```text
.#nixosConfigurations.test.config.system.build.toplevel
```

This proves the NixOS dom0 closure can be built. A real target should replace
or extend `airlock-machine/lifecycle/image/profiles/test.nix` with its hardware-specific disk layout.

## Nested GUI Dom0

```bash
make vm-gui-config-image
make vm-gui-config-run
```

Builds and launches the current nested GUI dom0 image. The launcher uses
QEMU/KVM, OVMF, `-cpu host`, and a writable qcow2 overlay under the generated
VM directory. Defaults:

- 12288 MiB RAM
- 4 vCPUs
- SSH forwarded on `127.0.0.1:2222`
- user `airlock`
- password `airlock`

Useful overrides:

```bash
AIRLOCK_VM_MEMORY=16384 AIRLOCK_VM_SMP=6 make vm-gui-config-run
AIRLOCK_VM_SSH_PORT=2223 make vm-gui-config-run
AIRLOCK_KEEP_VM=1 make vm-config-flow
```

The config validator starts with 2048 MiB and can grow to 4096 MiB for NixOS
candidate evaluation. `make test-warm` uses 16384 MiB by default; override it
with `TEST_WARM_VM_MEMORY=20480` or `AIRLOCK_VM_MEMORY=20480`. A running warm VM retains its original outer RAM size.
After increasing `AIRLOCK_VM_MEMORY` or the launcher default, power off nested
dom0 and wait for QEMU to exit before running `make test-warm` again:

```bash
./lifecycle/misc/tools/scripts/ssh-nested-vm.sh 'sudo -n systemctl poweroff'
```

The next warm start reuses the existing disk; warm-baseline applies the updated
config-domain memory settings before validation.

Connect with:

```bash
make vm-ssh
```

## Config Authority Flow

```bash
make vm-config-flow
```

This is the main cold nested integration path for the current architecture. It
starts from a fresh writable nested dom0 disk, waits for Qubes services, runs
the setup command, and verifies:

- `sys-config-vault`, `sys-config-staging`, and `sys-config-validator` exist.
- each config VM is a Qubes `StandaloneVM`.
- each uses `backend.type = "nix-closure"`.
- each has a separate read-only `store` volume attached at `/dev/xvde`.
- `/nix/store` is mounted read-only at runtime.
- qrexec works in each config VM.
- GUI appmenus can be synchronized for the config VMs.
- private state under `/home/user` survives restart.
- `sys-config-staging` can modify and commit a candidate.
- `sys-config-validator` validates and evaluates the candidate.
- dom0 applies the resolved topology/system plan.
- `sys-config-vault` records candidate and deployment history.
- generated Airlock-owned qrexec policy is active.
- a staged Nix-native domain can be created and booted through qrexec.

Results are copied to:

```text
/run/media/user/data/airlock/logs/vm-gui-config-vms-smoke.json
```

The script name still contains `smoke` for historical reasons; the current
flow is an end-to-end runtime integration proof.

## Dom0 Generation Flow

```bash
make vm-config-boot-flow
make vm-config-boot-activate-flow
```

`make vm-config-boot-flow` runs the same candidate path but requests
`switch-to-configuration boot`, proving dom0 can install a next-boot NixOS
generation and report a pending reboot without changing the running generation.

`make vm-config-boot-activate-flow` preserves the same writable nested disk,
boots it again, and runs `airlock-config-boot-verify` in dom0. The verifier
checks that `/nix/var/nix/profiles/system` and `/run/current-system` resolve to
the staged generation and that status no longer reports a pending reboot.

## Closure Domain Flow

```bash
make vm-nix-closure-flow
```

Runs a narrower real Xen/Qubes test for one representative `nix-closure`
domain. It verifies the small root image, separate read-only `/nix/store`
volume, early boot mount path, qrexec, restart behavior, and absence of a full
duplicate Nix store in the root image.

## Conventional Qubes Guests

```bash
make vm-gui-smoke
```

Runs the broader GUI path that can download/import conventional Qubes Fedora
kernel/template RPMs, set Qubes defaults, run qrexec against a Fedora-backed
AppVM, and try to detect a Qube GUI window. Fedora/Debian TemplateVM support is
kept separate from NixOS `nix-closure` domains and should continue to pass this
path.

The Fedora template RPM is large. When `/run/media/user/data` is mounted,
generated VM disks, logs, caches, temp files, and Airlock Nix build outputs
default to:

```text
/run/media/user/data/airlock/
```

Inside nested dom0, cached Qubes RPM/template inputs are copied to:

```text
/var/cache/airlock/qubes-rpms/
```

## Generated State

The source tree, Nix code, scripts, docs, and Git history stay in
`/path/to/airlock`. The Airlock Nix store is a chroot local store at:

```text
/run/media/user/data/airlock/nix-root
```

Logical store paths remain `/nix/store/...`; the physical files live under the
data-disk root. Named build roots that should survive Airlock-only GC live under:

```text
/run/media/user/data/airlock/result-links
```

If local builds run low on disk:

```bash
make disk-usage
make gc-auto
make gc-roots
make gc-dry-run
make gc
make clean-vm-disks
make clean-vm-workdirs
make clean-logs
```

`make gc` refreshes Airlock-managed GC roots and deletes unreachable paths only
from the separate Airlock store. It refuses to operate on the host `/nix/store`.

## Physical Target

Nested Xen/KVM is useful for development, but production security validation
requires a physical Xen-capable test machine that can boot a Airlock dom0 image,
run Qubes control services, import conventional templates, boot closure-backed
NixOS domains, and archive logs/results for inspection.
