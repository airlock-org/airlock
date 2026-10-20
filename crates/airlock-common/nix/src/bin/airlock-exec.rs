//! Execute a fixed command contract installed beside this copied native launcher.
//! Arguments cannot select another contract; privileged installations own both files.

use serde::Deserialize;
use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    fs,
    os::unix::process::CommandExt,
    process::{Command, ExitCode},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Contract {
    /// Executable selected by the trusted package definition.
    program: String,
    /// Compatibility executable override exposed only by administrative tool packages.
    program_env: Option<String>,
    /// Fixed arguments prepended to caller arguments; @self@ denotes this installed command.
    #[serde(default)]
    args: Vec<String>,
    /// Exact environment bindings supplied by the package.
    #[serde(default)]
    env: BTreeMap<String, String>,
    /// Colon-separated prefixes retaining a nonempty inherited suffix.
    #[serde(default)]
    prefix_env: BTreeMap<String, String>,
    /// Environment bindings intentionally removed at this boundary.
    #[serde(default)]
    unset_env: Vec<String>,
    /// Whether caller arguments are forwarded after the fixed arguments.
    #[serde(default = "yes")]
    inherit_args: bool,
    /// Optional exact argument count enforced before command execution.
    argument_count: Option<usize>,
    /// Optional authenticated Qubes caller allowlist, including an empty deny-all list.
    callers: Option<Vec<String>>,
    /// Reject a nonempty Qubes RPC argument when this service has no argument protocol.
    #[serde(default)]
    empty_service_argument: bool,
}

/// Forward ordinary CLI arguments unless the package explicitly selects a fixed invocation.
fn yes() -> bool {
    true
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let executable = env::current_exe()?;
    let mut contract_name = executable
        .file_name()
        .ok_or("launcher has no filename")?
        .to_os_string();
    contract_name.push(".json");
    let contract: Contract =
        serde_json::from_slice(&fs::read(executable.with_file_name(contract_name))?)?;
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
