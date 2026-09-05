SHELL := bash

.PHONY: build test vm-image vm-image-dry-run vm-run vm-gui-image vm-gui-image-dry-run vm-gui-run

build:
	nix build .#nixosConfigurations.test.config.system.build.toplevel --print-build-logs --show-trace

test:
	./tests/run-local.sh

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
