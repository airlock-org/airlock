//! Describe the Nix syntax emitted by the backend.
//!
//! An abstract syntax tree (AST) represents source code as expressions instead of
//! strings. This advanced API lets backend authors build and inspect that tree.
//! Normal configuration authors use `rusix`; they do not construct Nix syntax.
//! These constructors do not validate arbitrary trees. Invalid generated syntax
//! is a compiler failure when evaluated, not an error in user configuration.
use crate::ir::Origin;

/// One Nix source expression, optionally linked to a Rust operation.
/// Backend authors combine these expressions and render them into Nix text.
/// Attribution supplies source-map entries; optional runtime markers help map
/// failures whose Nix locations point into external code.
#[derive(Clone, Debug)]
pub struct NixExpr {
    /// Rust operation linked to this expression's generated text range, when available.
    /// Optional inspection comments also use this location.
    pub origin: Option<Origin>,
    /// Whether generated Nix attaches the Rust location to this operation's error trace.
    /// Used for fallible operations and calls into external Nix code.
    pub error_context: bool,
    /// Syntax to render; attribution does not alter its semantic operation.
    pub kind: NixKind,
}

/// The syntax of a Nix expression, such as a literal, record or function call.
/// An attribute set is a group of named fields. Nix evaluates expressions only
/// when their results are needed, so building this tree does not evaluate values.
/// Child order determines generated source order.
#[derive(Clone, Debug)]
pub enum NixKind {
    /// A boolean literal.
    Bool(bool),
    /// A signed integer literal, including the minimum 64-bit value.
    Int(i64),
    /// A floating literal; finite representable values should be validated in IR.
    Float(f64),
    /// The Nix null literal.
    Null,
    /// Text rendered as an escaped Nix string, never as source syntax.
    String(String),
    /// Compiler-owned relative paths; never supplied by the frontend as syntax.
    Path(String),
    /// Ordered expressions in a lazy Nix list.
    List(Vec<NixExpr>),
    /// Explicit parentheses around a child expression.
    Group(Box<NixExpr>),
    /// Attribute bindings with literal path segments, escaped during rendering.
    AttrSet(Vec<(Vec<String>, NixExpr)>),
    /// A supported builtin with curried arguments in application order.
    Call(Builtin, Vec<NixExpr>),
    /// General function application to one argument.
    Apply(Box<NixExpr>, Box<NixExpr>),
    /// A binary operator applied to left and right operands.
    Binary(BinaryOp, Box<NixExpr>, Box<NixExpr>),
    /// Condition, then branch and else branch, with ordinary Nix laziness.
    If(Box<NixExpr>, Box<NixExpr>, Box<NixExpr>),
    /// Native expression assertion, leaving the result unused when the condition is false.
    Assert(Box<NixExpr>, Box<NixExpr>),
    /// One local binding name, its value and the body using it.
    Let(String, Box<NixExpr>, Box<NixExpr>),
    /// A compiler-selected identifier; the renderer does not escape or validate it.
    Variable(String),
    /// Select literal attribute segments from a deferred value.
    Select(Box<NixExpr>, Vec<String>),
    /// Lexical argument access; safe path names use identifiers, others are quoted.
    /// An empty path preserves the attributed lexical root without selecting a field.
    ArgumentSelect(Box<NixExpr>, Vec<String>),
    /// Attribute-pattern function with the compiler-owned argument names.
    Function(Vec<String>, Box<NixExpr>),
    /// One compiler-selected parameter name and its deferred function body.
    Lambda(String, Box<NixExpr>),
    /// Native named arguments, including lazy default expressions; extra names are rejected.
    ArgumentFunction(Vec<(String, Option<NixExpr>)>, Box<NixExpr>),

    /// Named arguments with lazy defaults and `...` accepting unlisted caller fields.
    OpenArgumentFunction(Vec<(String, Option<NixExpr>)>, Box<NixExpr>),
}

/// Functions provided by the Nix language itself that this backend can emit.
/// For example, `builtins.div` performs integer division. These are distinct
/// from utility functions in nixpkgs’ separately supplied `lib` attribute set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Builtin {
    /// Signed integer division via `builtins.div`.
    Div,
    /// Evaluator failure with a supplied message via `builtins.throw`.
    Throw,
    /// Import an existing Nix expression file.
    Import,
    /// Select an attribute by a string key, including literal punctuation.
    GetAttr,
    /// Canonicalize absolute path text; Nix returns a string, not a path value.
    ToPath,
    /// Coerce to a string while retaining Nix dependency context.
    ToString,
}

/// Two-operand Nix operators represented by this backend.
/// They operate when Nix evaluates the generated source, not when Rust builds
/// the expression tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    /// Native value equality (`==`).
    Equal,
    /// Inclusive lower-bound comparison (`>=`).
    GreaterEqual,
    /// Inclusive upper-bound comparison (`<=`).
    LessEqual,
    /// Short-circuit boolean conjunction (`&&`).
    And,
    /// Native addition, also used for string/path concatenation (`+`).
    Add,
    /// Shallow attribute-set union (`//`), with right-hand fields taking precedence.
    AttrMerge,
}

impl NixExpr {
    /// Construct backend syntax without Rust source attribution or runtime context.
    pub fn plain(kind: NixKind) -> Self {
        Self {
            origin: None,
            error_context: false,
            kind,
        }
    }

    /// Attach source-map attribution without adding evaluation or forcing.
    pub fn attributed(kind: NixKind, origin: Origin) -> Self {
        Self {
            origin: Some(origin),
            error_context: false,
            kind,
        }
    }

    /// Attach attribution and `builtins.addErrorContext` around this operation.
    /// Children remain lazy; this does not force containers to retain context.
    pub fn contextual(kind: NixKind, origin: Origin) -> Self {
        Self {
            origin: Some(origin),
            error_context: true,
            kind,
        }
    }
}
