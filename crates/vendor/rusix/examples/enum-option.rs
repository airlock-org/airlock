//! Shows how a Rust enum constrains configuration choices before emitting a fictional `demo` tree.
//! Rust computes the connection policy; Rusix maps variant and field names to Nix strings and
//! attributes.
//!
//! ```nix
//! demo = { mode = "server"; acceptsConnections = true; };
//! ```

/// Defines the connection mode and Rust policy emitted under the demo Nix attribute set.
/// Enum variants become strings and nested structs determine the output's fields.
#[rusix::config]
mod config {
    /// A choice defined by this configuration, not a built-in Rusix type.
    /// Rusix maps these unit variants to "server" and "client" automatically.
    pub enum Mode {
        /// Accept incoming connections.
        Server,
        /// Make outgoing connections without accepting incoming ones.
        Client,
    }

    // This is Rust policy, so adding a mode requires updating this match.
    pub fn accepts_connections(mode: &Mode) -> bool {
        match mode {
            Mode::Server => true,
            Mode::Client => false,
        }
    }

    // Groups a typed choice with a decision computed from it before lowering.
    struct ConnectionPolicy {
        /// Typed choice emitted as server or client in the generated Nix record.
        mode: Mode,
        // A concrete Rust decision, not a read of final NixOS configuration.
        accepts_connections: bool,
    }

    /// A rooted contribution whose nested fields define the generated attribute paths.
    #[rusix(root)]
    pub struct Root {
        // Places the policy under the fictional `demo` option tree.
        demo: ConnectionPolicy,
    }

    pub fn model() -> Root {
        let mode = Mode::Server;

        Root {
            demo: ConnectionPolicy {
                accepts_connections: accepts_connections(&mode),
                mode,
            },
        }
    }
}

pub use config::{Mode, accepts_connections, model};

fn main() {
    // Rust chooses the mode and connection policy before Nix is involved.
    // Conversion turns the enum into "server" and the nested structs into Nix fields.
    let generated = rusix::compile(model()).unwrap();

    // Emit configuration data; generating source does not configure a service.
    println!("{}", generated.source);
}
