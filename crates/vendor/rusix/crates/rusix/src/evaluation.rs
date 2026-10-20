//! Run Nix evaluation in a temporary workspace without using the host’s store.
//!
//! Nix stores build recipes and outputs in a *store*. Each [`NixSession`] selects
//! its own disposable local store and fixed offline options. Callers cannot
//! substitute another store or supply arbitrary Nix command flags.
use crate::{Diagnostic, DiagnosticKind, Generated, compiler::render::quote};
use std::{
    fs, io,
    path::Path,
    process::{Command, Output},
    sync::{Mutex, OnceLock},
};
use tempfile::TempDir;

/// A temporary workspace for asking Nix to evaluate generated expressions.
///
/// Nix’s store contains build recipes and outputs. Each session uses its own
/// disposable store, configuration and cache directories, so evaluation does not
/// use the host/default store. Methods return JSON-compatible results or Rust-facing
/// errors; they do not build packages, install software or activate a system.
///
/// Evaluation is offline. Loading the result of an unbuilt derivation as Nix code
/// (*import from derivation*) is disabled. Every Nix subprocess selects the session’s
/// store explicitly and removes host-selection environment settings.
/// Dropping the session deletes its workspace; save artifacts or diagnostics
/// elsewhere if they must remain available.
/// Calls sharing a session are serialized across staging, parsing and evaluation.
pub struct NixSession {
    /// Temporary directory containing the session’s local store and evaluator files.
    disposable: TempDir,
    /// Lock keeping staging, parsing and evaluation in this disposable workspace sequential.
    evaluation: Mutex<()>,
    /// Marker set after the checked minimal nixpkgs files have been staged for this session.
    pub(crate) pinned_staged: OnceLock<()>,
    /// Retained checked nixpkgs checkout used for full-package interoperability in this session.
    pub(crate) full_source:
        std::sync::OnceLock<std::sync::Arc<crate::compiler::interop::FullSource>>,
}

/// The JSON-compatible result of evaluating a Nix expression.
/// It includes Nix’s original diagnostic output for inspection. A package or
/// function cannot necessarily be serialized directly; select suitable data
/// such as a package’s name before requesting JSON output.
#[derive(Debug)]
pub struct Evaluation {
    /// Selected Nix result serialized as JSON; unsupported Nix values may fail serialization.
    pub value: serde_json::Value,
    /// Original parser/evaluator stderr, including warnings and structured trace events.
    pub raw_nix: String,
}

impl NixSession {
    /// Create a fresh workspace and isolated directories without launching Nix.
    /// Fails if temporary filesystem setup cannot be completed.
    pub fn new() -> io::Result<Self> {
        let disposable = tempfile::Builder::new().prefix("rusix-").tempdir()?;

        for directory in ["store", "config", "cache", "state", "home"] {
            fs::create_dir(disposable.path().join(directory))?;
        }

        Ok(Self {
            disposable,
            evaluation: Mutex::new(()),
            pinned_staged: OnceLock::new(),
            full_source: std::sync::OnceLock::new(),
        })
    }

    /// Create an isolated session using an existing local nixpkgs checkout.
    /// Use this constructor when depending on Rusix outside its repository.
    /// The checkout must be clean and match [`crate::nixos::NIXPKGS_REVISION`].
    /// Verification reads local files and Git metadata; it never downloads sources
    /// or launches Nix. Keep the checkout unchanged while the session is alive.
    /// Both minimal NixOS evaluation and full interoperability use this source.
    pub fn with_nixpkgs(path: impl AsRef<Path>) -> Result<Self, Box<Diagnostic>> {
        let source = crate::compiler::interop::FullSource::checked(path.as_ref())?;
        let session = Self::new().map_err(|e| Diagnostic::tooling(e.to_string()))?;
        let _ = session.full_source.set(source);
        Ok(session)
    }

    /// Directory for staging local fixtures/drivers; it is deleted when this session drops.
    pub fn root(&self) -> &Path {
        self.disposable.path()
    }

    // Private; store selection is structural and not an optional caller flag.
    fn command(&self, parse: bool) -> Command {
        let store = self.root().join("store");
        assert!(store.is_absolute() && store.starts_with(self.root()));

        let mut command = Command::new(if parse { "nix-instantiate" } else { "nix" });
        command.arg("--store").arg(&store).args([
            "--extra-experimental-features",
            "nix-command",
            "--log-format",
            "internal-json",
            "--show-trace",
            "--option",
            "substituters",
            "",
            "--option",
            "builders",
            "",
            "--option",
            "build-users-group",
            "",
            "--option",
            "allow-import-from-derivation",
            "false",
        ]);

        // Do not inherit host store selection, remote builders, includes, daemon
        // configuration, or the user's Nix cache/state directories.
        command
            .env_remove("NIX_REMOTE")
            .env_remove("NIX_STORE_DIR")
            .env_remove("NIX_STATE_DIR")
            .env_remove("NIX_LOG_DIR")
            .env_remove("NIX_DAEMON_SOCKET_PATH")
            .env_remove("NIXOS_LABEL")
            .env_remove("NIXOS_LABEL_VERSION")
            .env("NIX_CONFIG", "")
            .env("NIX_PATH", "")
            .env("NIX_CONF_DIR", self.root().join("config"))
            .env("NIX_USER_CONF_FILES", self.root().join("config/nix.conf"))
            .env("XDG_CONFIG_HOME", self.root().join("config"))
            .env("XDG_CACHE_HOME", self.root().join("cache"))
            .env("XDG_STATE_HOME", self.root().join("state"))
            .env("XDG_DATA_HOME", self.root().join("state"))
            .env("HOME", self.root().join("home"))
            .env("NO_COLOR", "1")
            .env("TERM", "dumb");

        command
    }

    /// Query the installed Nix version through the same explicit disposable-store boundary.
    pub fn version(&self) -> io::Result<String> {
        let output = self.command(false).arg("--version").output()?;
        if !output.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().into())
    }

    /// Evaluate generated Nix and convert its complete result to JSON.
    /// Conversion causes Nix to evaluate all values needed for that result, so
    /// a failing nested value can make the whole request fail. Use
    /// [`Self::evaluate_attribute`] to select one field, or [`Self::evaluate_interop`]
    /// when the expression needs pinned nixpkgs inputs. Invalid generated syntax
    /// or static bindings are reported as compiler failures.
    pub fn evaluate(&self, generated: &Generated) -> Result<Evaluation, Box<Diagnostic>> {
        self.with_evaluation_lock(|| self.evaluate_staged(generated))
    }

    /// Evaluate one top-level field of the generated Nix result as JSON.
    /// Other fields are left unevaluated unless the selected value depends on them.
    /// `attribute` is one literal name: dots are not nested traversal, and names
    /// that resemble command flags are escaped as data.
    pub fn evaluate_attribute(
        &self,
        generated: &Generated,
        attribute: &str,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        if attribute.contains('\0') {
            return Err(Diagnostic::tooling("NUL is not supported in attribute selection").into());
        }
        self.with_evaluation_lock(|| self.evaluate_selection(generated, Some(attribute)))
    }

    // Public evaluation entry points hold this lock through all input staging and
    // subprocesses. Internal workers must not call a public entry point recursively.
    pub(crate) fn with_evaluation_lock<T>(
        &self,
        evaluate: impl FnOnce() -> Result<T, Box<Diagnostic>>,
    ) -> Result<T, Box<Diagnostic>> {
        let _guard = self
            .evaluation
            .lock()
            .map_err(|_| Diagnostic::tooling("evaluation session lock is poisoned"))?;
        evaluate()
    }

    /// Evaluate while the caller holds the session lock and has staged its inputs.
    pub(crate) fn evaluate_staged(
        &self,
        generated: &Generated,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        self.evaluate_selection(generated, None)
    }

    fn evaluate_selection(
        &self,
        generated: &Generated,
        attribute: Option<&str>,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        let file = self.root().join("generated.nix");
        let parse_stderr = self.validate_generated(generated, "generated.nix")?;
        let output = self.run(&file, false, attribute)?;
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if !output.status.success() {
            return Err(Diagnostic::from_nix(
                DiagnosticKind::NixEval,
                &format!("{parse_stderr}{stderr}"),
                generated,
                &file,
            )
            .into());
        }

        let value = serde_json::from_slice(&output.stdout).map_err(|e| {
            let mut diagnostic = Diagnostic::tooling(format!("Nix returned invalid JSON: {e}"));
            diagnostic.raw_nix = stderr.clone();
            diagnostic
        })?;

        Ok(Evaluation {
            value,
            raw_nix: format!("{parse_stderr}{stderr}"),
        })
    }

    pub(crate) fn validate_generated(
        &self,
        generated: &Generated,
        name: &str,
    ) -> Result<String, Box<Diagnostic>> {
        let file = self.root().join(name);
        fs::write(&file, &generated.source).map_err(|e| Diagnostic::tooling(e.to_string()))?;

        // A true parse-only subprocess. `nix eval --apply 'x: true'` still forces
        // the file in Nix 2.34.8 and cannot distinguish syntax from user errors.
        let parse = self.run(&file, true, None)?;
        let parse_stderr = String::from_utf8_lossy(&parse.stderr);

        if !parse.status.success() {
            let diagnostic =
                Diagnostic::from_nix(DiagnosticKind::Compiler, &parse_stderr, generated, &file);
            // Recognize parser/static-binding failures narrowly. Store startup
            // or CLI failures remain tooling errors, not invented compiler bugs.
            let kind = if diagnostic.reason.contains("syntax error")
                || diagnostic.reason.contains("undefined variable")
                || diagnostic.reason.contains("already defined")
            {
                DiagnosticKind::Compiler
            } else {
                DiagnosticKind::Tooling
            };
            return Err(Diagnostic::from_nix(kind, &parse_stderr, generated, &file).into());
        }

        Ok(parse_stderr.into_owned())
    }

    fn run(
        &self,
        file: &Path,
        parse_only: bool,
        attribute: Option<&str>,
    ) -> Result<Output, Box<Diagnostic>> {
        let mut command = if parse_only {
            self.command(true)
        } else {
            self.eval_command(attribute)
        };
        if parse_only {
            command.arg("--parse").arg(file);
        } else {
            command.arg("--file").arg(file);
        }

        command
            .output()
            .map_err(|e| Diagnostic::tooling(format!("cannot execute Nix subprocess: {e}")).into())
    }

    fn eval_command(&self, attribute: Option<&str>) -> Command {
        let mut command = self.command(false);
        command
            .args(["--offline", "eval", "--json", "--eval-store"])
            .arg(self.root().join("store"));

        if let Some(attribute) = attribute {
            // Attribute names are escaped Nix string data, never CLI flags or
            // raw source. Store selection cannot be replaced by a selection.
            command.arg("--apply").arg(format!(
                "value: builtins.getAttr {} value",
                quote(attribute)
            ));
        }

        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn every_command_explicitly_selects_the_owned_disposable_store() {
        let session = NixSession::new().unwrap();

        for command in [session.command(true), session.command(false)] {
            let args: Vec<_> = command.get_args().collect();
            assert_eq!(args[0], "--store");
            assert_eq!(Path::new(args[1]), session.root().join("store"));
            for key in [
                "NIX_REMOTE",
                "NIX_STORE_DIR",
                "NIX_STATE_DIR",
                "NIX_DAEMON_SOCKET_PATH",
                "NIXOS_LABEL",
                "NIXOS_LABEL_VERSION",
            ] {
                assert!(
                    command
                        .get_envs()
                        .any(|(name, value)| name == key && value.is_none())
                );
            }
        }

        let command = session.eval_command(Some("good"));
        let args: Vec<_> = command.get_args().collect();
        for flag in ["--store", "--eval-store"] {
            assert_eq!(
                args.iter().filter(|arg| **arg == OsStr::new(flag)).count(),
                1
            );
            let index = args
                .iter()
                .position(|arg| *arg == OsStr::new(flag))
                .unwrap();
            let store = Path::new(args[index + 1]);
            assert_eq!(store, session.root().join("store"));
            assert!(store.is_absolute());
            assert_ne!(store, Path::new("/nix/store"));
        }

        assert!(args.windows(2).any(|pair| pair
            == [
                OsStr::new("--apply"),
                OsStr::new("value: builtins.getAttr \"good\" value")
            ]));
        assert!(args.windows(3).any(|v| v
            == [
                OsStr::new("--option"),
                OsStr::new("substituters"),
                OsStr::new("")
            ]));
        assert!(args.windows(3).any(|v| v
            == [
                OsStr::new("--option"),
                OsStr::new("allow-import-from-derivation"),
                OsStr::new("false")
            ]));
        assert!(
            command
                .get_envs()
                .any(|(key, value)| key == "NIX_CONFIG" && value == Some(OsStr::new("")))
        );

        let root = session.root().to_owned();
        drop(session);
        assert!(!root.exists());
    }
}
