SHELL := bash
AIRLOCK_APPS ?= /etc/airlock/config-inputs/airlock-apps
AIRLOCK_CARGO_VENDOR ?= /etc/airlock/authoring/vendor

AIRLOCK_PREPARED_CACHE ?= $(CURDIR)/target/nix
OUTPUT ?=

.PHONY: prepare generate check-generated
prepare generate:
	cargo --config 'patch.crates-io.airlock-config.path="$(AIRLOCK_APPS)/airlock-config"' \
	  --config 'source.airlock-admitted.directory="$(AIRLOCK_CARGO_VENDOR)"' \
	  --config 'build.rustflags=["--remap-path-prefix=$(AIRLOCK_APPS)/="]' run --locked --offline -- \
	  $(if $(OUTPUT),--emit "$(OUTPUT)",prepare "$(AIRLOCK_PREPARED_CACHE)")

.PHONY: build
build:
	cargo --config 'patch.crates-io.airlock-config.path="$(AIRLOCK_APPS)/airlock-config"' \
	  --config 'source.airlock-admitted.directory="$(AIRLOCK_CARGO_VENDOR)"' \
	  --config 'build.rustflags=["--remap-path-prefix=$(AIRLOCK_APPS)/="]' build --locked --offline

# Repeated preparation verifies an existing content-addressed artifact before reusing it.
check-generated: prepare

rust-check:
	cargo fmt --all -- --check
	cargo clippy --config 'patch.crates-io.airlock-config.path="$(AIRLOCK_APPS)/airlock-config"' \
	  --config 'source.airlock-admitted.directory="$(AIRLOCK_CARGO_VENDOR)"' \
	  --config 'build.rustflags=["--remap-path-prefix=$(AIRLOCK_APPS)/="]' --locked --offline --all-targets -- -D warnings
