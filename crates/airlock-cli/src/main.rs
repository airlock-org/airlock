//! Thin command entry point. Backend authority stays behind existing qrexec policy.
mod artifact;

mod builder_store;

mod inspect;

mod machine;

mod submit;

use std::{env, process::ExitCode};

fn main() -> ExitCode {
    let mut args = env::args_os();
    let _program = args.next();

    match args.next() {
        Some(command) if command == "builder-store" => {
            builder_store::main_from(std::iter::once("airlock builder-store".into()).chain(args))
        }
        Some(command) if command == "artifact" => {
            artifact::main_from(std::iter::once("airlock artifact".into()).chain(args))
        }
        Some(command) if command == "inspect" => {
            inspect::main_from(std::iter::once("airlock inspect".into()).chain(args))
        }
        Some(command) if command == "machine" => {
            machine::main_from(std::iter::once("airlock machine".into()).chain(args))
        }
        Some(command) if command == "--version" || command == "-V" => {
            println!("airlock {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(command) if command == "--help" || command == "-h" => {
            help();
            ExitCode::SUCCESS
        }
        None => {
            help();
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("airlock: unknown command {other:?}; use --help");
            ExitCode::from(2)
        }
    }
}

fn help() {
    println!(
        "{}\n\nUsage: airlock <COMMAND>\n\nCommands:\n  builder-store Mount or stop the guest builder store\n  artifact Transfer a frozen Nix configuration through dom0 admission\n  inspect  Read-only local record and lock inspection (see inspect --help)\n  machine  Configuration, deployment and maintenance (see machine --help)\n\nOptions:\n  -h, --help     Show help\n  -V, --version  Show version",
        env!("CARGO_PKG_DESCRIPTION")
    );
}
