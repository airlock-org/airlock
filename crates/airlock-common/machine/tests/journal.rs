//! Journal versions distinguish missing historical evidence from damaged current authority.

use airlock_machine_core::model::JournalMetadata;
use serde_json::{Value, json};

/// Minimal current journal metadata, including an explicit accounted-for nonnative VM.
fn metadata() -> Value {
    let operation = json!({"id":"100", "authority":{"kind":"review_not_required"}});
    json!({
        "native_coordinator":"airlock-dom0-service-v3",
        "release_fingerprint":"a".repeat(64), "previous_fingerprint":"b".repeat(64),
        "operation":operation,
        "guest_generations":[{"operation":operation, "release":"/var/lib/airlock/candidate",
            "fingerprint":"a".repeat(64), "entries":[{"kind":"non_native", "domain":"usr-home"}]}],
    })
}

/// Missing/null evidence and a downgraded protocol cannot turn v3 into a legacy fallback.
#[test]
fn current_journal_requires_guest_selections_and_closed_classification() {
    let raw = metadata();
    let decoded: JournalMetadata = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), raw);
    for change in ["missing", "null", "unknown", "malformed", "downgrade"] {
        let mut changed = metadata();
        match change {
            "missing" => {
                changed.as_object_mut().unwrap().remove("guest_generations");
            }
            "null" => changed["guest_generations"] = Value::Null,
            "unknown" => {
                changed["guest_generations"][0]["entries"][0]["kind"] = json!("maybe_native")
            }
            "malformed" => {
                changed["guest_generations"][0]["entries"][0]["domain"] = json!("../other")
            }
            _ => changed["native_coordinator"] = json!("airlock-dom0-service-v2"),
        }
        assert!(
            serde_json::from_value::<JournalMetadata>(changed).is_err(),
            "accepted {change}"
        );
    }
}

/// Historical v2 stays readable, without claiming that it contains concrete guest selections.
#[test]
fn historical_journal_without_guest_snapshot_remains_inspectable() {
    let mut historical = metadata();
    historical["native_coordinator"] = json!("airlock-dom0-service-v2");
    historical
        .as_object_mut()
        .unwrap()
        .remove("guest_generations");
    let decoded: JournalMetadata = serde_json::from_value(historical).unwrap();
    assert!(decoded.details.guest_generations.is_none());
}
