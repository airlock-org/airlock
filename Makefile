SHELL := bash

.PHONY: build test nixos-templates nixos-templates-dry-run vm-image vm-image-dry-run vm-run vm-gui-image vm-gui-image-dry-run vm-gui-run vm-gui-config-image vm-gui-config-image-dry-run vm-gui-config-run vm-gui-check vm-gui-smoke vm-config-reconcile vm-config-smoke clean-vm-disks

build:
	nix build .#nixosConfigurations.test.config.system.build.toplevel --print-build-logs --show-trace

test:
	./tests/run-local.sh

nixos-templates:
	nix build .#nixome-nixos-config-vault-template .#nixome-nixos-agent-template --print-build-logs --show-trace

nixos-templates-dry-run:
	nix build .#nixome-nixos-config-vault-template .#nixome-nixos-agent-template --dry-run --show-trace

vm-image:
	nix build .#nixome-vm-image --print-build-logs --show-trace

vm-image-dry-run:
	nix build .#nixome-vm-image --dry-run --show-trace

vm-run:
	./scripts/run-nested-vm.sh

vm-gui-image:
	nix build .#nixome-vm-gui-image --print-build-logs --show-trace

vm-gui-image-dry-run:
	nix build .#nixome-vm-gui-image --dry-run --show-trace

vm-gui-run:
	NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-image NIXOME_VM_DISPLAY=gtk ./scripts/run-nested-vm.sh

vm-gui-config-image:
	nix build .#nixome-vm-gui-config-image --print-build-logs --show-trace

vm-gui-config-image-dry-run:
	nix build .#nixome-vm-gui-config-image --dry-run --show-trace

vm-gui-config-run:
	NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=gtk ./scripts/run-nested-vm.sh

vm-gui-check:
	NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

vm-gui-smoke:
	./scripts/run-nested-gui-smoke.sh

vm-config-reconcile:
	NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=none NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_QUBES_RECONCILE=1 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

vm-config-smoke:
	NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-config-image NIXOME_VM_DISPLAY=none NIXOME_BOOTSTRAP_QUBES=0 NIXOME_RUN_QUBES_RECONCILE=1 NIXOME_RUN_CONFIG_VMS_SMOKE=1 NIXOME_RUN_QUBES_SMOKE=0 NIXOME_RUN_GUI_SMOKE=0 ./scripts/run-nested-gui-smoke.sh

clean-vm-disks:
	rm -rf .nixome-vm/*-result
	rm -f .nixome-vm/*.qcow2 .nixome-vm/*.qcow2.*.bak .nixome-vm/*.monitor .nixome-vm/serial.log
