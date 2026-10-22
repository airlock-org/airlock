//! Exercise exclusive installation claims without evaluating or inspecting opaque Nix values.

use rusix::{
    installation::{self, Application, CollisionChecks, InstallationError},
    interop::{
        Package,
        raw::{NixRepresentation, NixValue},
    },
};

/// Describes an inert package export without building or reading a Nix store path.
fn app(owner: &str, executable: &str, commands: &[&[&str]]) -> Application {
    Application::new(owner).executable(
        Package::from_expression(NixValue::from("/nix/store/test-package")),
        executable,
        commands
            .iter()
            .map(|path| path.iter().map(|word| (*word).into()).collect()),
    )
}

/// Creates a file declaration whose deferred contents have no role in ownership checks.
fn file(owner: &str, path: &str) -> Application {
    Application::new(owner).etc_text(path, "same contents")
}

#[test]
fn command_paths_use_executable_and_parent_namespaces() {
    installation::compose(
        "desktop",
        CollisionChecks::Enabled,
        [
            app(
                "first",
                "first-app",
                &[
                    &["capture"],
                    &["capture", "retry"],
                    &["queue"],
                    &["queue", "retry"],
                ],
            ),
            app("second", "second-app", &[&["capture"]]),
        ],
    )
    .unwrap();

    for scope in ["first-vm", "second-vm"] {
        installation::compose(
            scope,
            CollisionChecks::Enabled,
            [app("same-app", "same-app", &[&["capture"]])],
        )
        .unwrap();
    }

    let error = installation::compose(
        "desktop",
        CollisionChecks::Enabled,
        [app(
            "capture-app",
            "capture-app",
            &[&["capture"], &["capture"]],
        )],
    )
    .unwrap_err();
    assert!(matches!(error, InstallationError::Collision { .. }));
    assert!(error.to_string().contains("capture-app capture"));
}

#[test]
fn exports_and_application_identities_are_exclusive_even_when_equal() {
    let error = installation::compose(
        "sys-gui",
        CollisionChecks::Enabled,
        [
            app("first", "same-executable", &[]),
            app("second", "same-executable", &[]),
        ],
    )
    .unwrap_err();
    assert!(
        matches!(&error, InstallationError::Collision { scope, first, second, .. }
        if scope == "sys-gui" && first == "first" && second == "second")
    );

    let shared = app("shared", "shared-app", &[]);
    let error = installation::compose(
        "sys-gui",
        CollisionChecks::Enabled,
        [shared.clone(), shared],
    )
    .unwrap_err();
    assert!(error.to_string().contains("application \"shared\""));
}

#[test]
fn file_collisions_include_ancestors_but_respect_component_boundaries() {
    for (first, second) in [
        ("app/config", "app/config"),
        ("app", "app/config"),
        ("app/config", "app"),
    ] {
        let error = installation::compose(
            "desktop",
            CollisionChecks::Enabled,
            [file("first", first), file("second", second)],
        )
        .unwrap_err();
        assert!(matches!(error, InstallationError::Collision { .. }));
    }
    installation::compose(
        "desktop",
        CollisionChecks::Enabled,
        [file("first", "app"), file("second", "app-extra/config")],
    )
    .unwrap();
}

#[test]
fn malformed_declarations_fail_before_emitting_modules() {
    for path in [
        "",
        "/absolute",
        "app/../config",
        "app/./config",
        "app//config",
        "app/",
    ] {
        assert!(matches!(
            installation::compose("desktop", CollisionChecks::Enabled, [file("app", path)]),
            Err(InstallationError::Invalid { .. })
        ));
    }
    for name in ["", "bin/app", "--option", "two words", ".."] {
        assert!(matches!(
            installation::compose("desktop", CollisionChecks::Enabled, [app("app", name, &[])]),
            Err(InstallationError::Invalid { .. })
        ));
    }
    assert!(matches!(
        installation::compose(
            "desktop",
            CollisionChecks::Enabled,
            [app("app", "app", &[&["capture", "retry"]]),]
        ),
        Err(InstallationError::Invalid { .. })
    ));
    assert!(matches!(
        installation::compose("", CollisionChecks::Enabled, [Application::new("app")]),
        Err(InstallationError::Invalid { .. })
    ));
    assert!(matches!(
        installation::compose("desktop", CollisionChecks::Enabled, [Application::new("")]),
        Err(InstallationError::Invalid { .. })
    ));
}

#[test]
fn opting_out_of_collisions_does_not_allow_malformed_destinations() {
    assert!(matches!(
        installation::compose(
            "desktop",
            CollisionChecks::Disabled,
            [file("app", "app/../config")]
        ),
        Err(InstallationError::Invalid { .. })
    ));
    assert!(matches!(
        installation::compose(
            "desktop",
            CollisionChecks::Disabled,
            [app("app", "path/to/executable", &[])]
        ),
        Err(InstallationError::Invalid { .. })
    ));
    assert!(matches!(
        installation::compose("", CollisionChecks::Disabled, []),
        Err(InstallationError::Invalid { .. })
    ));
}
