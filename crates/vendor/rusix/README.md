<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/rndhouse/rusix/master/docs/assets/rusix-wordmark-dark.svg">
    <img alt="Rusix" src="https://raw.githubusercontent.com/rndhouse/rusix/master/docs/assets/rusix-wordmark-light.svg" height="64">
  </picture>
</p>

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

A shared Rust model can express the SSH access choices used by your application
and generate settings for NixOS's existing OpenSSH service. This excerpt omits
`ssh_module`, your adapter from the policy to a `NixosModule`.

```rust
use rusix::nixos::compile_module;

/// Callers choose disabled or key-based access; enabling SSH requires a port.
enum SshAccess {
    Disabled,
    KeysOnly {
        /// Port on which SSH accepts connections.
        port: u16,
    },
}

fn main() {
    let module = ssh_module(SshAccess::KeysOnly { port: 2222 });
    let artifact = compile_module(&module).expect("valid SSH configuration");
    println!("{}", artifact.module.source);
}
```

`SshAccess` has no dependency on Rusix: the same type can drive a configuration
UI, deployment planning, and NixOS generation. The adapter translates each choice
into settings, including disabling password and keyboard-interactive login for
`KeysOnly`. You can test that translation with Cargo.

The generated module can be imported alongside modules written in Nix. It uses
the existing OpenSSH option declarations and implementation; your configuration
supplies users and authorized keys. NixOS validates and merges all contributions,
which can affect the final settings. Compilation itself does not run Nix or
change the system.

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
