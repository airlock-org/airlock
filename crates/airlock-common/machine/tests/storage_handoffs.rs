//! Storage formats preserve their different target identities and required completion evidence.

use airlock_machine_core::model::{GenerationReceipt, InstalledStorageReceipt};
use serde_json::{Value, json};

/// A loaded generation fixture includes both namespace identity and attachment coordinates.
fn generation() -> Value {
    json!({"domain":"sys-gui-runtime-store", "generation":7, "system":"/nix/store/system",
        "root_source":"/nix/store/root/root.img", "source":"/nix/store/descriptor", "kernel_name":"kernel",
        "mode":"shared-closure", "device":"/dev/xvde", "blob_device":"/dev/xvdf", "mountpoint":"/nix/store",
        "view_backend":"erofs-objectmap", "manifest":"/pool/manifest.json", "metadata_image":"/pool/store.erofs",
        "blob_map":"/pool/blobs.json", "blob_logical_size":65536, "closure_hash":"closure", "diagnostic_counter":12})
}

/// An installation uses an explicit result schema, guest identity and mandatory daemon outcome.
fn installed() -> Value {
    let mut value = generation();
    value["domain"] = json!("sys-gui");
    value["schema"] = json!("airlock.nix-domain-storage.v1");
    value["qubesd_restart_required"] = json!(false);
    value["qubesd_restarted"] = json!(false);
    value
}

#[test]
fn distinct_wire_formats_reject_wrong_decoders_and_zero_generations() {
    assert!(serde_json::from_value::<InstalledStorageReceipt>(generation()).is_err());
    assert!(serde_json::from_value::<GenerationReceipt>(installed()).is_err());

    // Even a guest whose legal name ends in the namespace suffix is distinguished by schema.
    let mut ambiguous = installed();
    ambiguous["domain"] = json!("sys-gui-runtime-store");
    assert!(serde_json::from_value::<GenerationReceipt>(ambiguous).is_err());

    for mut value in [generation(), installed()] {
        value["generation"] = json!(0);
        assert!(serde_json::from_value::<GenerationReceipt>(value.clone()).is_err());
        assert!(serde_json::from_value::<InstalledStorageReceipt>(value).is_err());
    }
}

#[test]
fn incomplete_or_malformed_daemon_results_cannot_mean_no_restart() {
    for field in ["qubesd_restart_required", "qubesd_restarted"] {
        let mut missing = installed();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<InstalledStorageReceipt>(missing).is_err());

        let mut malformed = installed();
        malformed[field] = json!("false");
        assert!(serde_json::from_value::<InstalledStorageReceipt>(malformed).is_err());
    }
}

#[test]
fn round_trips_preserve_identity_and_diagnostics_without_duplicate_fields() {
    let original = generation();
    let typed: GenerationReceipt = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(typed.identity.generation.get(), 7);

    let encoded = serde_json::to_vec(&typed).unwrap();
    let decoded: Value =
        airlock_nix_core::state::parse_json(&encoded, std::path::Path::new("<test>")).unwrap();
    for (key, value) in original.as_object().unwrap() {
        assert_eq!(&decoded[key], value, "{key}");
    }

    let original = installed();
    let typed: InstalledStorageReceipt = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(typed.domain.as_str(), "sys-gui");

    let encoded = serde_json::to_vec(&typed).unwrap();
    let decoded: Value =
        airlock_nix_core::state::parse_json(&encoded, std::path::Path::new("<test>")).unwrap();
    for (key, value) in original.as_object().unwrap() {
        assert_eq!(&decoded[key], value, "{key}");
    }
}
