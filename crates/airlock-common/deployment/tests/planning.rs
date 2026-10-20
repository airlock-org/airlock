//! Compose production graph planning with the checked deployment approval.

use airlock_deployment_core::{
    effects::Targets,
    review::{Binding, Candidate, Review, Status},
};
use airlock_machine_planning::graph::{Edge, Graph, name_targets};

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
fn required_dependencies_still_need_original_approval() {
    let graph = graph(2, &[(1, 0)]);
    let seeds = vec![false, true];
    let order = graph.startup(seeds).unwrap();
    let names = vec!["sys-net".into(), "usr-home".into()];
    let targets = name_targets(&names, &order).unwrap();
    assert_eq!(targets, ["sys-net", "usr-home"]);
    assert!(name_targets(&names, &[2]).is_none());
    let mut review = Review {
        request: "review",
        revision: "source",
        lock: "lock",
        guest: None,
        baseline: "baseline",
        fingerprint: Some("shown"),
        status: Status::Approved,
        release: None,
        operation: None,
        allowed: vec!["usr-home"],
        allow_reboot: false,
    };
    let binding = Binding {
        request: "review",
        release: "candidate",
        revision: "source",
        lock: "lock",
    };
    let candidate = Candidate {
        release: "candidate",
        revision: "source",
        lock: "lock",
        guest: None,
        baseline: Some("baseline"),
        previous_operation: None,
        effects: targets.iter().map(String::as_str).collect(),
        may_reboot: false,
    };
    assert!(Targets::reviewed(&review, &binding, &candidate, targets.clone()).is_err());
    review.allowed.push("sys-net");
    assert!(Targets::reviewed(&review, &binding, &candidate, targets.clone()).is_ok());
}
