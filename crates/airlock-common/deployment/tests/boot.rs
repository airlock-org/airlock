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

/// A checked selection is mandatory for requests; changed topology cannot reuse an earlier check.
#[test]
fn selected_setup_preserves_identity_and_requires_retained_storage() {
    use airlock_deployment_core::boot::{Argument, BootField, SelectedGeneration};
    let selected = SelectedGeneration::check(selection(), &selection()).unwrap();
    let request = selected.setup(&selection()).unwrap();
    assert_eq!(request.selection().generation, 7);
    for field in ["domain", "generation", "system", "root", "source", "kernel"] {
        let mut changed = selection();
        match field {
            "domain" => changed.domain = "other".into(),
            "generation" => changed.generation += 1,
            "system" => changed.system.push_str("-other"),
            "root" => changed.root.push_str("-other"),
            "source" => changed.source.push_str("-other"),
            _ => changed.kernel.push_str("-other"),
        }
        assert!(selected.setup(&changed).is_none(), "{field}");
        assert!(
            SelectedGeneration::check(changed, &selection()).is_none(),
            "{field}"
        );
    }
    let root_args = request.root_arguments();
    assert!(
        matches!(root_args.get(4), Some(Argument::Text(value)) if value == "sys-gui-runtime-store")
    );
    let args = request.store_arguments();
    assert!(matches!(args.last(), Some(Argument::Text(value)) if value == "--require-retained"));
    assert!(matches!(args.get(23), Some(Argument::Generation(7))));
    assert_eq!(
        request.setting_arguments(BootField::Kernel),
        ["sys-gui", "kernel", "qubes-kernel"]
    );
    assert_eq!(
        request.setting_arguments(BootField::System),
        ["sys-gui", "airlock-system-source", "/nix/store/system"]
    );
    let handoff = Handoff::check(vec![selection()], &[selection()]).unwrap();
    assert_eq!(
        handoff.setup_arguments("/topology", "/checked", "digest"),
        [
            "--topology",
            "/topology",
            "--boot-selection",
            "/checked",
            "--boot-selection-hash",
            "digest"
        ]
    );
}

/// Root import requires this VM's exact source and positive halted evidence, not merely a failed running check.
#[test]
fn selected_root_import_rejects_uncertain_power_and_substituted_artifacts() {
    use airlock_deployment_core::{
        boot::SelectedGeneration,
        inventory::PowerState,
        storage::{Power, RootArtifact},
    };
    let selected = SelectedGeneration::check(selection(), &selection()).unwrap();
    let request = selected.setup(&selection()).unwrap();
    let mut artifact = RootArtifact {
        name: "sys-gui".into(),
        source: "/nix/store/root/root.img".into(),
        image: "/pool/root.img".into(),
        hash: "digest".into(),
    };
    for state in ["Running", "Paused", "", "unknown"] {
        let power = Power {
            name: "sys-gui".into(),
            state: PowerState::classify(state),
        };
        assert!(request.root(&artifact, &power).is_none(), "{state}");
    }
    let power = Power {
        name: "sys-gui".into(),
        state: PowerState::classify("Halted"),
    };
    assert_eq!(
        request.root(&artifact, &power).unwrap().arguments(),
        ["import", "sys-gui:root", "/pool/root.img"]
    );
    artifact.source.push_str("-wrong");
    assert!(request.root(&artifact, &power).is_none());
    artifact.source = selection().root;
    artifact.name = "other".into();
    assert!(request.root(&artifact, &power).is_none());
}
