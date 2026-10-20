//! Declarative inert command replies keep process fixtures out of production binaries and shell code.

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Resolve a fixture-relative file while refusing traversal outside the temporary directory.
fn local(root: &Path, name: &str) -> Result<PathBuf> {
    let path = Path::new(name);
    ensure!(
        path.components()
            .all(|part| matches!(part, std::path::Component::Normal(_))),
        "peer file must be relative to its fixture"
    );
    Ok(root.join(path))
}

/// Match literal command arguments and optional marker state without interpreting executable text.
fn matches(root: &Path, rule: &Value, args: &[String]) -> Result<bool> {
    if let Some(environment) = rule["env"].as_object() {
        for (key, value) in environment {
            if std::env::var(key).ok().as_deref() != value.as_str() {
                return Ok(false);
            }
        }
    }
    if let Some(count) = rule["argc"].as_u64()
        && args.len() as u64 != count
    {
        return Ok(false);
    }
    if let Some(argument) = rule["contains"].as_str()
        && !args.iter().any(|value| value == argument)
    {
        return Ok(false);
    }
    for (field, prefix) in [("args", false), ("prefix", true)] {
        if let Some(expected) = rule.get(field) {
            let expected: Vec<&str> = expected
                .as_array()
                .context("peer arguments")?
                .iter()
                .map(|v| v.as_str().context("peer argument"))
                .collect::<Result<_>>()?;
            if !args.iter().map(String::as_str).eq(expected.iter().copied())
                && !(prefix
                    && args
                        .iter()
                        .map(String::as_str)
                        .take(expected.len())
                        .eq(expected.iter().copied()))
            {
                return Ok(false);
            }
        }
    }
    for (field, wanted) in [("exists", true), ("missing", false)] {
        if let Some(name) = rule.get(field)
            && local(root, name.as_str().context("peer marker")?)?.exists() != wanted
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Record one invocation and reply from finite scenario data, failing on unexpected commands.
pub(super) fn execute(specification: &Path, name: &str, args: &[String]) -> Result<()> {
    let root = specification.parent().context("peer fixture root")?;
    let specification: Value = serde_json::from_slice(&fs::read(specification)?)?;
    let mut calls = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("calls.jsonl"))?;
    writeln!(calls, "{}", json!({"command":name,"args":args}))?;

    if name == "airlock-dom0-service"
        && args.first().is_some_and(|arg| arg == "setup-config")
        && args.get(1).is_some_and(|arg| {
            matches!(
                arg.as_str(),
                "property-explicit" | "configure-domain" | "finalize" | "run" | "policy"
            )
        })
    {
        let native = specification["native_dom0"]
            .as_str()
            .context("native dom0 executable")?;
        let status = std::process::Command::new(native).args(args).status()?;
        std::process::exit(status.code().unwrap_or(1));
    }

    let rules = specification["commands"][name]
        .as_array()
        .with_context(|| format!("no peer for {name}"))?;
    for rule in rules {
        if !matches(root, rule, args)? {
            continue;
        }
        if let Some(required) = rule.get("require_record") {
            let directory = local(
                root,
                required["directory"]
                    .as_str()
                    .context("required record directory")?,
            )?;
            let field = required["field"]
                .as_str()
                .context("required record field")?;
            let mut found = false;
            for entry in fs::read_dir(directory)? {
                let entry = entry?;
                if entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
                {
                    let value: Value = serde_json::from_slice(&fs::read(entry.path())?)?;
                    found |= value[field] == required["value"];
                }
            }
            ensure!(
                found,
                "required record was not durable before helper execution"
            );
        }
        if let Some(writes) = rule.get("write") {
            for (name, value) in writes.as_object().context("peer writes")? {
                let path = local(root, name)?;
                fs::create_dir_all(path.parent().context("peer file parent")?)?;
                fs::write(path, value.as_str().context("peer file contents")?)?;
            }
        }
        if let Some(milliseconds) = rule["delay_ms"].as_u64() {
            std::thread::sleep(std::time::Duration::from_millis(milliseconds));
        }

        if let Some(file) = rule["stdout_file"].as_str() {
            std::io::stdout().write_all(&fs::read(local(root, file)?)?)?;
        } else if let Some(text) = rule["stdout"].as_str() {
            print!("{text}");
        } else if rule["echo_args"] == true {
            println!("{arguments}", arguments = args.join(" "));
        }
        if let Some(text) = rule["stderr"].as_str() {
            eprint!("{text}");
        }
        std::process::exit(rule["exit"].as_i64().unwrap_or(0).try_into()?);
    }
    bail!("unexpected fixture command: {name} {args:?}")
}
