//! Build and install the GUI service using the caller's packages and explicit source trees.
//! These functions construct lazy Nix expressions; they never build or run the service in Rust.

mod capture;

mod package;

use crate::command::{DesktopCommand, EXECUTABLE, WINDOW_IDENTITY};
use rusix::{
    interop::{
        NixPath, Package,
        raw::{NixRepresentation, NixValue},
    },
    nix_text,
};

/// Source trees needed by the native build and integration patches.
#[derive(Clone)]
pub struct Sources {
    /// Cargo workspace containing service sources, manifests and the shared lockfile.
    apps: NixPath,
    /// Machine repository containing Qubes and GNOME integration assets.
    machine: NixPath,
}

impl Sources {
    /// Uses caller-supplied symbolic paths without resolving a developer checkout.
    pub fn new(apps: NixPath, machine: NixPath) -> Self {
        Self { apps, machine }
    }

    /// Resolves an implementation-owned relative path inside the machine repository.
    fn machine_file(&self, relative: &str) -> NixValue {
        self.machine
            .join_relative(relative)
            .expect("service asset paths are static and relative")
            .as_expression()
    }
}

/// The packaged GUI service and its installation helpers for one package set.
/// Machine authors retain ownership of VM placement and Qubes permissions.
pub struct Deployment {
    /// Final caller package set, including its platform and overlays.
    pkgs: NixValue,
    /// Source paths retained for patches and completion adapters.
    sources: Sources,
    /// Native service with its Qubes RPC tools and runtime environment.
    package: Package,
}

/// Build the standalone native executable before adding its runtime Qubes tools.
pub fn native(pkgs: &NixValue, apps: &NixPath) -> Package {
    package::native(pkgs, apps)
}

/// Describes the complete GUI service package in the caller's dependency scope.
pub fn deployment(pkgs: &NixValue, sources: &Sources) -> Deployment {
    Deployment {
        package: package::wrapped(pkgs, sources),
        pkgs: pkgs.clone(),
        sources: sources.clone(),
    }
}

impl Deployment {
    /// Returns the package to install in this VM's system environment.
    pub fn package(&self) -> Package {
        self.package.clone()
    }

    /// Returns the executable with its Nix store dependency context intact.
    pub fn executable(&self) -> NixValue {
        nix_text!(
            "{package}/bin/{name}",
            package = self.package.clone(),
            name = EXECUTABLE
        )
        .into()
    }

    /// Installs the extension adapter that forwards its X11 window ID to the trusted reader.
    /// The daemon package supplies the executable used to verify the window's provenance.
    pub fn window_identity_reader(&self, gui_daemon: Package) -> NixValue {
        let arguments = NixValue::list([
            self.executable(),
            WINDOW_IDENTITY.into(),
            "--guid".into(),
            nix_text!("{daemon}/bin/qubes-guid", daemon = gui_daemon).into(),
        ]);
        let command = self
            .pkgs
            .clone()
            .select("lib.escapeShellArgs")
            .call(arguments);

        self.pkgs
            .clone()
            .select("writeShellScript")
            .call("airlock-focused-vm-identity")
            .call(nix_text!(
                r#"
                    exec {command} "$@"
                "#,
                command = command,
            ))
    }

    /// Constructs a shell-safe invocation of a command understood by the runtime parser.
    /// This is shell syntax; desktop entries must use the separate desktop renderer.
    ///
    /// # Panics
    /// Panics if a path argument cannot be represented as UTF-8 Nix text.
    pub fn shell_command(&self, command: &DesktopCommand) -> NixValue {
        let arguments = command.arguments().into_iter().map(|argument| {
            argument
                .into_string()
                .expect("authored command arguments must be UTF-8")
                .into()
        });
        let arguments = NixValue::list(std::iter::once(self.executable()).chain(arguments));

        self.pkgs
            .clone()
            .select("lib.escapeShellArgs")
            .call(arguments)
    }

    /// Constructs a desktop-entry Exec value with desktop-specific escaping.
    /// Shell quoting is insufficient here: percent signs are desktop field-code markers.
    ///
    /// # Panics
    /// Panics if a path argument cannot be represented as UTF-8 desktop-entry text.
    pub fn desktop_command(&self, command: &DesktopCommand) -> NixValue {
        let arguments = command
            .arguments()
            .into_iter()
            .map(|argument| {
                let argument = argument
                    .into_string()
                    .expect("authored arguments must be UTF-8");
                desktop_argument(&argument)
            })
            .collect::<Vec<_>>()
            .join(" ");

        nix_text!(
            "{executable} {arguments}",
            executable = self.executable(),
            arguments = arguments
        )
        .into()
    }
}

/// Quotes literal desktop arguments according to the Exec grammar, without shell evaluation.
fn desktop_argument(argument: &str) -> String {
    if !argument.is_empty()
        && argument
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_/.:".contains(&byte))
    {
        return argument.into();
    }

    let escaped = argument
        .replace('\\', "\\\\\\\\")
        .replace('"', "\\\\\"")
        .replace('`', "\\\\`")
        .replace('$', "\\\\$")
        .replace('%', "%%")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{escaped}\"")
}

/// Exports native and Qubes-integrated package constructors for Nix consumers.
/// Both functions use the same recipes as Rust callers; generated Nix is an artifact.
pub fn packages() -> impl rusix::IntoConfig {
    /// Package entry points emitted together from the service-owned recipes.
    #[derive(rusix::IntoConfig)]
    struct Packages {
        /// Native Cargo build for standalone application consumers.
        native: NixValue,
        /// Complete Qubes desktop service for machine consumers.
        deployment: NixValue,
    }

    let native = NixValue::function_attrs_open(["pkgs", "apps"], |args| {
        let apps = NixPath::from_expression(args.clone().select("apps"));
        let package = package::native(&args.select("pkgs"), &apps);

        (Vec::<(&str, NixValue)>::new(), package.as_expression())
    });
    let wrapped = NixValue::function_attrs_open(["pkgs", "apps", "machine"], |args| {
        let sources = Sources::new(
            NixPath::from_expression(args.clone().select("apps")),
            NixPath::from_expression(args.clone().select("machine")),
        );
        let service = deployment(&args.select("pkgs"), &sources);

        (
            Vec::<(&str, NixValue)>::new(),
            service.package().as_expression(),
        )
    });

    Packages {
        native,
        deployment: wrapped,
    }
}
