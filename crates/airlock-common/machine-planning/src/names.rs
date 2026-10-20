//! Compare VM-name contents in proposed plans without granting execution authority.

use vstd::prelude::*;

verus! {

/// Membership for owned names retained in plans and observations.
pub open spec fn owned_member(names: Seq<String>, target: Seq<char>) -> bool {
    exists|i: int| 0 <= i < names.len() && #[trigger] names[i]@ == target
}

/// Compare an observed or requested name against an immutable owned list.
pub fn owned_contains(names: &[String], target: &str) -> (found: bool)
    ensures found == owned_member(names@, target@),
{
    let mut i = 0;
    while i < names.len()
        invariant
            0 <= i <= names.len(),
            forall|k: int| 0 <= k < i ==> #[trigger] names[k]@ != target@,
        decreases names.len() - i,
    {
        if names[i].as_str() == target { return true; }
        i += 1;
    }
    false
}

}
