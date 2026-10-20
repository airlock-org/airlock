//! Demonstrates validation layers: Rust checks field types, Rusix checks bindings,
//! and NixOS checks the final configuration against its option declarations.
use rusix::IntoRusixValue;

/// A reusable port-domain number; Rusix lowers the inner u16 without checking NixOS's schema.
#[derive(IntoRusixValue)]
pub struct Port(
    /// Listening port emitted as an integer in Nix.
    pub u16,
);

/// Contributes SSH settings and imports the upstream NixOS module that declares their options.
/// Rust structs provide the setting paths; NixOS checks their values during evaluation.
#[rusix::config]
mod config {
    use super::Port;
    use rusix::nixos::NixosModule;

    /// Defines only this contribution's tree, not a Rust binding for all NixOS services.
    #[rusix(root)]
    pub struct SshContribution<T> {
        /// Places the supplied service values under the existing NixOS `services` namespace.
        pub services: Services<T>,
    }

    /// Places a supplied SSH model under services; imported NixOS declarations check its values.
    pub struct Services<T> {
        /// NixOS's imported schema ultimately checks whatever T lowers into this option.
        pub openssh: T,
    }

    // A small user-defined model of just the SSH fields used here.
    struct SshOptions {
        // Defines the existing enable option; the example keeps the daemon disabled.
        enable: bool,
        // Rust checks the element type; NixOS decides which lowered port values are valid.
        ports: Vec<Port>,
    }

    pub fn module() -> NixosModule {
        // Each add keeps a separate contribution for NixOS merging and priorities.
        // The import supplies real upstream option declarations without modeling their internals.
        NixosModule::empty()
            .add(SshContribution {
                services: Services {
                    openssh: SshOptions {
                        enable: false,
                        ports: vec![Port(22)],
                    },
                },
            })
            .import("nixos/modules/services/networking/ssh/sshd.nix")
    }
}

pub use config::{Services, SshContribution, module};

fn main() {
    // Describe SSH settings and import the upstream module that declares their options.
    // NixOS will use those declarations to check the settings during evaluation.
    let module = module();

    // Compilation checks the Rust description and emits a NixOS module.
    // It does not run the NixOS option checks or change the system's SSH service.
    let artifact = rusix::nixos::compile_module(&module).unwrap();
    println!("{}", artifact.module.source);
}
