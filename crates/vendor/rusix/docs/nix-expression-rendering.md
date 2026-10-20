# Nix expression rendering

Rusix renders its Nix AST with the parentheses required by Nix's grammar and
operator precedence. It preserves the expression tree, literal contents and
explicit AST groups. The [layout engine](generated-nix-layout.md) then formats
the result and records source spans.

## Grouping rules

[Precedence handling](../crates/rusix/src/compiler/render/precedence.rs) groups
weaker child expressions and preserves the side required by associativity.
For example, these groups retain distinct expression trees:

```nix
f (g x)
(f x).out
map (x: x.field) xs
f (if enabled then a else b)
a + (b + c)
(a // b) // c
(a == b) == c
```

Attribute values, argument defaults, function bodies and conditional branches
accept full expressions without blanket parentheses. Curried calls render as
`f a b`, while source spans still distinguish the `f a` prefix from the full call.

List items and application arguments have narrower grammar. `[ (f x) y ]`
contains two elements; `[ f x y ]` contains three. Attribute-set arguments can
appear directly, as in `f { key = value; }`.

Path selections require a lexical boundary: `(./source.path).field` selects an
attribute, whereas `./source.path.field` is a single path token. Negative numeric
arguments remain grouped, as in `f (-42)`.

## Attribute names and diagnostic boundaries

Selections use identifiers when safe and escaped quoted names otherwise.
`value."literal.dot"` selects one literal key. Reserved words are quoted;
hyphenated identifiers such as `pkg-config` remain valid.

Independently attributed lookup steps retain a boundary group, such as
`(value.present).missing`. This gives the evaluator distinct positions for
failures at either step. A single structural accessor containing several path
segments can render directly.

Runtime `addErrorContext` wrappers follow application grammar. Their child and
the wrapper itself are grouped only when the surrounding syntax requires it.
The renderer retains origin IDs, occurrence-specific spans and ancestry in both
normal and inspection modes.

When an evaluator error position overlaps a non-fallible variable and its
consuming lookup, diagnostic recovery uses the enclosing diagnostic-site span.
This identifies the Rust lookup while retaining the variable's span for general
source-position inspection. See [runtime diagnostics](runtime-diagnostics.md)
for recovery rules and limitations.
