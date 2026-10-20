# Developing Rusix

This guide covers repository setup and how to verify changes. Read
[AGENTS.md](../AGENTS.md) for the development rules.
For library usage, start with the [library guide](library-usage.md).

## Setup

You need:

- Rust 1.88 or newer, with Cargo.
- Git.
- `nix` and `nix-instantiate` on PATH for evaluation tests and CLI checks.

The structured diagnostic tests expect the `raw_msg` and `trace` fields used by
Nix 2.34.8. Older formats have adapter tests rather than a full compatibility
suite.

Run all commands in this guide from the repository root. Initialize the pinned
nixpkgs checkout before running evaluation tests:

```bash
git submodule update --init --depth=1
```

The checkout must remain clean and match the compiled-in revision. Rusix verifies
it before staging evaluation sources. See [the source notes](../vendor/README.md)
for the pin and update procedure.

Cargo may need network access to acquire dependencies initially. Once they are
cached, add `--offline` to Cargo commands. Nix evaluation itself runs offline.

## Repository layout

The workspace contains two crates. `rusix` provides the library and fixture CLI;
`rusix-derive` implements the procedural macros re-exported by the library.
The fixture CLI requires `dev-cli`; the spacing checker requires `dev-tools`.

| Location | Responsibility |
| --- | --- |
| [authoring.rs](../crates/rusix/src/authoring.rs) and [value.rs](../crates/rusix/src/value.rs) | Rust configuration values and structural conversion |
| [ir/](../crates/rusix/src/ir/) | Semantic nodes and validation |
| [compiler.rs](../crates/rusix/src/compiler.rs) | Compilation entry points and lowering |
| [compiler/ast.rs](../crates/rusix/src/compiler/ast.rs) | Nix syntax representation |
| [compiler/render/](../crates/rusix/src/compiler/render/) | Expression precedence and formatted output |
| [compiler/provenance.rs](../crates/rusix/src/compiler/provenance.rs) | Metadata for delayed backend diagnostics |
| [diagnostic.rs](../crates/rusix/src/diagnostic.rs) | Upstream diagnostic parsing and Rust-facing reports |
| [evaluation.rs](../crates/rusix/src/evaluation.rs) | Isolated Nix sessions and subprocesses |
| [nixos/](../crates/rusix/src/nixos/) | Option schemas and module compilation |
| [interop/](../crates/rusix/src/interop/) and [package.rs](../crates/rusix/src/package.rs) | Deferred Nix interfaces and package recipes |
| [rusix-derive/src/](../crates/rusix-derive/src/) | Conversion derives and authoring macros |
| [src/bin/cli/](../crates/rusix/src/bin/cli/) | Fixture definitions used by the CLI |
| [rusix/tests/](../crates/rusix/tests/) | Library and CLI integration tests |
| [tests/](../tests/) | Nix fixtures, compile-fail cases and diagnostic snapshots |
| [examples/](../examples/README.md) | Runnable configuration and package definitions |

### Compiler pipeline

```text
Rust definitions → semantic IR → Nix AST → generated Nix
```

Rust authoring constructs values and records deferred expressions. IR validation
checks their structure before lowering. Rendering emits Nix source and records
its byte spans alongside Rust origins.

NixOS lowering also retains definition and import metadata. The evaluator parses
generated source before asking Nix to evaluate it; diagnostic translation then
uses source spans and backend metadata to recover Rust locations.

Public API documentation is available locally:

```bash
cargo doc -p rusix --no-deps --open
```

### Evaluation boundary

All Nix subprocesses go through `NixSession` in `evaluation.rs`, including parse
and version commands. Keep that boundary when adding evaluation features.

Each session owns a disposable local store. Every command selects it explicitly;
evaluation also selects the same evaluation store. Nix still uses logical
`/nix/store` paths, but physical writes live under the session's temporary root.
Host configuration and store-selection environment variables are cleared or
redirected. This provides store isolation, not filesystem sandboxing.

Commands disable fetching and building. Do not add activation or deployment to
this evaluation harness. Sessions stage checked sources and serialize their own
evaluations. Dropping a session removes its temporary root; separate sessions
have separate stores.

Preserve lazy evaluation when changing generated expressions. Do not force
containers to keep diagnostic contexts alive. Keep the original Nix diagnostic
alongside any recovered Rust location, and report unavailable or ambiguous
origins explicitly. See [runtime diagnostics](runtime-diagnostics.md).

## Verification

Choose checks based on the change, following [AGENTS.md](../AGENTS.md):

| Change | Checks |
| --- | --- |
| Prose, comments or Markdown links | `git diff --check` and changed links |
| Documentation code example | A focused check of that example |
| Rust code, dependencies or build behavior | Workspace tests, formatting and Clippy |
| Public API code | Strict rustdoc checks alongside the applicable code checks |

For Rust or build changes:

```bash
cargo test --workspace --all-features --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run --locked --quiet -p rusix-derive --features dev-tools --bin check-rust-spacing
```

The spacing checker supplements rustfmt by requiring a blank line between
separate Rust items. After rustfmt, its `--fix` option can insert missing gaps.
Document struct fields and keep examples focused on the authoring code.

For changes to public API code, also check documentation warnings:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
```

During development, run the relevant test target to get faster feedback. The
integration targets correspond to files in `crates/rusix/tests/`:

```bash
cargo test --locked -p rusix --test roundtrip
cargo test --locked -p rusix --test ui
```

The full workspace checks still apply to code changes. Evaluation tests require
Nix; missing tooling is a failure rather than a skipped test.

Compile-fail expectations live in [tests/ui/expected.json](../tests/ui/expected.json).
The harness checks diagnostic codes and useful source locations. Diagnostic
snapshots live in [tests/snapshots/](../tests/snapshots/). Change an expected result
only after understanding the behavior that changed.

## Releases

The two crates release together. `rusix` pins the exact `rusix-derive` version;
normal users need only the main dependency. Public IR and AST APIs are intentional
extension points. Macro expansion hooks remain hidden from documentation.

Before a release:

1. Run the verification commands above, including CLI tests with `--all-features`.
2. Check Rust 1.88 and compilation without evaluation. CI also checks Windows.
3. Build strict rustdoc with and without default features.
4. Verify the crate archives and independent consumers:

```bash
python3 scripts/check-release.py
```

The release script uses current stable Cargo and Python 3.11 or newer. Cargo
dependencies must already be cached. It packages both crates and tests ordinary
and renamed dependencies against their unpacked contents. It also runs compilation
with an empty PATH and evaluates through `NixSession` using the pinned checkout.
Use `--allow-dirty` while developing or `--skip-evaluation` for packaging-only work.

Update both crate versions through the workspace manifest and keep the exact macro
dependency in sync. Review package contents and record changes in
[CHANGELOG.md](../CHANGELOG.md). Before 1.0, minor releases may change public APIs;
patch releases should preserve compatibility. Serialized diagnostics and source
maps are version-specific; read them with the version that produced them.
Optional serialized fields retain defaults for absent metadata, without promising
compatibility with earlier development artifacts.

Publish `rusix-derive` first, then `rusix`, after both dry-runs pass:

```bash
cargo publish -p rusix-derive --dry-run --locked
cargo publish -p rusix --dry-run --locked
cargo publish -p rusix-derive --locked
cargo publish -p rusix --locked
```

Before the first macro publication, the dependent crate's individual dry-run
cannot resolve it from crates.io. Workspace packaging verifies the pair locally;
repeat the main crate's dry-run once the macro version is available.
Tag the published commit as `v<version>` and use the changelog for release notes.
The `dev-cli` executable is a fixture harness; `dev-tools` enables the repository
spacing checker, whose source is excluded from the macro crate archive.

## Diagnostic fixtures

The `rusix` executable exposes fixed demonstration fixtures. Their Rust
definitions live in `crates/rusix/src/bin/cli/`; other definitions can use the
library directly.

### Running fixtures

Generate plain Nix without evaluating it:

```bash
cargo run --locked -p rusix --features dev-cli -- emit good --out target/demo/good
```

Evaluate a successful fixture and one that deliberately fails:

```bash
cargo run --locked -p rusix --features dev-cli -- check good --out target/demo/good
cargo run --locked -p rusix --features dev-cli -- check nested --out target/demo/nested
```

`nested` exits with status 1 and maps a division-by-zero error back to Rust.
Compiler fault injections such as `codegen-bug` are fixtures for checking error
classification.

The `selective` fixture checks lazy evaluation:

```bash
cargo run --locked -p rusix --features dev-cli -- check selective --out target/selective/good --select good
cargo run --locked -p rusix --features dev-cli -- check selective --out target/selective/bad --select bad
```

Selecting `good` returns `42`; selecting `bad` reports a division failure.
Plain `check --select` takes one literal top-level attribute name.

For NixOS module behavior:

```bash
cargo run --locked -p rusix --features dev-cli -- check-nixos good --out target/nixos/good
cargo run --locked -p rusix --features dev-cli -- check-nixos merge-two --out target/nixos/merge-two
```

`merge-two` exits with status 1 and reports conflicting definitions.
`check-nixos --select` accepts a dot-separated option path, such as
`services.openssh.ports`. The fixture driver uses `lib.evalModules` with selected
upstream modules; it does not construct or activate a complete NixOS system.

### Inspecting artifacts

| Artifact | Contents |
| --- | --- |
| `generated.nix` | Compiled plain configuration |
| `source-map.json` | Plain source and Rust origin metadata |
| `module.nix` | Compiled NixOS module |
| `module-map.json` | Module source and definition metadata |
| `nixos-driver.nix` | Module evaluation harness |
| `evaluation.nix` | Expression selecting the requested module result |
| `value.json` | Successful evaluation result |
| `diagnostic.json` | Structured Rusix error report |
| `diagnostic.txt` | Rendered Rust-facing error |
| `nix.stderr` | Original Nix output |

Compilation failures may produce only diagnostic files. Reusing an output
directory replaces compiler-owned artifacts while preserving unrelated files.
Generated harness paths refer to session-staged sources, so re-evaluate through
`NixSession` rather than treating the output directory as a standalone Nix environment.

The fixture script compares snapshots and saves CLI artifacts under
`target/diagnostic-fixtures/`:

```bash
bash scripts/check-fixtures.sh
```

Deliberate fixture failures are expected. The script checks their diagnostic
categories. When changing diagnostics, compare the Rust-facing result with the
original Nix reason and preserve explicit unmapped cases.

Further documentation:

- [Runtime diagnostics](runtime-diagnostics.md) explains source recovery and its limits.
- [Expression rendering](nix-expression-rendering.md) covers precedence rules.
- [Generated Nix layout](generated-nix-layout.md) explains formatting and source spans.
- [Typed package authoring](typed-package-values.md) describes the deferred interfaces.
- [Examples](../examples/README.md) introduces the models and package implementations.
