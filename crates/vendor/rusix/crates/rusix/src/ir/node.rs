//! Deferred values and configuration assignments in the semantic representation.
use super::{Origin, Reference};
use crate::ValidationError;
use crate::interop::raw::AttrPath;

/// Lower a supported literal or symbolic value into the semantic IR.
/// Compiler implementations supply an origin for concrete leaves; symbolic
/// expressions retain their previously captured origins.
pub trait IntoNode {
    /// Consume the authoring value without evaluating its generated Nix expression.
    fn into_node(self, origin: Origin) -> Node;
}

/// Inspect the lookup metadata of an external reference handle.
/// This is compiler/provenance information, not the referenced Nix value.
pub trait ReferencedExpression {
    /// Obtain the source, literal attribute path and original Rust location.
    fn reference(&self) -> &Reference;
}

/// A description of one value or operation, with its Rust source location.
/// Backend authors use these nodes to generate Nix; the collection of nodes is
/// Rusix’s *intermediate representation* (IR). It describes computation without
/// performing it. Normal authoring uses [`crate::Expr`], [`crate::IntoRusixValue`] or
/// [`crate::interop::raw::NixValue`] instead of constructing nodes.
#[derive(Clone, Debug)]
pub struct Node {
    /// Rust operation that introduced this expression.
    pub origin: Origin,
    /// Deferred operation or literal; evaluating it is the backend's responsibility.
    pub kind: ValueKind,
}

impl Node {
    /// Check expression invariants without evaluating Nix or checking its actual types.
    /// Rejects invalid strings, paths, literals and escaped callback parameters.
    pub fn validate(&self) -> Result<(), ValidationError> {
        super::validation::validate_scoped(self, &[])
    }
}

/// The value or operation described by a [`Node`].
/// Variants distinguish literals, containers and computations Nix will perform
/// later. This is an interface for backend implementation and inspection, not
/// a way for Rust authors to obtain evaluated Nix values.
#[derive(Clone, Debug)]
pub enum ValueKind {
    /// A concrete Rust boolean embedded as a literal.
    Bool(bool),
    /// A signed integer literal using Nix's 64-bit integer range.
    Int(i64),
    /// A floating literal; validation accepts finite normal values and zero.
    Float(f64),
    /// Nix's explicit null value.
    Null,
    /// Literal text, escaped by the backend rather than interpreted as Nix source.
    String(String),
    /// Ordered deferred elements; constructing the list does not demand its children.
    List(Vec<Node>),
    /// A structural record whose fields can become configuration paths.
    AttrSet(Vec<(String, Node)>),
    /// An interop record stays one value; structural authoring must not flatten
    /// its literal keys into NixOS option paths.
    OpaqueRecord(Vec<(String, Node)>),
    /// An opaque ecosystem lookup; existence and internal type are checked by Nix.
    Reference(Reference),
    /// One deferred application of a function to its argument.
    Apply(Box<Node>, Box<Node>),
    /// Native shallow attribute-set union; right-hand fields replace left-hand fields.
    AttrMerge(Box<Node>, Box<Node>),
    /// Native expression assertion; only a true condition permits demanding the result.
    Assert(Box<Node>, Box<Node>),
    /// A deferred lookup through literal attribute segments.
    Select(Box<Node>, AttrPath),
    /// Scoped callbacks at the opaque Nix boundary, not Rust-side evaluation.
    Function {
        /// Lexical identity shared with parameter references in this callback.
        binding: u64,
        /// Deferred callback result, possibly referring to the symbolic parameter.
        body: Box<Node>,
    },
    /// A finite Nix argument-set callback, retaining native defaults and callPackage introspection.
    FunctionAttrs {
        /// Lexical identity of the named argument scope, including resolved defaults.
        binding: u64,
        /// Accepted Nix argument names; names without defaults are required.
        arguments: Vec<String>,
        /// Accept additional caller arguments without exposing them in the finite view.
        open: bool,
        /// Deferred defaults, which can refer to other resolved arguments.
        defaults: Vec<(String, Node)>,
        /// Deferred result, potentially a derivation from an existing builder.
        body: Box<Node>,
    },
    /// A symbolic callback parameter; validation rejects uses outside its scope.
    Parameter(u64),
    /// A deferred condition, then branch and else branch; only one branch is demanded.
    If(Box<Node>, Box<Node>, Box<Node>),
    /// Equality checked by the backend using its native value semantics.
    Equal(Box<Node>, Box<Node>),
    /// A NixOS-scoped dependency, never a concrete Rust value.
    OptionReference(AttrPath),
    /// Deferred text coercion that retains Nix string dependency context.
    ToText(Box<Node>),
    /// Concrete text followed by a deferred string value.
    StringPrefix {
        /// Literal prefix, escaped as data during code generation.
        prefix: String,
        /// String expression whose dependency context survives concatenation.
        value: Box<Node>,
    },
    /// Convert absolute text into a filesystem path without snapshotting its contents.
    FilesystemPath(
        /// Absolute path text checked by Nix when demanded.
        Box<Node>,
    ),
    /// Append a validated relative suffix to a deferred Nix filesystem path.
    PathAppend {
        /// Path supplied by the caller, kept as a path rather than coerced text.
        value: Box<Node>,
        /// Slash-prefixed, literal relative suffix; the backend escapes it as text.
        suffix: String,
    },
    /// Signed integer division, including evaluator-side division-by-zero failures.
    Divide(Box<Node>, Box<Node>),
    /// An inclusive range constraint checked when the expression is evaluated.
    InRange {
        /// Deferred integer to check.
        value: Box<Node>,
        /// Inclusive lower bound.
        min: i64,
        /// Inclusive upper bound.
        max: i64,
        /// Failure reason presented if the value falls outside the bounds.
        message: String,
    },
}

/// A setting with its destination path, value expression and Rust location.
/// This is inspection data within [`crate::Config`]. The backend turns it into a Nix
/// field assignment; its value is evaluated later by Nix.
#[derive(Clone, Debug)]
pub struct Assignment {
    /// Rust operation that introduced this definition, distinct from child expression origins.
    pub origin: Origin,
    /// Human-readable path; use [`Self::path_segments`] for unambiguous code generation.
    pub path: String,
    /// Deferred right-hand side of the definition.
    pub value: Node,
    /// Literal destination field names; a dot within one name is preserved as data.
    pub(crate) segments: Vec<String>,
}

impl Assignment {
    /// Attribute segments are data; a renamed field may contain a literal dot.
    pub fn path_segments(&self) -> &[String] {
        &self.segments
    }
}
