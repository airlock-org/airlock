#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

cargo run --locked --quiet -p rusix-derive --features dev-tools --bin check-rust-spacing

# Real evaluator round-trips compare the committed Rusix diagnostic snapshots.
cargo test --locked -p rusix --lib context_audit
cargo test --locked -p rusix --lib render_audit
cargo test --locked -p rusix --lib compiler::render::tests
cargo test --locked -p rusix --lib compiler::render::precedence_tests
cargo test --locked -p rusix --test roundtrip
cargo test --locked -p rusix --test nixos
cargo test --locked -p rusix --test session
cargo test --locked -p rusix --test consumers
cargo test --locked -p rusix --features dev-cli --test cli
cargo test --locked -p rusix --test merges
cargo test --locked -p rusix --test authoring
cargo test --locked -p rusix --test symbolic_options
cargo test --locked -p rusix --test derive
cargo test --locked -p rusix --test config_module
cargo test --locked -p rusix --test options --test args
cargo test --locked -p rusix --test postgresql
cargo test --locked -p rusix --test schema --test postgresql_schema
cargo test --locked -p rusix --test symbolic_text
cargo test --locked -p rusix --test structured_interop --test library --test typed_interop
cargo test --locked -p rusix --test package --test package_functions --test git --test curl --test nix_operations
cargo test --locked -p rusix --test openssl --test mariadb --test composed --test backend_provenance --test backend_correlation
cargo test --locked -p rusix --test overlay
cargo test --locked -p rusix --test typed_examples --test ui --test interop

for example in enum-option typed-submodule invalid-states function-contracts exhaustive-match typed-values layered-validation nix-interop symbolic-option postgresql-nixos-module git-nixpkg curl-nixpkg openssl-nixpkg mariadb-nixpkg composed-packages overlay; do
    cargo run --locked --quiet -p rusix --example "$example"
done

# Preserve reviewable outputs from the executable harness as well.
cargo run --locked --quiet -p rusix --features dev-cli -- check good --out target/diagnostic-fixtures/good
cargo run --locked --quiet -p rusix --features dev-cli -- check selective --out target/diagnostic-fixtures/selective-good --select good
for fixture in bad-port nested conflict codegen-bug unmapped selective; do
    out="target/diagnostic-fixtures/$fixture"
    selection=()
    if [ "$fixture" = selective ]; then selection=(--select bad); fi
    if cargo run --locked --quiet -p rusix --features dev-cli -- check "$fixture" --out "$out" "${selection[@]}"; then
        echo "Fixture $fixture unexpectedly succeeded" >&2
        exit 1
    else
        status=$?
        [ "$status" -eq 1 ] || exit "$status"
    fi
    case "$fixture" in
        conflict) kind=Validation ;;
        codegen-bug) kind=Compiler ;;
        *) kind=NixEval ;;
    esac
    diagnostic=$(cat "$out/diagnostic.json")
    expected="\"kind\": \"$kind\""
    case "$diagnostic" in
        *"$expected"*) ;;
        *) echo "Unexpected diagnostic kind for $fixture; expected $kind" >&2; exit 1 ;;
    esac
done

for fixture in merge-ok merge-priority; do
    cargo run --locked --quiet -p rusix --features dev-cli -- check-nixos "$fixture" --out "target/diagnostic-fixtures/$fixture"
done
for fixture in merge-two merge-three merge-mixed merge-three-type; do
    out="target/diagnostic-fixtures/$fixture"
    if cargo run --locked --quiet -p rusix --features dev-cli -- check-nixos "$fixture" --out "$out"; then
        echo "Merge fixture $fixture unexpectedly succeeded" >&2
        exit 1
    else
        status=$?
        [ "$status" -eq 1 ] || exit "$status"
    fi
    if [ "$fixture" = merge-three-type ]; then kind=NixosType; else kind=NixosMerge; fi
    diagnostic=$(cat "$out/diagnostic.json")
    expected="\"kind\": \"$kind\""
    case "$diagnostic" in
        *"$expected"*) ;;
        *) echo "Unexpected merge diagnostic kind for $fixture; expected $kind" >&2; exit 1 ;;
    esac
done

cargo run --locked --quiet -p rusix --features dev-cli -- check-nixos good --out target/diagnostic-fixtures/nixos-good
cargo run --locked --quiet -p rusix --features dev-cli -- check-nixos lazy --out target/diagnostic-fixtures/nixos-lazy
for fixture in type unknown assertion external; do
    out="target/diagnostic-fixtures/nixos-$fixture"
    if cargo run --locked --quiet -p rusix --features dev-cli -- check-nixos "$fixture" --out "$out"; then
        echo "NixOS fixture $fixture unexpectedly succeeded" >&2
        exit 1
    else
        status=$?
        [ "$status" -eq 1 ] || exit "$status"
    fi
    case "$fixture" in
        type) kind=NixosType ;;
        unknown) kind=NixosModule ;;
        assertion) kind=NixosAssertion ;;
        external) kind=ExternalNix ;;
    esac
    diagnostic=$(cat "$out/diagnostic.json")
    expected="\"kind\": \"$kind\""
    case "$diagnostic" in
        *"$expected"*) ;;
        *) echo "Unexpected NixOS diagnostic kind for $fixture; expected $kind" >&2; exit 1 ;;
    esac
done
