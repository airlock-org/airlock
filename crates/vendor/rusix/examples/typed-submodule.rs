//! Reuses a typed Rust endpoint by placing it in a fictional `demo` configuration tree.
//! Nested structs become nested Nix attributes, while single-field wrappers become their
//! primitive values.
//!
//! ```nix
//! demo.endpoint = { host = "service.internal"; port = 443; };
//! ```

use rusix::IntoRusixValue;

/// An ordinary Rust domain type; its inner value lowers to a Nix string.
/// This separates names from other strings without adding hostname validation.
#[derive(IntoRusixValue)]
pub struct Hostname(
    /// Hostname preserved as text in the generated Nix value.
    pub String,
);

/// A distinct Rust type for a listening port; its inner value lowers to an integer.
#[derive(IntoRusixValue)]
pub struct Port(
    /// Listening port emitted as an integer in Nix.
    pub u16,
);

/// A reusable record that can appear wherever a parent places it.
#[derive(IntoRusixValue)]
pub struct Endpoint {
    /// Requires a hostname-domain value rather than an arbitrary domain string.
    pub host: Hostname,
    /// Requires a Port; another integer-backed domain type will not compile here.
    pub port: Port,
}

/// Places the reusable endpoint under demo.endpoint in the generated Nix attribute set.
/// Parent structs define its location while the endpoint retains its own value conversion.
#[rusix::config]
mod config {
    use super::{Endpoint, Hostname, Port};

    /// Places the reusable Endpoint in this configuration's attribute tree.
    #[rusix(root)]
    pub struct Root {
        /// Fictional demo namespace where the reusable endpoint becomes nested Nix fields.
        demo: Demo,
    }

    // Structural placement belongs to the parent, not to the reusable value.
    struct Demo {
        // Produces `demo.endpoint.host` and `demo.endpoint.port`.
        endpoint: Endpoint,
    }

    pub fn model() -> Root {
        Root {
            demo: Demo {
                endpoint: Endpoint {
                    host: Hostname("service.internal".into()),
                    port: Port(443),
                },
            },
        }
    }
}

pub use config::model;

fn main() {
    // The parent structs place the endpoint at demo.endpoint in the Nix attribute set.
    // Hostname and Port become a string and an integer, rather than nested wrappers.
    let generated = rusix::compile(model()).unwrap();

    // Emit configuration data; using it with NixOS needs declarations for the demo options.
    println!("{}", generated.source);
}
