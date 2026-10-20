# Rusix

Rusix is a Rust library for defining packages and NixOS configuration in ordinary
Rust, as an alternative to the Nix language. It compiles these definitions into Nix
expressions that can use existing Nix packages and modules. Existing Nix code can
import and use the generated definitions.

- Model configuration with your own Rust types and compose definitions through
  ordinary Rust functions.
- Generated expressions preserve Nix's lazy evaluation. NixOS handles option
  validation and module merging.
- Source mappings help trace Nix evaluation errors back to the Rust code that
  produced them.

Add Rusix to `Cargo.toml`:

```toml
[dependencies]
rusix = "0.1"
```

```rust
use rusix::{IntoConfig, compile};

#[derive(IntoConfig)]
struct Service {
    /// Whether the service should run.
    enable: bool,
    /// Port on which the service listens.
    port: u16,
}

fn main() {
    let generated = compile(Service { enable: true, port: 8080 })
        .expect("valid configuration");
    println!("{}", generated.source);
}
```

In this example, the compiler turns `Service` into a Nix attribute set with
`enable` and `port` fields.

Use `rusix` to write package and configuration definitions in your own Rust crates.
Definitions can be shared through libraries and tested with Cargo. Tests can
evaluate the generated Nix and check its results.

Rusix can identify both Rust definitions in a NixOS merge conflict. An excerpt
from the `merge-two` fixture:

```text
error[nixos-merge]: conflicting definitions for a NixOS option
  --> crates/rusix/src/bin/cli/merge_fixtures.rs:16:23
   = conflicting definition
  --> crates/rusix/src/bin/cli/merge_fixtures.rs:10:23
   = conflicting definition
   = option: services.openssh.authorizedKeysCommandUser
```

[Error reporting](https://github.com/rndhouse/rusix/blob/master/docs/error-reporting.md) shows complete diagnostics and their
original Nix errors.

Rusix is experimental, and some Nix errors cannot be traced to a Rust source
location. Contextual modules created with `NixosModule::with_context` or
`with_arguments` preserve each `ModuleBody` contribution's location. For opaque
configuration records, diagnostics identify the contribution and the NixOS option
path; they do not infer a Rust source location for each nested field. Imported
Nix definitions retain their own source locations.

- [Examples](https://github.com/rndhouse/rusix/blob/master/examples/README.md)
- [Library usage](https://github.com/rndhouse/rusix/blob/master/docs/library-usage.md)
- [Developing Rusix](https://github.com/rndhouse/rusix/blob/master/docs/development.md)
