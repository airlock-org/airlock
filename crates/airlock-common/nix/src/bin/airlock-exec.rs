//! Execute a fixed command contract installed beside this copied native launcher.
//! Arguments cannot select another contract; privileged installations own both files.

use airlock_nix_core::command::Contract;
use std::{
    env,
    ffi::OsString,
    os::unix::process::CommandExt,
    process::{Command, ExitCode},
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let executable = env::current_exe()?;
    let contract = Contract::read(&executable)?;
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    if contract
        .argument_count
        .is_some_and(|count| count != arguments.len())
    {
        return Err("unexpected command arguments".into());
    }
    if let Some(callers) = &contract.callers {
        let caller = env::var("QREXEC_REMOTE_DOMAIN").unwrap_or_default();
        if caller.is_empty() || !callers.contains(&caller) {
            return Err("Qubes caller is not authorized".into());
        }
    }
    if contract.empty_service_argument
        && env::var_os("QREXEC_SERVICE_ARGUMENT").is_some_and(|arg| !arg.is_empty())
    {
        return Err("this Qubes service accepts no argument".into());
    }

    let program = contract
        .program_env
        .as_ref()
        .and_then(env::var_os)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| contract.program.clone().into());
    let mut command = Command::new(program);
    for argument in &contract.args {
        if argument == "@self@" {
            command.arg(&executable);
        } else if argument == "@prefix@" {
            command.arg(
                executable
                    .parent()
                    .and_then(|path| path.parent())
                    .ok_or("launcher has no package prefix")?,
            );
        } else {
            command.arg(argument);
        }
    }
    if contract.inherit_args {
        command.args(arguments);
    }
    command.envs(contract.env);
    for (name, prefix) in contract.prefix_env {
        let mut value = OsString::from(prefix);
        if let Some(suffix) = env::var_os(&name).filter(|value| !value.is_empty()) {
            if !value.is_empty() {
                value.push(":");
            }
            value.push(suffix);
        }
        command.env(name, value);
    }
    for name in contract.unset_env {
        command.env_remove(name);
    }
    Err(command.exec().into())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("airlock-exec: {error}");
            ExitCode::FAILURE
        }
    }
}
