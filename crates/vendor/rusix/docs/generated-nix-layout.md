# Generated Nix layout

Generated Nix is compiler output. Rusix formats it for inspection while recording
source spans that connect evaluator errors to Rust origins. Edit the Rust model
and compile again to change the output.

The readable renderer targets 100 characters per line and uses two-space indentation,
capped at 24 spaces so deeply nested expressions still have room to fit.
`RenderOptions { layout: RenderLayout::Compact, ..Default::default() }`
omits optional line breaks and indentation for smaller build artifacts.
Indivisible literals, including escaped shell text, may exceed that width.
Literal contents, runtime diagnostic contexts and NixOS metadata are preserved.

## Layout rules

| Construct | Layout |
| --- | --- |
| Literal, variable or path | Unbroken text with preserved escaping |
| List or attribute set | Inline when it fits; otherwise one item or binding per line |
| Binding or lambda | Inline short values or bodies; indent longer expressions |
| Application | Break long argument lists onto indented lines |
| Function argument set | Inline short patterns; otherwise one argument per line |
| Conditional | Inline short branches; otherwise indent branches and align `else` |
| Let expression | Inline small bindings; otherwise separate bindings, `in` and body |
| Binary operation | Break long expressions before the operator |
| Selection | Use safe identifiers or escaped quoted attribute names |

[Expression rendering](nix-expression-rendering.md) determines parentheses before
layout. Line breaks preserve the AST's meaning and lazy evaluation.

## Source maps and inspection

The compiler uses one pipeline:

```text
Nix AST → attributed documents → formatted Nix text + source spans
```

The [document engine](../crates/rusix/src/compiler/render/layout.rs) groups text
and optional breaks, retaining origin and diagnostic-site metadata. It records
each attributed expression's byte range and enclosing origins during emission.
Width counts characters; source-map offsets count UTF-8 bytes.

Normal output omits fine-grained origin comments. Explicit inspection rendering
adds those comments through `RenderOptions { origin_comments: true, ..Default::default() }`; see
[runtime diagnostics](runtime-diagnostics.md#inspection-rendering).
Both modes retain the same mapped origins, with offsets calculated for their own
text. Save each source map with its corresponding output. Editing generated text
or running an external formatter afterward invalidates those offsets.
