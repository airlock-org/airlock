mod layout;

mod precedence;

use crate::compiler::ast::{Builtin, NixExpr, NixKind};
use crate::ir::Origin;
use layout::{Doc, attributed, concat, group, hard_line, line, nest, text};
use precedence::{Context, precedence};
use serde::{Deserialize, Serialize};

/// A range of generated Nix text linked to the Rust operation that created it.
/// `start` is inclusive and `end` exclusive. Nested ranges let a Nix error
/// position identify the most specific Rust expression and its surrounding
/// configuration path.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Inclusive UTF-8 byte offset from the start of [`Generated::source`].
    pub start: usize,
    /// Exclusive byte offset; the byte at `end` belongs outside this span.
    pub end: usize,
    /// Rust origin of the expression occupying this range.
    pub origin: Origin,
    /// Enclosing Rust operations, such as the containing field assignment.
    /// These explain the configuration path even without runtime error markers.
    #[serde(default)]
    pub enclosing: Vec<Origin>,
    /// Whether this range identifies an operation’s own failure without a runtime marker.
    /// Used when choosing between a generated failure position and an enclosing call;
    /// literal values and function definitions cannot displace the consuming operation.
    #[serde(default)]
    pub diagnostic_site: bool,
}

/// Nix source produced by the compiler, with its links back to Rust locations.
/// Save the source and spans together to translate later Nix error positions.
/// This is output to inspect or evaluate; normal configuration authors edit
/// Rust rather than this generated text.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Generated {
    /// Complete rendered expression, including selected runtime error contexts.
    /// Fine-grained origin comments are present only when explicitly requested.
    pub source: String,
    /// Text ranges linked to Rust operations and their enclosing configuration fields.
    pub spans: Vec<SourceSpan>,
    /// Saved authored package dependencies used to recover Rust locations from
    /// nixpkgs build-recipe validation failures. Absent when no metadata is needed.
    /// This diagnostic metadata is independent of the generated Nix expression.
    #[doc(hidden)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_metadata: Option<serde_json::Value>,
}

/// Choose readable compiler output or smaller text for build artifacts.
/// Both layouts preserve expression tokens, literal contents and diagnostic mappings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderLayout {
    /// Wrap at 100 characters with two-space nesting, capped at 24 spaces.
    #[default]
    Readable,
    /// Replace optional line breaks with spaces and omit layout indentation.
    /// Inspection comments still end with a newline.
    Compact,
}

/// Control layout and optional aids for manually inspecting generated Nix.
/// Source maps and runtime diagnostic boundaries are preserved regardless of
/// these options. Each rendering produces spans for its own source text.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderOptions {
    /// Layout used for generated source; defaults to readable, bounded indentation.
    pub layout: RenderLayout,
    /// Show `# rn-...` before each attributed expression for human inspection.
    /// Defaults to false. Diagnostics use source maps and selected runtime
    /// contexts, not these comments.
    pub origin_comments: bool,
}

impl Generated {
    /// Find the first source-map origin with this ID; reused expressions may have several spans.
    pub fn origin(&self, id: &str) -> Option<&Origin> {
        self.spans
            .iter()
            .find(|span| span.origin.id == id)
            .map(|span| &span.origin)
    }

    /// Find the narrowest origin at a one-based generated line and byte column.
    /// Returns `None` for invalid positions or compiler syntax with no attribution.
    pub fn at_position(&self, line: usize, column: usize) -> Option<&Origin> {
        self.span_at_position(line, column).map(|span| &span.origin)
    }

    /// Find the narrowest half-open span containing a one-based line and byte column.
    /// This preserves ancestry when runtime error contexts have disappeared.
    pub fn span_at_position(&self, line: usize, column: usize) -> Option<&SourceSpan> {
        if line == 0 || column == 0 {
            return None;
        }

        let start = if line == 1 {
            0
        } else {
            self.source.match_indices('\n').nth(line - 2)?.0 + 1
        };
        let line_end = self.source[start..]
            .find('\n')
            .map_or(self.source.len(), |n| start + n);
        let offset = start.checked_add(column - 1)?;
        if offset >= line_end {
            return None;
        }

        self.spans
            .iter()
            .filter(|span| span.start <= offset && offset < span.end)
            .min_by_key(|span| span.end - span.start)
    }

    /// Without a leading parenthesis, a lookup and its variable can start at the
    /// same byte (or be separated only by inspection comments). A failure there
    /// belongs to the nearest consuming operation,
    /// rather than to the variable's declaration. Keep general span lookup exact.
    pub(crate) fn diagnostic_span_at_position(
        &self,
        line: usize,
        column: usize,
        boundary: Option<&str>,
    ) -> Option<&SourceSpan> {
        let smallest = self.span_at_position(line, column)?;
        if smallest.diagnostic_site {
            return Some(smallest);
        }

        // A known runtime boundary remains authoritative unless a more precise
        // failing operation exists inside it. Do not promote to an outer consumer.
        let limit = self
            .spans
            .iter()
            .filter(|span| {
                Some(span.origin.id.as_str()) == boundary
                    && span.start <= smallest.start
                    && span.end >= smallest.end
            })
            .min_by_key(|span| span.end - span.start);

        self.spans
            .iter()
            .filter(|span| {
                span.diagnostic_site
                    && span.start <= smallest.start
                    && span.end >= smallest.end
                    && limit.is_none_or(|limit| span.start >= limit.start && span.end <= limit.end)
            })
            .min_by_key(|span| span.end - span.start)
            .or(Some(smallest))
    }
}

/// Render advanced backend syntax into escaped Nix source and attribution ranges.
/// Does not validate or evaluate the AST; ordinary callers should use [`crate::compile`]
/// or [`crate::nixos::compile_module`] to validate semantic IR first.
/// Parentheses follow Nix precedence and preserve explicit AST groups and distinct
/// attributed lookup boundaries. Layout uses a 100-character target and two-space
/// indentation capped at 24 spaces. Indivisible literals can exceed that target; string contents are
/// never reformatted.
pub fn render(ast: &NixExpr) -> Generated {
    render_with_options(ast, RenderOptions::default())
}

/// Render Nix with optional origin comments for manual inspection.
/// This uses the same renderer and runtime contexts as [`render`]; spans are
/// recomputed for this output rather than reused from another rendering.
pub fn render_with_options(ast: &NixExpr, options: RenderOptions) -> Generated {
    let document = expression(ast, options);
    layout::render(&document, 100, options.layout)
}

/// Preserve an expression's attribution until the final layout writes its bytes.
fn expression(expr: &NixExpr, options: RenderOptions) -> Doc<'_> {
    in_context(expr, options, Context::Expression)
}

/// Keep intrinsic attribution separate from parentheses owned by the parent syntax.
fn in_context(expr: &NixExpr, options: RenderOptions, context: Context) -> Doc<'_> {
    let mut body = syntax(&expr.kind, options);
    let runtime_context = expr.error_context && expr.origin.is_some();
    if let Some(origin) = &expr.origin
        && runtime_context
    {
        body = application(
            text("builtins.addErrorContext"),
            vec![
                text(quote(&origin.id)),
                group_for(body, precedence::kind(&expr.kind), Context::Simple),
            ],
        );
    }
    if let Some(origin) = &expr.origin {
        let diagnostic_site = !expr.error_context
            && match &expr.kind {
                NixKind::Call(..)
                | NixKind::Apply(..)
                | NixKind::Binary(..)
                | NixKind::If(..)
                | NixKind::Assert(..) => true,
                // Empty view roots render only their child, with no lookup.
                NixKind::Select(_, path) | NixKind::ArgumentSelect(_, path) => !path.is_empty(),
                _ => false,
            };
        body = attributed(origin, diagnostic_site, body);

        if options.origin_comments {
            // Comments are an inspection aid, outside this expression's own span.
            body = concat(vec![text(format!("# {}", origin.id)), hard_line(), body]);
        }
    }

    // Implicit parentheses belong to the enclosing syntax, outside the child's
    // own span. This distinguishes an outer missing selection from its base.
    if context.requires_parentheses(precedence(expr))
        || matches!(context, Context::SelectionBase)
            && (precedence::path_base(expr) || precedence::selection_boundary(expr))
    {
        parens(body)
    } else {
        body
    }
}

/// Group only where the enclosing Nix grammar cannot accept this expression.
fn group_for(body: Doc<'_>, precedence: precedence::Precedence, context: Context) -> Doc<'_> {
    if context.requires_parentheses(precedence) {
        parens(body)
    } else {
        body
    }
}

/// Explicit groups and grammar-required parentheses retain the child layout.
fn parens(body: Doc<'_>) -> Doc<'_> {
    // Parentheses preserve syntax; only the expression inside owns indentation.
    concat(vec![text("("), body, text(")")])
}

fn join<'a>(parts: Vec<Doc<'a>>, separator: impl Fn() -> Doc<'a>) -> Doc<'a> {
    let mut joined = Vec::new();
    for (index, part) in parts.into_iter().enumerate() {
        if index > 0 {
            joined.push(separator());
        }
        joined.push(part);
    }

    concat(joined)
}

/// Collections stay inline when their entire group fits; otherwise each item
/// gets a line and may choose its own independent layout.
fn collection<'a>(open: &str, items: Vec<Doc<'a>>, close: &str) -> Doc<'a> {
    if items.is_empty() {
        return text(format!("{open} {close}"));
    }

    group(concat(vec![
        text(open),
        nest(concat(vec![line(), join(items, line)])),
        line(),
        text(close),
    ]))
}

fn application<'a>(head: Doc<'a>, arguments: Vec<Doc<'a>>) -> Doc<'a> {
    let mut parts = vec![head];
    for argument in arguments {
        parts.push(nest(concat(vec![line(), argument])));
    }

    group(concat(parts))
}

fn binding<'a>(key: String, value: Doc<'a>) -> Doc<'a> {
    group(concat(vec![
        text(format!("{key} =")),
        nest(concat(vec![line(), value])),
        text(";"),
    ]))
}

fn syntax(kind: &NixKind, options: RenderOptions) -> Doc<'_> {
    let child = |expr| expression(expr, options);
    match kind {
        NixKind::Bool(v) => text(if *v { "true" } else { "false" }),
        NixKind::Int(i64::MIN) => text("(-9223372036854775807 - 1)"),
        NixKind::Int(v) => text(v.to_string()),
        NixKind::Float(v) => {
            // Preserve floating types and Rust's shortest round-trip precision.
            let mut literal = format!("{v:?}");
            if let Some(exponent) = literal.find('e')
                && !literal[..exponent].contains('.')
            {
                literal.insert_str(exponent, ".0");
            }
            text(literal)
        }
        NixKind::Null => text("null"),
        NixKind::String(v) => text(quote(v)),
        NixKind::Path(v) | NixKind::Variable(v) => text(v.clone()),
        NixKind::Select(value, path) | NixKind::ArgumentSelect(value, path) => {
            if path.is_empty() {
                return child(value);
            }

            let mut parts = vec![in_context(value, options, Context::SelectionBase)];
            parts.extend(path.iter().map(|part| {
                let attribute = if attribute_identifier(part) {
                    part.clone()
                } else {
                    quote(part)
                };
                text(format!(".{attribute}"))
            }));
            concat(parts)
        }
        NixKind::Lambda(argument, body) => group(concat(vec![
            text(format!("{argument}:")),
            nest(concat(vec![line(), child(body)])),
        ])),
        NixKind::Function(arguments, body) => {
            let mut parameters: Vec<_> = arguments.iter().map(|name| text(name.clone())).collect();
            if parameters.is_empty() {
                // Preserve the backend constructor's existing tokens even for
                // this invalid compiler-owned empty pattern.
                parameters.push(text(""));
            }
            parameters.push(text("..."));
            function(parameters, child(body))
        }
        NixKind::ArgumentFunction(arguments, body)
        | NixKind::OpenArgumentFunction(arguments, body) => {
            let mut parameters: Vec<_> = arguments
                .iter()
                .map(|(name, default)| match default {
                    None => text(name.clone()),
                    Some(value) => group(concat(vec![
                        text(format!("{name} ?")),
                        nest(concat(vec![line(), child(value)])),
                    ])),
                })
                .collect();
            if matches!(kind, NixKind::OpenArgumentFunction(..)) {
                parameters.push(text("..."));
            }

            function(parameters, child(body))
        }
        NixKind::Group(value) => parens(child(value)),
        NixKind::List(items) => collection(
            "[",
            items
                .iter()
                .map(|item| in_context(item, options, Context::Simple))
                .collect(),
            "]",
        ),
        NixKind::AttrSet(bindings) => collection(
            "{",
            bindings
                .iter()
                .map(|(path, value)| {
                    let key = path
                        .iter()
                        .map(|part| quote(part))
                        .collect::<Vec<_>>()
                        .join(".");
                    binding(key, child(value))
                })
                .collect(),
            "}",
        ),
        NixKind::Call(builtin, args) => application(
            text(match builtin {
                Builtin::Div => "builtins.div",
                Builtin::Throw => "builtins.throw",
                Builtin::Import => "builtins.import",
                Builtin::GetAttr => "builtins.getAttr",
                Builtin::ToPath => "builtins.toPath",
                Builtin::ToString => "builtins.toString",
            }),
            args.iter()
                .map(|arg| in_context(arg, options, Context::Simple))
                .collect(),
        ),
        NixKind::Binary(op, left, right) => {
            let (operator, left_context, right_context) = precedence::binary(*op);
            group(concat(vec![
                in_context(left, options, left_context),
                nest(concat(vec![
                    line(),
                    text(format!("{operator} ")),
                    in_context(right, options, right_context),
                ])),
            ]))
        }
        NixKind::If(condition, yes, no) => group(concat(vec![
            text("if "),
            child(condition),
            text(" then"),
            nest(concat(vec![line(), child(yes)])),
            line(),
            text("else"),
            nest(concat(vec![line(), child(no)])),
        ])),
        NixKind::Assert(condition, value) => group(concat(vec![
            text("assert "),
            child(condition),
            text(";"),
            nest(concat(vec![line(), child(value)])),
        ])),
        NixKind::Let(name, value, body) => group(concat(vec![
            text("let"),
            nest(concat(vec![line(), binding(name.clone(), child(value))])),
            line(),
            text("in"),
            nest(concat(vec![line(), child(body)])),
        ])),
        NixKind::Apply(function, argument) => application(
            in_context(function, options, Context::ApplicationFunction),
            vec![in_context(argument, options, Context::Simple)],
        ),
    }
}

fn function<'a>(parameters: Vec<Doc<'a>>, body: Doc<'a>) -> Doc<'a> {
    let pattern = if parameters.is_empty() {
        text("{  }:")
    } else {
        group(concat(vec![
            text("{"),
            nest(concat(vec![
                line(),
                join(parameters, || concat(vec![text(","), line()])),
            ])),
            line(),
            text("}:"),
        ]))
    };
    group(concat(vec![pattern, nest(concat(vec![line(), body]))]))
}

/// Quoting is required for literal punctuation, interpolation and reserved words.
fn attribute_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || "_-'".contains(c))
        && ![
            "if", "then", "else", "assert", "with", "let", "in", "rec", "inherit", "or",
        ]
        .contains(&name)
}

/// Nix string literals, including escaping interpolation; JSON escaping alone
/// would wrongly leave `${...}` executable and use unsupported `\u` escapes.
pub(crate) fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    let mut chars = value.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '$' if chars.peek() == Some(&'{') => out.push_str("\\$"),
            _ => out.push(c),
        }
    }

    out.push('"');
    out
}

#[cfg(all(test, feature = "evaluation"))]
mod tests;

#[cfg(all(test, feature = "evaluation"))]
mod precedence_tests;
