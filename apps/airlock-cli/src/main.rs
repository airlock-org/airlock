//! Thin command entry point. Backend authority stays behind existing qrexec policy.
use std::{
    env,
    os::unix::process::CommandExt,
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    let mut args = env::args_os();
    let program = args.next().unwrap_or_else(|| "airlock".into());
    match args.next() {
        Some(command) if command == "__nix-submit" => {
            airlock_nix::runtime::client_main_from(std::iter::once(program).chain(args))
        }
        Some(command) if command == "machine" => {
            // Keep the mature machine command parser and maintenance operations.
            // Nix supplies an absolute helper path; this never invokes a shell.
            let helper = env::var_os("AIRLOCK_MACHINE_CLI")
                .unwrap_or_else(|| "/run/current-system/sw/bin/airlock-machine-cli".into());
            let error = Command::new(helper).arg("machine").args(args).exec();
            eprintln!("airlock: could not start machine command helper: {error}");
            ExitCode::FAILURE
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
        "Airlock command line interface\n\nUsage: airlock <COMMAND>\n\nCommands:\n  machine  Configuration, deployment and maintenance (see machine --help)\n\nOptions:\n  -h, --help     Show help\n  -V, --version  Show version"
    );
}
