//! Uses a Rust enum to require both TLS credentials and exclude them from plain connections.
//! Conversion emits the chosen record shape under a fictional `demo` tree, with credential
//! paths as strings.
//!
//! ```nix
//! demo.transport = {
//!   tls = true;
//!   certificate = "/run/keys/service.pem";
//!   privateKey = "/run/keys/service.key";
//! };
//! ```

use rusix::{IntoRusixValue, RusixValue};

/// An ordinary Rust sum type: credentials exist only in the TLS alternative.
/// This shape cannot express disabled TLS with credentials or TLS missing a key.
pub enum Transport {
    /// Carries no credentials, so a plain connection cannot accidentally retain them.
    Plain,
    /// Requires both credential paths; file existence and contents are not checked in Rust.
    Tls {
        /// Identifies the public certificate file, distinct from the private key.
        certificate: Certificate,
        /// Identifies the matching private key file; cannot be replaced by a Certificate.
        private_key: PrivateKey,
    },
}

/// A reusable certificate-path type whose inner string is preserved during lowering.
#[derive(IntoRusixValue)]
pub struct Certificate(
    /// Certificate filename emitted as Nix text without reading the file.
    pub String,
);

/// A separate key-path type; Rust prevents mixing it up with a Certificate.
#[derive(IntoRusixValue)]
pub struct PrivateKey(
    /// Private-key filename emitted as Nix text without reading the file.
    pub String,
);

/// Places one complete transport choice under demo.transport in the generated Nix record.
/// The transport conversion decides which credential fields accompany that choice.
#[rusix::config]
mod config {
    use super::{Certificate, PrivateKey, Transport};

    /// Places this service model under the fictional `demo` namespace.
    #[rusix(root)]
    pub struct Root {
        /// The complete service contribution; nested fields determine its Nix shape.
        pub demo: ServiceConfig,
    }

    /// A component built from one valid transport alternative.
    pub struct ServiceConfig {
        /// Requires a complete Transport rather than independent TLS flags and credentials.
        pub transport: Transport,
    }

    pub fn model() -> Root {
        Root {
            demo: ServiceConfig {
                transport: Transport::Tls {
                    certificate: Certificate("/run/keys/service.pem".into()),
                    private_key: PrivateKey("/run/keys/service.key".into()),
                },
            },
        }
    }
}

pub use config::{Root, ServiceConfig, model};

impl IntoRusixValue for Transport {
    fn into_value(self) -> RusixValue {
        // This custom mapping chooses the Nix shape for each Rust alternative:
        // Plain emits { tls = false; }; Tls adds both credential paths with tls = true.
        #[derive(IntoRusixValue)]
        struct Plain {
            /// Disabled TLS flag; this record shape omits both credential paths.
            tls: bool,
        }

        #[derive(IntoRusixValue)]
        struct Tls {
            /// Enabled TLS flag emitted together with both credential paths.
            tls: bool,
            /// Certificate filename emitted as a Nix string without reading the file.
            certificate: Certificate,
            // Private-key filename emitted as `privateKey` without reading the file.
            private_key: PrivateKey,
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

fn main() {
    // Rust requires both credentials for TLS; conversion emits them beside tls = true.
    // The credential paths become Nix strings without reading the files.
    let generated = rusix::compile(model()).unwrap();

    // Emit configuration data. Whether the referenced files exist is a separate check.
    println!("{}", generated.source);
}
