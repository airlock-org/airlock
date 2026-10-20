//! Builds a service command that follows the final merged value of `services.example.port` in
//! NixOS.
//! The fictional service needs separate option declarations; Rust's OptionRef generates a
//! reference without reading the port.

use rusix::nixos::{NixosModule, OptionRef};

/// Places service settings and a dependent startup command in their NixOS option paths.
/// The command can refer to the final port that NixOS resolves after merging definitions.
#[rusix::config]
mod config {
    use rusix::Expr;

    /// A rooted contribution combining service inputs with a dependent systemd command.
    /// Nested structs determine the Nix attribute paths; this is an ordinary Rust model.
    #[rusix(root)]
    pub struct ExampleService {
        // Defines the example service's input options.
        services: Services,
        // Defines a systemd command that can depend on a final merged input.
        systemd: Systemd,
    }

    // Places this configuration's service under the `services` namespace.
    struct Services {
        /// Input options for the fictional service, emitted under services.example.
        example: ExampleOptions,
    }

    // Concrete service inputs supplied before lowering.
    struct ExampleOptions {
        // Emits a concrete enable definition, unlike the deferred port dependency.
        enable: bool,
    }

    struct Systemd {
        /// Named service units contributed under the NixOS systemd.services namespace.
        services: Units,
    }

    struct Units {
        /// Service unit whose startup command follows the final services.example.port option.
        example: Unit,
    }

    // Separates a unit's systemd properties from its NixOS wrapper.
    struct Unit {
        // Automatically becomes `serviceConfig` under the default naming rule.
        service_config: ServiceConfig,
    }

    // Systemd properties use PascalCase instead of the default lowerCamelCase.
    #[rusix(rename_all = "PascalCase")]
    struct ServiceConfig {
        // Expr<String> stays deferred and becomes `ExecStart`; it is not a Rust string.
        exec_start: Expr<String>,
    }

    // The command stays an Expr, so lowering preserves its symbolic dependency.
    pub fn service(command: Expr<String>) -> ExampleService {
        ExampleService {
            services: Services {
                example: ExampleOptions { enable: true },
            },
            systemd: Systemd {
                services: Units {
                    example: Unit {
                        service_config: ServiceConfig {
                            exec_start: command,
                        },
                    },
                },
            },
        }
    }
}

pub use config::service;

fn main() {
    // This command is computed entirely in Rust, so its port is already fixed.
    let concrete_port = 5432_i64;
    println!("Concrete Rust command: example --port={concrete_port}");

    // This command refers to config.services.example.port in Nix.
    // NixOS resolves the port after merging all definitions, including later overrides.
    // i64 declares our expected type; the actual NixOS option still needs a declaration.
    let port = OptionRef::<i64>::new("services.example.port");
    let command = port.into_expr().to_text().with_prefix("example --port=");
    let service = service(command);

    // Add the service's settings as one contribution to a larger NixOS configuration.
    // Other modules can provide the port or override it using NixOS priority rules.
    let module = NixosModule::empty().add(service);

    // Generate a module whose command follows the final port when NixOS evaluates it.
    // Printing the source does not start the service or evaluate the port.
    let artifact = rusix::nixos::compile_module(&module).unwrap();
    println!("{}", artifact.module.source);
}
