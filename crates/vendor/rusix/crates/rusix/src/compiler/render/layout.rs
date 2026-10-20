//! Lay out attributed documents without changing their expression tokens.
use super::{Generated, RenderLayout, SourceSpan};
use crate::ir::Origin;

/// An immutable layout document with a cached width for its single-line form.
pub(super) struct Doc<'a> {
    /// Text, break opportunities and attribution belonging to this fragment.
    kind: Kind<'a>,
    /// Character count when flattened; a mandatory newline cannot fit inline.
    flat_width: usize,
}

/// Layout instructions; attribution remains attached until final byte emission.
enum Kind<'a> {
    Text(String),
    Break(&'static str),
    HardLine,
    Concat(Vec<Doc<'a>>),
    Nest(Box<Doc<'a>>),
    Group(Box<Doc<'a>>),
    Attributed {
        /// Rust operation represented by this document's output range.
        origin: &'a Origin,
        /// Whether a Nix error position here identifies this operation's failure.
        diagnostic_site: bool,
        /// Expression layout, including child documents with their own origins.
        body: Box<Doc<'a>>,
    },
}

/// An indivisible token or literal; its contents never acquire layout whitespace.
pub(super) fn text(value: impl Into<String>) -> Doc<'static> {
    let value = value.into();
    let flat_width = if value.contains('\n') {
        usize::MAX
    } else {
        value.chars().count()
    };
    Doc {
        kind: Kind::Text(value),
        flat_width,
    }
}

/// A space when the enclosing group fits, otherwise an indented newline.
pub(super) fn line<'a>() -> Doc<'a> {
    Doc {
        kind: Kind::Break(" "),
        flat_width: 1,
    }
}

/// A mandatory newline, including the end of an inspection comment.
pub(super) fn hard_line() -> Doc<'static> {
    Doc {
        kind: Kind::HardLine,
        flat_width: usize::MAX,
    }
}

pub(super) fn concat(parts: Vec<Doc<'_>>) -> Doc<'_> {
    let flat_width = parts
        .iter()
        .fold(0_usize, |width, part| width.saturating_add(part.flat_width));
    Doc {
        kind: Kind::Concat(parts),
        flat_width,
    }
}

/// Indent lines broken within this document by two spaces.
pub(super) fn nest(body: Doc<'_>) -> Doc<'_> {
    Doc {
        flat_width: body.flat_width,
        kind: Kind::Nest(Box::new(body)),
    }
}

/// Flatten soft breaks if this group fits the remaining line.
pub(super) fn group(body: Doc<'_>) -> Doc<'_> {
    Doc {
        flat_width: body.flat_width,
        kind: Kind::Group(Box::new(body)),
    }
}

pub(super) fn attributed<'a>(origin: &'a Origin, diagnostic_site: bool, body: Doc<'a>) -> Doc<'a> {
    Doc {
        flat_width: body.flat_width,
        kind: Kind::Attributed {
            origin,
            diagnostic_site,
            body: Box::new(body),
        },
    }
}

/// Pending output or the end of a source range, kept in expression traversal order.
#[derive(Clone, Copy)]
enum Command<'d, 'a> {
    /// Remaining document, nesting indentation, and whether soft breaks stay flat.
    Visit(&'d Doc<'a>, usize, bool),
    /// Start byte, Rust origin and diagnostic-site flag for a closing source span.
    Close(usize, &'a Origin, bool),
}

/// Check a cached group width and the following line's suffix without rescanning
/// that group's subtree. Lookahead stops at the first break or exhausted width.
fn fits(width: usize, remaining: usize, pending: &[Command<'_, '_>]) -> bool {
    if width > remaining {
        return false;
    }

    let mut remaining = remaining - width;
    let mut suffix = pending.iter().rev().copied();
    let mut children = Vec::new();
    while let Some(command) = children.pop().or_else(|| suffix.next()) {
        let Command::Visit(doc, indent, flat) = command else {
            continue;
        };
        match &doc.kind {
            Kind::Text(value) => {
                let width = if doc.flat_width == usize::MAX {
                    value.split('\n').next().unwrap().chars().count()
                } else {
                    doc.flat_width
                };
                if width > remaining {
                    return false;
                }
                remaining -= width;
                if value.contains('\n') {
                    return true;
                }
            }
            Kind::Break(value) if flat => {
                if value.len() > remaining {
                    return false;
                }
                remaining -= value.len();
            }
            Kind::Break(_) | Kind::HardLine => return true,
            Kind::Concat(parts) => children.extend(
                parts
                    .iter()
                    .rev()
                    .map(|part| Command::Visit(part, indent, flat)),
            ),
            Kind::Nest(body) => children.push(Command::Visit(body, indent + 2, flat)),
            Kind::Group(body) | Kind::Attributed { body, .. } => {
                if flat && body.flat_width != usize::MAX {
                    if body.flat_width > remaining {
                        return false;
                    }
                    remaining -= body.flat_width;
                } else {
                    children.push(Command::Visit(body, indent, flat));
                }
            }
        }
    }

    true
}

/// Emit text and spans together. A delayed indent keeps blank lines free of
/// trailing spaces and excludes indentation from a child's own source range.
pub(super) fn render(doc: &Doc<'_>, width: usize, layout: RenderLayout) -> Generated {
    let mut generated = Generated::default();
    generated
        .source
        .push_str("# Generated by Rusix. Edit the Rust source.\n");
    let compact = layout == RenderLayout::Compact;
    let mut pending = vec![Command::Visit(doc, 0, compact)];
    let mut enclosing = Vec::new();
    let mut column = 0;
    let mut indentation = Some(0);

    while let Some(command) = pending.pop() {
        let (doc, indent, flat) = match command {
            Command::Visit(doc, indent, flat) => (doc, indent, flat),
            Command::Close(start, origin, diagnostic_site) => {
                enclosing.pop();
                generated.spans.push(SourceSpan {
                    start,
                    end: generated.source.len(),
                    origin: origin.clone(),
                    enclosing: enclosing
                        .iter()
                        .map(|origin: &&Origin| (*origin).clone())
                        .collect(),
                    diagnostic_site,
                });
                continue;
            }
        };
        match &doc.kind {
            Kind::Text(value) => {
                if !value.is_empty() {
                    flush_indent(&mut generated, &mut column, &mut indentation);
                    generated.source.push_str(value);
                    column = if value.contains('\n') {
                        value.rsplit('\n').next().unwrap().chars().count()
                    } else {
                        column + doc.flat_width
                    };
                }
            }
            Kind::Break(value) if flat => {
                generated.source.push_str(value);
                column += value.len();
            }
            Kind::Break(_) | Kind::HardLine => {
                generated.source.push('\n');
                column = 0;
                // Deep expression trees must not consume the entire line with
                // indentation and force every small child onto another line.
                indentation = Some(if compact { 0 } else { indent.min(24) });
            }
            Kind::Concat(parts) => pending.extend(
                parts
                    .iter()
                    .rev()
                    .map(|part| Command::Visit(part, indent, flat)),
            ),
            Kind::Nest(body) => pending.push(Command::Visit(body, indent + 2, flat)),
            Kind::Group(body) => {
                let available = width.saturating_sub(indentation.unwrap_or(column));
                let flat = flat || fits(body.flat_width, available, &pending);
                pending.push(Command::Visit(body, indent, flat));
            }
            Kind::Attributed {
                origin,
                diagnostic_site,
                body,
            } => {
                flush_indent(&mut generated, &mut column, &mut indentation);
                pending.push(Command::Close(
                    generated.source.len(),
                    origin,
                    *diagnostic_site,
                ));
                enclosing.push(origin);
                pending.push(Command::Visit(body, indent, flat));
            }
        }
    }

    generated.source.push('\n');
    generated
}

fn flush_indent(generated: &mut Generated, column: &mut usize, indentation: &mut Option<usize>) {
    if let Some(indent) = indentation.take() {
        generated.source.extend(std::iter::repeat_n(' ', indent));
        *column = indent;
    }
}
