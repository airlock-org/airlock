//! Keeps ports, user IDs and names distinct in Rust while emitting primitive values in a
//! fictional `demo` tree.
//! Renaming and flattening place the ownership fields beside the listener's fields in Nix.
//!
//! ```nix
//! demo = { port = 1000; host = "admin"; userId = 1000; owner = "admin"; };
//! ```

use rusix::IntoRusixValue;

/// A listening-port domain value; the inner u16 lowers to a Nix integer.
/// The distinction from UserId is checked by Rust, not by a range validator.
#[derive(IntoRusixValue)]
pub struct Port(
    /// Listening port emitted as an integer in Nix.
    pub u16,
);

/// A username-domain string, distinct from other strings in typed function calls.
#[derive(IntoRusixValue)]
pub struct UserName(
    /// Account name emitted as a Nix string, distinct from a hostname in Rust.
    pub String,
);

/// A Unix-user identity with the same primitive representation as Port.
#[derive(IntoRusixValue)]
pub struct UserId(
    /// User identifier emitted as a Nix integer, distinct from a port in Rust.
    pub u16,
);

/// Accepts only the port domain; `listen(UserId(1000))` cannot compile.
pub fn listen(port: Port) -> Port {
    port
}

/// Defines the demo listener's Nix fields, including renamed and flattened ownership values.
/// Rust domain types become primitive values while the structs determine their placement.
#[rusix::config]
mod config {
    use super::{Port, UserId, UserName, listen};

    // Local host-domain string; the module macro supplies its value conversion.
    struct Hostname(
        /// Hostname preserved as a Nix string; Rust keeps it separate from usernames.
        String,
    );

    // Keeps ownership information typed before placing it in the configuration tree.
    struct Identity {
        // Becomes `userId`, but cannot be passed to an API expecting Port in Rust.
        user_id: UserId,
        // This semantic rename calls the lowered username `owner`.
        #[rusix(rename = "owner")]
        name: UserName,
    }

    // A listener uses a port and a host, with ownership fields flattened beside them.
    struct Listener {
        // Requires a port-domain value even though UserId also contains a u16.
        port: Port,
        /// Hostname emitted beside the port as a Nix string.
        host: Hostname,
        // Put Identity's fields beside port and host in the generated Nix record.
        #[rusix(flatten)]
        owner: Identity,
    }

    /// Places the listener in the fictional `demo` option tree.
    #[rusix(root)]
    pub struct Root {
        /// Listener fields emitted under demo, with user identity fields flattened beside them.
        demo: Listener,
    }

    pub fn model() -> Root {
        Root {
            demo: Listener {
                port: listen(Port(1000)),
                host: Hostname("admin".into()),
                owner: Identity {
                    user_id: UserId(1000),
                    name: UserName("admin".into()),
                },
            },
        }
    }
}

pub use config::model;

fn main() {
    // Rust keeps ports, user IDs and names distinct; Nix receives integers and strings.
    // Conversion flattens the ownership fields into demo beside the listener fields.
    let generated = rusix::compile(model()).unwrap();

    // Emit configuration data; generating this source does not create a user or listener.
    println!("{}", generated.source);
}
