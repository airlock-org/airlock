//! Share equal computations at the nearest scope containing all their uses.
//!
//! Variable identities refer to their actual binders, not their spelling. This
//! lets sibling functions reuse captured work without merging their arguments.
//! Attribution remains part of identity, including the complete authored path.

use super::{
    Names, NixExpr, NixKind, Origin, Shape, boundary, children, children_mut, implicit_operation,
};
use std::collections::HashMap;

/// One lexical binder, which may have the same spelling as an unrelated binder.
#[derive(Clone, Copy)]
struct Binding {
    /// Unique identity for this variable declaration.
    identity: usize,
    /// Minimum scope depth at which this declaration is available.
    depth: usize,
}

/// Nonrecursive expression identity with explicit lexical and diagnostic context.
#[derive(PartialEq, Eq, Hash)]
struct Signature<'a> {
    /// Operation authored in Rust, if any.
    origin: Option<&'a Origin>,
    /// Interned chain of enclosing authored operations.
    ancestry: usize,
    /// Whether evaluating this operation contributes a runtime diagnostic frame.
    error_context: bool,
    /// Literal syntax and operation, excluding descendants.
    shape: Shape<'a>,
    /// Declaration resolved by a variable reference; absent for global identifiers.
    binding: Option<usize>,
    /// Declaration of `builtins` used by an emitted diagnostic wrapper.
    namespace: Option<usize>,
    /// Declaration of the prelude name read by this operation's rendered syntax.
    operation: Option<usize>,
    /// Structurally interned descendants in source order.
    children: Vec<usize>,
}

/// An expression region where a lazy binding may be inserted without changing its inputs.
struct Scope {
    /// Immediately enclosing expression, absent at the root.
    parent: Option<usize>,
    /// Syntax depth used to find the smallest common enclosing expression.
    depth: usize,
    /// Number of enclosing lexical binders; ordinary syntax nodes do not increase it.
    binding_depth: usize,
    /// Authored ancestry already enclosing a definition placed at this expression.
    ancestry: usize,
    /// Original expression at which bindings for this region are installed.
    root: *const NixExpr,
    /// Whether wrapping this region would obscure a directly exported function.
    function_root: bool,
}

/// Uses of one exact expression, summarized before any syntax is rewritten.
#[derive(Clone)]
struct Uses {
    /// Approximate syntax size, excluding descendants inside binding boundaries.
    weight: usize,
    /// Deepest declaration referenced by this subtree; binding nodes remain unique.
    required_depth: usize,
    /// Number of original occurrences of this expression.
    count: usize,
    /// Nearest region containing every occurrence.
    scope: usize,
    /// Authored ancestry at the original uses, shared by all equal occurrences.
    ancestry: usize,
}

/// Expression identity and lexical region recorded while original addresses are stable.
#[derive(Clone, Copy)]
struct Occurrence {
    /// Memoization group, initially the expression's structural identity.
    identity: usize,
    /// Lexical region containing this original node.
    scope: usize,
}

/// Index expressions once, retaining lexical declarations and interned diagnostic paths.
#[derive(Default)]
struct Index<'a> {
    /// Complete keys resolve structural hash collisions without relying on hashes alone.
    signatures: HashMap<Signature<'a>, usize>,
    /// Equal syntax occurrences and their eventual binding region.
    uses: Vec<Uses>,
    /// Original node addresses used only until the rewrite consumes those nodes.
    occurrences: HashMap<*const NixExpr, Occurrence>,
    /// Nested expression regions, with lexical depth tracked separately.
    scopes: Vec<Scope>,
    /// Visible declarations, stacked to account for shadowing and recursive lets.
    bindings: HashMap<&'a str, Vec<Binding>>,
    /// Next unique declaration identifier.
    next_binding: usize,
    /// Next unique boundary identifier; function and let bodies are never merged as opaque syntax.
    next_boundary: usize,
    /// Interned authored ancestry, using zero for the empty chain.
    ancestry: HashMap<(usize, &'a Origin), usize>,
    /// Parent and origin for each nonempty diagnostic path.
    paths: Vec<(usize, &'a Origin)>,
}

impl<'a> Index<'a> {
    /// Add a region without changing the environment of its original expression.
    fn scope(
        &mut self,
        expr: &NixExpr,
        parent: Option<usize>,
        binding_depth: usize,
        ancestry: usize,
    ) -> usize {
        let identity = self.scopes.len();
        self.scopes.push(Scope {
            parent,
            depth: parent.map_or(0, |parent| self.scopes[parent].depth + 1),
            binding_depth,
            ancestry,
            root: expr,
            function_root: function_root(expr),
        });
        identity
    }

    /// Intern one ancestry extension instead of cloning full paths at every node.
    fn path(&mut self, parent: usize, origin: &'a Origin) -> usize {
        let next = self.paths.len() + 1;
        *self.ancestry.entry((parent, origin)).or_insert_with(|| {
            self.paths.push((parent, origin));
            next
        })
    }

    /// Find the smallest common expression, keeping bindings close to their actual uses.
    fn common_scope(&self, mut left: usize, mut right: usize) -> usize {
        while left != right {
            if self.scopes[left].depth >= self.scopes[right].depth {
                left = self.scopes[left].parent.expect("non-root deeper region");
            } else {
                right = self.scopes[right].parent.expect("non-root deeper region");
            }
        }
        left
    }

    /// Check whether one binding can serve this group without changing scope or function shape.
    fn allowed(&self, uses: &Uses) -> bool {
        let scope = &self.scopes[uses.scope];
        uses.required_depth <= scope.binding_depth && !scope.function_root
    }

    /// Retain sharing inside each child when a binder prevents sharing the complete group.
    fn partition(&mut self) {
        loop {
            let blocked: Vec<_> = self
                .uses
                .iter()
                .map(|uses| uses.count > 1 && uses.weight >= 8 && !self.allowed(uses))
                .collect();
            if !blocked.iter().any(|blocked| *blocked) {
                return;
            }

            let mut groups = HashMap::new();
            // Syntax order makes grouping deterministic. The signature index
            // remains structural; only memoization identities are partitioned.
            for node in 0..self.scopes.len() {
                let address = self.scopes[node].root;
                let occurrence = self.occurrences[&address];
                if !blocked[occurrence.identity] {
                    continue;
                }
                let original = occurrence.identity;
                let split = self.uses[original].scope;
                let mut branch = node;
                while self.scopes[branch].parent != Some(split) {
                    branch = self.scopes[branch]
                        .parent
                        .expect("blocked placement encloses its uses");
                }
                let identity = *groups.entry((original, branch)).or_insert_with(|| {
                    let identity = self.uses.len();
                    let mut uses = self.uses[original].clone();
                    uses.scope = node;
                    uses.count = 0;
                    self.uses.push(uses);
                    identity
                });
                let common = self.common_scope(self.uses[identity].scope, node);
                self.uses[identity].scope = common;
                self.uses[identity].count += 1;
                self.occurrences
                    .get_mut(&address)
                    .expect("indexed node")
                    .identity = identity;
            }
            for (identity, blocked) in blocked.into_iter().enumerate() {
                if blocked {
                    self.uses[identity].count = 0;
                }
            }
        }
    }

    /// Index descendants with their resolved variables, including native argument defaults.
    fn visit(&mut self, expr: &'a NixExpr, scope: usize, ancestry: usize) -> usize {
        let descendant_path = expr
            .origin
            .as_ref()
            .map_or(ancestry, |origin| self.path(ancestry, origin));
        let declarations: Vec<_> = match &expr.kind {
            NixKind::Lambda(name, _) | NixKind::Let(name, ..) => vec![name.as_str()],
            NixKind::Function(names, _) => names.iter().map(String::as_str).collect(),
            NixKind::ArgumentFunction(arguments, _)
            | NixKind::OpenArgumentFunction(arguments, _) => {
                arguments.iter().map(|(name, _)| name.as_str()).collect()
            }
            _ => Vec::new(),
        };
        for name in &declarations {
            let binding = Binding {
                identity: self.next_binding,
                depth: self.scopes[scope].binding_depth + 1,
            };
            self.next_binding += 1;
            self.bindings.entry(name).or_default().push(binding);
        }

        let mut descendants = Vec::new();
        for child in children(expr) {
            let binding_depth = self.scopes[scope].binding_depth + usize::from(boundary(expr));
            let child_scope = self.scope(child, Some(scope), binding_depth, descendant_path);
            descendants.push(self.visit(child, child_scope, descendant_path));
        }
        for name in &declarations {
            self.bindings
                .get_mut(name)
                .expect("entered declaration")
                .pop();
        }

        let binding = match &expr.kind {
            NixKind::Variable(name) => self
                .bindings
                .get(name.as_str())
                .and_then(|stack| stack.last())
                .copied(),
            _ => None,
        };
        let namespace = if expr.error_context && expr.origin.is_some() {
            self.bindings
                .get("builtins")
                .and_then(|stack| stack.last())
                .copied()
        } else {
            None
        };
        let operation = implicit_operation(expr).and_then(|name| {
            self.bindings
                .get(name)
                .and_then(|stack| stack.last())
                .copied()
        });
        // A shareable subtree cannot contain a binder: those nodes have unique
        // identities. Its referenced declarations lie on one enclosing chain.
        // Resolved identities keep that chain equal across uses, so its deepest
        // member determines placement without a dependency set at every node.
        let required_depth = descendants.iter().fold(
            binding
                .map_or(0, |binding| binding.depth)
                .max(namespace.map_or(0, |binding| binding.depth))
                .max(operation.map_or(0, |binding| binding.depth)),
            |depth, child| depth.max(self.uses[*child].required_depth),
        );
        let weight = if boundary(expr) {
            1
        } else {
            descendants.iter().fold(1_usize, |weight, child| {
                weight.saturating_add(self.uses[*child].weight)
            })
        };
        let shape = match &expr.kind {
            NixKind::Bool(value) => Shape::Bool(*value),
            NixKind::Int(value) => Shape::Int(*value),
            NixKind::Float(value) => Shape::Float(value.to_bits()),
            NixKind::Null => Shape::Null,
            NixKind::String(value) => Shape::String(value),
            NixKind::Path(value) => Shape::Path(value),
            NixKind::List(_) => Shape::List,
            NixKind::Group(_) => Shape::Group,
            NixKind::AttrSet(fields) => {
                Shape::AttrSet(fields.iter().map(|(path, _)| path.as_slice()).collect())
            }
            NixKind::Call(builtin, _) => Shape::Call(*builtin),
            NixKind::Apply(..) => Shape::Apply,
            NixKind::Binary(operator, ..) => Shape::Binary(*operator),
            NixKind::If(..) => Shape::If,
            NixKind::Assert(..) => Shape::Assert,
            NixKind::Variable(name) => Shape::Variable(name),
            NixKind::Select(_, path) => Shape::Select(path),
            NixKind::ArgumentSelect(_, path) => Shape::ArgumentSelect(path),
            _ => {
                let identity = self.next_boundary;
                self.next_boundary += 1;
                Shape::Boundary(identity)
            }
        };
        let literal_weight = match &shape {
            Shape::String(value) | Shape::Path(value) => value.len() / 64,
            _ => 0,
        };
        let key = Signature {
            origin: expr.origin.as_ref(),
            ancestry,
            error_context: expr.error_context,
            shape,
            binding: binding.map(|binding| binding.identity),
            namespace: namespace.map(|binding| binding.identity),
            operation: operation.map(|binding| binding.identity),
            children: if boundary(expr) {
                Vec::new()
            } else {
                descendants
            },
        };
        let next = self.uses.len();
        let identity = *self.signatures.entry(key).or_insert_with(|| {
            self.uses.push(Uses {
                weight: weight.saturating_add(literal_weight),
                required_depth,
                count: 0,
                scope,
                ancestry,
            });
            next
        });
        if self.uses[identity].weight >= 8 {
            let common = self.common_scope(self.uses[identity].scope, scope);
            self.uses[identity].scope = common;
        }
        self.uses[identity].count += 1;
        self.occurrences
            .insert(expr, Occurrence { identity, scope });
        identity
    }
}

/// Some consumers inspect function syntax without forcing an enclosing lazy binding.
fn function_root(expr: &NixExpr) -> bool {
    match &expr.kind {
        NixKind::Group(value) => function_root(value),
        NixKind::Lambda(..)
        | NixKind::Function(..)
        | NixKind::ArgumentFunction(..)
        | NixKind::OpenArgumentFunction(..) => true,
        _ => false,
    }
}

/// Count references that survive collapsing repeated parents, including across lexical regions.
fn effective_counts(
    expr: &NixExpr,
    occurrences: &HashMap<*const NixExpr, Occurrence>,
    selected: &[bool],
    counts: &mut [usize],
) {
    let identity = occurrences[&(expr as *const NixExpr)].identity;
    counts[identity] += 1;
    if selected[identity] && counts[identity] > 1 {
        return;
    }
    for child in children(expr) {
        effective_counts(child, occurrences, selected, counts);
    }
}

/// Rewrite once after dependency and placement decisions, consuming only original node addresses.
struct Rewrite {
    /// Exact identities and regions of original syntax nodes.
    occurrences: HashMap<*const NixExpr, Occurrence>,
    /// Chosen region for each profitable expression, absent when not shared.
    targets: Vec<Option<usize>>,
    /// Authored ancestry to preserve around each moved definition.
    ancestry: Vec<Vec<Origin>>,
    /// Name of each expression already emitted into a lazy binding.
    shared: HashMap<usize, String>,
    /// Definitions collected for each lexical region in dependency order.
    definitions: Vec<Vec<(String, NixExpr)>>,
    /// Original root address for each lexical region.
    roots: Vec<*const NixExpr>,
}

impl Rewrite {
    /// Replace repeated computations and install bindings only after visiting the entire region.
    fn visit(&mut self, expr: &mut NixExpr, names: &mut Names) {
        let address = expr as *const NixExpr;
        let occurrence = self.occurrences[&address];
        if let Some(target) = self.targets[occurrence.identity] {
            if let Some(name) = self.shared.get(&occurrence.identity) {
                expr.kind = NixKind::Variable(name.clone());
                expr.error_context = false;
            } else {
                for child in children_mut(expr) {
                    self.visit(child, names);
                }
                let name = names.fresh();
                let reference = NixExpr {
                    origin: expr.origin.clone(),
                    error_context: false,
                    kind: NixKind::Variable(name.clone()),
                };
                let mut definition = std::mem::replace(expr, reference);
                for origin in self.ancestry[occurrence.identity].iter().rev() {
                    definition =
                        NixExpr::attributed(NixKind::Group(Box::new(definition)), origin.clone());
                }
                self.definitions[target].push((name.clone(), definition));
                self.shared.insert(occurrence.identity, name);
            }
        } else {
            for child in children_mut(expr) {
                self.visit(child, names);
            }
        }

        if self.roots[occurrence.scope] == address {
            for (name, value) in std::mem::take(&mut self.definitions[occurrence.scope])
                .into_iter()
                .rev()
            {
                let body = std::mem::replace(expr, NixExpr::plain(NixKind::Null));
                *expr = NixExpr::plain(NixKind::Let(name, Box::new(value), Box::new(body)));
            }
        }
    }
}

/// Share substantial repeated expressions without moving dependencies out of their declaring scopes.
pub(super) fn share(expr: &mut NixExpr, names: &mut Names) {
    let mut index = Index::default();
    let root = index.scope(expr, None, 0, 0);
    index.visit(expr, root, 0);
    index.partition();
    let mut selected: Vec<_> = index
        .uses
        .iter()
        .map(|uses| uses.count > 1 && uses.weight >= 8 && index.allowed(uses))
        .collect();
    let mut counts = vec![0; selected.len()];
    effective_counts(expr, &index.occurrences, &selected, &mut counts);
    for (selected, count) in selected.iter_mut().zip(counts) {
        *selected &= count > 1;
    }

    let ancestry = index
        .uses
        .iter()
        .enumerate()
        .map(|(identity, uses)| {
            let mut result = Vec::new();
            if selected[identity] {
                let mut path = uses.ancestry;
                let enclosing = index.scopes[uses.scope].ancestry;
                while path != enclosing {
                    assert_ne!(
                        path, 0,
                        "placement must retain a prefix of the use ancestry"
                    );
                    let (parent, origin) = index.paths[path - 1];
                    result.push(origin.clone());
                    path = parent;
                }
                result.reverse();
            }
            result
        })
        .collect();
    let targets = index
        .uses
        .iter()
        .zip(selected)
        .map(|(uses, selected)| selected.then_some(uses.scope))
        .collect();
    drop(index.signatures);
    drop(index.ancestry);
    drop(index.paths);
    let mut rewrite = Rewrite {
        occurrences: index.occurrences,
        targets,
        ancestry,
        shared: HashMap::new(),
        definitions: (0..index.scopes.len()).map(|_| Vec::new()).collect(),
        roots: index.scopes.into_iter().map(|scope| scope.root).collect(),
    };
    rewrite.visit(expr, names);
}
