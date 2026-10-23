//! Production request constructors preserve hardware incarnation, source identity and explicit intent.

use airlock_provisioning_core::*;

/// Independent hardware observations, without opening any real device.
fn observation() -> Observation {
    Observation {
        disk: Disk {
            name: "/dev/fake".into(),
            number: 42,
            sequence: 7,
            serial: "confirmed-serial".into(),
            bytes: 4 * 1024 * 1024,
        },
        usb: true,
        whole: true,
        writable: true,
        unmounted: true,
        unheld: true,
    }
}

/// Facts reported by the exact opened descriptor, separate from pathname observations.
fn opened() -> OpenedDisk {
    OpenedDisk {
        number: 42,
        sequence: 7,
        bytes: 4 * 1024 * 1024,
    }
}

/// Metadata of one stable source file, not a disk or a user-supplied pathname.
fn version() -> ImageVersion {
    ImageVersion {
        device: 9,
        inode: 100,
        bytes: 2 * 1024 * 1024,
        modified: 10,
        modified_ns: 20,
        changed: 30,
        changed_ns: 40,
    }
}

/// A checked source with its actual digest and optional supplying publication.
fn image(publication: Option<&str>) -> CheckedImage {
    CheckedImage::check(
        version(),
        version(),
        "d".repeat(64),
        publication.map(str::to_owned),
        true,
        true,
    )
    .unwrap()
}

/// Original eligible USB confirmation constructed through the production checks.
fn confirmed() -> ConfirmedUsb {
    SelectedUsb::check(observation())
        .unwrap()
        .confirm(Intent::Write, "confirmed-serial")
        .unwrap()
}

/// Private build coordinate and original source selection.
fn run(result: &str) -> BuildRun {
    BuildRun {
        result_link: result.into(),
        revision: "a".repeat(40),
        machine_revision: "b".repeat(40),
    }
}

/// Independent observed successful output from the matching run.
fn built(result: &str) -> BuildObservation {
    BuildObservation {
        outcome: BuildOutcome::Success,
        result_link: result.into(),
        revision: "a".repeat(40),
        machine_revision: "b".repeat(40),
        output: Some("/nix/store/new-image".into()),
    }
}

#[test]
fn inspection_wrong_serial_and_ineligible_hardware_cannot_grant_write_intent() {
    assert!(
        SelectedUsb::check(observation())
            .unwrap()
            .confirm(Intent::Inspect, "confirmed-serial")
            .is_none()
    );
    assert!(
        SelectedUsb::check(observation())
            .unwrap()
            .confirm(Intent::Write, "different-serial")
            .is_none()
    );
    for defect in ["transport", "partition", "readonly", "sequence", "serial"] {
        let mut disk = observation();
        match defect {
            "transport" => disk.usb = false,
            "partition" => disk.whole = false,
            "readonly" => disk.writable = false,
            "sequence" => disk.disk.sequence = 0,
            "serial" => disk.disk.serial.clear(),
            _ => unreachable!(),
        }
        assert!(SelectedUsb::check(disk).is_none(), "accepted {defect}");
    }
}

#[test]
fn unsuccessful_stale_or_other_run_builds_cannot_release_a_waiting_writer() {
    for defect in ["failed", "missing", "run", "revision", "machine"] {
        let mut actual = built("/private/run/image");
        match defect {
            "failed" => actual.outcome = BuildOutcome::Failed,
            "missing" => actual.output = None,
            "run" => actual.result_link = "/private/other/image".into(),
            "revision" => actual.revision = "c".repeat(40),
            "machine" => actual.machine_revision = "c".repeat(40),
            _ => unreachable!(),
        }
        assert!(
            CompletedBuild::check(run("/private/run/image"), actual).is_none(),
            "accepted {defect}"
        );
    }
    let other =
        CompletedBuild::check(run("/private/other/image"), built("/private/other/image")).unwrap();
    assert!(
        confirmed()
            .wait(run("/private/run/image"))
            .complete(other)
            .is_none()
    );
    let completed =
        CompletedBuild::check(run("/private/run/image"), built("/private/run/image")).unwrap();
    assert_eq!(completed.output(), "/nix/store/new-image");
    let authority = confirmed()
        .wait(run("/private/run/image"))
        .complete(completed)
        .unwrap();
    assert!(
        authority
            .request(
                image(Some("/nix/store/old-image")),
                &observation(),
                &opened(),
                &version(),
                CopyMode::Sparse
            )
            .is_none()
    );
}

#[test]
fn replugged_changed_in_use_or_wrongly_opened_devices_fail_before_dispatch() {
    for defect in [
        "name",
        "number",
        "sequence",
        "serial",
        "capacity",
        "mounted",
        "holders",
        "readonly",
        "opened-number",
        "opened-sequence",
        "opened-capacity",
        "source-inode",
        "source-time",
    ] {
        let mut actual = observation();
        let mut descriptor = opened();
        let mut source = version();
        match defect {
            "name" => actual.disk.name = "/dev/other".into(),
            "number" => actual.disk.number += 1,
            "sequence" => actual.disk.sequence += 1,
            "serial" => actual.disk.serial = "other".into(),
            "capacity" => actual.disk.bytes += 1,
            "mounted" => actual.unmounted = false,
            "holders" => actual.unheld = false,
            "readonly" => actual.writable = false,
            "opened-number" => descriptor.number += 1,
            "opened-sequence" => descriptor.sequence += 1,
            "opened-capacity" => descriptor.bytes += 1,
            "source-inode" => source.inode += 1,
            "source-time" => source.changed_ns += 1,
            _ => unreachable!(),
        }
        let authority = confirmed().direct("d".repeat(64)).unwrap();
        assert!(
            authority
                .request(image(None), &actual, &descriptor, &source, CopyMode::Sparse)
                .is_none(),
            "accepted {defect}"
        );
    }
}

#[test]
fn exact_image_policy_capacity_and_requested_verification_mode_are_preserved() {
    assert!(
        confirmed()
            .direct("c".repeat(64))
            .unwrap()
            .request(
                image(None),
                &observation(),
                &opened(),
                &version(),
                CopyMode::Sparse
            )
            .is_none()
    );
    assert!(
        confirmed()
            .published("/nix/store/new-image".into())
            .unwrap()
            .request(
                image(Some("/nix/store/old-image")),
                &observation(),
                &opened(),
                &version(),
                CopyMode::Full
            )
            .is_none()
    );
    let mut large = version();
    large.bytes = 5 * 1024 * 1024;
    let large_image = CheckedImage::check(large, large, "d".repeat(64), None, true, true).unwrap();
    assert!(
        confirmed()
            .direct("d".repeat(64))
            .unwrap()
            .request(
                large_image,
                &observation(),
                &opened(),
                &large,
                CopyMode::Full
            )
            .is_none()
    );

    let request = confirmed()
        .direct("d".repeat(64))
        .unwrap()
        .request(
            image(None),
            &observation(),
            &opened(),
            &version(),
            CopyMode::Verified,
        )
        .unwrap();
    assert!(request.verify());
    assert!(!request.sparse());
    assert_eq!(request.bytes(), 2 * 1024 * 1024);
    assert_eq!(request.device(), 42);
    assert_eq!(request.digest(), "d".repeat(64));
    let mut changed = version();
    changed.modified_ns += 1;
    assert!(!request.source_matches(&changed));
}

#[test]
fn changed_hashing_inputs_and_invalid_image_formats_cannot_be_sealed() {
    let mut changed = version();
    changed.changed_ns += 1;
    assert!(CheckedImage::check(version(), changed, "d".repeat(64), None, true, true).is_none());
    assert!(CheckedImage::check(version(), version(), "d".repeat(64), None, false, true).is_none());
    assert!(CheckedImage::check(version(), version(), "d".repeat(64), None, true, false).is_none());
    assert!(CheckedImage::check(version(), version(), "d".repeat(63), None, true, true).is_none());
}
