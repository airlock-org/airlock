//! Exercise the same handoff and cache gates used by production first boot.

use airlock_deployment_core::boot::{DomainState, Handoff, Reuse, Selection};

/// One generation with separate root, descriptor and guest device identities.
fn selection() -> Selection {
    Selection {
        domain: "sys-gui".into(),
        generation: 7,
        system: "/nix/store/system".into(),
        root: "/nix/store/root/root.img".into(),
        source: "/nix/store/descriptors/sys-gui".into(),
        kernel: "qubes-kernel".into(),
        mode: "shared-closure".into(),
        device: "/dev/xvde".into(),
        blob_device: "/dev/xvdf".into(),
        mountpoint: "/nix/store".into(),
        backend: "erofs-objectmap".into(),
    }
}

#[test]
fn complete_handoff_refuses_missing_substituted_and_invalid_selections() {
    let checked = Handoff::check(vec![selection()], &[selection()]).unwrap();
    assert_eq!(checked.selections()[0].generation, 7);
    assert!(Handoff::check(vec![selection()], &[]).is_none());
    assert!(Handoff::check(vec![], &[selection()]).is_none());
    for field in [
        "domain",
        "generation",
        "source",
        "root",
        "system",
        "kernel",
        "device",
        "blob",
        "mode",
        "mount",
        "backend",
    ] {
        let mut observed = selection();
        match field {
            "domain" => observed.domain = "usr-home".into(),
            "generation" => observed.generation += 1,
            "source" => observed.source.push_str("/other"),
            "root" => observed.root.push_str(".other"),
            "system" => observed.system.push_str("-other"),
            "kernel" => observed.kernel.push_str("-other"),
            "device" => observed.device = "/dev/xvdg".into(),
            "blob" => observed.blob_device = "/dev/xvdh".into(),
            "mode" => observed.mode = "other".into(),
            "mount" => observed.mountpoint = "/other".into(),
            _ => observed.backend = "other".into(),
        }
        assert!(
            Handoff::check(vec![selection()], &[observed]).is_none(),
            "{field}"
        );
    }
    let mut invalid = selection();
    invalid.generation = 0;
    assert!(Handoff::check(vec![invalid], &[selection()]).is_none());
}

/// Configuration evidence differs independently of VM liveness, which startup checks separately.
fn state(fingerprint: &str) -> DomainState {
    DomainState {
        domain: "sys-gui".into(),
        fingerprint: fingerprint.into(),
    }
}

#[test]
fn reuse_requires_the_exact_input_shared_state_and_complete_fresh_snapshots() {
    let checked = Reuse::check(
        "input".into(),
        "input",
        "shared",
        "shared",
        &[state("snapshot")],
        &[state("snapshot")],
    )
    .unwrap();
    assert_eq!(checked.identity(), "input");
    for (identity, shared) in [("stale", "shared"), ("input", "stale"), ("", "shared")] {
        assert!(
            Reuse::check(
                identity.into(),
                "input",
                shared,
                "shared",
                &[state("snapshot")],
                &[state("snapshot")]
            )
            .is_none()
        );
    }
    for saved in [
        vec![],
        vec![state("stale")],
        vec![DomainState {
            domain: "other".into(),
            fingerprint: "snapshot".into(),
        }],
    ] {
        assert!(
            Reuse::check(
                "input".into(),
                "input",
                "shared",
                "shared",
                &[state("snapshot")],
                &saved
            )
            .is_none()
        );
    }
    assert!(
        Reuse::check(
            "input".into(),
            "input",
            "shared",
            "shared",
            &[state("")],
            &[state("")]
        )
        .is_none()
    );
}
