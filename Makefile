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
	lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/__init__.py \
	lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/cli.py \
	lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/engine.py \
	lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/*.py \
	lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/qubes.py \
	lifecycle/runtime/tools/scripts/nix-domain-storage.py \
	tests/conftest.py \
	tests/local_harness.py \
	tests/local_pychecks.py \
	tests/run_local.py \
	tests/test_local.py \
	tests/test_vm.py \
	tests/test_vm_staged.py

PYTEST_LOG_DIR = $${AIRLOCK_LOG_DIR:-.airlock-logs}
TEST_JOBS ?= 4
TEST_VM_JOBS ?= 2
PYTEST_CONFIG_ARGS = -c "$(CURDIR)/pyproject.toml" --rootdir "$(CURDIR)"
PYTEST_WORKERS = $(if $(TEST_JOBS),-n $(TEST_JOBS),)
PYTEST_DISTRIBUTION = $(if $(TEST_JOBS),--dist loadgroup,)
PYTEST_REPORT_ARGS = $(PYTEST_CONFIG_ARGS) --durations=20 --junitxml "$(PYTEST_LOG_DIR)/pytest-junit.xml" --json-report --json-report-file "$(PYTEST_LOG_DIR)/pytest-report.json"
PYTEST_VM_OUTPUT_ARGS ?= --capture=tee-sys
PYTEST_VM_STAGED_ARGS ?= --dist loadgroup
PYTEST_DEFAULT_TARGETS = $(if $(TESTS),,tests/test_local.py)
PYTEST_SELECTED = $(foreach test,$(TESTS),'tests/test_local.py::test_local_check[$(test)]')
VM_STAGED_PYTEST_SELECTED = $(foreach test,$(TESTS),'tests/test_vm_staged.py::test_staged_vm_step[$(test)]')
VM_SLOW_DEFAULT_PARALLEL_FLOW_IDS = vm-config-flow vm-nix-closure-flow
VM_SLOW_PARALLEL_FLOW_IDS = $(VM_SLOW_DEFAULT_PARALLEL_FLOW_IDS) vm-config-boot-flow
VM_SLOW_SERIAL_FLOW_IDS = vm-config-boot-activate-flow
VM_SLOW_FLOW_IDS = $(VM_SLOW_PARALLEL_FLOW_IDS) $(VM_SLOW_SERIAL_FLOW_IDS)
VM_SLOW_SELECTED_PARALLEL_FLOW_IDS = $(filter $(VM_SLOW_PARALLEL_FLOW_IDS),$(TESTS))
VM_SLOW_SELECTED_SERIAL_FLOW_IDS = $(filter $(VM_SLOW_SERIAL_FLOW_IDS),$(TESTS))
VM_SLOW_SELECTED_UNKNOWN_FLOW_IDS = $(filter-out $(VM_SLOW_FLOW_IDS),$(TESTS))
VM_SLOW_DEFAULT_PARALLEL_TARGETS = $(foreach test,$(VM_SLOW_DEFAULT_PARALLEL_FLOW_IDS),'tests/test_vm.py::test_vm_flow[$(test)]')
VM_SLOW_SELECTED_PARALLEL_TARGETS = $(foreach test,$(VM_SLOW_SELECTED_PARALLEL_FLOW_IDS),'tests/test_vm.py::test_vm_flow[$(test)]')
VM_SLOW_SELECTED_SERIAL_TARGETS = $(foreach test,$(VM_SLOW_SELECTED_SERIAL_FLOW_IDS),'tests/test_vm.py::test_vm_flow[$(test)]')
VM_SLOW_SERIAL_TARGETS = $(foreach test,$(VM_SLOW_SERIAL_FLOW_IDS),'tests/test_vm.py::test_vm_flow[$(test)]')

.PHONY: ensure-generated-dirs build test-quick test-warm test-slow py-sync py-format py-check py-lint py-complexity py-complexity-check install-git-hooks vm-prebuild-shared vm-parallel-prebuild-image vm-parallel-prebuild-baseline vm-image vm-image-dry-run vm-run vm-ssh vm-gui-image vm-gui-image-dry-run vm-gui-run vm-gui-config-image vm-gui-config-image-dry-run vm-gui-config-run vm-gui-config-network-debug-image vm-gui-config-network-debug-run vm-gui-check vm-gui-smoke vm-warm-start vm-warm-sync vm-sync-codex-auth vm-warm-apply vm-warm-test-config vm-config-setup vm-config-flow vm-config-warm-flow vm-config-boot-flow vm-config-boot-activate-flow vm-nix-closure-flow vm-nix-closure-warm-flow disk-usage gc-auto gc-roots gc-dry-run gc clean clean-generated clean-vm clean-vm-disks clean-result-links clean-logs clean-cache

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

test-warm: ensure-generated-dirs
	@mkdir -p "$(PYTEST_LOG_DIR)"
	AIRLOCK_RUN_VM_TESTS=1 AIRLOCK_SKIP_GC_AUTO=1 uv run pytest $(if $(TESTS),$(VM_STAGED_PYTEST_SELECTED),tests/test_vm_staged.py) $(PYTEST_VM_STAGED_ARGS) $(PYTEST_REPORT_ARGS) $(PYTEST_VM_OUTPUT_ARGS) $(PYTEST_ARGS)

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

py-format:
	uv run black $(PYTHON_FILES)

py-lint:
	uv run ruff check $(PYTHON_FILES)

py-check:
	uv run black --check $(PYTHON_FILES)
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
		AIRLOCK_KEEP_VM=1 AIRLOCK_VM_SKIP_BASE_BUILD=1 AIRLOCK_VM_IMAGE_ATTR="$(VM_GUI_CONFIG_IMAGE_ATTR)" AIRLOCK_VM_DISPLAY=none AIRLOCK_VM_FRESH=0 AIRLOCK_BOOTSTRAP_QUBES=0 AIRLOCK_RUN_SETUP_CONFIG=0 AIRLOCK_RUN_CONFIG_INPUTS_IMPORT=0 AIRLOCK_RUN_CONFIG_VMS_SMOKE=0 AIRLOCK_RUN_NIX_CLOSURE_SMOKE=0 AIRLOCK_RUN_QUBES_SMOKE=0 AIRLOCK_RUN_GUI_SMOKE=0 ./lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh; \
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
