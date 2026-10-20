//! Validate and install only Airlock-owned topology policy; bootstrap transport rules stay separate.

use crate::process::Runner;
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::{fs, os::unix::fs::PermissionsExt, path::Path, time::Duration};

const MARKER: &str = "# Managed by Airlock setup-config.";

/// Check the limited Qubes name grammar used by topology-managed policy endpoints.
fn name(value: &str) -> bool {
    value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
}

/// Render the complete rule set before permitting any policy installation.
fn render(topology: &Value) -> Result<String> {
    let mut output = format!(
        "{MARKER}\n# Source: airlock.qubes.topology.qrexec_policy.\n# Bootstrap config transport policy remains in 30-airlock-config.policy.\n"
    );
    let rules = match topology["qrexec_policy"].get("rules") {
        None | Some(Value::Null) => &[][..],
        Some(value) => value
            .as_array()
            .context("policy rules must be an array")?
            .as_slice(),
    };
    for (index, rule) in rules.iter().enumerate() {
        let field = |key: &str| {
            rule[key]
                .as_str()
                .with_context(|| format!("qrexec policy rule {index} missing {key}"))
        };
        let service = field("service")?;
        let argument = field("argument")?;
        let source = field("source")?;
        let target = field("target")?;
        let action = field("action")?;
        ensure!(
            service
                .strip_prefix("airlock.")
                .is_some_and(|suffix| !suffix.is_empty()
                    && suffix
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))),
            "qrexec policy rule {index} has invalid service: {service}"
        );
        ensure!(
            !matches!(
                service,
                "airlock.Config.CandidateExport"
                    | "airlock.Config.ValidateCandidate"
                    | "airlock.Config.InputsImport"
                    | "airlock.Config.ArchiveCandidate"
                    | "airlock.Config.ArchiveDeployment"
                    | "airlock.Config.Status"
                    | "airlock.Config.Check"
                    | "airlock.Config.Apply"
                    | "airlock.Config.Rollback"
            ),
            "qrexec policy rule {index} uses reserved bootstrap service: {service}"
        );
        ensure!(
            argument == "*"
                || (!argument.is_empty()
                    && argument
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"_.:+-".contains(&byte))),
            "qrexec policy rule {index} has invalid argument: {argument}"
        );
        ensure!(
            source == "@anyvm" || name(source),
            "qrexec policy rule {index} has invalid source: {source}"
        );
        ensure!(
            target == "@anyvm" || name(target),
            "qrexec policy rule {index} has invalid target: {target}"
        );
        ensure!(
            target != "dom0",
            "qrexec policy rule {index} targets dom0; topology-managed policy may not target dom0"
        );
        match action {
            "allow" => ensure!(
                source != "@anyvm" && target != "@anyvm",
                "qrexec policy rule {index} allow uses @anyvm endpoint"
            ),
            "deny" => {}
            _ => bail!("qrexec policy rule {index} has invalid action: {action}"),
        }
        output.push_str(&format!(
            "{service} {argument} {source} {target} {action}\n"
        ));
    }
    Ok(output)
}

/// Replace a changed managed policy and ask the upstream daemon to reload it.
/// The rendered preview is also written in plan mode, which never changes active policy.
pub(super) fn reconcile(
    topology: &Value,
    rendered: &Path,
    target: &Path,
    apply: bool,
) -> Result<bool> {
    let output = render(topology)?;
    fs::write(rendered, &output)?;
    match fs::read_to_string(target) {
        Ok(current) => {
            ensure!(
                current.lines().any(|line| line == MARKER),
                "qrexec policy file exists but is not Airlock-managed: {target}",
                target = target.display()
            );
            if current == output {
                return Ok(false);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    if apply {
        fs::create_dir_all(target.parent().context("policy directory")?)?;
        // Keep the active path whole if writing the replacement is interrupted.
        let mut temporary = tempfile::NamedTempFile::new_in(target.parent().unwrap())?;
        use std::io::Write;
        temporary.write_all(output.as_bytes())?;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o644))?;
        temporary.persist(target).map_err(|error| error.error)?;
        let runner = Runner::default();
        let output = runner.capture(
            "systemctl",
            &["reload".into(), "qubes-qrexec-policy-daemon.service".into()],
            Duration::from_secs(60),
            &[],
        )?;
        if output.limited || !output.status.success() {
            runner.text(
                "systemctl",
                &["try-restart", "qubes-qrexec-policy-daemon.service"],
            )?;
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Reserved transport authority and broad allows cannot enter topology-managed policy.
    #[test]
    fn rejects_authority_expansion() {
        for rule in [
            json!({"service":"airlock.Config.Apply","argument":"*","source":"src","target":"dst","action":"deny"}),
            json!({"service":"airlock.Example","argument":"*","source":"@anyvm","target":"dst","action":"allow"}),
            json!({"service":"airlock.Example","argument":"*","source":"src","target":"dom0","action":"deny"}),
        ] {
            assert!(render(&json!({"qrexec_policy":{"rules":[rule]}})).is_err());
        }
    }

    /// Planning writes a preview; applying refuses to overwrite an unrelated administrator policy.
    #[test]
    fn preview_preserves_unmanaged_policy() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let rendered = directory.path().join("rendered");
        let target = directory.path().join("active");
        assert!(reconcile(&json!({}), &rendered, &target, false)?);
        assert!(!target.exists());
        fs::write(&target, "# administrator policy\n")?;
        assert!(reconcile(&json!({}), &rendered, &target, true).is_err());
        assert_eq!(fs::read_to_string(target)?, "# administrator policy\n");
        Ok(())
    }
}
