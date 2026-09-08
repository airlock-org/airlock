SHELL := bash

ifneq ($(wildcard /run/media/user/data/.),)
NIXOME_DATA_ROOT ?= /run/media/user/data/nixome
endif

ifneq ($(strip $(NIXOME_DATA_ROOT)),)
NIXOME_VM_DIR ?= $(NIXOME_DATA_ROOT)/vm
NIXOME_LOG_DIR ?= $(NIXOME_DATA_ROOT)/logs
NIXOME_QUBES_CACHE_DIR ?= $(NIXOME_DATA_ROOT)/cache/rpms
NIXOME_RESULT_LINK_DIR ?= $(NIXOME_DATA_ROOT)/result-links
NIXOME_NIX_ROOT ?= $(NIXOME_DATA_ROOT)/nix-root
NIXOME_NIX_STORE ?= $(NIXOME_NIX_ROOT)
TMPDIR ?= $(NIXOME_DATA_ROOT)/tmp
XDG_CACHE_HOME ?= $(NIXOME_DATA_ROOT)/cache/xdg
export NIXOME_DATA_ROOT
export NIXOME_VM_DIR
export NIXOME_LOG_DIR
export NIXOME_QUBES_CACHE_DIR
export NIXOME_RESULT_LINK_DIR
export NIXOME_NIX_ROOT
export NIXOME_NIX_STORE
export TMPDIR
export XDG_CACHE_HOME
endif

NIXOME_NIX ?= ./scripts/nixome-nix.sh

VM_PREBUILD_ATTRS := \
	packages.x86_64-linux.nixome-setup-config \
	packages.x86_64-linux.nixome-config-seed-inputs-import \
	packages.x86_64-linux.nixome-config-vms-smoke \
	packages.x86_64-linux.nixome-nix-closure-domain-smoke \
	packages.x86_64-linux.nixome-qubes-storage \
	packages.x86_64-linux.nixome-qubes-vm-kernel \
	nixome-vm-gui-config-image

PYTHON_FILES := \
	pkgs/nixome-qubes-storage/src/nixome_qubes_storage/__init__.py \
	pkgs/nixome-qubes-storage/src/nixome_qubes_storage/cli.py \
	pkgs/nixome-qubes-storage/src/nixome_qubes_storage/engine.py \
	pkgs/nixome-qubes-storage/src/nixome_qubes_storage/_engine/*.py \
	pkgs/nixome-qubes-storage/src/nixome_qubes_storage/qubes.py \
	scripts/nix-domain-storage.py \
	tests/conftest.py \
	tests/test_local.py \
	tests/test_vm.py

PYTEST_LOG_DIR = $${NIXOME_LOG_DIR:-.nixome-logs}
TEST_JOBS ?= 4
TEST_VM_JOBS ?= 2
PYTEST_WORKERS = $(if $(TEST_JOBS),-n $(TEST_JOBS),)
PYTEST_DISTRIBUTION = $(if $(TEST_JOBS),--dist loadgroup,)
PYTEST_REPORT_ARGS = --durations=20 --junitxml "$(PYTEST_LOG_DIR)/pytest-junit.xml" --json-report --json-report-file "$(PYTEST_LOG_DIR)/pytest-report.json"
PYTEST_DEFAULT_TARGETS = $(if $(TESTS),,tests/test_local.py)
PYTEST_SELECTED = $(foreach test,$(TESTS),'tests/test_local.py::test_local_check[$(test)]')
VM_PYTEST_SELECTED = $(foreach test,$(TESTS),'tests/test_vm.py::test_vm_flow[$(test)]')

.PHONY: ensure-generated-dirs build test test-parallel test-serial test-vm test-vm-parallel test-vm-warm test-list py-sync py-format py-check py-lint py-complexity py-complexity-check install-git-hooks vm-prebuild-shared vm-image vm-image-dry-run vm-run vm-ssh vm-gui-image vm-gui-image-dry-run vm-gui-run vm-gui-config-image vm-gui-config-image-dry-run vm-gui-config-run vm-gui-check vm-gui-smoke vm-warm-start vm-warm-sync vm-warm-apply vm-warm-test-config vm-config-setup vm-config-flow vm-config-warm-flow vm-config-boot-flow vm-config-boot-activate-flow vm-config-smoke vm-nix-closure-flow vm-nix-closure-warm-flow disk-usage gc-auto gc-roots gc-dry-run gc clean clean-generated clean-vm clean-vm-disks clean-vm-workdirs clean-result-links clean-logs clean-cache

ensure-generated-dirs:
ifneq ($(strip $(NIXOME_DATA_ROOT)),)
	@mkdir -p "$(NIXOME_VM_DIR)" "$(NIXOME_LOG_DIR)" "$(NIXOME_QUBES_CACHE_DIR)" "$(NIXOME_RESULT_LINK_DIR)" "$(NIXOME_NIX_ROOT)" "$(TMPDIR)" "$(XDG_CACHE_HOME)"
else
	@:
endif

build: ensure-generated-dirs
	$(NIXOME_NIX) build .#nixosConfigurations.test.config.system.build.toplevel --out-link "$(NIXOME_RESULT_LINK_DIR)/test-system" --print-build-logs --show-trace

test: ensure-generated-dirs
	@mkdir -p "$(PYTEST_LOG_DIR)"
	uv run pytest $(PYTEST_DEFAULT_TARGETS) $(PYTEST_SELECTED) $(PYTEST_WORKERS) $(PYTEST_DISTRIBUTION) $(PYTEST_REPORT_ARGS) $(PYTEST_ARGS)

test-parallel: TEST_JOBS = 4
test-parallel: test

test-serial: TEST_JOBS =
test-serial: test

test-vm: ensure-generated-dirs
	@mkdir -p "$(PYTEST_LOG_DIR)"
	NIXOME_RUN_VM_TESTS=1 uv run pytest $(VM_PYTEST_SELECTED) $(if $(TESTS),,-m "vm and not warm") $(PYTEST_REPORT_ARGS) $(PYTEST_ARGS)

test-vm-parallel: ensure-generated-dirs
	@mkdir -p "$(PYTEST_LOG_DIR)"
	NIXOME_RUN_VM_TESTS=1 NIXOME_VM_PARALLEL_LANES="$(TEST_VM_JOBS)" uv run pytest $(VM_PYTEST_SELECTED) $(if $(TESTS),,-m "vm and not warm and not serial_vm") -n $(TEST_VM_JOBS) --dist load --durations=20 --junitxml "$(PYTEST_LOG_DIR)/pytest-junit.xml" --json-report --json-report-file "$(PYTEST_LOG_DIR)/pytest-report.json" $(PYTEST_ARGS)

test-vm-warm: ensure-generated-dirs
	@mkdir -p "$(PYTEST_LOG_DIR)"
	NIXOME_RUN_VM_TESTS=1 uv run pytest $(VM_PYTEST_SELECTED) $(if $(TESTS),,-m "vm and warm") $(PYTEST_REPORT_ARGS) $(PYTEST_ARGS)

test-list: ensure-generated-dirs
	uv run pytest tests --collect-only -q

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
	install -m 0755 scripts/git-hooks/pre-commit .git/hooks/pre-commit

vm-prebuild-shared: ensure-generated-dirs gc-auto
	$(NIXOME_NIX) build $(foreach attr,$(VM_PREBUILD_ATTRS),.#$(attr)) --no-link --print-build-logs --show-trace

vm-image: ensure-generated-dirs gc-auto
	$(NIXOME_NIX) build .#nixome-vm-image --out-link "$(NIXOME_RESULT_LINK_DIR)/nixome-vm-image" --print-build-logs --show-trace

vm-image-dry-run: ensure-generated-dirs
	$(NIXOME_NIX) build .#nixome-vm-image --dry-run --show-trace

vm-run: ensure-generated-dirs gc-auto
	./scripts/run-nested-vm.sh

vm-ssh: ensure-generated-dirs
	./scripts/ssh-nested-vm.sh

vm-gui-image: ensure-generated-dirs gc-auto
	$(NIXOME_NIX) build .#nixome-vm-gui-image --out-link "$(NIXOME_RESULT_LINK_DIR)/nixome-vm-gui-image" --print-build-logs --show-trace

vm-gui-image-dry-run: ensure-generated-dirs
	$(NIXOME_NIX) build .#nixome-vm-gui-image --dry-run --show-trace

vm-gui-run: ensure-generated-dirs gc-auto
	NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-image NIXOME_VM_DISPLAY=gtk ./scripts/run-nested-vm.sh

vm-gui-config-image: ensure-generated-dirs gc-auto
	$(NIXOME_NIX) build .#nixome-vm-gui-config-image --out-link "$(NIXOME_RESULT_LINK_DIR)/nixome-vm-gui-config-image" --print-build-logs --show-trace

vm-gui-config-image-dry-run: ensure-generated-dirs
	$(NIXOME_NIX) build .#nixome-vm-gui-config-image --dry-run --show-trace

vm-gui-config-run: ensure-generated-dirs gc-auto
	NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=gtk ./scripts/run-nested-vm.sh

vm-gui-check: ensure-generated-dirs gc-auto
	NIXOME_AUTO_GC_AFTER_RUN=1 NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

vm-gui-smoke: ensure-generated-dirs gc-auto
	NIXOME_AUTO_GC_AFTER_RUN=1 ./scripts/run-nested-gui-smoke.sh

vm-warm-start: ensure-generated-dirs
	@if ./scripts/ssh-nested-vm.sh true >/dev/null 2>&1; then \
		printf 'nested dom0 already reachable on NIXOME_VM_SSH_PORT=%s\n' "$${NIXOME_VM_SSH_PORT:-2222}"; \
	else \
		NIXOME_KEEP_VM=1 NIXOME_VM_SKIP_BASE_BUILD=1 NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=none NIXOME_VM_FRESH=0 NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_SETUP_CONFIG=0 NIXOME_RUN_CONFIG_SEED_INPUTS_IMPORT=0 NIXOME_RUN_CONFIG_VMS_SMOKE=0 NIXOME_RUN_NIX_CLOSURE_SMOKE=0 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh; \
	fi

vm-warm-sync: ensure-generated-dirs
	NIXOME_SYNC_DOM0_TOOLS=1 ./scripts/sync-nested-dom0-tools.sh

vm-warm-apply: ensure-generated-dirs
	NIXOME_KEEP_VM=1 NIXOME_REUSE_WARM_VM=1 NIXOME_SYNC_DOM0_TOOLS=1 NIXOME_NESTED_WARM_STOP_NIX_DOMAINS=1 NIXOME_VM_SKIP_BASE_BUILD=1 NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=none NIXOME_VM_FRESH=0 NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_SETUP_CONFIG=1 NIXOME_RUN_CONFIG_SEED_INPUTS_IMPORT=1 NIXOME_RUN_CONFIG_VMS_SMOKE=0 NIXOME_RUN_NIX_CLOSURE_SMOKE=0 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

vm-warm-test-config: ensure-generated-dirs
	NIXOME_KEEP_VM=1 NIXOME_REUSE_WARM_VM=1 NIXOME_SYNC_DOM0_TOOLS=1 NIXOME_NESTED_WARM_STOP_NIX_DOMAINS=1 NIXOME_VM_SKIP_BASE_BUILD=1 NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=none NIXOME_VM_FRESH=0 NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_SETUP_CONFIG=1 NIXOME_RUN_CONFIG_SEED_INPUTS_IMPORT=0 NIXOME_RUN_CONFIG_VMS_SMOKE=1 NIXOME_RUN_NIX_CLOSURE_SMOKE=0 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

vm-config-setup: ensure-generated-dirs gc-auto
	NIXOME_AUTO_GC_AFTER_RUN=1 NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=none NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_SETUP_CONFIG=1 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

vm-config-flow: ensure-generated-dirs gc-auto
	NIXOME_AUTO_GC_AFTER_RUN=1 NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=none NIXOME_VM_FRESH=1 NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_SETUP_CONFIG=1 NIXOME_RUN_CONFIG_VMS_SMOKE=1 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

vm-config-warm-flow: ensure-generated-dirs gc-auto
	NIXOME_AUTO_GC_AFTER_RUN=1 NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=none NIXOME_VM_FRESH=0 NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_SETUP_CONFIG=1 NIXOME_RUN_CONFIG_VMS_SMOKE=1 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

vm-config-boot-flow: ensure-generated-dirs gc-auto
	NIXOME_AUTO_GC_AFTER_RUN=1 NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=none NIXOME_VM_FRESH=1 NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_SETUP_CONFIG=1 NIXOME_RUN_CONFIG_VMS_SMOKE=1 NIXOME_CONFIG_DOM0_SYSTEM_SWITCH_ACTION=boot NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

vm-config-boot-activate-flow: ensure-generated-dirs gc-auto
	./scripts/run-config-boot-activation-flow.sh

vm-config-smoke: vm-config-flow

vm-nix-closure-flow: ensure-generated-dirs gc-auto
	NIXOME_AUTO_GC_AFTER_RUN=1 NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=none NIXOME_VM_FRESH=1 NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_SETUP_CONFIG=1 NIXOME_RUN_CONFIG_VMS_SMOKE=0 NIXOME_RUN_NIX_CLOSURE_SMOKE=1 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

vm-nix-closure-warm-flow: ensure-generated-dirs gc-auto
	NIXOME_AUTO_GC_AFTER_RUN=1 NIXOME_KEEP_VM=1 NIXOME_SYNC_DOM0_TOOLS=1 NIXOME_VM_SKIP_BASE_BUILD=1 NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=none NIXOME_VM_FRESH=0 NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_SETUP_CONFIG=1 NIXOME_RUN_CONFIG_VMS_SMOKE=0 NIXOME_RUN_NIX_CLOSURE_SMOKE=1 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

disk-usage:
	./scripts/clean-generated.sh disk-usage

gc-auto: ensure-generated-dirs
	@if [ "$${NIXOME_SKIP_GC_AUTO:-0}" = "1" ]; then \
		printf 'gc-auto skipped: NIXOME_SKIP_GC_AUTO=1\n'; \
	else \
		./scripts/gc-nixome-store.sh auto; \
	fi

gc-roots: ensure-generated-dirs
	./scripts/gc-nixome-store.sh roots

gc-dry-run: ensure-generated-dirs
	./scripts/gc-nixome-store.sh dry-run

gc: ensure-generated-dirs
	./scripts/gc-nixome-store.sh run

clean: clean-generated

clean-generated: clean-vm-workdirs clean-logs

clean-vm: clean-vm-workdirs

clean-vm-disks:
	./scripts/clean-generated.sh vm-disks

clean-vm-workdirs:
	./scripts/clean-generated.sh vm-workdirs

clean-result-links:
	./scripts/clean-generated.sh result-links

clean-logs:
	./scripts/clean-generated.sh logs

clean-cache:
	./scripts/clean-generated.sh cache
