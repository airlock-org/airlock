//! Explain compilation and Nix evaluation failures in terms of Rust source.
//! [`Diagnostic`] keeps the useful reason, contributing source locations and
//! original Nix output. Consumers can inspect these fields without parsing Nix
//! error text themselves.
use crate::ir::Origin;
use crate::{Generated, SourceSpan};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

/// The layer that rejected a configuration or prevented its evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticKind {
    /// Rust compilation failure category; not produced by Nix evaluation itself.
    Rust,
    /// A generic IR invariant failed before code generation or evaluation.
    Validation,
    /// A deferred expression failed during ordinary Nix evaluation.
    NixEval,
    /// NixOS module processing rejected a definition, such as an unknown option.
    NixosModule,
    /// A value disagreed with its authoritative NixOS option type.
    NixosType,
    /// Independent option definitions could not be merged by NixOS.
    NixosMerge,
    /// A demanded NixOS assertion evaluated to false.
    NixosAssertion,
    /// A failure crossed into an imported existing module or external Nix code.
    ExternalNix,
    /// Generated syntax/static bindings are invalid; a compiler bug, without user blame.
    Compiler,
    /// Tool execution, input staging, filesystem or JSON-output infrastructure failed.
    Tooling,
}

/// How Rusix found the Rust location associated with an error.
/// This is evidence for source attribution, also called *provenance*, rather
/// than the kind of failure. [`DiagnosticKind`] identifies what failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provenance {
    /// A Rust-side IR check retained its introducing operation directly.
    RustValidation,
    /// An origin marker survived in Nix runtime error context.
    ErrorContext,
    /// A generated source position mapped back to a Rust expression span.
    SourceMap,
    /// A pinned backend validator and field/index matched compiler-owned metadata.
    BackendCorrelation,
    /// NixOS definition-file metadata identified one or more contributing operations.
    ModuleDefinition,
    /// An assertion message retained its Rust-origin marker.
    AssertionMessage,
    /// An external failure could be attributed to its Rust import boundary.
    ImportBoundary,
    /// No useful Rust origin could be recovered; inspect the original Nix diagnostic.
    Unavailable,
}

/// Why a source location is included in an error report.
/// An expression failure usually has one primary operation; a NixOS conflict
/// can instead have several independent definitions that all contributed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OriginRole {
    /// The initiating operation for a single-expression failure.
    Primary,
    /// One of several definitions participating in a NixOS merge conflict.
    ConflictingDefinition,
    /// A definition reported by NixOS while validating combined option values.
    ContributingDefinition,
    /// The Rust operation that introduced external Nix code.
    ImportedBoundary,
    /// One of several indistinguishable backend suppliers; no unique culprit is known.
    BackendCandidate,
}

/// One confirmed source or possible backend supplier associated with an error.
/// This differs from [`Diagnostic::related`], which describes enclosing operations
/// that help explain where the failed computation fits in the configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticOrigin {
    /// Recovered Rust location, or `None` for a source known only in Nix.
    pub origin: Option<Origin>,
    /// Why this source belongs to the causal set, rather than merely the surrounding trace.
    pub role: OriginRole,
    /// Evidence that recovered this particular source.
    pub provenance: Provenance,
    /// Upstream definition/import location when useful alongside or instead of Rust.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nix_file: Option<String>,
}

/// A compilation or evaluation failure explained using Rust source locations.
///
/// Inspect [`Self::reason`] for the useful message and [`Self::origins`] for the
/// sources that contributed. NixOS can combine definitions from several modules,
/// so a merge conflict may have several causes rather than one Rust line.
/// [`Self::primary`] is a convenience for single-operation failures.
///
/// [`Self::related`] gives surrounding operations and configuration paths, not
/// additional conflicting definitions. Keep [`Self::raw_nix`] for Nix’s complete
/// original message and trace; Rust-facing rendering omits some of that detail.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Layer that rejected the configuration or failed to run.
    pub kind: DiagnosticKind,
    /// The useful error message, with source markers translated into readable locations and paths.
    pub reason: String,
    /// Single-origin convenience; consult [`Self::origins`] to avoid discarding other causes.
    pub primary: Option<Origin>,
    /// Confirmed sources and explicit candidate suppliers, including merge conflicts.
    #[serde(default)]
    pub origins: Vec<DiagnosticOrigin>,
    /// Enclosing configuration operations, not additional conflicting definitions.
    pub related: Vec<Origin>,
    /// Recovery mechanism for the primary Rust location, if any.
    pub provenance: Provenance,
    /// Unmodified Nix stderr, including JSON events, full traces and temporary paths.
    /// Empty for failures that occur before invoking Nix.
    pub raw_nix: String,
    /// Affected configuration path, such as `services.example.port`, when it can be identified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub option_path: Option<String>,
    /// Imported Nix location useful when Rust attribution stops at the boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_file: Option<String>,
}

impl Diagnostic {
    /// Create an error for a Rust-side configuration check, before running Nix.
    /// `origin` identifies the rejected Rust operation; there is no original Nix trace.
    pub fn validation(origin: Origin, reason: String) -> Self {
        Self {
            kind: DiagnosticKind::Validation,
            reason,
            primary: Some(origin),
            origins: vec![],
            related: vec![],
            provenance: Provenance::RustValidation,
            raw_nix: String::new(),
            option_path: None,
            external_file: None,
        }
        .with_origin_set()
    }

    /// Construct an infrastructure failure without attributing it to user configuration.
    pub fn tooling(reason: impl Into<String>) -> Self {
        Self {
            kind: DiagnosticKind::Tooling,
            reason: reason.into(),
            primary: None,
            origins: vec![],
            related: vec![],
            provenance: Provenance::Unavailable,
            raw_nix: String::new(),
            option_path: None,
            external_file: None,
        }
    }

    /// Translate evaluator stderr using structured events and a textual fallback.
    /// `file` identifies the generated source whose positions match `generated`.
    /// Compiler/tooling failures intentionally receive no Rust blame; NixOS-specific
    /// translation is applied by `crate::NixSession` module evaluation methods.
    pub fn from_nix(kind: DiagnosticKind, raw: &str, generated: &Generated, file: &Path) -> Self {
        if let Some(event) = structured_error(raw) {
            return Self::from_structured(kind, raw, &event, generated, file);
        }

        let messages = decode_messages(raw);
        let text = strip_ansi(&messages.join("\n"));
        let reason = text
            .lines()
            .filter_map(|line| line.trim().strip_prefix("error:"))
            .next_back()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Nix failed; inspect the retained original diagnostic")
            .to_owned();

        let mut related = Vec::new();
        let prefix = format!("at {}:", file.display());
        let lines: Vec<_> = text.lines().collect();
        let innermost_context = lines.iter().rposition(|line| {
            let line = line.trim();
            line.strip_prefix("… ")
                .or_else(|| line.strip_prefix("... "))
                .and_then(origin_id)
                .is_some()
        });
        let boundary = lines.iter().rev().find_map(|line| {
            let trace = line
                .trim()
                .strip_prefix("… ")
                .or_else(|| line.trim().strip_prefix("... "))?;
            generated.origin(origin_id(trace)?)
        });
        let positioned: Vec<_> = lines
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(index, line)| {
                let rest = line.trim().strip_prefix(&prefix)?;
                let mut parts = rest.split(':');
                let line = parts.next()?.parse().ok()?;
                let column = parts.next()?.parse().ok()?;
                let message = lines[..index].iter().rev().find(|l| !l.trim().is_empty())?;
                let operation = (message.trim().starts_with("error:") || operation_frame(message))
                    && innermost_context.is_none_or(|boundary| index > boundary);
                let span = if operation {
                    generated.diagnostic_span_at_position(
                        line,
                        column,
                        boundary.map(|origin| origin.id.as_str()),
                    )?
                } else {
                    generated.span_at_position(line, column)?
                };
                Some((span, operation))
            })
            .collect();
        let local_spans: Vec<_> = positioned.iter().map(|(span, _)| *span).collect();
        let failure_spans: Vec<_> = positioned
            .iter()
            .filter_map(|(span, operation)| operation.then_some(*span))
            .collect();

        // Match trace-shaped lines, excluding ordinary source excerpts and
        // single-line throws. This text fallback is a heuristic, not a parser.
        // Trace order is outermost -> innermost on the tested Nix versions.
        for line in text.lines() {
            let trace = line
                .trim()
                .strip_prefix("… ")
                .or_else(|| line.trim().strip_prefix("... "));
            if let Some(id) = trace.and_then(|s| origin_id(s.trim()))
                && let Some(origin) = generated.origin(id)
                && !related.iter().any(|o: &Origin| o.id == origin.id)
            {
                related.push(origin.clone());
            }
        }

        let (primary, provenance) =
            if kind == DiagnosticKind::Compiler || kind == DiagnosticKind::Tooling {
                // Syntax/static backend errors must never accuse a user's Rust line.
                related.clear();
                (None, Provenance::Unavailable)
            } else {
                recover_operation(&related, &local_spans, &failure_spans)
            };

        Self {
            kind,
            reason,
            primary,
            origins: vec![],
            related,
            provenance,
            raw_nix: raw.into(),
            option_path: None,
            external_file: None,
        }
        .with_enclosing(generated, &local_spans)
        .with_backend(generated, &text_frames(&text), &local_spans, false)
    }

    fn from_structured(
        kind: DiagnosticKind,
        raw: &str,
        event: &serde_json::Value,
        generated: &Generated,
        file: &Path,
    ) -> Self {
        let reason = strip_ansi(
            event["raw_msg"]
                .as_str()
                .unwrap_or("Nix failed; inspect the retained original diagnostic"),
        );
        let frames = event["trace"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        let boundary = frames
            .iter()
            .find_map(|frame| generated.origin(frame["raw_msg"].as_str().and_then(origin_id)?));
        let mapped = |frame: &serde_json::Value| {
            let line = usize::try_from(frame["line"].as_u64()?).ok()?;
            let column = usize::try_from(frame["column"].as_u64()?).ok()?;
            let reported = frame["file"].as_str()?;
            let path = file.to_string_lossy();
            if reported != path && reported != format!("{path}:{line}:{column}") {
                return None;
            }
            if std::ptr::eq(frame, event)
                || operation_frame(frame["raw_msg"].as_str().unwrap_or(""))
            {
                generated.diagnostic_span_at_position(
                    line,
                    column,
                    boundary.map(|origin| origin.id.as_str()),
                )
            } else {
                generated.span_at_position(line, column)
            }
        };
        // The error's own position is more precise than lambda/call-site frames.
        let local_spans: Vec<_> = std::iter::once(event)
            .chain(frames.iter())
            .filter_map(mapped)
            .collect();
        let failure_spans: Vec<_> = std::iter::once(event)
            .chain(
                frames
                    .iter()
                    .take_while(|f| f["raw_msg"].as_str().and_then(origin_id).is_none())
                    .filter(|f| operation_frame(f["raw_msg"].as_str().unwrap_or(""))),
            )
            .filter_map(mapped)
            .collect();

        let mut related = Vec::new();

        // JSON traces are innermost first, unlike the rendered message.
        for frame in frames.iter().rev() {
            if let Some(id) = frame["raw_msg"].as_str().and_then(origin_id)
                && let Some(origin) = generated.origin(id)
                && !related.iter().any(|o: &Origin| o.id == origin.id)
            {
                related.push(origin.clone());
            }
        }

        let (primary, provenance) =
            if matches!(kind, DiagnosticKind::Compiler | DiagnosticKind::Tooling) {
                related.clear();
                (None, Provenance::Unavailable)
            } else {
                recover_operation(&related, &local_spans, &failure_spans)
            };

        Self {
            kind,
            reason,
            primary,
            origins: vec![],
            related,
            provenance,
            raw_nix: raw.into(),
            option_path: None,
            external_file: None,
        }
        .with_enclosing(generated, &local_spans)
        .with_backend(
            generated,
            &structured_frames(event, frames),
            &local_spans,
            mapped(event).is_some(),
        )
    }

    fn with_enclosing(mut self, generated: &Generated, local_spans: &[&SourceSpan]) -> Self {
        if let Some(primary) = &self.primary {
            // Prefer a concrete generated occurrence: an Expr cloned into two
            // assignments has one origin ID but two different enclosing paths.
            let span = local_spans
                .iter()
                .copied()
                .find(|span| span.origin.id == primary.id)
                .or_else(|| {
                    generated
                        .spans
                        .iter()
                        .find(|span| span.origin.id == primary.id)
                });

            if let Some(span) = span {
                let mut related = span.enclosing.clone();
                for origin in self.related.iter().chain(std::iter::once(primary)) {
                    if !related.iter().any(|known| known.id == origin.id) {
                        related.push(origin.clone());
                    }
                }
                self.related = related;
            }
        }

        self.with_origin_set()
    }

    fn with_backend(
        mut self,
        generated: &Generated,
        frames: &[BackendFrame],
        local_spans: &[&SourceSpan],
        direct_failure: bool,
    ) -> Self {
        if self.kind != DiagnosticKind::NixEval || direct_failure {
            return self;
        }
        let Some(matches) = correlate_backend(&self.reason, frames, generated, local_spans) else {
            return self;
        };
        let mut sources = Vec::new();
        for BackendMatch {
            boundary,
            field,
            child,
        } in matches
        {
            for origin in [&boundary.call, &field.origin, &child.consumer] {
                if !self.related.iter().any(|o| o.id == origin.id) {
                    self.related.push(origin.clone());
                }
            }
            if !sources.iter().any(|o: &Origin| o.id == child.origin.id) {
                sources.push(child.origin.clone());
            }
        }
        self.primary = (sources.len() == 1).then(|| sources[0].clone());
        self.provenance = Provenance::BackendCorrelation;
        self.origins = sources
            .into_iter()
            .map(|origin| DiagnosticOrigin {
                origin: Some(origin),
                role: if self.primary.is_some() {
                    OriginRole::Primary
                } else {
                    OriginRole::BackendCandidate
                },
                provenance: Provenance::BackendCorrelation,
                nix_file: None,
            })
            .collect();
        self.with_enclosing(generated, local_spans)
    }

    pub(crate) fn with_origin_set(mut self) -> Self {
        if self.origins.is_empty()
            && let Some(origin) = &self.primary
        {
            self.origins.push(DiagnosticOrigin {
                origin: Some(origin.clone()),
                role: OriginRole::Primary,
                provenance: self.provenance,
                nix_file: self.external_file.clone(),
            });
        }
        self
    }

    /// Produce a compact snapshot-oriented diagnostic without the original Nix stack.
    /// Includes causal origins and the useful reason; use [`Self::render`] for source excerpts.
    pub fn summary(&self) -> String {
        let mut out = format!("error[{}]: {}\n", self.code(), self.message());

        if self.origins.len() > 1 || (self.primary.is_none() && !self.origins.is_empty()) {
            for source in &self.origins {
                if let Some(origin) = &source.origin {
                    out.push_str(&format!(
                        "  --> {}:{}:{}\n   = origin: {}\n",
                        origin.file, origin.line, origin.column, origin.purpose
                    ));
                }
                out.push_str(&format!(
                    "   = role: {:?}; provenance: {:?}\n",
                    source.role, source.provenance
                ));
                if let Some(file) = &source.nix_file {
                    out.push_str(&format!("   = Nix definition: {file}\n"));
                }
            }
        } else if let Some(origin) = &self.primary {
            out.push_str(&format!(
                "  --> {}:{}:{}\n   = origin: {}\n",
                origin.file, origin.line, origin.column, origin.purpose
            ));
        } else if self.kind == DiagnosticKind::NixEval {
            out.push_str("   = Rust origin unavailable\n");
        }

        if let Some(path) = &self.option_path {
            out.push_str(&format!("   = option: {path}\n"));
        }

        if let Some(file) = &self.external_file {
            out.push_str(&format!("   = imported Nix: {file}\n"));
        }

        out.push_str(&format!(
            "   = {}: {}\n",
            if self.kind == DiagnosticKind::Validation {
                "Rusix"
            } else {
                "Nix"
            },
            self.reason
        ));
        out.push_str(&format!("   = provenance: {:?}\n", self.provenance));

        out
    }

    /// Render causal sources, Rust excerpts when available, option paths and the reason.
    /// Relative Rust source files are opened under `source_root`; missing files
    /// leave useful locations intact. The full Nix trace remains in [`Self::raw_nix`].
    pub fn render(&self, source_root: &Path) -> String {
        let mut out = format!("error[{}]: {}\n", self.code(), self.message());

        if self.origins.len() > 1 || (self.primary.is_none() && !self.origins.is_empty()) {
            for source in &self.origins {
                out.push('\n');
                if let Some(origin) = &source.origin {
                    render_location(&mut out, origin, source_root);
                    out.push_str(&format!("   = origin: {}\n", origin.purpose));
                }
                out.push_str(&format!(
                    "   = {}\n",
                    match source.role {
                        OriginRole::Primary => "originating operation",
                        OriginRole::ConflictingDefinition => "conflicting definition",
                        OriginRole::ContributingDefinition => "contributing definition",
                        OriginRole::ImportedBoundary => "imported module boundary",
                        OriginRole::BackendCandidate => "possible backend supplier",
                    }
                ));
                if let Some(file) = &source.nix_file {
                    out.push_str(&format!("   = Nix definition: {file}\n"));
                }
            }
            if let Some(path) = &self.option_path {
                out.push_str(&format!("\n   = option: {path}\n"));
            }
        } else if let Some(origin) = &self.primary {
            render_location(&mut out, origin, source_root);
            out.push_str(&format!("   = origin: {}\n", origin.purpose));
            if let Some(path) = &self.option_path {
                out.push_str(&format!("   = option: {path}\n"));
            } else if let Some(assignment) = self
                .related
                .iter()
                .rev()
                .find(|o| o.purpose.starts_with("set "))
            {
                out.push_str(&format!(
                    "   = option: {}\n",
                    assignment.purpose.trim_start_matches("set ")
                ));
            }
        } else if self.kind == DiagnosticKind::NixEval {
            out.push_str(
                "   = Rust origin unavailable; inspect generated source and raw Nix diagnostic\n",
            );
        }

        if let Some(file) = &self.external_file {
            out.push_str(&format!("   = imported Nix: {file}\n"));
        }

        out.push_str(&format!(
            "   = {}: {}\n",
            if self.kind == DiagnosticKind::Validation {
                "Rusix"
            } else {
                "Nix"
            },
            self.reason.trim().replace('\n', "\n     ")
        ));

        out
    }

    fn code(&self) -> &'static str {
        match self.kind {
            DiagnosticKind::Rust => "rust",
            DiagnosticKind::Validation => "validation",
            DiagnosticKind::NixEval => "nix-eval",
            DiagnosticKind::NixosModule => "nixos-module",
            DiagnosticKind::NixosType => "nixos-type",
            DiagnosticKind::NixosMerge => "nixos-merge",
            DiagnosticKind::NixosAssertion => "nixos-assertion",
            DiagnosticKind::ExternalNix => "external-nix",
            DiagnosticKind::Compiler => "codegen",
            DiagnosticKind::Tooling => "nix-tooling",
        }
    }

    fn message(&self) -> &'static str {
        match self.kind {
            DiagnosticKind::Rust => "Rust rejected the configuration",
            DiagnosticKind::Validation => "invalid Rust configuration IR",
            DiagnosticKind::NixEval => "generated configuration was rejected by Nix",
            DiagnosticKind::NixosModule => "NixOS rejected the generated module",
            DiagnosticKind::NixosType => "NixOS rejected an option value",
            DiagnosticKind::NixosMerge => "conflicting definitions for a NixOS option",
            DiagnosticKind::NixosAssertion => "NixOS configuration assertion failed",
            DiagnosticKind::ExternalNix => "imported NixOS module evaluation failed",
            DiagnosticKind::Compiler => "generated Nix is invalid (Rusix compiler bug)",
            DiagnosticKind::Tooling => "could not run the isolated Nix evaluator",
        }
    }
}

// Runtime origin frames contain only the canonical ID, not arbitrary text
// mentioning an ID. Keep source excerpts and user error messages out of attribution.
pub(crate) fn origin_id(message: &str) -> Option<&str> {
    let hash = message.strip_prefix("rn-")?;
    (hash.len() == 16 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        .then_some(message)
}

/// A builtin/condition frame identifies an operation, unlike a function's definition site.
fn operation_frame(message: &str) -> bool {
    let message = strip_ansi(message);
    message.contains("while calling the '") && message.contains("' builtin")
        || message.contains("while evaluating a branch condition")
}

/// Prefer an inner generated failure over its runtime boundary, never a caller frame.
/// The trace may cross separately generated NixOS definitions, so static nesting alone
/// cannot decide which operation failed; static ancestry supplies the resulting path.
fn recover_operation(
    contexts: &[Origin],
    local_spans: &[&SourceSpan],
    failure_spans: &[&SourceSpan],
) -> (Option<Origin>, Provenance) {
    if let Some(boundary) = contexts.last() {
        if let Some(span) = failure_spans.first()
            && span.diagnostic_site
            && span.origin.id != boundary.id
        {
            return (Some(span.origin.clone()), Provenance::SourceMap);
        }
        return (Some(boundary.clone()), Provenance::ErrorContext);
    }

    match local_spans.first() {
        Some(span) => (Some(span.origin.clone()), Provenance::SourceMap),
        None => (None, Provenance::Unavailable),
    }
}

// The dependency index is emitted by the pinned validator, not by arbitrary
// English error matching. Require its source frame and the ordered owner chain.
struct BackendFrame {
    /// Original trace message with terminal color escapes removed, before interpreting dependency clues.
    message: String,
    /// Nix source filename reported for this trace step, if the evaluator supplied one.
    file: Option<String>,
    /// One-based Nix source line reported for the trace step, when available.
    line: Option<u64>,
    /// One-based Nix source column reported for the trace step, when available.
    column: Option<u64>,
}

fn structured_frames(event: &serde_json::Value, frames: &[serde_json::Value]) -> Vec<BackendFrame> {
    std::iter::once(event)
        .chain(frames)
        .map(|frame| BackendFrame {
            message: strip_ansi(frame["raw_msg"].as_str().unwrap_or("")),
            file: frame["file"].as_str().map(str::to_owned),
            line: frame["line"].as_u64(),
            column: frame["column"].as_u64(),
        })
        .collect()
}

fn text_frames(text: &str) -> Vec<BackendFrame> {
    let mut frames: Vec<BackendFrame> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(message) = line
            .strip_prefix("… ")
            .or_else(|| line.strip_prefix("... "))
        {
            frames.push(BackendFrame {
                message: message.into(),
                file: None,
                line: None,
                column: None,
            });
        } else if let Some(location) = line.strip_prefix("at ")
            && let Some(frame) = frames.last_mut()
            && frame.file.is_none()
        {
            let mut parts = location.trim_end_matches(':').rsplitn(3, ':');
            frame.column = parts.next().and_then(|s| s.parse().ok());
            frame.line = parts.next().and_then(|s| s.parse().ok());
            frame.file = parts.next().map(str::to_owned);
        }
    }
    frames.reverse();
    frames
}

fn pinned_frame(frame: &BackendFrame) -> bool {
    let Some(file) = &frame.file else {
        return false;
    };
    let suffix = format!(":{}:{}", frame.line.unwrap_or(0), frame.column.unwrap_or(0));
    let file = Path::new(file.strip_suffix(&suffix).unwrap_or(file));
    let Ok(pin) = crate::nixos::pin() else {
        return false;
    };

    pin.diagnostic_files.iter().any(|(name, hash)| {
        file.ends_with(name)
            && std::fs::read(file)
                .is_ok_and(|bytes| format!("{:x}", Sha256::digest(bytes)) == *hash)
    })
}

fn dependency_clue(reason: &str) -> Option<(&str, &str, Vec<usize>)> {
    let mut rest = reason.strip_prefix("Dependency is not of a valid type: ")?;
    let mut path = Vec::new();
    while let Some(indexed) = rest.strip_prefix("element ") {
        let (index, tail) = indexed.split_once(" of ")?;
        path.push(index.parse::<usize>().ok()?.checked_sub(1)?);
        rest = tail;
        if path.len() > 12 {
            return None;
        }
    }
    let (field, name) = rest.split_once(" for ")?;
    if path.is_empty() || name.is_empty() || !crate::compiler::provenance::dependency_field(field) {
        return None;
    }
    path.reverse();
    Some((field, name, path))
}

fn owner_frame(frame: &BackendFrame) -> Option<(&str, &str)> {
    if !pinned_frame(frame) {
        return None;
    }
    let rest = frame.message.strip_prefix("while evaluating attribute '")?;
    let (field, owner) = rest.split_once("' of derivation '")?;
    Some((field, owner.strip_suffix('\'')?))
}

fn owner_chain(
    boundary: &crate::compiler::provenance::Boundary,
    owners: &[&str],
    metadata: &crate::compiler::provenance::Metadata,
) -> bool {
    if owners.first().copied() != Some(boundary.full_name.as_str()) {
        return false;
    }
    if owners.len() == 1 {
        return boundary.parents.is_empty();
    }
    boundary.parents.iter().any(|parent| {
        parent == owners[1]
            && metadata.boundaries.iter().any(|b| {
                b.entry.id == boundary.entry.id
                    && !b.opaque_children
                    && b.full_name == *parent
                    && owner_chain(b, &owners[1..], metadata)
            })
    })
}

struct BackendMatch {
    /// Authored build recipe matched to nixpkgs' dependency-validation trace.
    boundary: crate::compiler::provenance::Boundary,
    /// Dependency list in that recipe containing the rejected member.
    field: crate::compiler::provenance::Field,
    /// Matched list member, including the Rust locations that supplied and consumed it.
    child: crate::compiler::provenance::Child,
}

fn correlate_backend(
    reason: &str,
    frames: &[BackendFrame],
    generated: &Generated,
    local_spans: &[&SourceSpan],
) -> Option<Vec<BackendMatch>> {
    let (field, name, path) = dependency_clue(reason)?;
    let validator = frames.iter().position(|frame| {
        pinned_frame(frame)
            && frame.line == Some(284)
            && frame.column == Some(14)
            && frame.message == "while calling the 'throw' builtin"
    })?;
    // A generated operation preceding the backend validator remains stronger.
    if frames[..validator]
        .iter()
        .any(|f| operation_frame(&f.message) && !pinned_frame(f))
    {
        return None;
    }
    let owner_frames: Vec<_> = frames[validator..].iter().filter_map(owner_frame).collect();
    if owner_frames.first()?.0 != field {
        return None;
    }
    // Repeated owner names can be distinct nested calls. Preserve depth rather
    // than collapsing a child and parent with identical derivation names.
    let owners: Vec<_> = owner_frames.into_iter().map(|(_, owner)| owner).collect();
    let metadata = crate::compiler::provenance::read(&generated.backend_metadata)?;
    if local_spans.iter().any(|span| {
        std::iter::once(&span.origin)
            .chain(&span.enclosing)
            .any(|origin| metadata.guard_operations.contains(&origin.id))
    }) || frames.iter().any(|frame| {
        metadata
            .guard_operations
            .iter()
            .any(|id| id == &frame.message)
    }) {
        return None;
    }
    // Use the last mapped frame (the demanded assignment), not all occurrences
    // of a shared origin or unrelated exported package descriptions.
    let scope = local_spans.iter().rev().find(|span| {
        metadata.boundaries.iter().any(|b| {
            span.origin.id == b.entry.id || span.enclosing.iter().any(|o| o.id == b.entry.id)
        })
    })?;
    let mut candidates = metadata
        .boundaries
        .iter()
        .filter(|b| {
            b.name == name
                && (scope.origin.id == b.entry.id
                    || scope.enclosing.iter().any(|o| o.id == b.entry.id))
                && owner_chain(b, &owners, &metadata)
        })
        .peekable();
    candidates.peek()?;
    let mut result = Vec::new();
    for boundary in candidates {
        // Every plausible boundary must have the index. Unknown list lengths or
        // merged check-input tails cannot justify choosing another candidate.
        let field = boundary.fields.iter().find(|f| f.name == field)?;
        let child = field.children.iter().find(|c| c.path == path)?;
        result.push(BackendMatch {
            boundary: boundary.clone(),
            field: field.clone(),
            child: child.clone(),
        });
    }
    Some(result)
}

fn render_location(out: &mut String, origin: &Origin, source_root: &Path) {
    out.push_str(&format!(
        "  --> {}:{}:{}\n",
        origin.file, origin.line, origin.column
    ));
    if let Ok(source) = std::fs::read_to_string(source_root.join(&origin.file))
        && let Some(line) = source.lines().nth(origin.line.saturating_sub(1) as usize)
    {
        out.push_str(&format!(
            "   |\n{:>3} | {line}\n   | {}^\n",
            origin.line,
            " ".repeat(origin.column.saturating_sub(1) as usize)
        ));
    }
}

/// NixOS exposes definition lists only inside raw_msg, not as JSON fields.
/// Keep the pinned showDefs/mergeEqualOption message adapter at the wire boundary.
pub(crate) struct ModuleFailure {
    /// Whether NixOS rejected an unknown option, a value type or conflicting definitions.
    pub(crate) kind: DiagnosticKind,
    /// Configuration option path named in the original NixOS error message.
    pub(crate) option: String,
    /// Definition or import filenames NixOS reported as contributors to this failure.
    pub(crate) files: Vec<String>,
}

pub(crate) fn module_failure(reason: &str) -> Option<ModuleFailure> {
    let (kind, rest) = if let Some(rest) = reason.strip_prefix("A definition for option `") {
        if !rest.split_once("' ")?.1.starts_with("is not of type") {
            return None;
        }
        (DiagnosticKind::NixosType, rest)
    } else {
        let rest = reason.strip_prefix("The option `")?;
        let tail = rest.split_once("' ")?.1;
        let kind = if tail.starts_with("has conflicting definition values:") {
            DiagnosticKind::NixosMerge
        } else if tail.starts_with("does not exist.") {
            DiagnosticKind::NixosModule
        } else {
            return None;
        };
        (kind, rest)
    };

    let option = rest.split_once('\'')?.0.to_owned();
    let files = reason
        .lines()
        .filter_map(|line| {
            line.strip_prefix("- In `")?
                .split_once("':")
                .map(|(file, _)| file.to_owned())
        })
        .collect();

    Some(ModuleFailure {
        kind,
        option,
        files,
    })
}

fn structured_error(raw: &str) -> Option<serde_json::Value> {
    raw.lines()
        .filter_map(|line| {
            let event: serde_json::Value =
                serde_json::from_str(line.strip_prefix("@nix ")?).ok()?;
            (event["action"] == "msg" && event["level"] == 0 && event["raw_msg"].is_string())
                .then_some(event)
        })
        .next_back()
}

/// External frame evidence; callers do not depend on Nix's JSON schema.
pub(crate) fn nix_evidence(raw: &str) -> Vec<(Option<String>, String)> {
    if let Some(event) = structured_error(raw) {
        event["trace"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .map(|frame| {
                (
                    frame["file"].as_str().map(str::to_owned),
                    strip_ansi(frame["raw_msg"].as_str().unwrap_or("")),
                )
            })
            .collect()
    } else {
        vec![]
    }
}

fn decode_messages(raw: &str) -> Vec<String> {
    raw.lines()
        .filter_map(|line| {
            if let Some(json) = line.strip_prefix("@nix ") {
                match serde_json::from_str::<serde_json::Value>(json) {
                    Ok(event) => event.get("msg").and_then(|v| v.as_str()).map(str::to_owned),
                    Err(_) => Some(line.into()),
                }
            } else {
                Some(line.into())
            }
        })
        .collect()
}

fn strip_ansi(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for code in chars.by_ref() {
                if ('@'..='~').contains(&code) {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SourceSpan;

    #[cfg(feature = "evaluation")]
    #[test]
    fn backend_frames_require_pinned_contents_in_external_checkouts() {
        let source = crate::compiler::interop::full_source().unwrap();
        let name = "pkgs/stdenv/generic/make-derivation.nix";
        let external = tempfile::tempdir().unwrap();
        let file = external.path().join(name);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::copy(source.path.join(name), &file).unwrap();
        let frame = BackendFrame {
            message: "upstream dependency validation".into(),
            file: Some(file.to_string_lossy().into_owned()),
            line: Some(1),
            column: Some(1),
        };
        assert!(pinned_frame(&frame));

        let located = BackendFrame {
            message: frame.message.clone(),
            file: Some(format!("{}:1:1", file.display())),
            line: frame.line,
            column: frame.column,
        };
        assert!(pinned_frame(&located));

        std::fs::write(&file, "builtins.throw \"unreviewed contents\"").unwrap();
        assert!(!pinned_frame(&frame));
        assert!(!pinned_frame(&located));
    }

    #[test]
    fn origin_frames_require_one_complete_canonical_id() {
        let id = "rn-e160b9de21c72674";
        assert_eq!(origin_id(id), Some(id));
        for message in [
            "rn-e160b9de21c7267",
            "rn-e160b9de21c726740",
            "rn-e160b9de21c7267g",
            "rn-E160B9DE21C72674",
            "context:rn-e160b9de21c72674",
            "rn-e160b9de21c72674 extra text",
        ] {
            assert_eq!(origin_id(message), None, "{message}");
        }
    }

    #[test]
    fn compact_structured_context_retains_the_origin_and_unmodified_raw_error() {
        let origin = Origin::new("config.rs", 12, 9, "opaque Nix function call");
        let generated = Generated {
            backend_metadata: None,
            source: "f x".into(),
            spans: vec![SourceSpan {
                start: 0,
                end: 3,
                origin: origin.clone(),
                enclosing: vec![],
                diagnostic_site: false,
            }],
        };
        let raw = format!(
            "@nix {}",
            serde_json::json!({
                "action": "msg",
                "level": 0,
                "raw_msg": "external function failed",
                "msg": "unrelated human-readable rendering",
                "trace": [{"raw_msg": origin.id}],
            })
        );

        let diagnostic = Diagnostic::from_nix(
            DiagnosticKind::NixEval,
            &raw,
            &generated,
            Path::new("generated.nix"),
        );
        assert_eq!(diagnostic.primary, Some(origin));
        assert_eq!(diagnostic.provenance, Provenance::ErrorContext);
        assert_eq!(diagnostic.reason, "external function failed");
        assert_eq!(diagnostic.raw_nix, raw);
    }

    #[test]
    fn module_adapter_reads_reported_definition_lines_without_scanning_value_excerpts() {
        let reason = "The option `services.openssh.authorizedKeysCommandUser' has conflicting definition values:\n- In `rn-000000000000000a': \"root\"\n- In `/pinned/sshd.nix': {\n    text = \"- In `innocent.nix': value\";\n}\nUse `lib.mkForce value` or `lib.mkDefault value` to change the priority on any of these definitions.";
        let failure = module_failure(reason).unwrap();
        assert_eq!(failure.kind, DiagnosticKind::NixosMerge);
        assert_eq!(failure.option, "services.openssh.authorizedKeysCommandUser");
        assert_eq!(failure.files, ["rn-000000000000000a", "/pinned/sshd.nix"]);
        assert!(
            module_failure(
                "The option `x' returned a string mentioning ' has conflicting definition values:"
            )
            .is_none()
        );
        assert!(module_failure("unrecognized future NixOS format").is_none());
    }

    #[test]
    fn external_definition_without_rust_boundary_still_renders_its_nix_file() {
        let mut d = Diagnostic::tooling("upstream conflict");
        d.kind = DiagnosticKind::NixosMerge;
        d.origins.push(DiagnosticOrigin {
            origin: None,
            role: OriginRole::ImportedBoundary,
            provenance: Provenance::Unavailable,
            nix_file: Some("upstream/module.nix".into()),
        });
        assert!(
            d.render(Path::new("."))
                .contains("Nix definition: upstream/module.nix")
        );
    }

    #[test]
    fn maps_text_context_without_matching_a_spoofed_throw_or_excerpt() {
        let origin = Origin::new("config.rs", 12, 9, "integer division");
        let generated = Generated {
            backend_metadata: None,
            source: "builtins.div 1 0".into(),
            spans: vec![SourceSpan {
                start: 0,
                end: 16,
                origin: origin.clone(),
                enclosing: vec![],
                diagnostic_site: false,
            }],
        };
        let msg = format!("error:\n … {}\n error: division by zero", origin.id);
        let raw = format!(
            "@nix {}",
            serde_json::json!({"action":"msg", "msg": msg, "level":0})
        );
        let diagnostic = Diagnostic::from_nix(
            DiagnosticKind::NixEval,
            &raw,
            &generated,
            Path::new("/tmp/generated.nix"),
        );
        assert_eq!(diagnostic.primary, Some(origin.clone()));
        assert_eq!(diagnostic.reason, "division by zero");
        for raw in [
            format!("error: {}", origin.id),
            format!("1| # {}", origin.id),
        ] {
            assert!(
                Diagnostic::from_nix(DiagnosticKind::NixEval, &raw, &generated, Path::new("x"))
                    .primary
                    .is_none()
            );
        }
    }

    #[test]
    fn unknown_protocol_degrades_without_losing_original() {
        let raw = "@nix {broken wire format}";
        let diagnostic = Diagnostic::from_nix(
            DiagnosticKind::NixEval,
            raw,
            &Generated::default(),
            Path::new("x"),
        );
        assert_eq!(diagnostic.provenance, Provenance::Unavailable);
        assert_eq!(diagnostic.raw_nix, raw);
    }
}
