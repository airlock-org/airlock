//! Finite dependency graphs used by the production machine planner.
//! Vertices are canonical VM indices; reference interpretation stays with machine topology.
//! A dependency points from a VM to its provider. Closure is specified independently
//! as the least closed superset of the roots; ordering requires providers before
//! dependents in a full permutation. Neither specification mentions a worklist or waves.
//!
//! Startup composes closure and ordering inside this verified implementation.
//! Shutdown selects exactly the affected/running intersection in reverse order;
//! omitted endpoints can already be halted or preserved by GUI policy. Reference
//! decoding belongs to the machine adapter; root selection and GUI eligibility
//! are checked by the deployment-planning module.
//! These read-only decisions grant no execution authority.

use vstd::prelude::*;

verus! {

broadcast use vstd::seq_lib::group_seq_properties;

/// A VM-to-provider dependency, with both endpoints in the graph's declared index space.
#[derive(Clone, Copy)]
pub struct Edge {
    /// VM requiring another vertex's service.
    pub dependent: usize,
    /// Provider required by the dependent.
    pub dependency: usize,
}

/// Mathematical graph projected from the immutable executable representation.
pub ghost struct GraphState {
    /// Number of vertices, whose identities are exactly the indices below this bound.
    pub count: usize,
    /// Dependency relation; duplicate edges and edge enumeration order have no meaning.
    pub edges: Seq<Edge>,
}

/// Every edge refers to two vertices of this graph.
pub open spec fn valid(g: GraphState) -> bool {
    forall|k: int| 0 <= k < g.edges.len() ==> #[trigger] valid_edge(g.edges[k], g.count)
}

/// Bounds required before indexing either endpoint.
pub open spec fn valid_edge(edge: Edge, count: usize) -> bool {
    edge.dependent < count && edge.dependency < count
}

/// Immutable graph whose constructor checks all endpoint bounds.
/// Vertices are indices in the machine adapter's canonical VM-name table.
///
/// ```
/// use airlock_machine_planning::graph::{Edge, Graph};
/// let graph = Graph::new(2, vec![Edge { dependent: 1, dependency: 0 }]).unwrap();
/// assert_eq!(graph.startup(vec![false, true]).unwrap(), vec![0, 1]);
/// ```
///
/// ```compile_fail
/// use airlock_machine_planning::graph::Graph;
/// // Raw fields cannot bypass the checked constructor and its graph invariant.
/// let graph = Graph { count: 1, edges: vec![] };
/// ```
pub struct Graph {
    // Vertex count fixes the interpretation of every mask and edge.
    count: usize,
    // Caller-supplied dependencies, without traversal or authority metadata.
    edges: Vec<Edge>,
}

impl View for Graph {
    type V = GraphState;

    closed spec fn view(&self) -> GraphState {
        GraphState { count: self.count, edges: self.edges@ }
    }
}

/// Vertex set represented by an executable selection mask.
pub open spec fn selected(mask: Seq<bool>) -> ISet<usize> {
    ISet::new(|v: usize| v < mask.len() && mask[v as int])
}

/// Set of vertices in an executable operation sequence.
pub open spec fn listed(order: Seq<usize>) -> ISet<usize> {
    ISet::new(|v: usize| order.contains(v))
}

/// Origin of a traversal edge, reversed when finding affected dependents.
pub open spec fn source(edge: Edge, reverse: bool) -> usize {
    if reverse { edge.dependency } else { edge.dependent }
}

/// Destination of a traversal edge.
pub open spec fn destination(edge: Edge, reverse: bool) -> usize {
    if reverse { edge.dependent } else { edge.dependency }
}

/// A set contains every required successor of each of its members.
pub open spec fn closed(g: GraphState, members: ISet<usize>, reverse: bool) -> bool {
    forall|k: int| 0 <= k < g.edges.len() && members.contains(source(g.edges[k], reverse))
        ==> members.contains(#[trigger] destination(g.edges[k], reverse))
}

/// No vertex is justified unless every closed superset of the seeds requires it.
pub open spec fn minimal(g: GraphState, seeds: ISet<usize>, members: ISet<usize>, reverse: bool) -> bool {
    forall|other: ISet<usize>| seeds.subset_of(other) && closed(g, other, reverse)
        ==> #[trigger] members.subset_of(other)
}

/// Independent least-closed-superset specification: completeness and absence of extra effects.
pub open spec fn closure(g: GraphState, seeds: ISet<usize>, members: ISet<usize>, reverse: bool) -> bool {
    seeds.subset_of(members) && closed(g, members, reverse) && minimal(g, seeds, members, reverse)
}

/// Mathematical dependency relation, independent of edge enumeration and duplicates.
pub open spec fn dependency(g: GraphState, dependent: usize, provider: usize) -> bool {
    exists|e: int| 0 <= e < g.edges.len() && #[trigger] g.edges[e].dependent == dependent
        && g.edges[e].dependency == provider
}

/// Graphs with the same vertex identities and dependency relation are equivalent.
pub open spec fn equivalent(left: GraphState, right: GraphState) -> bool {
    left.count == right.count && forall|d: usize, p: usize|
        #[trigger] dependency(left, d, p) == dependency(right, d, p)
}

/// Closure depends on the relation, not the representation's collection order.
proof fn closed_transfer(left: GraphState, right: GraphState, members: ISet<usize>, reverse: bool)
    requires equivalent(left, right), closed(left, members, reverse),
    ensures closed(right, members, reverse),
{
    assert forall|e: int| 0 <= e < right.edges.len() && members.contains(source(right.edges[e], reverse))
        implies members.contains(#[trigger] destination(right.edges[e], reverse)) by {
        assert(dependency(right, right.edges[e].dependent, right.edges[e].dependency));
        assert(dependency(left, right.edges[e].dependent, right.edges[e].dependency));
        let k = choose|k: int| 0 <= k < left.edges.len()
            && #[trigger] left.edges[k].dependent == right.edges[e].dependent
            && left.edges[k].dependency == right.edges[e].dependency;
        assert(members.contains(source(left.edges[k], reverse)));
        assert(members.contains(destination(left.edges[k], reverse)));
    }
}

/// Equivalent dependency relations produce exactly the same least-closure mask.
pub proof fn closure_deterministic(left: GraphState, right: GraphState, seeds: ISet<usize>,
    first: Seq<bool>, second: Seq<bool>, reverse: bool)
    requires
        equivalent(left, right), first.len() == left.count, second.len() == right.count,
        closure(left, seeds, selected(first), reverse), closure(right, seeds, selected(second), reverse),
    ensures first == second,
{
    closed_transfer(left, right, selected(first), reverse);
    closed_transfer(right, left, selected(second), reverse);
    assert(selected(first).subset_of(selected(second)));
    assert(selected(second).subset_of(selected(first)));
    assert(selected(first) =~= selected(second));
    assert forall|i: int| 0 <= i < first.len() implies #[trigger] first[i] == second[i] by {
        assert(selected(first).contains(i as usize) == first[i]);
        assert(selected(second).contains(i as usize) == second[i]);
    }
    assert(first =~= second);
}

/// Every successor of one processed vertex is already selected.
spec fn settled(g: GraphState, mask: Seq<bool>, vertex: usize, reverse: bool) -> bool {
    forall|k: int| 0 <= k < g.edges.len() && source(g.edges[k], reverse) == vertex
        ==> selected(mask).contains(#[trigger] destination(g.edges[k], reverse))
}

/// Readiness against the dependency edges examined so far in one complete wave.
spec fn settled_through(g: GraphState, mask: Seq<bool>, vertex: usize, limit: int) -> bool {
    forall|e: int| 0 <= e < limit && g.edges[e].dependent == vertex
        ==> selected(mask).contains(#[trigger] g.edges[e].dependency)
}

/// One more edge contributes exactly its provider requirement to readiness.
proof fn through_step(g: GraphState, mask: Seq<bool>, vertex: usize, e: int)
    requires 0 <= e < g.edges.len(),
    ensures settled_through(g, mask, vertex, e + 1) == (settled_through(g, mask, vertex, e)
        && (g.edges[e].dependent != vertex || selected(mask).contains(g.edges[e].dependency))),
{
    assert(settled_through(g, mask, vertex, e + 1) ==> settled_through(g, mask, vertex, e));
    if settled_through(g, mask, vertex, e)
        && (g.edges[e].dependent != vertex || selected(mask).contains(g.edges[e].dependency)) {
        assert forall|k: int| 0 <= k < e + 1 && g.edges[k].dependent == vertex
            implies selected(mask).contains(#[trigger] g.edges[k].dependency) by {
            if k < e { assert(selected(mask).contains(g.edges[k].dependency)); }
        }
    }
}

/// Examining the entire edge relation is the mathematical readiness predicate.
proof fn through_complete(g: GraphState, mask: Seq<bool>, vertex: usize)
    ensures settled_through(g, mask, vertex, g.edges.len() as int) == settled(g, mask, vertex, false),
{
    assert forall|k: int| 0 <= k < g.edges.len()
        implies source(g.edges[k], false) == g.edges[k].dependent
            && destination(g.edges[k], false) == #[trigger] g.edges[k].dependency by { }
}

/// Selecting another vertex preserves successors already accounted for.
proof fn settled_insert(g: GraphState, mask: Seq<bool>, vertex: usize, reverse: bool, added: usize)
    requires added < mask.len(), settled(g, mask, vertex, reverse),
    ensures settled(g, mask.update(added as int, true), vertex, reverse),
{
    select_one(mask, added);
}

/// Relate a mask update to mathematical set insertion.
proof fn select_one(mask: Seq<bool>, vertex: usize)
    requires vertex < mask.len(),
    ensures selected(mask.update(vertex as int, true)) == selected(mask).insert(vertex),
{
    assert(selected(mask.update(vertex as int, true)) =~= selected(mask).insert(vertex));
}

/// Following an existing edge cannot introduce a vertex absent from a closed superset.
proof fn extend_minimal(g: GraphState, seeds: ISet<usize>, members: ISet<usize>, reverse: bool, e: int)
    requires
        0 <= e < g.edges.len(), minimal(g, seeds, members, reverse),
        members.contains(source(g.edges[e], reverse)),
    ensures minimal(g, seeds, members.insert(destination(g.edges[e], reverse)), reverse),
{
    let to = destination(g.edges[e], reverse);
    assert forall|other: ISet<usize>| seeds.subset_of(other) && closed(g, other, reverse)
        implies #[trigger] members.insert(to).subset_of(other) by {
        assert(members.subset_of(other));
        assert(other.contains(source(g.edges[e], reverse)));
        assert(other.contains(to));
    }
}

/// Distinct bounded vertices fit in the graph, bounding the traversal worklist.
proof fn bounded_length(vertices: Seq<usize>, count: usize)
    requires vertices.no_duplicates(), forall|i: int| 0 <= i < vertices.len() ==> #[trigger] vertices[i] < count,
    ensures vertices.len() <= count,
{
    let universe = Seq::new(count as nat, |i: int| i as usize);
    assert(universe.no_duplicates());
    vertices.unique_seq_to_set();
    universe.unique_seq_to_set();
    assert(vertices.to_set().subset_of(universe.to_set())) by {
        assert forall|v: usize| vertices.to_set().contains(v) implies universe.to_set().contains(v) by {
            let i = choose|i: int| 0 <= i < vertices.len() && vertices[i] == v;
            assert(universe[v as int] == v);
        }
    }
    vstd::set_lib::lemma_len_subset(vertices.to_set(), universe.to_set());
}

/// A domain's required providers occur strictly before its position.
pub open spec fn dependencies_before(g: GraphState, order: Seq<usize>, vertex: usize, limit: int) -> bool {
    forall|e: int| 0 <= e < g.edges.len() && #[trigger] g.edges[e].dependent == vertex ==>
        exists|j: int| 0 <= j < limit && #[trigger] order[j] == g.edges[e].dependency
}

/// Extending an order does not change witnesses already present in its prefix.
proof fn before_push(g: GraphState, order: Seq<usize>, vertex: usize, limit: int, added: usize)
    requires 0 <= limit <= order.len(), dependencies_before(g, order, vertex, limit),
    ensures dependencies_before(g, order.push(added), vertex, limit),
{
    assert forall|e: int| 0 <= e < g.edges.len() && #[trigger] g.edges[e].dependent == vertex
        implies exists|j: int| 0 <= j < limit && order.push(added)[j] == g.edges[e].dependency by {
        let j = choose|j: int| 0 <= j < limit && order[j] == g.edges[e].dependency;
        assert(order.push(added)[j] == order[j]);
    }
}

/// A duplicate-free dependency-first prefix containing only declared vertices.
pub open spec fn prefix_ordered(g: GraphState, order: Seq<usize>) -> bool {
    &&& order.no_duplicates()
    &&& forall|i: int| 0 <= i < order.len() ==> #[trigger] order[i] < g.count
    &&& forall|i: int| 0 <= i < order.len() ==>
        #[trigger] dependencies_before(g, order, order[i], i)
}

/// A dependency-first permutation of the entire graph, independent of traversal strategy.
pub open spec fn full_order(g: GraphState, order: Seq<usize>) -> bool {
    order.len() == g.count && prefix_ordered(g, order)
}

/// Append only a new vertex whose dependencies already appear in the prefix.
proof fn append_ready(g: GraphState, order: Seq<usize>, vertex: usize)
    requires prefix_ordered(g, order), vertex < g.count, !order.contains(vertex),
        dependencies_before(g, order, vertex, order.len() as int),
    ensures prefix_ordered(g, order.push(vertex)),
{
    assert forall|i: int| 0 <= i < order.push(vertex).len()
        implies #[trigger] dependencies_before(g, order.push(vertex), order.push(vertex)[i], i) by {
        if i < order.len() { before_push(g, order, order[i], i, vertex); }
        else { before_push(g, order, vertex, order.len() as int, vertex); }
    }
}

/// A selected-provider mask and its order describe the same ready vertex.
proof fn ready_prefix(g: GraphState, mask: Seq<bool>, order: Seq<usize>, vertex: usize)
    requires
        valid(g), mask.len() == g.count, settled(g, mask, vertex, false),
        forall|v: usize| v < g.count ==> mask[v as int] == #[trigger] order.contains(v),
    ensures dependencies_before(g, order, vertex, order.len() as int),
{
    assert forall|e: int| 0 <= e < g.edges.len() && #[trigger] g.edges[e].dependent == vertex
        implies exists|j: int| 0 <= j < order.len() && #[trigger] order[j] == g.edges[e].dependency by {
        assert(valid_edge(g.edges[e], g.count));
        assert(source(g.edges[e], false) == vertex);
        assert(destination(g.edges[e], false) == g.edges[e].dependency);
        assert(selected(mask).contains(g.edges[e].dependency));
        assert(order.contains(g.edges[e].dependency));
    }
}

/// A full bounded permutation contains every graph vertex.
proof fn full_coverage(order: Seq<usize>, count: usize)
    requires order.len() == count, order.no_duplicates(),
        forall|i: int| 0 <= i < order.len() ==> #[trigger] order[i] < count,
    ensures forall|v: usize| v < count ==> #[trigger] order.contains(v),
{
    assert forall|v: usize| v < count implies #[trigger] order.contains(v) by {
        if !order.contains(v) { bounded_length(order.push(v), count); }
    }
}

/// A shorter bounded permutation necessarily leaves an unprocessed vertex.
proof fn missing_vertex(order: Seq<usize>, count: usize) -> (vertex: usize)
    requires order.len() < count, order.no_duplicates(),
        forall|i: int| 0 <= i < order.len() ==> #[trigger] order[i] < count,
    ensures vertex < count && !order.contains(vertex),
{
    let universe = Seq::new(count as nat, |i: int| i as usize);
    assert(universe.no_duplicates());
    universe.unique_seq_to_set();
    order.unique_seq_to_set();
    if !(exists|v: usize| v < count && !order.contains(v)) {
        assert(universe.to_set().subset_of(order.to_set()));
        vstd::set_lib::lemma_len_subset(universe.to_set(), order.to_set());
    }
    choose|v: usize| v < count && !order.contains(v)
}

/// Select the earliest unprocessed position in a proposed topological order.
proof fn first_unselected(order: Seq<usize>, mask: Seq<bool>, start: int) -> (position: int)
    requires
        0 <= start < order.len(),
        forall|j: int| 0 <= j < order.len() ==> #[trigger] order[j] < mask.len(),
        exists|j: int| start <= j < order.len() && !#[trigger] mask[order[j] as int],
    ensures
        start <= position < order.len(), !mask[order[position] as int],
        forall|j: int| start <= j < position ==> mask[#[trigger] order[j] as int],
    decreases order.len() - start,
{
    if !mask[order[start] as int] { start }
    else { first_unselected(order, mask, start + 1) }
}

/// A nonempty residual graph with no ready vertex cannot have a topological order.
proof fn no_ready_no_order(g: GraphState, mask: Seq<bool>, partial: Seq<usize>)
    requires
        valid(g), mask.len() == g.count, prefix_ordered(g, partial), partial.len() < g.count,
        forall|v: usize| v < g.count ==> mask[v as int] == #[trigger] partial.contains(v),
        forall|v: usize| v < g.count && !mask[v as int] ==> !#[trigger] settled(g, mask, v, false),
    ensures !(exists|candidate: Seq<usize>| full_order(g, candidate)),
{
    let missing = missing_vertex(partial, g.count);
    assert forall|candidate: Seq<usize>| !#[trigger] full_order(g, candidate) by {
        if full_order(g, candidate) {
            full_coverage(candidate, g.count);
            assert(candidate.contains(missing));
            assert(!mask[missing as int]);
            let at = choose|j: int| 0 <= j < candidate.len() && candidate[j] == missing;
            let first = first_unselected(candidate, mask, 0);
            let vertex = candidate[first];
            assert(dependencies_before(g, candidate, vertex, first));
            assert(settled(g, mask, vertex, false)) by {
                assert forall|e: int| 0 <= e < g.edges.len() && source(g.edges[e], false) == vertex
                    implies selected(mask).contains(#[trigger] destination(g.edges[e], false)) by {
                    let j = choose|j: int| 0 <= j < first && candidate[j] == g.edges[e].dependency;
                    assert(mask[candidate[j] as int]);
                }
            }
        }
    }
}

/// Dependency constraints between vertices actually selected for an operation list.
pub open spec fn respects(g: GraphState, order: Seq<usize>, reverse: bool) -> bool {
    forall|e: int| 0 <= e < g.edges.len() && order.contains(g.edges[e].dependent)
        && order.contains(g.edges[e].dependency) ==> #[trigger] edge_ordered(g.edges[e], order, reverse)
}

/// Startup places providers first; shutdown places dependents first.
pub open spec fn edge_ordered(edge: Edge, order: Seq<usize>, reverse: bool) -> bool {
    exists|i: int, j: int| 0 <= i < j < order.len()
        && order[i] == (if reverse { edge.dependent } else { edge.dependency })
        && order[j] == (if reverse { edge.dependency } else { edge.dependent })
}

/// Exactly the selected declared effects, without duplicates, in dependency-safe order.
pub open spec fn schedule(g: GraphState, members: Seq<bool>, order: Seq<usize>, reverse: bool) -> bool {
    &&& members.len() == g.count
    &&& order.no_duplicates()
    &&& forall|i: int| 0 <= i < order.len() ==> #[trigger] order[i] < g.count
    &&& forall|v: usize| v < g.count ==> #[trigger] order.contains(v) == members[v as int]
    &&& respects(g, order, reverse)
}

/// Exact selection masks and executable operation sequences denote the same vertex set.
proof fn schedule_set(g: GraphState, members: Seq<bool>, order: Seq<usize>, reverse: bool)
    requires schedule(g, members, order, reverse),
    ensures selected(members) == listed(order),
{
    assert forall|v: usize| selected(members).contains(v) == #[trigger] listed(order).contains(v) by {
        if v >= g.count && order.contains(v) {
            let i = choose|i: int| 0 <= i < order.len() && order[i] == v;
            assert(order[i] < g.count);
        }
    }
    assert(selected(members) =~= listed(order));
}

/// Position in the full permutation for a forward or reverse traversal step.
spec fn slot(length: int, position: int, reverse: bool) -> int {
    if reverse { length - 1 - position } else { position }
}

/// Projecting a topological permutation in one direction preserves its edge constraints.
proof fn selected_order(g: GraphState, full: Seq<usize>, order: Seq<usize>, slots: Seq<int>, reverse: bool)
    requires
        full_order(g, full), slots.len() == order.len(),
        forall|i: int| 0 <= i < order.len() ==> 0 <= #[trigger] slots[i] < full.len() && order[i] == full[slots[i]],
        forall|i: int, j: int| 0 <= i < j < slots.len() ==>
            if reverse { #[trigger] slots[i] > #[trigger] slots[j] }
            else { #[trigger] slots[i] < #[trigger] slots[j] },
    ensures respects(g, order, reverse),
{
    assert forall|e: int| 0 <= e < g.edges.len() && order.contains(g.edges[e].dependent)
        && order.contains(g.edges[e].dependency) implies #[trigger] edge_ordered(g.edges[e], order, reverse) by {
        let i = choose|i: int| 0 <= i < order.len() && order[i] == g.edges[e].dependent;
        let j = choose|j: int| 0 <= j < order.len() && order[j] == g.edges[e].dependency;
        assert(dependencies_before(g, full, full[slots[i]], slots[i]));
        let provider = choose|p: int| 0 <= p < slots[i] && full[p] == g.edges[e].dependency;
        assert(provider == slots[j]);
        if reverse { assert(i < j); } else { assert(j < i); }
    }
}

/// VM names preserve the exact contents and order of the computed vertex list.
pub open spec fn names_match(names: Seq<String>, vertices: Seq<usize>, targets: Seq<String>) -> bool {
    targets.len() == vertices.len() && forall|i: int| 0 <= i < vertices.len() ==>
        vertices[i] < names.len() && #[trigger] targets[i]@ == names[vertices[i] as int]@
}

/// Translate computed indices into the same executable strings subsequently checked for approval.
pub fn name_targets(names: &[String], vertices: &[usize]) -> (result: Option<Vec<String>>)
    ensures
        result.is_some() == (forall|i: int| 0 <= i < vertices.len() ==> #[trigger] vertices[i] < names.len()),
        result.is_some() ==> names_match(names@, vertices@, result.unwrap()@),
{
    let mut targets: Vec<String> = Vec::new();
    let mut i = 0;
    while i < vertices.len()
        invariant
            0 <= i <= vertices.len(), targets.len() == i,
            forall|j: int| 0 <= j < i ==> #[trigger] vertices[j] < names.len(),
            forall|j: int| 0 <= j < i ==> #[trigger] targets[j]@ == names[vertices[j] as int]@,
        decreases vertices.len() - i,
    {
        let vertex = vertices[i];
        if vertex >= names.len() { return None; }
        let ghost previous = targets@;
        let target = names[vertex].as_str().to_string();
        assert(target@ == names[vertex as int]@);
        targets.push(target);
        assert forall|j: int| 0 <= j < i + 1 implies #[trigger] targets[j]@ == names[vertices[j] as int]@ by {
            if j < i { assert(previous[j]@ == names[vertices[j] as int]@); }
        }
        i += 1;
    }
    Some(targets)
}

impl Graph {
    /// Encapsulate bounds so no caller needs to assert an unchecked graph precondition.
    #[verifier::type_invariant]
    closed spec fn well_formed(self) -> bool { valid(self@) }

    /// Admit arbitrary edge lists only when every endpoint belongs to the index space.
    pub fn new(count: usize, edges: Vec<Edge>) -> (result: Option<Self>)
        ensures
            result.is_some() == valid(GraphState { count, edges: edges@ }),
            result.is_some() ==> result.unwrap()@ == (GraphState { count, edges: edges@ }),
    {
        let mut k = 0;
        while k < edges.len()
            invariant
                0 <= k <= edges.len(),
                forall|i: int| 0 <= i < k ==> #[trigger] valid_edge(edges[i], count),
            decreases edges.len() - k,
        {
            if edges[k].dependent >= count || edges[k].dependency >= count {
                assert(!valid_edge(edges[k as int], count));
                let ghost g = GraphState { count, edges: edges@ };
                assert(g.edges[k as int] == edges[k as int]);
                assert(!valid_edge(g.edges[k as int], g.count));
                assert(!valid(GraphState { count, edges: edges@ }));
                return None;
            }
            k += 1;
        }
        Some(Self { count, edges })
    }

    /// Compute the least dependency closure, or affected-dependent closure when reversed.
    /// Only a wrong-size seed mask is rejected; cyclic graphs still have a finite closure.
    pub fn expand(&self, seeds: Vec<bool>, reverse: bool) -> (result: Option<Vec<bool>>)
        ensures
            result.is_some() == (seeds.len() == self@.count),
            result.is_some() ==> result.unwrap().len() == self@.count
                && closure(self@, selected(seeds@), selected(result.unwrap()@), reverse),
    {
        proof { use_type_invariant(self); }
        assert(self@.count == self.count && self@.edges == self.edges@);
        if seeds.len() != self.count { return None; }

        let ghost initial = seeds@;
        let mut mask = seeds;
        let mut queue: Vec<usize> = Vec::new();
        let mut i = 0;
        while i < self.count
            invariant
                0 <= i <= self.count,
                mask@ == initial, mask.len() == self.count,
                queue@.no_duplicates(),
                forall|j: int| 0 <= j < queue.len() ==> #[trigger] queue[j] < i && mask[queue[j] as int],
                forall|v: usize| v < i && mask[v as int] ==> queue@.contains(v),
            decreases self.count - i,
        {
            let ghost previous = queue@;
            if mask[i] { queue.push(i); }
            assert forall|v: usize| v < i + 1 && mask[v as int] implies queue@.contains(v) by {
                if v != i { assert(v < i); assert(previous.contains(v)); }
            }
            i += 1;
        }
        proof { bounded_length(queue@, self.count); }

        let mut head = 0;
        while head < queue.len()
            invariant
                valid(self@),
                self@.count == self.count, self@.edges == self.edges@,
                mask.len() == self.count,
                0 <= head <= queue.len() <= self.count,
                queue@.no_duplicates(),
                forall|j: int| 0 <= j < queue.len() ==> #[trigger] queue[j] < self.count,
                forall|v: usize| v < self.count ==> mask[v as int] == #[trigger] queue@.contains(v),
                selected(initial).subset_of(selected(mask@)),
                minimal(self@, selected(initial), selected(mask@), reverse),
                forall|j: int| 0 <= j < head ==> #[trigger] settled(self@, mask@, queue[j], reverse),
            decreases self.count - head,
        {
            let current = queue[head];
            let mut k = 0;
            while k < self.edges.len()
                invariant
                    valid(self@),
                    self@.count == self.count, self@.edges == self.edges@,
                    mask.len() == self.count,
                    0 <= head < queue.len() <= self.count,
                    current == queue[head as int],
                    queue@.no_duplicates(),
                    forall|j: int| 0 <= j < queue.len() ==> #[trigger] queue[j] < self.count,
                    forall|v: usize| v < self.count ==> mask[v as int] == #[trigger] queue@.contains(v),
                    selected(initial).subset_of(selected(mask@)),
                    minimal(self@, selected(initial), selected(mask@), reverse),
                    forall|j: int| 0 <= j < head ==> #[trigger] settled(self@, mask@, queue[j], reverse),
                    0 <= k <= self.edges.len(),
                    forall|e: int| 0 <= e < k && source(self@.edges[e], reverse) == current
                        ==> selected(mask@).contains(#[trigger] destination(self@.edges[e], reverse)),
                decreases self.edges.len() - k,
            {
                let edge = self.edges[k];
                assert(valid_edge(self@.edges[k as int], self.count));
                assert(edge.dependent < self.count && edge.dependency < self.count);
                let from = if reverse { edge.dependency } else { edge.dependent };
                let to = if reverse { edge.dependent } else { edge.dependency };
                if from == current && !mask[to] {
                    let ghost before = mask@;
                    let ghost prior_queue = queue@;
                    proof {
                        assert(!queue@.contains(to));
                        assert(queue@.push(to).no_duplicates());
                        assert(queue@.contains(current));
                        assert(mask[current as int]);
                        assert(selected(before).contains(from));
                        select_one(before, to);
                        extend_minimal(self@, selected(initial), selected(before), reverse, k as int);
                    }
                    mask.set(to, true);
                    queue.push(to);
                    proof {
                        bounded_length(queue@, self.count);
                        assert forall|j: int| 0 <= j < head implies #[trigger] settled(self@, mask@, queue[j], reverse) by {
                            assert(queue[j] == prior_queue[j]);
                            settled_insert(self@, before, queue[j], reverse, to);
                        }
                    }
                }
                k += 1;
            }
            head += 1;
        }
        assert forall|e: int| 0 <= e < self@.edges.len()
            && selected(mask@).contains(source(self@.edges[e], reverse))
            implies selected(mask@).contains(#[trigger] destination(self@.edges[e], reverse)) by {
            let from = source(self@.edges[e], reverse);
            assert(queue@.contains(from));
            let q = choose|q: int| 0 <= q < queue.len() && queue[q] == from;
            assert(settled(self@, mask@, queue[q], reverse));
            assert(source(self@.edges[e], reverse) == queue[q]);
            assert(selected(mask@).contains(destination(self@.edges[e], reverse)));
        }
        Some(mask)
    }

    /// Compute all ready vertices with one edge scan, preserving ready-wave complexity.
    fn ready_wave(&self, done: &[bool]) -> (ready: Vec<bool>)
        requires done.len() == self@.count,
        ensures ready.len() == self@.count,
            forall|v: usize| v < self@.count ==> ready[v as int] == #[trigger] settled(self@, done@, v, false),
    {
        proof { use_type_invariant(self); }
        let mut ready: Vec<bool> = Vec::new();
        while ready.len() < self.count
            invariant
                ready.len() <= self.count,
                forall|i: int| 0 <= i < ready.len() ==> #[trigger] ready[i],
            decreases self.count - ready.len(),
        { ready.push(true); }

        let mut k = 0;
        while k < self.edges.len()
            invariant
                valid(self@), self@.count == self.count, self@.edges == self.edges@,
                done.len() == self.count, ready.len() == self.count,
                0 <= k <= self.edges.len(),
                forall|v: usize| v < self.count ==> ready[v as int]
                    == #[trigger] settled_through(self@, done@, v, k as int),
            decreases self.edges.len() - k,
        {
            let edge = self.edges[k];
            assert(valid_edge(self@.edges[k as int], self.count));
            let ghost before = ready@;
            if !done[edge.dependency] { ready.set(edge.dependent, false); }
            assert forall|v: usize| v < self.count implies ready[v as int]
                == #[trigger] settled_through(self@, done@, v, k + 1) by {
                assert(before[v as int] == settled_through(self@, done@, v, k as int));
                through_step(self@, done@, v, k as int);
                if v == edge.dependent {
                    assert(selected(done@).contains(edge.dependency) == done[edge.dependency as int]);
                }
            }
            k += 1;
        }
        assert forall|v: usize| v < self.count implies ready[v as int] == #[trigger] settled(self@, done@, v, false) by {
            through_complete(self@, done@, v);
        }
        ready
    }

    /// Return the existing canonical ready-wave order, rejecting exactly cyclic graphs.
    pub fn order(&self) -> (result: Option<Vec<usize>>)
        ensures
            result.is_some() ==> full_order(self@, result.unwrap()@),
            result.is_none() ==> !(exists|candidate: Seq<usize>| full_order(self@, candidate)),
    {
        proof { use_type_invariant(self); }
        let mut done: Vec<bool> = Vec::new();
        while done.len() < self.count
            invariant
                done.len() <= self.count,
                forall|v: int| 0 <= v < done.len() ==> !#[trigger] done[v],
            decreases self.count - done.len(),
        { done.push(false); }

        let mut order: Vec<usize> = Vec::new();
        while order.len() < self.count
            invariant
                valid(self@), self@.count == self.count, self@.edges == self.edges@,
                done.len() == self.count, order.len() <= self.count,
                prefix_ordered(self@, order@),
                forall|v: usize| v < self.count ==> done[v as int] == #[trigger] order@.contains(v),
            decreases self.count - order.len(),
        {
            let ready_mask = self.ready_wave(done.as_slice());
            let mut ready: Vec<usize> = Vec::new();
            let mut v = 0;
            while v < self.count
                invariant
                    valid(self@), done.len() == self.count, self@.count == self.count,
                    ready_mask.len() == self.count,
                    forall|vertex: usize| vertex < self.count ==> ready_mask[vertex as int]
                        == #[trigger] settled(self@, done@, vertex, false),
                    0 <= v <= self.count, ready@.no_duplicates(),
                    forall|j: int| 0 <= j < ready.len() ==> #[trigger] ready[j] < v
                        && !done[ready[j] as int] && settled(self@, done@, ready[j], false),
                    forall|vertex: usize| #![trigger done[vertex as int]] #![trigger ready@.contains(vertex)]
                        vertex < v && !done[vertex as int]
                        && settled(self@, done@, vertex, false) ==> ready@.contains(vertex),
                decreases self.count - v,
            {
                if !done[v] && ready_mask[v] { ready.push(v); }
                v += 1;
            }
            if ready.is_empty() {
                proof { no_ready_no_order(self@, done@, order@); }
                return None;
            }

            let ghost prior_len = order.len();
            let mut j = 0;
            while j < ready.len()
                invariant
                    valid(self@), self@.count == self.count,
                    done.len() == self.count, prefix_ordered(self@, order@),
                    0 <= j <= ready.len(), ready.len() > 0,
                    order.len() == prior_len + j, order.len() <= self.count,
                    ready@.no_duplicates(),
                    forall|r: int| 0 <= r < ready.len() ==> #[trigger] ready[r] < self.count
                        && settled(self@, done@, ready[r], false),
                    forall|r: int| j <= r < ready.len() ==> !done[#[trigger] ready[r] as int],
                    forall|vertex: usize| vertex < self.count ==> done[vertex as int] == #[trigger] order@.contains(vertex),
                decreases ready.len() - j,
            {
                let vertex = ready[j];
                let ghost before = done@;
                proof {
                    ready_prefix(self@, done@, order@, vertex);
                    append_ready(self@, order@, vertex);
                }
                done.set(vertex, true);
                order.push(vertex);
                proof {
                    bounded_length(order@, self.count);
                    assert forall|r: int| 0 <= r < ready.len()
                        implies #[trigger] settled(self@, done@, ready[r], false) by {
                        settled_insert(self@, before, ready[r], false, vertex);
                    }
                }
                j += 1;
            }
        }
        Some(order)
    }

    /// Select exactly the requested operations, in forward startup or reverse shutdown order.
    /// No dependency gains authority here: the returned list still needs approval checking.
    pub fn schedule(&self, members: Vec<bool>, reverse: bool) -> (result: Option<Vec<usize>>)
        ensures
            result.is_some() ==> schedule(self@, members@, result.unwrap()@, reverse),
            result.is_none() == (members.len() != self@.count
                || !(exists|candidate: Seq<usize>| full_order(self@, candidate))),
    {
        if members.len() != self.count { return None; }
        let full = self.order()?;
        proof { full_coverage(full@, self.count); }

        let mut order: Vec<usize> = Vec::new();
        let ghost mut slots: Seq<int> = Seq::empty();
        let mut i = 0;
        while i < full.len()
            invariant
                full_order(self@, full@), self@.count == self.count,
                forall|v: usize| v < self.count ==> #[trigger] full@.contains(v),
                members.len() == self.count, full.len() == self.count,
                0 <= i <= full.len(), order@.no_duplicates(),
                slots.len() == order.len(),
                forall|j: int| 0 <= j < order.len() ==> 0 <= #[trigger] slots[j] < full.len()
                    && order[j] == full@[slots[j]] && members[order[j] as int]
                    && (if reverse { slots[j] >= full.len() - i } else { slots[j] < i }),
                forall|a: int, b: int| 0 <= a < b < slots.len() ==>
                    if reverse { #[trigger] slots[a] > #[trigger] slots[b] }
                    else { #[trigger] slots[a] < #[trigger] slots[b] },
                forall|p: int| 0 <= p < i && members[full@[slot(full.len() as int, p, reverse)] as int]
                    ==> order@.contains(#[trigger] full@[slot(full.len() as int, p, reverse)]),
            decreases full.len() - i,
        {
            let position = if reverse { full.len() - 1 - i } else { i };
            let vertex = full[position];
            if members[vertex] {
                assert(!order@.contains(vertex)) by {
                    if order@.contains(vertex) {
                        let j = choose|j: int| 0 <= j < order.len() && order[j] == vertex;
                        assert(full@[slots[j]] == full@[position as int]);
                        assert(slots[j] == position);
                    }
                }
                order.push(vertex);
                proof { slots = slots.push(position as int); }
            }
            i += 1;
        }
        proof {
            assert forall|j: int| 0 <= j < order.len() implies #[trigger] order[j] < self.count by {
                assert(order[j] == full@[slots[j]]);
            }
            assert forall|v: usize| v < self.count implies #[trigger] order@.contains(v) == members[v as int] by {
                if members[v as int] {
                    assert(full@.contains(v));
                    let p = choose|p: int| 0 <= p < full.len() && full[p] == v;
                    let traversal = if reverse { full.len() - 1 - p } else { p };
                    assert(0 <= traversal < full.len());
                    assert(slot(full.len() as int, traversal, reverse) == p);
                    assert(order@.contains(full@[slot(full.len() as int, traversal, reverse)]));
                } else if order@.contains(v) {
                    let j = choose|j: int| 0 <= j < order.len() && order[j] == v;
                    assert(0 <= slots[j] < full.len());
                    assert(members[order[j] as int]);
                }
            }
            selected_order(self@, full@, order@, slots, reverse);
        }
        Some(order)
    }

    /// Compute complete startup operations and their order in one verified production call.
    /// The result is exactly the least closed superset of the supplied startup roots.
    pub fn startup(&self, seeds: Vec<bool>) -> (result: Option<Vec<usize>>)
        ensures
            result.is_none() == (seeds.len() != self@.count
                || !(exists|candidate: Seq<usize>| full_order(self@, candidate))),
            result.is_some() ==> closure(self@, selected(seeds@), listed(result.unwrap()@), false)
                && result.unwrap()@.no_duplicates() && respects(self@, result.unwrap()@, false),
            result.is_some() ==> forall|i: int| 0 <= i < result.unwrap().len() ==>
                #[trigger] result.unwrap()[i] < self@.count,
    {
        let mask = self.expand(seeds, false)?;
        let ghost members = mask@;
        let order = self.schedule(mask, false)?;
        proof { schedule_set(self@, members, order@, false); }
        Some(order)
    }

    /// Stop exactly the affected vertices still observed running, in reverse dependency order.
    pub fn shutdown(&self, affected: Vec<bool>, running: Vec<bool>) -> (result: Option<Vec<usize>>)
        ensures
            result.is_none() == (affected.len() != self@.count || running.len() != self@.count
                || !(exists|candidate: Seq<usize>| full_order(self@, candidate))),
            result.is_some() ==> result.unwrap()@.no_duplicates() && respects(self@, result.unwrap()@, true),
            result.is_some() ==> forall|i: int| 0 <= i < result.unwrap().len() ==>
                #[trigger] result.unwrap()[i] < self@.count,
            result.is_some() ==> forall|v: usize| v < self@.count ==>
                #[trigger] result.unwrap()@.contains(v) == (affected[v as int] && running[v as int]),
    {
        if affected.len() != self.count || running.len() != self.count { return None; }
        let mut members: Vec<bool> = Vec::new();
        let mut i = 0;
        while i < self.count
            invariant
                affected.len() == self.count, running.len() == self.count,
                0 <= i <= self.count, members.len() == i,
                forall|v: int| 0 <= v < i ==> #[trigger] members[v] == (affected[v] && running[v]),
            decreases self.count - i,
        {
            members.push(affected[i] && running[i]);
            i += 1;
        }

        self.schedule(members, true)
    }
}

}
