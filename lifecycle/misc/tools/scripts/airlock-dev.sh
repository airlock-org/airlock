#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
# Bootstrap native workstation tools; Rust selects the data/store environment.
cargo build --quiet --locked --manifest-path "$ROOT/crates/Cargo.toml" -p airlock-dev
exec "${CARGO_TARGET_DIR:-$ROOT/crates/target}/debug/airlock-dev" --repository-root "$ROOT" with-environment -- "$@"
