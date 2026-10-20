//! Exercise the verified graph kernel as ordinary production Rust.

use airlock_machine_planning::graph::{Edge, Graph};

/// Build a checked graph from literal dependent/provider pairs.
fn graph(count: usize, edges: &[(usize, usize)]) -> Graph {
    Graph::new(
        count,
        edges
            .iter()
            .map(|&(dependent, dependency)| Edge {
                dependent,
                dependency,
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn least_closures_include_dependencies_and_exclude_unjustified_vertices() {
    let graph = graph(5, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
    assert_eq!(
        graph
            .expand(vec![true, false, false, false, false], false)
            .unwrap(),
        [true, true, true, true, false]
    );
    assert_eq!(
        graph
            .expand(vec![false, false, false, true, false], true)
            .unwrap(),
        [true, true, true, true, false]
    );
    assert_eq!(
        graph
            .expand(vec![false, false, false, false, true], false)
            .unwrap(),
        [false, false, false, false, true]
    );
    assert!(graph.expand(vec![], false).is_none());
}

#[test]
fn ready_waves_and_exact_operation_selection_preserve_order() {
    let graph = graph(5, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
    assert_eq!(graph.order().unwrap(), [3, 4, 1, 2, 0]);
    assert_eq!(
        graph
            .startup(vec![true, false, false, false, false])
            .unwrap(),
        [3, 1, 2, 0]
    );
    // Shutdown selects only the affected peers still observed active; another provider may be halted.
    assert_eq!(
        graph
            .shutdown(
                vec![true, true, true, true, false],
                vec![true, true, false, true, true]
            )
            .unwrap(),
        [0, 1, 3]
    );
    assert!(graph.schedule(vec![], false).is_none());
}

#[test]
fn cycles_are_rejected_even_outside_the_selected_operations() {
    let cyclic = graph(4, &[(0, 1), (1, 2), (2, 1)]);
    assert_eq!(
        cyclic
            .expand(vec![true, false, false, false], false)
            .unwrap(),
        [true, true, true, false]
    );
    assert!(cyclic.order().is_none());
    assert!(
        cyclic
            .schedule(vec![false, false, false, true], false)
            .is_none()
    );
    assert!(graph(1, &[(0, 0)]).order().is_none());
    assert!(
        Graph::new(
            1,
            vec![Edge {
                dependent: 0,
                dependency: 1
            }]
        )
        .is_none()
    );
    assert_eq!(
        graph(0, &[]).schedule(vec![], false).unwrap(),
        Vec::<usize>::new()
    );
}

#[test]
fn edge_enumeration_and_duplicates_do_not_change_results() {
    let first = graph(4, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
    let second = graph(4, &[(2, 3), (1, 3), (0, 2), (0, 1), (0, 1)]);
    let seeds = vec![true, false, false, false];
    assert_eq!(
        first.expand(seeds.clone(), false),
        second.expand(seeds, false)
    );
    assert_eq!(first.order(), second.order());
    let affected = vec![false, false, false, true];
    assert_eq!(
        first.expand(affected.clone(), true),
        second.expand(affected, true)
    );
}
