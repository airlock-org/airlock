SHELL := bash

ifneq ($(wildcard /run/media/user/data/.),)
NIXOME_DATA_ROOT ?= /run/media/user/data/nixome
endif

ifneq ($(strip $(NIXOME_DATA_ROOT)),)
NIXOME_VM_DIR ?= $(NIXOME_DATA_ROOT)/vm
NIXOME_LOG_DIR ?= $(NIXOME_DATA_ROOT)/logs
NIXOME_QUBES_CACHE_DIR ?= $(NIXOME_DATA_ROOT)/cache/rpms
NIXOME_NIX_ROOT ?= $(NIXOME_DATA_ROOT)/nix-root
NIXOME_NIX_STORE ?= $(NIXOME_NIX_ROOT)
TMPDIR ?= $(NIXOME_DATA_ROOT)/tmp
XDG_CACHE_HOME ?= $(NIXOME_DATA_ROOT)/cache/xdg
export NIXOME_DATA_ROOT
export NIXOME_VM_DIR
export NIXOME_LOG_DIR
export NIXOME_QUBES_CACHE_DIR
export NIXOME_NIX_ROOT
export NIXOME_NIX_STORE
export TMPDIR
export XDG_CACHE_HOME
endif

NIXOME_NIX ?= ./scripts/nixome-nix.sh

.PHONY: ensure-generated-dirs build test nixos-templates nixos-templates-dry-run vm-image vm-image-dry-run vm-run vm-ssh vm-gui-image vm-gui-image-dry-run vm-gui-run vm-gui-config-image vm-gui-config-image-dry-run vm-gui-config-run vm-gui-check vm-gui-smoke vm-config-setup vm-config-smoke disk-usage clean clean-generated clean-vm clean-vm-disks clean-vm-workdirs clean-logs clean-cache

ensure-generated-dirs:
ifneq ($(strip $(NIXOME_DATA_ROOT)),)
	@mkdir -p "$(NIXOME_VM_DIR)" "$(NIXOME_LOG_DIR)" "$(NIXOME_QUBES_CACHE_DIR)" "$(NIXOME_NIX_ROOT)" "$(TMPDIR)" "$(XDG_CACHE_HOME)"
else
	@:
endif

build: ensure-generated-dirs
	$(NIXOME_NIX) build .#nixosConfigurations.test.config.system.build.toplevel --print-build-logs --show-trace

test: ensure-generated-dirs
	./tests/run-local.sh

nixos-templates: ensure-generated-dirs
	$(NIXOME_NIX) build .#nixome-nixos-config-vault-template .#nixome-nixos-config-staging-template .#nixome-nixos-config-validator-template --print-build-logs --show-trace

nixos-templates-dry-run: ensure-generated-dirs
	$(NIXOME_NIX) build .#nixome-nixos-config-vault-template .#nixome-nixos-config-staging-template .#nixome-nixos-config-validator-template --dry-run --show-trace

vm-image: ensure-generated-dirs
	$(NIXOME_NIX) build .#nixome-vm-image --print-build-logs --show-trace

vm-image-dry-run: ensure-generated-dirs
	$(NIXOME_NIX) build .#nixome-vm-image --dry-run --show-trace

vm-run: ensure-generated-dirs
	./scripts/run-nested-vm.sh

vm-ssh: ensure-generated-dirs
	./scripts/ssh-nested-vm.sh

vm-gui-image: ensure-generated-dirs
	$(NIXOME_NIX) build .#nixome-vm-gui-image --print-build-logs --show-trace

vm-gui-image-dry-run: ensure-generated-dirs
	$(NIXOME_NIX) build .#nixome-vm-gui-image --dry-run --show-trace

vm-gui-run: ensure-generated-dirs
	NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-image NIXOME_VM_DISPLAY=gtk ./scripts/run-nested-vm.sh

vm-gui-config-image: ensure-generated-dirs
	$(NIXOME_NIX) build .#nixome-vm-gui-config-image --print-build-logs --show-trace

vm-gui-config-image-dry-run: ensure-generated-dirs
	$(NIXOME_NIX) build .#nixome-vm-gui-config-image --dry-run --show-trace

vm-gui-config-run: ensure-generated-dirs
	NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=gtk ./scripts/run-nested-vm.sh

vm-gui-check: ensure-generated-dirs
	NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

vm-gui-smoke: ensure-generated-dirs
	./scripts/run-nested-gui-smoke.sh

vm-config-setup: ensure-generated-dirs
	NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=none NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_SETUP_CONFIG=1 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

vm-config-smoke: ensure-generated-dirs
	NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=none NIXOME_VM_FRESH=1 NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_SETUP_CONFIG=1 NIXOME_RUN_CONFIG_VMS_SMOKE=1 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

disk-usage:
	./scripts/clean-generated.sh disk-usage

clean: clean-generated

clean-generated: clean-vm-workdirs clean-logs

clean-vm: clean-vm-workdirs

clean-vm-disks:
	./scripts/clean-generated.sh vm-disks

clean-vm-workdirs:
	./scripts/clean-generated.sh vm-workdirs

clean-logs:
	./scripts/clean-generated.sh logs

clean-cache:
	./scripts/clean-generated.sh cache
