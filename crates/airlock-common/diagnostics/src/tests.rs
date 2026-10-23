//! Check strict protocol decoding and the executable bodies of the verified gates.

use crate::*;
use serde_json::{Value, json};

/// A catalog request has no source paths, caller identity or approval switches.
fn query() -> Value {
    json!({"schema":REQUEST_SCHEMA,"id":"request-test","target":"usr-home","reason":"Investigate startup","action":{"operation":"catalog"}})
}

#[test]
fn decoding_rejects_authority_injection_unknown_fields_and_oversized_requests() {
    assert!(decode_query(&serde_json::to_vec(&query()).unwrap()).is_ok());
    for (field, value) in [
        ("caller", json!("dom0")),
        ("approved", json!(true)),
        ("command", json!("cat /etc/shadow")),
    ] {
        let mut request = query();
        request[field] = value;
        assert!(decode_query(&serde_json::to_vec(&request).unwrap()).is_err());
    }
    let mut request = query();
    request["action"]["path"] = json!("/etc/shadow");
    assert!(decode_query(&serde_json::to_vec(&request).unwrap()).is_err());
    let mut request = query();
    request["schema"] = json!("future-unknown-schema");
    assert!(decode_query(&serde_json::to_vec(&request).unwrap()).is_err());
    assert!(decode_query(&vec![b' '; REQUEST_LIMIT + 1]).is_err());
    assert!(serde_json::from_str::<TargetId>("\"--dom0\"").is_err());
    assert!(serde_json::from_str::<BootId>("\"current OR previous\"").is_err());
}

#[test]
fn approval_cannot_be_reused_for_a_different_query_or_larger_scope() {
    let policy = authority::Policy {
        target: "usr-home".into(),
        callers: vec!["usr-assistant".into()],
        sources: vec!["complete-reader-descriptor".into()],
        ceiling: 8,
    };
    let request = || authority::Request {
        caller: "usr-assistant".into(),
        target: "usr-home".into(),
        source: "complete-reader-descriptor".into(),
        binding: "complete-query".into(),
        bytes: 8,
    };
    let mut decision = authority::Decision {
        binding: "complete-query".into(),
        approved: true,
    };
    assert!(authority::Permit::check(&policy, request(), &decision).is_some());
    decision.binding = "another-query".into();
    assert!(authority::Permit::check(&policy, request(), &decision).is_none());
    decision.binding = "complete-query".into();
    decision.approved = false;
    assert!(authority::Permit::check(&policy, request(), &decision).is_none());
    decision.approved = true;
    let mut larger = request();
    larger.bytes += 1;
    assert!(authority::Permit::check(&policy, larger, &decision).is_none());
    let mut changed = request();
    changed.source = "other-reader".into();
    assert!(authority::Permit::check(&policy, changed, &decision).is_none());
}

#[test]
fn self_relay_can_only_return_to_the_installed_authenticated_observer() {
    let checked =
        authority::SelfRelay::check("usr-assistant", "usr-assistant", "usr-assistant".into())
            .unwrap();
    assert_eq!(checked.target(), "usr-assistant");
    assert!(authority::SelfRelay::check("usr-assistant", "usr-home", "usr-home".into()).is_none());
    assert!(authority::SelfRelay::check("usr-assistant", "usr-assistant", "dom0".into()).is_none());
}
