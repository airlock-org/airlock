//! Shows PostgreSQL authoring and prints its complete generated NixOS module.
//! Rust supplies both the public option declarations and configuration implementation.
pub(crate) mod lowering;

pub mod model;

mod options;

pub(crate) mod schema;

fn main() {
    // Declare PostgreSQL's options and combine their implementation with our settings.
    // NixOS merges these contributions and resolves defaults when it evaluates the module.
    let module = schema::module()
        .module(lowering::implementation())
        .add(model::model());

    // Generate a NixOS module, which provides settings for a larger configuration.
    // Printing it does not evaluate that configuration or start PostgreSQL.
    let artifact = rusix::nixos::compile_module(&module).unwrap();
    println!("{}", artifact.module.source);
}
