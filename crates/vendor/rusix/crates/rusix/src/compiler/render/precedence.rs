//! Binding strength and grammar contexts for the backend's existing Nix syntax.
use crate::compiler::ast::{BinaryOp, NixExpr, NixKind};

/// Increasing strength follows Nix, including right-associative set updates.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Precedence {
    Expression,
    And,
    Equality,
    Comparison,
    Update,
    Add,
    Negate,
    Application,
    Selection,
    Atom,
}

/// Nix applications and lists accept selections as arguments, not full expressions.
#[derive(Clone, Copy)]
pub(super) enum Context {
    Expression,
    ApplicationFunction,
    Simple,
    SelectionBase,
    Operand {
        /// Strength of the enclosing binary operator.
        precedence: Precedence,
        /// Whether this side agrees with the operator's associativity.
        equal_allowed: bool,
    },
}

impl Context {
    /// Equal-strength operands group on the side opposite the parser's association.
    pub(super) fn requires_parentheses(self, child: Precedence) -> bool {
        match self {
            Self::Expression => false,
            Self::ApplicationFunction => child < Precedence::Application,
            Self::Simple | Self::SelectionBase => child < Precedence::Selection,
            Self::Operand {
                precedence,
                equal_allowed,
            } => child < precedence || (child == precedence && !equal_allowed),
        }
    }
}

/// Runtime wrappers are applications even when their child is an atomic value.
pub(super) fn precedence(expr: &NixExpr) -> Precedence {
    if expr.error_context && expr.origin.is_some() {
        Precedence::Application
    } else {
        kind(&expr.kind)
    }
}

/// A following dot belongs to an ungrouped path token, including transparent roots.
pub(super) fn path_base(expr: &NixExpr) -> bool {
    if expr.error_context && expr.origin.is_some() {
        return false;
    }

    match &expr.kind {
        NixKind::Path(_) => true,
        NixKind::Select(value, path) | NixKind::ArgumentSelect(value, path) if path.is_empty() => {
            path_base(value)
        }
        _ => false,
    }
}

/// Separate attributed lookup steps need distinct evaluator failure positions.
/// Nix otherwise folds `a.b.c` into one selection and reports its beginning for
/// either missing segment. A single attributed multi-segment path stays direct.
pub(super) fn selection_boundary(expr: &NixExpr) -> bool {
    expr.origin.is_some()
        && !expr.error_context
        && matches!(&expr.kind, NixKind::Select(_, path) | NixKind::ArgumentSelect(_, path) if !path.is_empty())
}

/// Explicit AST groups remain atomic; no tree rewriting or reassociation occurs.
pub(super) fn kind(kind: &NixKind) -> Precedence {
    match kind {
        NixKind::If(..)
        | NixKind::Assert(..)
        | NixKind::Let(..)
        | NixKind::Lambda(..)
        | NixKind::Function(..)
        | NixKind::ArgumentFunction(..)
        | NixKind::OpenArgumentFunction(..) => Precedence::Expression,
        NixKind::Binary(op, ..) => operator_rule(*op).1,
        NixKind::Apply(..) => Precedence::Application,
        NixKind::Call(_, args) if !args.is_empty() => Precedence::Application,
        NixKind::Call(..) => Precedence::Selection,
        NixKind::Select(value, path) | NixKind::ArgumentSelect(value, path) => {
            if path.is_empty() {
                precedence(value)
            } else {
                Precedence::Selection
            }
        }
        NixKind::Int(value) if *value < 0 && *value != i64::MIN => Precedence::Negate,
        NixKind::Float(value) if value.is_sign_negative() => Precedence::Negate,
        _ => Precedence::Atom,
    }
}

/// One operator table supplies binding strength, token and permitted association.
fn operator_rule(op: BinaryOp) -> (&'static str, Precedence, bool, bool) {
    match op {
        BinaryOp::And => ("&&", Precedence::And, true, false),
        BinaryOp::Equal => ("==", Precedence::Equality, false, false),
        BinaryOp::GreaterEqual => (">=", Precedence::Comparison, false, false),
        BinaryOp::LessEqual => ("<=", Precedence::Comparison, false, false),
        BinaryOp::AttrMerge => ("//", Precedence::Update, false, true),
        BinaryOp::Add => ("+", Precedence::Add, true, false),
    }
}

/// Both operands and node classification use the same operator rule.
pub(super) fn binary(op: BinaryOp) -> (&'static str, Context, Context) {
    let (token, precedence, left_equal, right_equal) = operator_rule(op);
    let operand = |equal_allowed| Context::Operand {
        precedence,
        equal_allowed,
    };

    (token, operand(left_equal), operand(right_equal))
}
