//! Compose machine-planning contracts with the independently checked approval scope.

use vstd::prelude::*;

verus! {

/// Compose exact operation selection and name projection with the existing checked approval.
/// The caller must still supply a faithful topology index and preserve the projected names.
pub proof fn approved_schedule(g: airlock_machine_planning::graph::GraphState, members: Seq<bool>, vertices: Seq<usize>, reverse: bool,
    names: Seq<String>, scope: crate::effects::Scope)
    requires
        airlock_machine_planning::graph::schedule(g, members, vertices, reverse), names.len() == g.count,
        airlock_machine_planning::graph::names_match(names, vertices, scope.targets), scope.reviewed, crate::effects::valid(scope),
    ensures forall|v: usize| v < g.count && members[v as int] ==>
        scope.allowed.contains(#[trigger] names[v as int]@),
{
    assert forall|v: usize| v < g.count && members[v as int]
        implies scope.allowed.contains(#[trigger] names[v as int]@) by {
        assert(vertices.contains(v));
        let i = choose|i: int| 0 <= i < vertices.len() && vertices[i] == v;
        assert(scope.targets[i]@ == names[v as int]@);
        assert(scope.allowed.contains(scope.targets[i]@));
    }
}

}
