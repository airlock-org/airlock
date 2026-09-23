SHELL := bash

ifneq ($(wildcard /run/media/user/data/.),)
AIRLOCK_DATA_ROOT ?= /run/media/user/data/airlock
endif

ifneq ($(strip $(AIRLOCK_DATA_ROOT)),)
AIRLOCK_VM_DIR ?= $(AIRLOCK_DATA_ROOT)/vm
AIRLOCK_LOG_DIR ?= $(AIRLOCK_DATA_ROOT)/logs
AIRLOCK_QUBES_CACHE_DIR ?= $(AIRLOCK_DATA_ROOT)/cache/rpms
AIRLOCK_RESULT_LINK_DIR ?= $(AIRLOCK_DATA_ROOT)/result-links
AIRLOCK_NIX_ROOT ?= $(AIRLOCK_DATA_ROOT)/nix-root
AIRLOCK_NIX_STORE ?= $(AIRLOCK_NIX_ROOT)
TMPDIR ?= $(AIRLOCK_DATA_ROOT)/tmp
XDG_CACHE_HOME ?= $(AIRLOCK_DATA_ROOT)/cache/xdg
export AIRLOCK_DATA_ROOT
export AIRLOCK_VM_DIR
export AIRLOCK_LOG_DIR
export AIRLOCK_QUBES_CACHE_DIR
export AIRLOCK_RESULT_LINK_DIR
export AIRLOCK_NIX_ROOT
export AIRLOCK_NIX_STORE
export TMPDIR
export XDG_CACHE_HOME
endif

AIRLOCK_NIX ?= ./lifecycle/misc/tools/scripts/airlock-nix.sh
VM_IMAGE_MODE ?= dev
ifeq ($(VM_IMAGE_MODE),dev)
VM_IMAGE_MODE_SUFFIX := dev
else ifeq ($(VM_IMAGE_MODE),release)
VM_IMAGE_MODE_SUFFIX := release
else
$(error VM_IMAGE_MODE must be dev or release)
endif
VM_IMAGE_ATTR ?= airlock-vm-$(VM_IMAGE_MODE_SUFFIX)-image
VM_GUI_IMAGE_ATTR ?= airlock-vm-gui-$(VM_IMAGE_MODE_SUFFIX)-image
VM_GUI_CONFIG_IMAGE_ATTR ?= airlock-vm-gui-config-$(VM_IMAGE_MODE_SUFFIX)-image
VM_GUI_CONFIG_NETWORK_DEBUG_IMAGE_ATTR ?= airlock-vm-gui-config-network-debug-image
VM_GUI_MINIMAL_IMAGE_ATTR ?= airlock-vm-gui-minimal-image
VM_GUI_MINIMAL_DIR ?= $(or $(AIRLOCK_DATA_ROOT),$(CURDIR)/.airlock)/vm-gui-minimal
VM_GUI_MINIMAL_SSH_PORT ?= 2332
AIRLOCK_VM_GDK_BACKEND ?= x11
VM_PARALLEL_IMAGE_ATTR ?= $(VM_GUI_CONFIG_IMAGE_ATTR)
VM_PARALLEL_IMAGE_LINK ?= $(AIRLOCK_RESULT_LINK_DIR)/$(VM_PARALLEL_IMAGE_ATTR)-parallel-shared-result
VM_PARALLEL_BASELINE_ROOT ?= $(or $(AIRLOCK_DATA_ROOT),$(CURDIR)/.airlock)/vm-baselines
VM_PARALLEL_BASELINE_DIR ?= $(VM_PARALLEL_BASELINE_ROOT)/$(VM_PARALLEL_IMAGE_ATTR)-setup
VM_PARALLEL_BASELINE_DISK ?= $(VM_PARALLEL_BASELINE_DIR)/$(VM_PARALLEL_IMAGE_ATTR).qcow2
VM_PARALLEL_BASELINE_OVMF_VARS ?= $(VM_PARALLEL_BASELINE_DIR)/$(VM_PARALLEL_IMAGE_ATTR)-OVMF_VARS.fd
VM_PARALLEL_BASELINE_LOG_DIR ?= $(or $(AIRLOCK_LOG_DIR),$(CURDIR)/.airlock-logs)/vm-baseline
VM_PARALLEL_BASELINE_SSH_PORT ?= 2299
VM_PARALLEL_SSH_BASE_PORT ?= 2300
VM_SLOW_SERIAL_VM_DIR ?= $(or $(AIRLOCK_DATA_ROOT),$(CURDIR)/.airlock)/vm-slow-serial
VM_SLOW_SERIAL_SSH_PORT ?= 2310
VM_FLOW_RUN_SETUP_CONFIG ?= 1

VM_PREBUILD_ATTRS := \
	packages.x86_64-linux.airlock-setup-config \
	packages.x86_64-linux.airlock-config-inputs-import \
	packages.x86_64-linux.airlock-config-vms-smoke \
	packages.x86_64-linux.airlock-nix-closure-domain-smoke \
	packages.x86_64-linux.airlock-qubes-storage \
	packages.x86_64-linux.airlock-qubes-vm-kernel \
	$(VM_GUI_CONFIG_IMAGE_ATTR)

PYTHON_FILES := \
	airlock-machine/lifecycle/runtime/tools/machine-deploy.py \
	airlock-machine/lifecycle/runtime/tools/airlock_machine_deploy/*.py \
	airlock-machine/lifecycle/runtime/tools/sys-dev-machine-deploy.py \
	lifecycle/misc/tools/scripts/machine-deploy.py \
	lifecycle/misc/tools/scripts/sys-dev-machine-source.py \
	tests/test_machine_deploy_client.py \
	tests/test_sys_dev_machine_source.py \
	airlock-machine/tests/test_sys_dev_machine_build.py \
	airlock-machine/tests/test_sys_dev_machine_deploy.py \
	airlock-machine/tests/test_qubes_guest_session.py \
	airlock-machine/lifecycle/misc/tools/pkgs/airlock-gui-gpu-check/src/check.py \
	airlock-machine/tests/test_gui_gpu_assignment.py \
	airlock-machine/lifecycle/runtime/tools/scripts/input-events.py \
	airlock-machine/tests/test_input_events.py \
	airlock-machine/tests/test_gui_gpu_boot_order.py \
	airlock-machine/tests/test_dev_dom0_terminal.py \
	airlock-machine/tests/test_qubes_gui_window_state.py \
	airlock-machine/lifecycle/misc/tools/scripts/boot-framebuffer.py \
	airlock-machine/lifecycle/misc/tools/scripts/boot-menu.py \
	airlock-machine/tests/test_boot_framebuffer.py \
	airlock-machine/tests/test_boot_entry_labels.py \
	airlock-machine/tests/test_boot_menu.py \
	tests/test_prepared_store.py \
	airlock-machine/lifecycle/runtime/tools/pkgs/airlock-amd-vbios/src/airlock_amd_vbios/*.py \
	lifecycle/image/tools/amd-vbios.py \
	airlock-machine/tests/test_amd_vbios.py \
	airlock-machine/tests/test_amd_vbios_runtime.py \
	airlock-machine/tests/test_amd_vbios_libvirt.py \
	airlock-machine/tests/test_amd_vbios_events.py \
	airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/__init__.py \
	airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/cli.py \
	airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/engine.py \
	airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/*.py \
	airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/qubes.py \
	airlock-machine/lifecycle/runtime/tools/scripts/nix-domain-storage.py \
	airlock-machine/lifecycle/runtime/tools/scripts/sys-dev-build-store.py \
	tests/conftest.py \
	airlock-machine/tests/local_harness.py \
	airlock-machine/tests/machine_pychecks.py \
	tests/run_local.py \
	tests/test_local.py \
	airlock-machine/tests/test_machine_local.py \
	airlock-machine/tests/test_machine_deploy.py \
	airlock-machine/tests/test_bootstrap_network.py \
	tests/test_vm.py \
	tests/test_vm_staged.py \
	tests/test_gpu.py \
	airlock-machine/tests/test_gpu_probe.py \
	airlock-machine/tests/test_gui_minimal.py \
	tests/test_usb_dev.py \
	lifecycle/image/tools/usb-dev.py \
	lifecycle/image/tools/amd-preflight.py \
	airlock-machine/lifecycle/misc/tools/pkgs/airlock-gpu-probe/src/gpu-probe.py

PYTEST_LOG_DIR = $${AIRLOCK_LOG_DIR:-.airlock-logs}
TEST_JOBS ?= 4
TEST_VM_JOBS ?= 2
TEST_WARM_VM_MEMORY ?= 16384
PYTEST_CONFIG_ARGS = -c "$(CURDIR)/pyproject.toml" --rootdir "$(CURDIR)"
PYTEST_WORKERS = $(if $(TEST_JOBS),-n $(TEST_JOBS),)
PYTEST_DISTRIBUTION = $(if $(TEST_JOBS),--dist loadgroup,)
PYTEST_REPORT_ARGS = $(PYTEST_CONFIG_ARGS) --durations=20 --junitxml "$(PYTEST_LOG_DIR)/pytest-junit.xml" --json-report --json-report-file "$(PYTEST_LOG_DIR)/pytest-report.json"
PYTEST_VM_OUTPUT_ARGS ?= --capture=tee-sys
PYTEST_VM_STAGED_ARGS ?= --dist loadgroup
# Run component tests from their canonical checkout, plus parent integration tests.
PYTEST_DEFAULT_TARGETS = $(if $(TESTS),,airlock-machine/tests tests/test_repository_split.py tests/test_local.py tests/test_gpu.py tests/test_usb_dev.py tests/test_usb_lab.py tests/test_prepared_store.py tests/test_machine_deploy_client.py tests/test_sys_dev_machine_source.py)
PYTEST_SELECTED = $(foreach test,$(TESTS),'$(if $(filter run-nested-gui-smoke-static,$(test)),tests/test_local.py,airlock-machine/tests/test_machine_local.py)::test_local_check[$(test)]')

# One committed configuration, one whole-machine release. No per-VM switches.
DEPLOY_PROFILE ?= usb-lab
DEPLOY_TARGET ?=
.PHONY: machine-build machine-plan machine-apply machine-status machine-verify machine-resume machine-rollback machine-guest-plan machine-guest-apply sys-dev-machine-source
machine-build machine-plan machine-apply machine-status machine-verify machine-resume machine-rollback:
	uv run python lifecycle/misc/tools/scripts/machine-deploy.py $(patsubst machine-%,%,$@) --profile "$(DEPLOY_PROFILE)" $(if $(DEPLOY_TARGET),--target "$(DEPLOY_TARGET)",)
machine-guest-plan machine-guest-apply:
	@test -n "$(DEPLOY_TARGET)" || { echo 'set DEPLOY_TARGET to the Tailscale sys-dev host'; exit 2; }
	@test -n "$(ADMISSION_RECEIPT)" || { echo 'set ADMISSION_RECEIPT to the receipt path inside sys-dev'; exit 2; }
	uv run python lifecycle/misc/tools/scripts/machine-deploy.py $(patsubst machine-%,%,$@) --profile "$(DEPLOY_PROFILE)" --target "$(DEPLOY_TARGET)" --receipt "$(ADMISSION_RECEIPT)" $(if $(filter 1,$(UPDATE_COORDINATOR)),--update-coordinator)
sys-dev-machine-source:
	@test -n "$(DEPLOY_TARGET)" || { echo 'set DEPLOY_TARGET to the Tailscale sys-dev host'; exit 2; }
	uv run python lifecycle/misc/tools/scripts/sys-dev-machine-source.py push --target "$(DEPLOY_TARGET)"
VM_STAGED_PYTEST_SELECTED = $(foreach test,$(TESTS),'tests/test_vm_staged.py::test_staged_vm_step[$(test)]')
VM_SLOW_DEFAULT_PARALLEL_FLOW_IDS = vm-config-flow vm-nix-closure-flow
VM_SLOW_PARALLEL_FLOW_IDS = $(VM_SLOW_DEFAULT_PARALLEL_FLOW_IDS) vm-config-boot-flow
VM_SLOW_SERIAL_FLOW_IDS = vm-config-boot-activate-flow vm-pool-admission-flow
VM_SLOW_FLOW_IDS = $(VM_SLOW_PARALLEL_FLOW_IDS) $(VM_SLOW_SERIAL_FLOW_IDS)
VM_SLOW_SELECTED_PARALLEL_FLOW_IDS = $(filter $(VM_SLOW_PARALLEL_FLOW_IDS),$(TESTS))
VM_SLOW_SELECTED_SERIAL_FLOW_IDS = $(filter $(VM_SLOW_SERIAL_FLOW_IDS),$(TESTS))
VM_SLOW_SELECTED_UNKNOWN_FLOW_IDS = $(filter-out $(VM_SLOW_FLOW_IDS),$(TESTS))
VM_SLOW_DEFAULT_PARALLEL_TARGETS = $(foreach test,$(VM_SLOW_DEFAULT_PARALLEL_FLOW_IDS),'tests/test_vm.py::test_vm_flow[$(test)]')
VM_SLOW_SELECTED_PARALLEL_TARGETS = $(foreach test,$(VM_SLOW_SELECTED_PARALLEL_FLOW_IDS),'tests/test_vm.py::test_vm_flow[$(test)]')
VM_SLOW_SELECTED_SERIAL_TARGETS = $(foreach test,$(VM_SLOW_SELECTED_SERIAL_FLOW_IDS),'tests/test_vm.py::test_vm_flow[$(test)]')
VM_SLOW_SERIAL_TARGETS = $(foreach test,$(VM_SLOW_SERIAL_FLOW_IDS),'tests/test_vm.py::test_vm_flow[$(test)]')

.PHONY: ensure-generated-dirs build test-quick test-warm test-slow py-sync py-check py-lint py-complexity py-complexity-check install-git-hooks vm-prebuild-shared vm-parallel-prebuild-image vm-parallel-prebuild-baseline vm-image vm-image-dry-run vm-run vm-ssh vm-gui-image vm-gui-image-dry-run vm-gui-run vm-gui-config-image vm-gui-config-image-dry-run vm-gui-config-run vm-gui-config-network-debug-image vm-gui-config-network-debug-run vm-gui-check vm-gui-smoke vm-warm-start vm-warm-sync vm-sync-codex-auth vm-warm-apply vm-warm-test-config vm-config-setup vm-config-flow vm-config-warm-flow vm-config-boot-flow vm-config-boot-activate-flow vm-nix-closure-flow vm-nix-closure-warm-flow disk-usage gc-auto gc-roots gc-dry-run gc clean clean-generated clean-vm clean-vm-disks clean-result-links clean-logs clean-cache

ensure-generated-dirs:
ifneq ($(strip $(AIRLOCK_DATA_ROOT)),)
	@mkdir -p "$(AIRLOCK_VM_DIR)" "$(AIRLOCK_LOG_DIR)" "$(AIRLOCK_QUBES_CACHE_DIR)" "$(AIRLOCK_RESULT_LINK_DIR)" "$(AIRLOCK_NIX_ROOT)" "$(TMPDIR)" "$(XDG_CACHE_HOME)"
else
	@:
endif

build: ensure-generated-dirs
	$(AIRLOCK_NIX) build .#nixosConfigurations.test.config.system.build.toplevel --out-link "$(AIRLOCK_RESULT_LINK_DIR)/test-system" --print-build-logs --show-trace

test-quick: ensure-generated-dirs
	@mkdir -p "$(PYTEST_LOG_DIR)"
	uv run pytest $(PYTEST_DEFAULT_TARGETS) $(PYTEST_SELECTED) $(PYTEST_WORKERS) $(PYTEST_DISTRIBUTION) $(PYTEST_REPORT_ARGS) $(PYTEST_ARGS)

.PHONY: vm-gui-minimal-image vm-gui-minimal-run vm-gui-minimal-ssh

.PHONY: usb-dev-image usb-dev-write usb-dev-provision usb-dev-help

USB_DEV_PROVISION = sudo python3 lifecycle/image/tools/usb-dev.py provision-latest --result-link "$(or $(AIRLOCK_RESULT_LINK_DIR),$(CURDIR)/.airlock)/usb-dev-image" --nix-root "$(AIRLOCK_NIX_ROOT)" --serial "$(USB_SERIAL)" --resize-backup-dir "$(or $(AIRLOCK_LOG_DIR),/var/log/airlock)/usb-root-expansion" --repo "$(CURDIR)" --public-key "$(HOME)/.ssh/id_ed25519.pub" --auth-file "$(HOME)/.codex/auth.json" --insecure-dev-password --insecure-plain-codex-auth

.PHONY: usb-dom0-console-image usb-dom0-console-write

usb-dom0-console-image: ensure-generated-dirs
	$(AIRLOCK_NIX) build .#airlock-usb-dom0-console-image --out-link "$(or $(AIRLOCK_RESULT_LINK_DIR),$(CURDIR)/.airlock)/usb-dom0-console-image" --print-build-logs

usb-dom0-console-write:
	@test -n "$(USB_SERIAL)" || (echo "Set USB_SERIAL to the exact USB serial" >&2; exit 2)
	sudo python3 lifecycle/image/tools/usb-dev.py write-latest --result-link "$(or $(AIRLOCK_RESULT_LINK_DIR),$(CURDIR)/.airlock)/usb-dom0-console-image" --nix-root "$(AIRLOCK_NIX_ROOT)" --serial "$(USB_SERIAL)" --write

usb-dev-image: ensure-generated-dirs
	@seed_archive="$(or $(AIRLOCK_TMPDIR),$(CURDIR)/.airlock)/airlock-machine-seed.tar.gz"; \
	uv run python lifecycle/misc/tools/scripts/sys-dev-machine-source.py seed --output "$$seed_archive" >/dev/null; \
	AIRLOCK_MACHINE_SEED_ARCHIVE="$$seed_archive" $(AIRLOCK_NIX) build --impure .#airlock-usb-dev-image --out-link "$(or $(AIRLOCK_RESULT_LINK_DIR),$(CURDIR)/.airlock)/usb-dev-image" --print-build-logs

usb-dev-write:
	@test -n "$(USB_SERIAL)" || (echo "Set USB_SERIAL to the exact USB serial, e.g. make usb-dev-write USB_SERIAL=25110814640207" >&2; exit 2)
	sudo python3 lifecycle/image/tools/usb-dev.py write-latest --result-link "$(or $(AIRLOCK_RESULT_LINK_DIR),$(CURDIR)/.airlock)/usb-dev-image" --nix-root "$(AIRLOCK_NIX_ROOT)" --serial "$(USB_SERIAL)" --write

usb-dev-provision:
	@test -n "$(USB_SERIAL)" || (echo "Set USB_SERIAL to the exact USB serial, e.g. make usb-dev-provision USB_SERIAL=25110814640207" >&2; exit 2)
	@test -f "$(HOME)/.ssh/id_ed25519.pub" || (echo "Missing $(HOME)/.ssh/id_ed25519.pub" >&2; exit 2)
	@test -f "$(HOME)/.codex/auth.json" || (echo "Missing $(HOME)/.codex/auth.json" >&2; exit 2)
	$(USB_DEV_PROVISION)

usb-dev-help:
	python3 lifecycle/image/tools/usb-dev.py --help

vm-gui-minimal-image: ensure-generated-dirs
	$(AIRLOCK_NIX) build .#$(VM_GUI_MINIMAL_IMAGE_ATTR) --out-link "$(AIRLOCK_RESULT_LINK_DIR)/gui-minimal-image" --print-build-logs

vm-gui-minimal-run: ensure-generated-dirs
	AIRLOCK_VM_DIR="$(VM_GUI_MINIMAL_DIR)" AIRLOCK_VM_SSH_PORT="$(VM_GUI_MINIMAL_SSH_PORT)" AIRLOCK_VM_IMAGE_ATTR="$(VM_GUI_MINIMAL_IMAGE_ATTR)" AIRLOCK_VM_MEMORY="$${AIRLOCK_VM_MEMORY:-8192}" AIRLOCK_VM_SYSNET_NIC=0 ./lifecycle/misc/tools/scripts/run-nested-vm.sh

vm-gui-minimal-ssh:
	AIRLOCK_VM_SSH_PORT="$(VM_GUI_MINIMAL_SSH_PORT)" ./lifecycle/misc/tools/scripts/ssh-nested-vm.sh

.PHONY: vm-gpu-lab-image vm-gpu-lab-run vm-gpu-lab-test

vm-gpu-lab-image: ensure-generated-dirs
	$(AIRLOCK_NIX) build .#airlock-vm-gpu-lab-image --out-link "$(AIRLOCK_RESULT_LINK_DIR)/sys-gpu-lab-image" --print-build-logs

vm-gpu-lab-run: ensure-generated-dirs
	AIRLOCK_VM_DIR="$(or $(AIRLOCK_DATA_ROOT),$(CURDIR)/.airlock)/vm-gpu-lab" AIRLOCK_VM_SSH_PORT=2322 AIRLOCK_VM_IMAGE_ATTR=airlock-vm-gpu-lab-image ./lifecycle/misc/tools/scripts/run-nested-vm.sh

vm-gpu-lab-test: ensure-generated-dirs
	@mkdir -p "$(PYTEST_LOG_DIR)"
	AIRLOCK_RUN_GPU_LAB_TESTS=1 uv run pytest tests/test_gpu.py -k lab_vm $(PYTEST_CONFIG_ARGS) --durations=20 --junitxml "$(PYTEST_LOG_DIR)/gpu-lab-junit.xml" --json-report --json-report-file "$(PYTEST_LOG_DIR)/gpu-lab-report.json" $(PYTEST_VM_OUTPUT_ARGS)

test-warm: ensure-generated-dirs
	@mkdir -p "$(PYTEST_LOG_DIR)"
	AIRLOCK_RUN_VM_TESTS=1 AIRLOCK_SKIP_GC_AUTO=1 AIRLOCK_VM_MEMORY="$${AIRLOCK_VM_MEMORY:-$(TEST_WARM_VM_MEMORY)}" uv run pytest $(if $(TESTS),$(VM_STAGED_PYTEST_SELECTED),tests/test_vm_staged.py) $(PYTEST_VM_STAGED_ARGS) $(PYTEST_REPORT_ARGS) $(PYTEST_VM_OUTPUT_ARGS) $(PYTEST_ARGS)

test-slow: export AIRLOCK_SKIP_GC_AUTO = 1
test-slow: $(if $(TESTS),$(if $(VM_SLOW_SELECTED_PARALLEL_FLOW_IDS),vm-parallel-prebuild-baseline,ensure-generated-dirs),vm-parallel-prebuild-baseline)
	@mkdir -p "$(PYTEST_LOG_DIR)"
ifneq ($(strip $(TESTS)),)
ifneq ($(strip $(VM_SLOW_SELECTED_UNKNOWN_FLOW_IDS)),)
	$(error unknown slow TESTS value(s): $(VM_SLOW_SELECTED_UNKNOWN_FLOW_IDS))
endif
ifneq ($(strip $(VM_SLOW_SELECTED_PARALLEL_FLOW_IDS)),)
	AIRLOCK_RUN_VM_TESTS=1 AIRLOCK_VM_PARALLEL_LANES="$(TEST_VM_JOBS)" AIRLOCK_VM_PARALLEL_SSH_BASE_PORT="$(VM_PARALLEL_SSH_BASE_PORT)" AIRLOCK_VM_PARALLEL_IMAGE_ATTR="$(VM_PARALLEL_IMAGE_ATTR)" AIRLOCK_VM_PARALLEL_IMAGE_LINK="$(VM_PARALLEL_IMAGE_LINK)" AIRLOCK_VM_PARALLEL_BASELINE_DISK="$(VM_PARALLEL_BASELINE_DISK)" AIRLOCK_VM_PARALLEL_BASELINE_OVMF_VARS="$(VM_PARALLEL_BASELINE_OVMF_VARS)" uv run pytest $(VM_SLOW_SELECTED_PARALLEL_TARGETS) -n $(TEST_VM_JOBS) --dist load $(PYTEST_REPORT_ARGS) $(PYTEST_VM_OUTPUT_ARGS) $(PYTEST_ARGS)
endif
ifneq ($(strip $(VM_SLOW_SELECTED_SERIAL_FLOW_IDS)),)
	AIRLOCK_RUN_VM_TESTS=1 AIRLOCK_VM_DIR="$(VM_SLOW_SERIAL_VM_DIR)" AIRLOCK_VM_SSH_PORT="$(VM_SLOW_SERIAL_SSH_PORT)" uv run pytest $(VM_SLOW_SELECTED_SERIAL_TARGETS) $(PYTEST_REPORT_ARGS) $(PYTEST_VM_OUTPUT_ARGS) $(PYTEST_ARGS)
endif
else
	AIRLOCK_RUN_VM_TESTS=1 AIRLOCK_VM_PARALLEL_LANES="$(TEST_VM_JOBS)" AIRLOCK_VM_PARALLEL_SSH_BASE_PORT="$(VM_PARALLEL_SSH_BASE_PORT)" AIRLOCK_VM_PARALLEL_IMAGE_ATTR="$(VM_PARALLEL_IMAGE_ATTR)" AIRLOCK_VM_PARALLEL_IMAGE_LINK="$(VM_PARALLEL_IMAGE_LINK)" AIRLOCK_VM_PARALLEL_BASELINE_DISK="$(VM_PARALLEL_BASELINE_DISK)" AIRLOCK_VM_PARALLEL_BASELINE_OVMF_VARS="$(VM_PARALLEL_BASELINE_OVMF_VARS)" uv run pytest $(VM_SLOW_DEFAULT_PARALLEL_TARGETS) -n $(TEST_VM_JOBS) --dist load $(PYTEST_REPORT_ARGS) $(PYTEST_VM_OUTPUT_ARGS) $(PYTEST_ARGS)
	AIRLOCK_RUN_VM_TESTS=1 AIRLOCK_VM_DIR="$(VM_SLOW_SERIAL_VM_DIR)" AIRLOCK_VM_SSH_PORT="$(VM_SLOW_SERIAL_SSH_PORT)" AIRLOCK_SKIP_GC_AUTO=1 uv run pytest $(VM_SLOW_SERIAL_TARGETS) $(PYTEST_REPORT_ARGS) $(PYTEST_VM_OUTPUT_ARGS) $(PYTEST_ARGS)
endif

py-sync:
	uv sync --only-dev

py-lint:
	uv run ruff check $(PYTHON_FILES)

py-check:
	uv run ruff check $(PYTHON_FILES)
	uv run python -m py_compile $(PYTHON_FILES)

py-complexity:
	uv run radon cc -s -a $(PYTHON_FILES)
	uv run radon mi -s $(PYTHON_FILES)

py-complexity-check:
	uv run xenon --max-absolute E --max-modules C --max-average A $(PYTHON_FILES)

install-git-hooks:
	install -m 0755 lifecycle/misc/tools/scripts/git-hooks/pre-commit .git/hooks/pre-commit

vm-prebuild-shared: ensure-generated-dirs gc-auto
	$(AIRLOCK_NIX) build $(foreach attr,$(VM_PREBUILD_ATTRS),.#$(attr)) --no-link --print-build-logs --show-trace

vm-parallel-prebuild-image: ensure-generated-dirs gc-auto
	@mkdir -p "$(PYTEST_LOG_DIR)"
	./lifecycle/misc/tools/scripts/resolve-vm-image-result.sh "$(VM_PARALLEL_IMAGE_ATTR)" "$(VM_PARALLEL_IMAGE_LINK)" > "$(PYTEST_LOG_DIR)/vm-parallel-shared-image-path.txt"

vm-parallel-prebuild-baseline: vm-parallel-prebuild-image
	@mkdir -p "$(PYTEST_LOG_DIR)" "$(VM_PARALLEL_BASELINE_LOG_DIR)"
	AIRLOCK_VM_IMAGE_ATTR="$(VM_PARALLEL_IMAGE_ATTR)" AIRLOCK_VM_SHARED_IMAGE_LINK="$(VM_PARALLEL_IMAGE_LINK)" AIRLOCK_VM_BASELINE_DIR="$(VM_PARALLEL_BASELINE_DIR)" AIRLOCK_VM_BASELINE_DISK="$(VM_PARALLEL_BASELINE_DISK)" AIRLOCK_VM_BASELINE_OVMF_VARS="$(VM_PARALLEL_BASELINE_OVMF_VARS)" AIRLOCK_VM_BASELINE_LOG_DIR="$(VM_PARALLEL_BASELINE_LOG_DIR)" AIRLOCK_VM_BASELINE_SSH_PORT="$(VM_PARALLEL_BASELINE_SSH_PORT)" ./lifecycle/misc/tools/scripts/prepare-vm-baseline.sh

vm-image: ensure-generated-dirs gc-auto
	$(AIRLOCK_NIX) build .#$(VM_IMAGE_ATTR) --out-link "$(AIRLOCK_RESULT_LINK_DIR)/$(VM_IMAGE_ATTR)" --print-build-logs --show-trace

vm-image-dry-run: ensure-generated-dirs
	$(AIRLOCK_NIX) build .#$(VM_IMAGE_ATTR) --dry-run --show-trace

vm-run: ensure-generated-dirs gc-auto
	AIRLOCK_VM_IMAGE_ATTR="$(VM_IMAGE_ATTR)" ./lifecycle/misc/tools/scripts/run-nested-vm.sh

vm-ssh: ensure-generated-dirs
	./lifecycle/misc/tools/scripts/ssh-nested-vm.sh

vm-gui-image: ensure-generated-dirs gc-auto
	$(AIRLOCK_NIX) build .#$(VM_GUI_IMAGE_ATTR) --out-link "$(AIRLOCK_RESULT_LINK_DIR)/$(VM_GUI_IMAGE_ATTR)" --print-build-logs --show-trace

vm-gui-image-dry-run: ensure-generated-dirs
	$(AIRLOCK_NIX) build .#$(VM_GUI_IMAGE_ATTR) --dry-run --show-trace

vm-gui-run: ensure-generated-dirs gc-auto
	AIRLOCK_VM_IMAGE_ATTR="$(VM_GUI_IMAGE_ATTR)" AIRLOCK_VM_DISPLAY=gtk ./lifecycle/misc/tools/scripts/run-nested-vm.sh

vm-gui-config-image: ensure-generated-dirs gc-auto
	$(AIRLOCK_NIX) build .#$(VM_GUI_CONFIG_IMAGE_ATTR) --out-link "$(AIRLOCK_RESULT_LINK_DIR)/$(VM_GUI_CONFIG_IMAGE_ATTR)" --print-build-logs --show-trace

vm-gui-config-image-dry-run: ensure-generated-dirs
	$(AIRLOCK_NIX) build .#$(VM_GUI_CONFIG_IMAGE_ATTR) --dry-run --show-trace

vm-gui-config-run: ensure-generated-dirs gc-auto
	GDK_BACKEND="$(AIRLOCK_VM_GDK_BACKEND)" AIRLOCK_VM_IMAGE_ATTR="$(VM_GUI_CONFIG_IMAGE_ATTR)" AIRLOCK_VM_DISPLAY=gtk ./lifecycle/misc/tools/scripts/run-nested-vm.sh

vm-gui-config-network-debug-image: ensure-generated-dirs gc-auto
	$(AIRLOCK_NIX) build .#$(VM_GUI_CONFIG_NETWORK_DEBUG_IMAGE_ATTR) --out-link "$(AIRLOCK_RESULT_LINK_DIR)/$(VM_GUI_CONFIG_NETWORK_DEBUG_IMAGE_ATTR)" --print-build-logs --show-trace

vm-gui-config-network-debug-run: ensure-generated-dirs gc-auto
	AIRLOCK_VM_IMAGE_ATTR="$(VM_GUI_CONFIG_NETWORK_DEBUG_IMAGE_ATTR)" AIRLOCK_VM_DISPLAY=gtk ./lifecycle/misc/tools/scripts/run-nested-vm.sh

vm-gui-check: ensure-generated-dirs gc-auto
	AIRLOCK_AUTO_GC_AFTER_RUN=1 AIRLOCK_BOOTSTRAP_QUBES=0 AIRLOCK_RUN_QUBES_SMOKE=0 AIRLOCK_RUN_GUI_SMOKE=0 ./lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh

vm-gui-smoke: ensure-generated-dirs gc-auto
	AIRLOCK_AUTO_GC_AFTER_RUN=1 ./lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh

vm-warm-start: ensure-generated-dirs
	@if ./lifecycle/misc/tools/scripts/ssh-nested-vm.sh true >/dev/null 2>&1; then \
		printf 'nested dom0 already reachable on AIRLOCK_VM_SSH_PORT=%s\n' "$${AIRLOCK_VM_SSH_PORT:-2222}"; \
	else \
		AIRLOCK_KEEP_VM=1 AIRLOCK_VM_SKIP_BASE_BUILD=0 AIRLOCK_VM_IMAGE_ATTR="$(VM_GUI_CONFIG_IMAGE_ATTR)" AIRLOCK_VM_DISPLAY=none AIRLOCK_VM_FRESH=0 AIRLOCK_BOOTSTRAP_QUBES=0 AIRLOCK_RUN_SETUP_CONFIG=0 AIRLOCK_RUN_CONFIG_INPUTS_IMPORT=0 AIRLOCK_RUN_CONFIG_VMS_SMOKE=0 AIRLOCK_RUN_NIX_CLOSURE_SMOKE=0 AIRLOCK_RUN_QUBES_SMOKE=0 AIRLOCK_RUN_GUI_SMOKE=0 ./lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh; \
	fi

vm-warm-sync: ensure-generated-dirs
	AIRLOCK_SYNC_DOM0_TOOLS=1 ./lifecycle/misc/tools/scripts/sync-nested-dom0-tools.sh

vm-sync-codex-auth: ensure-generated-dirs
	./lifecycle/misc/tools/scripts/sync-nested-codex-auth.sh

vm-warm-apply: ensure-generated-dirs
	AIRLOCK_KEEP_VM=1 AIRLOCK_REUSE_WARM_VM=1 AIRLOCK_SYNC_DOM0_TOOLS=1 AIRLOCK_NESTED_WARM_STOP_NIX_DOMAINS=1 AIRLOCK_VM_SKIP_BASE_BUILD=1 AIRLOCK_VM_IMAGE_ATTR="$(VM_GUI_CONFIG_IMAGE_ATTR)" AIRLOCK_VM_DISPLAY=none AIRLOCK_VM_FRESH=0 AIRLOCK_BOOTSTRAP_QUBES=0 AIRLOCK_RUN_SETUP_CONFIG=1 AIRLOCK_RUN_CONFIG_INPUTS_IMPORT=1 AIRLOCK_RUN_CONFIG_VMS_SMOKE=0 AIRLOCK_RUN_NIX_CLOSURE_SMOKE=0 AIRLOCK_RUN_QUBES_SMOKE=0 AIRLOCK_RUN_GUI_SMOKE=0 ./lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh

vm-warm-test-config: ensure-generated-dirs
	AIRLOCK_KEEP_VM=1 AIRLOCK_REUSE_WARM_VM=1 AIRLOCK_SYNC_DOM0_TOOLS=1 AIRLOCK_NESTED_WARM_STOP_NIX_DOMAINS=1 AIRLOCK_VM_SKIP_BASE_BUILD=1 AIRLOCK_VM_IMAGE_ATTR="$(VM_GUI_CONFIG_IMAGE_ATTR)" AIRLOCK_VM_DISPLAY=none AIRLOCK_VM_FRESH=0 AIRLOCK_BOOTSTRAP_QUBES=0 AIRLOCK_RUN_SETUP_CONFIG=1 AIRLOCK_RUN_CONFIG_INPUTS_IMPORT=0 AIRLOCK_RUN_CONFIG_VMS_SMOKE=1 AIRLOCK_RUN_NIX_CLOSURE_SMOKE=0 AIRLOCK_RUN_QUBES_SMOKE=0 AIRLOCK_RUN_GUI_SMOKE=0 ./lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh

vm-config-setup: ensure-generated-dirs gc-auto
	AIRLOCK_AUTO_GC_AFTER_RUN=1 AIRLOCK_VM_IMAGE_ATTR="$(VM_GUI_CONFIG_IMAGE_ATTR)" AIRLOCK_VM_DISPLAY=none AIRLOCK_BOOTSTRAP_QUBES=0 AIRLOCK_RUN_SETUP_CONFIG=1 AIRLOCK_RUN_QUBES_SMOKE=0 AIRLOCK_RUN_GUI_SMOKE=0 ./lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh

vm-config-flow: ensure-generated-dirs gc-auto
	AIRLOCK_AUTO_GC_AFTER_RUN=1 AIRLOCK_VM_IMAGE_ATTR="$(VM_GUI_CONFIG_IMAGE_ATTR)" AIRLOCK_VM_DISPLAY=none AIRLOCK_VM_FRESH=1 AIRLOCK_BOOTSTRAP_QUBES=0 AIRLOCK_RUN_SETUP_CONFIG=$(VM_FLOW_RUN_SETUP_CONFIG) AIRLOCK_RUN_CONFIG_VMS_SMOKE=1 AIRLOCK_RUN_QUBES_SMOKE=0 AIRLOCK_RUN_GUI_SMOKE=0 ./lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh

vm-config-warm-flow: ensure-generated-dirs gc-auto
	AIRLOCK_AUTO_GC_AFTER_RUN=1 AIRLOCK_VM_IMAGE_ATTR="$(VM_GUI_CONFIG_IMAGE_ATTR)" AIRLOCK_VM_DISPLAY=none AIRLOCK_VM_FRESH=0 AIRLOCK_BOOTSTRAP_QUBES=0 AIRLOCK_RUN_SETUP_CONFIG=1 AIRLOCK_RUN_CONFIG_VMS_SMOKE=1 AIRLOCK_RUN_QUBES_SMOKE=0 AIRLOCK_RUN_GUI_SMOKE=0 ./lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh

vm-config-boot-flow: ensure-generated-dirs gc-auto
	AIRLOCK_AUTO_GC_AFTER_RUN=1 AIRLOCK_VM_IMAGE_ATTR="$(VM_GUI_CONFIG_IMAGE_ATTR)" AIRLOCK_VM_DISPLAY=none AIRLOCK_VM_FRESH=1 AIRLOCK_BOOTSTRAP_QUBES=0 AIRLOCK_RUN_SETUP_CONFIG=$(VM_FLOW_RUN_SETUP_CONFIG) AIRLOCK_RUN_CONFIG_VMS_SMOKE=1 AIRLOCK_CONFIG_DOM0_SYSTEM_SWITCH_ACTION=boot AIRLOCK_RUN_QUBES_SMOKE=0 AIRLOCK_RUN_GUI_SMOKE=0 ./lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh

vm-config-boot-activate-flow: ensure-generated-dirs gc-auto
	./lifecycle/misc/tools/scripts/run-config-boot-activation-flow.sh

vm-nix-closure-flow: ensure-generated-dirs gc-auto
	AIRLOCK_AUTO_GC_AFTER_RUN=1 AIRLOCK_VM_IMAGE_ATTR="$(VM_GUI_CONFIG_IMAGE_ATTR)" AIRLOCK_VM_DISPLAY=none AIRLOCK_VM_FRESH=1 AIRLOCK_BOOTSTRAP_QUBES=0 AIRLOCK_RUN_SETUP_CONFIG=$(VM_FLOW_RUN_SETUP_CONFIG) AIRLOCK_RUN_CONFIG_VMS_SMOKE=0 AIRLOCK_RUN_NIX_CLOSURE_SMOKE=1 AIRLOCK_RUN_QUBES_SMOKE=0 AIRLOCK_RUN_GUI_SMOKE=0 ./lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh

vm-nix-closure-warm-flow: ensure-generated-dirs gc-auto
	AIRLOCK_AUTO_GC_AFTER_RUN=1 AIRLOCK_KEEP_VM=1 AIRLOCK_SYNC_DOM0_TOOLS=1 AIRLOCK_VM_SKIP_BASE_BUILD=1 AIRLOCK_VM_IMAGE_ATTR="$(VM_GUI_CONFIG_IMAGE_ATTR)" AIRLOCK_VM_DISPLAY=none AIRLOCK_VM_FRESH=0 AIRLOCK_BOOTSTRAP_QUBES=0 AIRLOCK_RUN_SETUP_CONFIG=1 AIRLOCK_RUN_CONFIG_VMS_SMOKE=0 AIRLOCK_RUN_NIX_CLOSURE_SMOKE=1 AIRLOCK_RUN_QUBES_SMOKE=0 AIRLOCK_RUN_GUI_SMOKE=0 ./lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh

vm-pool-admission-flow: ensure-generated-dirs gc-auto
	AIRLOCK_KEEP_VM=1 AIRLOCK_VM_IMAGE_ATTR=airlock-vm-pool-admission-test-image AIRLOCK_VM_DISPLAY=none AIRLOCK_VM_FRESH=1 AIRLOCK_BOOTSTRAP_QUBES=0 AIRLOCK_RUN_SETUP_CONFIG=1 AIRLOCK_RUN_CONFIG_INPUTS_IMPORT=0 AIRLOCK_RUN_CONFIG_VMS_SMOKE=0 AIRLOCK_RUN_NIX_CLOSURE_SMOKE=0 AIRLOCK_RUN_QUBES_SMOKE=0 AIRLOCK_RUN_GUI_SMOKE=0 ./lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh
	uv run python lifecycle/misc/tools/scripts/pool-admission-smoke.py

disk-usage:
	./lifecycle/misc/tools/scripts/clean-generated.sh disk-usage

gc-auto: ensure-generated-dirs
	@if [ "$${AIRLOCK_SKIP_GC_AUTO:-0}" = "1" ]; then \
		printf 'gc-auto skipped: AIRLOCK_SKIP_GC_AUTO=1\n'; \
	else \
		./lifecycle/misc/tools/scripts/gc-airlock-store.sh auto; \
	fi

gc-roots: ensure-generated-dirs
	./lifecycle/misc/tools/scripts/gc-airlock-store.sh roots

gc-dry-run: ensure-generated-dirs
	./lifecycle/misc/tools/scripts/gc-airlock-store.sh dry-run

gc: ensure-generated-dirs
	./lifecycle/misc/tools/scripts/gc-airlock-store.sh run

clean: clean-generated

clean-generated: clean-vm-workdirs clean-logs

clean-vm: clean-vm-workdirs

clean-vm-disks:
	./lifecycle/misc/tools/scripts/clean-generated.sh vm-disks

clean-vm-workdirs:
	./lifecycle/misc/tools/scripts/clean-generated.sh vm-workdirs

clean-result-links:
	./lifecycle/misc/tools/scripts/clean-generated.sh result-links

clean-logs:
	./lifecycle/misc/tools/scripts/clean-generated.sh logs

clean-cache:
	./lifecycle/misc/tools/scripts/clean-generated.sh cache
