//! One short-lived Qubes RPC process; the assistant uses the separate `airlock` CLI.

use std::process::ExitCode;

fn main() -> ExitCode {
    if std::env::args_os().len() != 1 {
        eprintln!("airlock-diagnostics: this endpoint accepts no arguments");
        return ExitCode::FAILURE;
    }
    match airlock_diagnostics::runtime::serve() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("airlock-diagnostics: {error:#}");
            ExitCode::FAILURE
        }
    }
}
