# Runtime diagnostics

Rusix connects Nix evaluation failures to the Rust code that produced the
expression. It retains the original Nix reason and stderr alongside the recovered
Rust location. Some upstream failures have only a broad boundary or no Rust
origin; diagnostics preserve that limitation.

See [error reporting](error-reporting.md) for examples of the user-facing output.

## Mapping failures to Rust

Source maps record generated UTF-8 byte spans, Rust origins and enclosing semantic
origins. Generated evaluator positions can identify a failing operation and its
configuration path, including when evaluation is delayed inside a container.
External-file positions are never treated as positions in generated Rust code.

Selected `builtins.addErrorContext` markers supplement source spans when Nix may
lose the useful generated operation. NixOS definition, schema and assertion
metadata retain module origins independently.

| Expression or boundary | Mapping policy |
| --- | --- |
| Literal, variable, function definition, list, record or assignment | Source spans and ancestry; no container forcing |
| Ordinary attribute or argument lookup, condition, equality, division or `toString` | Generated positions identify the consuming operation |
| Opaque application, imported value or nixpkgs lookup | Retain a runtime boundary |
| Final NixOS option dependency, string-prefix coercion or shallow record union | Retain a narrow runtime boundary |
| Explicit validation or imported-module category check | Retain a validation boundary |
| NixOS assertion, option type, merge or unknown-option error | Use assertion or module metadata |

The translator prefers a concrete generated failure position to function trace
frames. A precise child operation can outrank an enclosing runtime marker.
Literals and function definitions cannot displace a consuming operation.
Structured JSON trace fields are preferred; the text fallback uses narrower
heuristics and may need updates for different Nix diagnostic formats.

Generated source is parsed before evaluation. Invalid syntax or static bindings
are compiler failures and receive no Rust primary location, even when a source
span is available. A valid expression that fails during evaluation is treated
separately. Tool startup and staging failures remain tooling errors.

## NixOS module diagnostics

NixOS may reject an option after its value-level runtime context has ended.
Rusix therefore gives each generated assignment its own inline module with an
origin ID in `_file` metadata. Definition errors can identify that Rust assignment
and its option path without forcing the value early. Assertion messages also
carry origin markers; an evaluator stage marker identifies assertion enforcement.

Independent contributions retain separate module identities. NixOS performs
merging and priority filtering, so a discarded definition can stay unevaluated.
Rusix reports the contributing definitions present in NixOS's error message.
A merge failure may report only the first conflicting pair, while a type failure
can report several invalid definitions. The artifact's other definitions are
not additional evidence of blame.

Read `Diagnostic::origins` for the full reported set. Each entry records its role
and recovery method, with an upstream Nix filename where available. `primary`
is a single-location convenience; `related` describes enclosing operations
rather than additional conflicting definitions.

An upstream definition can map to the Rust import that introduced its file.
That is an import boundary, not an exact upstream assignment or a reconstructed
dependency graph. Unmatched external filenames remain available without a Rust
location. Repeated constructor sites can share origin IDs, so source identity
does not uniquely identify a runtime module instance.

Option classification combines recognized module-system frames with narrow
patterns in the upstream reason. The structured Nix log has no dedicated list
of contributing definitions; the adapter reads definition lines from `raw_msg`.
Changed formatting can defeat that mapping. Multi-definition reports currently
require the tested structured format; the legacy text fallback has more limited
coverage. Original Nix diagnostics remain unchanged.

## Delayed backend validation

A runtime context ends when its wrapped expression evaluates successfully.
A returned list or record can have children forced later, and stdenv can reject
a successfully evaluated dependency afterward. Extra wrappers cannot recover
those supplier origins. Rusix preserves laziness without forcing containers
to keep contexts alive.

For the pinned stdenv dependency-type validator, optional compiler-owned backend
metadata can correlate an owner, dependency field and list index with a Rust
supplier. Direct generated failure evidence takes precedence. A unique validated
supplier uses `Provenance::BackendCorrelation`; indistinguishable suppliers are
reported as candidates without a primary origin.

Unknown overrides or custom backends, unknown list shapes, backend-appended input
tails and failures without corroborating owner frames retain their existing
fallback. Unsupported metadata is ignored, and older artifacts without it still
load. Correlation adds no runtime contexts or early evaluation.

## Origin identifiers

Origins use IDs such as `rn-e160b9de21c72674`, shared by source maps, diagnostic
JSON, selected runtime contexts and NixOS metadata. IDs describe source locations
and semantic purposes, rather than unique runtime instances. Moving Rust source
or changing its line positions changes IDs.

Concrete generated positions distinguish cloned occurrences. Without positions,
ID-only recovery may choose the first matching span and yield an ambiguous path.
Captured Rust locations are points, so diagnostics underline a single caret.

## Inspection rendering

Normal compilation omits fine-grained origin comments. Request them explicitly
when inspecting generated Nix:

```rust
use rusix::{IntoConfig, RenderOptions, compile_with_options};

#[derive(IntoConfig)]
struct Output {
    /// Whether the generated configuration is enabled.
    enabled: bool,
}

let annotated = compile_with_options(
    Output { enabled: true },
    RenderOptions { origin_comments: true, ..Default::default() },
).unwrap();
```

The option is also available through `nixos::compile_module_with_options` and
`render_with_options`. Origin comments are human inspection aids; diagnostics
consume positions, runtime contexts and module metadata.

Normal and inspection maps retain the same origins, ancestry and diagnostic-site
flags, but have different offsets. Always keep each map with its own rendered
source. See [generated Nix layout](generated-nix-layout.md) for formatting rules.
