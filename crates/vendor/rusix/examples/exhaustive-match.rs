//! Uses one Rust enum to compute firewall and service choices in a fictional `demo` tree.
//! Adding a variant requires updating both Rust matches before Nix source can be generated.
//!
//! ```nix
//! demo = { mode = "client"; firewall.allowedPorts = []; service.acceptsConnections = false; };
//! ```

/// Computes firewall and service choices from one Rust mode and places them under demo in Nix.
/// Both policies are decided in Rust before their resulting fields are emitted.
#[rusix::config]
mod config {
    /// One shared model choice; Rusix automatically lowers unit variants to strings.
    pub enum Mode {
        /// A server needs both inbound firewall access and a listening service.
        Server,
        /// A client needs neither inbound access nor a listening service.
        Client,
    }

    /// An ordinary Rust port type; its inner number lowers to a Nix integer.
    pub struct Port(
        /// Listening port emitted as an integer in Nix.
        pub u16,
    );

    /// The firewall decision computed from the model before any Nix evaluation.
    pub struct FirewallPolicy {
        /// Ports selected by Rust policy; order is preserved in the lowered list.
        pub allowed_ports: Vec<Port>,
    }

    /// A separate consumer of the same model choice.
    pub struct ServicePolicy {
        /// A concrete decision to allow incoming connections.
        pub accepts_connections: bool,
    }

    // No wildcard: a new mode must have an explicit policy in both functions.
    pub fn firewall_policy(mode: &Mode) -> FirewallPolicy {
        match mode {
            Mode::Server => FirewallPolicy {
                allowed_ports: vec![Port(443)],
            },
            Mode::Client => FirewallPolicy {
                allowed_ports: vec![],
            },
        }
    }

    pub fn service_policy(mode: &Mode) -> ServicePolicy {
        match mode {
            Mode::Server => ServicePolicy {
                accepts_connections: true,
            },
            Mode::Client => ServicePolicy {
                accepts_connections: false,
            },
        }
    }

    // Collects the model choice and both decisions under one fictional option tree.
    struct Policies {
        /// Chosen mode emitted as server or client beside the policies derived from it.
        mode: Mode,
        /// Inbound ports computed in Rust and emitted under demo.firewall.
        firewall: FirewallPolicy,
        /// Connection behavior computed in Rust and emitted under demo.service.
        service: ServicePolicy,
    }

    /// A rooted contribution; the local structs supply automatic structural lowering.
    #[rusix(root)]
    pub struct Root {
        // Keeps both Rust-computed policies under the fictional demo namespace.
        demo: Policies,
    }

    pub fn model() -> Root {
        let mode = Mode::Client;

        Root {
            demo: Policies {
                firewall: firewall_policy(&mode),
                service: service_policy(&mode),
                mode,
            },
        }
    }
}

pub use config::{Mode, firewall_policy, model, service_policy};

fn main() {
    // Rust computes both policies from the same mode before generating Nix.
    // Nix receives their resulting fields, not the Rust enum or its match expressions.
    let generated = rusix::compile(model()).unwrap();

    // Emit configuration data; printing firewall settings does not apply them.
    println!("{}", generated.source);
}
