//! Compile an editable Rust candidate into a separate Nix build artifact.

use airlock_config::{model, runtime::Candidate};
use std::path::Path;

mod machine;

/// Emit the configuration selected by this checkout; Nix builds and deployment happen separately.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let candidate = Candidate::new(machine::machine()?)?;
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    match arguments.as_slice() {
        [command, directory] if command == "--emit" => candidate.export(Path::new(directory)),
        [command, cache] if command == "prepare" => {
            let tree = airlock_config::preparation::prepare(
                &std::env::current_dir()?,
                Path::new(cache),
                |tree| candidate.export(tree),
            )?;
            println!("path:{path}", path = tree.display());
            Ok(())
        }
        _ => Err("usage: airlock-runtime-config prepare CACHE | --emit DIRECTORY".into()),
    }
}
