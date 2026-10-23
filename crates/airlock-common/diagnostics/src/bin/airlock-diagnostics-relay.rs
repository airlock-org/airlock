//! Dom0-only return path for a VM to inspect its own declared diagnostics through Qubes.

use std::process::ExitCode;

fn main() -> ExitCode {
    if std::env::args_os().len() != 1 {
        eprintln!("airlock-diagnostics-relay: this endpoint accepts no arguments");
        return ExitCode::FAILURE;
    }
    match airlock_diagnostics::runtime::relay() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("airlock-diagnostics-relay: {error:#}");
            ExitCode::FAILURE
        }
    }
}
