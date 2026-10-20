# Rusix development

Keep the Rust → semantic IR → Nix AST → Nix → Rust diagnostic loop small.
Users edit Rust; generated Nix is compiler output.

## Documentation

Core docs explain concepts. Examples teach usage. Both assume Rust knowledge,
not deep Nix, nixpkgs or NixOS knowledge.

Every library and example struct field must have a meaningful comment, including
private fields, tuple fields and fields emitted by macro templates. Public fields use
Rust doc comments and explain what the field actually represents.

For core/library code:

- Explain the underlying Nix concept in plain language before Rusix semantics.
- Public structs, enums, traits and macros explain what they represent and why
  an author would use them. Avoid leading with unexplained jargon.
- Method docs describe observable behavior before implementation details.
- Distinguish Rust-time construction and validation from deferred Nix evaluation.
- Use small Rust-to-Nix examples when they clarify the mapping.
- Keep `#![warn(missing_docs)]` on intended public library crates and check docs
  with `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked`.
- Narrow accidental public visibility rather than documenting internal-only APIs.

For examples:

- Treat examples as executable documentation showing normal user-facing Rust
  with minimal boilerplate. Prefer ordinary structs and enums where possible.
- Start with 1–2 sentences of module-level `//!` purpose documentation.
- Comment important structs and fields to explain Nix concepts and Rusix
  behavior; do not narrate obvious Rust syntax.
- Keep substantial examples' model, inputs, lowering and main separate so the
  authoring surface is easy to find without reading compatibility plumbing.
- Separate declarations and conceptual blocks with blank lines. Keep simple
  concepts simple to use.

## Development checks and constraints

Commit after each reasonable, coherent, verified batch of work. Use the configured
human Git identity only; never add AI/LLM/assistant/agent/harness/tool/vendor
co-author or attribution trailers.

All Nix subprocesses, including version checks, must go through
`crates/rusix/src/evaluation.rs`. Each command explicitly selects a fresh
disposable local store. Never invoke Nix directly against the host store, even
for evaluation. Do not run activation, deployment, profile, rebuild, host GC,
privileged operations, or change host Nix configuration. Do not add network
fetches, flakes, or builders to this POC. The explicitly requested NixOS
experiment uses the checked, pinned `vendor/nixpkgs` Git submodule (a staged minimal
subset and the same full checkout for interoperability); evaluation is offline.

Choose checks based on what changed. For edits limited to prose, comments or
Markdown links, check `git diff --check` and any changed links. Do not run Cargo
tests, rustfmt, Clippy or rustdoc for these edits. If a documentation code example
changes, verify that example with a focused check instead of the workspace suite.

For changes to Rust code, dependencies or build behavior, run
`cargo test --workspace --locked`, `cargo fmt --all --check`, and
`cargo clippy --workspace --all-targets --locked -- -D warnings`.
The tests require `nix` and `nix-instantiate`; missing tooling is a failure,
not a reason to silently skip the round-trip tests.

Rust readability: separate distinct item declarations at the same level with
one blank line, including items in inline modules, impls, traits, and function
blocks. Keep comments/docs and attributes attached to their item. Consecutive
plain `use` declarations may stay together as an import group; separate the
group from other items. Keep tightly related statements and fields compact.
Use blank lines between conceptual blocks in non-trivial Rust functions; keep tightly related statements together.
Apply the convention to macro templates and documentation examples by review.

Stable rustfmt remains the canonical formatter. It preserves item spacing but
does not enforce it. Run the structural spacing check (also run by workspace
tests and the fixture script):

```bash
cargo run --locked --quiet -p rusix-derive --features dev-tools --bin check-rust-spacing
```

After rustfmt, its `--fix` option inserts missing blank lines. The checker parses
all repository `.rs` files except `target/`, `.git`, and nested Git checkouts;
it checks AST item lists, not opaque macro tokens or strings/docs.

`bash scripts/check-fixtures.sh` compares the evaluator diagnostic snapshots and
saves CLI artifacts under `target/diagnostic-fixtures/`. Snapshots live in
`tests/snapshots/`. Update an expected snapshot only after understanding the
behavior change. Preserve original Nix diagnostics and explicit unmapped cases.

Keep upstream diagnostic parsing in `diagnostic.rs`. Prefer JSON trace fields;
test the text fallback too. Syntax/static generated errors must remain compiler
failures with no Rust blame. Record provenance limitations in README.md.

Generated values must stay lazy: do not reintroduce broad `deepSeq`/`seq`
forcing. Runtime contexts belong on fallible operations; source-map ancestry
recovers enclosing paths. Keep the selective good/bad tests and the existing
nested operation diagnostic passing. Selection must use escaped attribute data
through the same isolated helper, never user-supplied command flags.
