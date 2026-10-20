//! Combines a Rust transport model with references to existing Nix packages, modules and functions.
//! Rust describes their use without inspecting their contents; Nix resolves them when the
//! output is evaluated.

use rusix::interop::raw::NixFunctionExt;
use rusix::{
    Config, IntoRusixValue, RusixValue,
    interop::{InputRef, ModuleRef, NixFunction, Nixpkgs, OverlayRef, PackageRef, raw::NixValue},
    nix_record,
    nixos::NixosModule,
};

/// A model owned by this configuration; Rust requires both credentials for TLS.
pub enum Transport {
    /// A plain connection carries no TLS paths.
    Plain,
    /// A TLS connection must supply both paths; this example does not inspect their files.
    Tls {
        /// Certificate path emitted as a string, not an opaque Nix ecosystem object.
        certificate: String,
        /// Private-key path required by the Rust alternative.
        private_key: String,
    },
}

impl IntoRusixValue for Transport {
    fn into_value(self) -> RusixValue {
        // Our domain chooses these two record shapes; opaque Nix objects need no such model.
        // The plain shape omits credential fields entirely.
        #[derive(IntoRusixValue)]
        struct Plain {
            /// Disabled TLS flag; this record shape omits both credential paths.
            tls: bool,
        }

        // The TLS shape carries the two paths required by Transport::Tls.
        #[derive(IntoRusixValue)]
        struct Tls {
            /// Enabled TLS flag emitted together with both credential paths.
            tls: bool,
            /// Certificate filename emitted as a Nix string without reading the file.
            certificate: String,
            // Private-key filename emitted as `privateKey` without reading the file.
            private_key: String,
        }

        match self {
            Self::Plain => Plain { tls: false }.into_value(),
            Self::Tls {
                certificate,
                private_key,
            } => Tls {
                tls: true,
                certificate,
                private_key,
            }
            .into_value(),
        }
    }
}

/// Defines separate Nix output trees for our transport model, package list and function results.
/// Each root can be compiled on its own or contributed to a NixOS module.
#[rusix::config]
mod config {
    use super::{NixValue, PackageRef, Transport};

    /// A rooted contribution for the domain model this Rust program owns.
    #[rusix(root)]
    pub struct OwnedContribution {
        /// Places our typed Transport at `demo`; NixOS can validate the resulting shape later.
        pub demo: Transport,
    }

    /// A separate rooted contribution for packages from the existing Nix ecosystem.
    #[rusix(root)]
    pub struct PackageContribution {
        // Introduces the existing NixOS environment namespace.
        environment: Environment,
    }

    // Rust describes the list's category, not the packages' internal schemas.
    struct Environment {
        // PackageRef keeps actual Nix package expressions; NixOS checks the resolved objects.
        system_packages: Vec<PackageRef>,
    }

    pub fn packages(packages: Vec<PackageRef>) -> PackageContribution {
        PackageContribution {
            environment: Environment {
                system_packages: packages,
            },
        }
    }

    /// A generic output contribution used to display an opaque function result.
    #[rusix(root)]
    pub struct FunctionResult {
        /// Keeps the Nix-side result deferred; Rust does not infer its type or stringify it.
        pub result: NixValue,
    }
}

use config::FunctionResult;
pub use config::{OwnedContribution, packages};

pub fn module(local: InputRef) -> NixosModule {
    // These lookups describe existing Nix objects; Rust never loads their internals.
    // Dotted paths select nested attributes without package-specific Rust bindings.
    let pkgs = Nixpkgs::new();
    let hello: PackageRef = pkgs.get("hello");
    let requests = pkgs.get("python312Packages.requests");
    let ssh: ModuleRef = pkgs.module("services/networking/ssh/sshd.nix");

    // Nix applies the overlay; a local input can supply packages and modules too.
    let overlay: OverlayRef = local.overlay("overlays.example");
    let overlaid = pkgs.with_overlay(overlay).get("rusixOverlayHello");
    let external = local.package("packages.example");

    // Config::set_dynamic is the escape hatch for an option path chosen at runtime.
    let arbitrary_path = ["services", "rusixExternal", "enable"].join(".");
    let arbitrary = Config::new().set_dynamic(arbitrary_path, true);

    // Each add remains an independent contribution, retaining NixOS merge and priority rules.
    // Imports reuse upstream modules and retain a Rust boundary if their Nix code fails.
    NixosModule::empty()
        .add(packages(vec![hello, requests, overlaid, external]))
        .add(arbitrary)
        .import_ref(ssh)
        .import_ref(local.module("nixosModules.example"))
}

fn main() {
    // Our Rust-owned model becomes a Nix record: demo = { tls = false; }.
    // This first output is ordinary configuration data, without external Nix inputs.
    let owned = OwnedContribution {
        demo: Transport::Plain,
    };
    println!("{}", rusix::compile(owned).unwrap().source);

    // Describe imports and package references from an existing Nix file and nixpkgs.
    // Nix loads input.nix and merges module settings when the generated module is evaluated.
    let local = InputRef::local("example", "input.nix");
    let module = module(local);
    let artifact = rusix::nixos::compile_module(&module).unwrap();
    println!("{}", artifact.module.source);

    // Describe the Nix call toUpper "rusix"; the Rust call records that expression.
    // Nix computes "RUSIX" later, when the generated result is evaluated.
    let uppercase: NixFunction = Nixpkgs::new().function("toUpper");
    let value: NixValue = uppercase.call("rusix");
    let generated = rusix::compile(FunctionResult { result: value }).unwrap();
    println!("{}", generated.source);

    // writeTextFile takes a Nix attribute set describing a file to produce.
    // Its passthru field attaches extra metadata, here a reference to the hello package.
    let pkgs = Nixpkgs::new();
    let args = nix_record! {
        "name": "example.conf",
        "text": "workers = 4\n",
        "executable": false,
        "passthru": nix_record! { "package": pkgs.get("hello") },
    };

    // Record the file recipe and print its Nix expression. Creating the file requires
    // building that recipe separately; this Rust program does not write example.conf.
    let file = pkgs.pkgs_function("writeTextFile").call(args);
    let generated = rusix::compile(FunctionResult { result: file }).unwrap();
    println!("{}", generated.source);
}
