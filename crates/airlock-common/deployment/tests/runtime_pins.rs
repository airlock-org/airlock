//! Unknown mapping observations preserve pins; exact absent identities alone permit release.

use airlock_deployment_core::retention::runtime::{Context, Observation, Phase, Presence};

#[test]
fn inventory_membership_is_exact_complete_and_order_independent() {
    for names in [
        vec!["airlock-one", "other"],
        vec!["other", "airlock-one", "airlock-one"],
    ] {
        for (name, expected) in [
            ("airlock-one", Presence::Present),
            ("airlock-on", Presence::Absent),
        ] {
            let observation = Observation::from_inventory(
                name.into(),
                Some(names.iter().map(|s| s.to_string()).collect()),
            );
            assert_eq!(observation.presence(), expected);
        }
    }
    for inventory in [
        None,
        Some(vec![String::new()]),
        Some(vec!["airlock-one".into(), String::new()]),
    ] {
        let observation = Observation::from_inventory("airlock-one".into(), inventory);
        assert_eq!(observation.presence(), Presence::Unknown);
        assert!(
            observation
                .release("/pool".into(), Phase::Active, Context::Reconciliation)
                .is_none()
        );
    }
    assert_eq!(
        Observation::from_inventory(String::new(), Some(vec![])).presence(),
        Presence::Unknown
    );
}

#[test]
fn release_requires_absence_and_respects_preparation_and_original_identity() {
    for phase in [Phase::Preparing, Phase::Active, Phase::Retiring] {
        for context in [Context::Collection, Context::Reconciliation] {
            let absent = Observation::from_inventory("airlock-original".into(), Some(vec![]));
            let request = absent.release("/original-pool".into(), phase, context);
            let expected =
                !matches!(phase, Phase::Preparing) || matches!(context, Context::Reconciliation);
            assert_eq!(request.is_some(), expected);
            if let Some(request) = request {
                assert_eq!(request.name(), "airlock-original");
                assert_eq!(request.pool(), "/original-pool");
            }
            let present = Observation::from_inventory(
                "airlock-original".into(),
                Some(vec!["airlock-original".into()]),
            );
            assert!(present.release("/pool".into(), phase, context).is_none());
            let unknown = Observation::from_inventory("airlock-original".into(), None);
            assert!(unknown.release("/pool".into(), phase, context).is_none());
        }
    }
    assert!(
        Observation::from_inventory("airlock-one".into(), Some(vec![]))
            .release(String::new(), Phase::Active, Context::Reconciliation)
            .is_none()
    );
    for status in ["preparing", "active", "retiring"] {
        assert!(Phase::decode(status).is_some());
    }
    assert!(Phase::decode("unknown").is_none());
}
