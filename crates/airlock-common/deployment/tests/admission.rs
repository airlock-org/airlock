//! Claims, actual storage and expected candidates remain separate inputs to admission decisions.

use airlock_deployment_core::admission::*;

/// Independent immutable artifact identity shared by the intended source and inspected generation.
fn artifacts() -> Artifacts {
    Artifacts {
        system: "/nix/store/system".into(),
        root_source: "/nix/store/root/root.img".into(),
        kernel: "kernel".into(),
        root: Root {
            digest: "d".repeat(64),
            logical_bytes: 65536,
            payload_bytes: 1024,
            format: "ext4".into(),
            boot_abi: "airlock.nix-closure-root.v1".into(),
        },
    }
}

/// Provenance of an existing admitted generation, not necessarily the latest source commit.
fn source(revision: &str) -> Source {
    Source {
        revision: revision.repeat(40),
        lock_hash: "a".repeat(64),
    }
}

/// Original transport claim from an inactive retained generation.
fn claim() -> Claim {
    Claim {
        guest: "app".into(),
        generation: 2,
        manifest: "/pool/app/2/manifest.json".into(),
        artifacts: artifacts(),
        closure_hash: "e".repeat(64),
        source: source("f"),
        passed: true,
        activated: false,
        retained: true,
        reused: None,
    }
}

/// Independently loaded storage records; these facts must not be inferred from the claim.
fn inspection() -> Inspection {
    Inspection {
        namespace: "app-runtime-store".into(),
        generation: 2,
        manifest: "/pool/app/2/manifest.json".into(),
        artifacts: artifacts(),
        closure_source: "/nix/store/closure".into(),
        closure_hash: "e".repeat(64),
        source: source("f"),
        passed: true,
        verified: true,
        retained: true,
        sender: Sender::Controller,
        approval_source: "/nix/store/closure".into(),
    }
}

/// Exact release requirement, distinct from generation provenance and transport claims.
fn expected(candidate: &str) -> Expected {
    Expected {
        guest: "app".into(),
        artifacts: artifacts(),
        closure_source: "/nix/store/closure".into(),
        source: source("f"),
        candidate: candidate.into(),
    }
}

/// Produce evidence only through the production authentication and candidate-binding constructors.
fn bound(candidate: &str) -> Bound {
    Authenticated::check(claim(), inspection())
        .unwrap()
        .bind(expected(candidate))
        .unwrap()
}

#[test]
fn independent_storage_identity_status_and_approval_are_mandatory() {
    for defect in [
        "namespace",
        "generation",
        "manifest",
        "system",
        "root",
        "kernel",
        "closure",
        "provenance",
        "failed",
        "unverified",
        "unretained",
        "approval",
    ] {
        let mut actual = inspection();
        match defect {
            "namespace" => actual.namespace = "other-runtime-store".into(),
            "generation" => actual.generation = 3,
            "manifest" => actual.manifest = "/pool/other/2/manifest.json".into(),
            "system" => actual.artifacts.system = "other".into(),
            "root" => actual.artifacts.root.logical_bytes += 1,
            "kernel" => actual.artifacts.kernel = "other".into(),
            "closure" => actual.closure_hash = "other".into(),
            "provenance" => actual.source = source("b"),
            "failed" => actual.passed = false,
            "unverified" => actual.verified = false,
            "unretained" => actual.retained = false,
            "approval" => actual.approval_source = "/other".into(),
            _ => unreachable!(),
        }
        assert!(
            Authenticated::check(claim(), actual).is_none(),
            "accepted {defect}"
        );
    }
}

#[test]
fn reuse_keeps_actual_old_provenance_and_never_bypasses_artifact_or_controller_checks() {
    for reused in [None, Some(false), Some(true)] {
        let mut receipt = claim();
        receipt.reused = reused;
        let evidence = Authenticated::check(receipt, inspection()).unwrap();
        let mut requirement = expected("candidate");
        requirement.source = source("b");
        let result = evidence.bind(requirement);
        assert_eq!(result.is_some(), reused == Some(true));
        if let Some(bound) = result {
            assert_eq!(bound.source().revision, "f".repeat(40));
        }
    }
    for defect in [
        "root-digest",
        "root-size",
        "payload-size",
        "root-format",
        "boot-abi",
        "source",
        "guest",
        "controller",
    ] {
        let mut receipt = claim();
        receipt.reused = Some(true);
        let mut actual = inspection();
        if defect == "controller" {
            actual.sender = Sender::Assistant;
        }
        let authenticated = Authenticated::check(receipt, actual).unwrap();
        let mut requirement = expected("candidate");
        match defect {
            "root-digest" => requirement.artifacts.root.digest = "a".repeat(64),
            "root-size" => requirement.artifacts.root.logical_bytes += 1,
            "payload-size" => requirement.artifacts.root.payload_bytes += 1,
            "root-format" => requirement.artifacts.root.format = "other".into(),
            "boot-abi" => requirement.artifacts.root.boot_abi = "other".into(),
            "source" => requirement.closure_source = "other".into(),
            "guest" => requirement.guest = "other".into(),
            "controller" => {}
            _ => unreachable!(),
        }
        assert!(
            authenticated.bind(requirement).is_none(),
            "reuse bypassed {defect}"
        );
    }
    let mut forged = claim();
    forged.reused = Some(true);
    let mut actual = inspection();
    actual.artifacts.root.digest = "other".into();
    assert!(Authenticated::check(forged, actual).is_none());
}

#[test]
fn complete_collection_rejects_omission_extra_duplicate_and_rebound_receipts() {
    for defect in ["missing", "extra", "duplicate", "candidate", "expectation"] {
        let mut requirements = vec![expected("candidate")];
        let mut records = vec![bound("candidate")];
        match defect {
            "missing" => records.clear(),
            "extra" => records.push(bound("candidate")),
            "duplicate" => {
                requirements.push(expected("candidate"));
                records.push(bound("candidate"));
            }
            "candidate" => requirements[0].candidate = "other".into(),
            "expectation" => requirements[0].artifacts.root.digest = "other".into(),
            _ => unreachable!(),
        }
        assert!(
            BoundSet::new(&requirements, records, "candidate".into()).is_none(),
            "accepted {defect}"
        );
    }
    let expected = [expected("candidate")];
    let complete = BoundSet::new(&expected, vec![bound("candidate")], "candidate".into()).unwrap();
    assert_eq!(complete.candidate(), "candidate");
    assert_eq!(complete.entries()[0].guest(), "app");
    assert_eq!(complete.entries()[0].generation(), 2);
}
