//! Check explicit application installation contracts before constructing NixOS modules.
//!
//! Packages can export executables into a system's command environment, and apps
//! can install files under `/etc`. Those names need exclusive owners within one
//! target system. [`compose`] checks these declarations in Rust and emits their
//! package and file settings from the same data. It does not inspect package
//! contents, evaluate Nix expressions, or change normal NixOS merge rules.
//!
//! Compose all participating apps for a target together. Separate calls are
//! independent: the scope label describes a target, not a global registry.
//! Opaque modules and downstream overrides remain outside this contract.
//!
//! ```
//! use rusix::{installation::{self, Application, CollisionChecks}, interop::Package};
//! use rusix::interop::raw::{NixRepresentation, NixValue};
//!
//! let package = Package::from_expression(NixValue::from("/nix/store/example-app"));
//! let app = Application::new("example-app")
//!     .executable(package, "example-app", [vec!["capture".into()]])
//!     .etc_text("example-app/config.json", "{}");
//! let module = installation::compose("desktop-system", CollisionChecks::Enabled, [app])?;
//! # Ok::<(), installation::InstallationError>(())
//! ```

use crate::interop::raw::NixValue;
use crate::{
    interop::{Package, raw::NixRepresentation},
    nix_record,
    nixos::ModuleBody,
};
use std::{collections::BTreeMap, fmt};

/// An app's package exports and exclusive files, awaiting checked composition.
/// Construct this alongside the package recipe, then pass it to [`compose`].
/// With collision checks enabled, repeated application identities are rejected,
/// including identical definitions; install a shared app once instead of relying
/// on implicit deduplication. Disabled checks preserve these contributions.
#[derive(Clone, Debug)]
pub struct Application {
    /// Stable identity used to attribute conflicts and reject repeated installation.
    owner: String,
    /// Packages and their declared executable command trees.
    executables: Vec<Executable>,
    /// Explicit files installed by the generated `environment.etc` definitions.
    files: Vec<EtcFile>,
    /// Additional contributions whose contents remain outside ownership checking.
    modules: Vec<ModuleBody>,
}

/// One package export and the literal subcommand paths described by its parser.
#[derive(Clone, Debug)]
struct Executable {
    /// Package installed into the target's system environment.
    package: Package,
    /// Basename exported by the package, without a directory component.
    name: String,
    /// Complete subcommand paths, excluding the executable itself.
    commands: Vec<Vec<String>>,
}

/// One exclusive file relative to the target system's `/etc` directory.
#[derive(Clone, Debug)]
struct EtcFile {
    /// Canonical relative destination, validated before module construction.
    path: String,
    /// Literal or deferred text for the explicitly declared destination.
    text: NixValue,
}

impl Application {
    /// Starts an application contract with a stable, nonempty diagnostic identity.
    /// Validation occurs when [`compose`] receives the complete target selection.
    pub fn new(owner: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            executables: Vec::new(),
            files: Vec::new(),
            modules: Vec::new(),
        }
    }

    /// Declares an exported executable and installs its package after validation.
    /// Each command is a nonempty path below this executable, for example
    /// `["screen-capture", "retry"]`. Include parent commands and aliases too.
    /// Derive these paths from the parser's schema to avoid maintaining two lists.
    ///
    /// Names are literal single command words; Nix-computed names cannot be
    /// checked here. Undeclared exports and package contents require build checks.
    /// With checks enabled, exporting an executable twice is a conflict, even
    /// from the same app. Disabled checks preserve both package contributions.
    pub fn executable(
        mut self,
        package: Package,
        name: impl Into<String>,
        commands: impl IntoIterator<Item = Vec<String>>,
    ) -> Self {
        self.executables.push(Executable {
            package,
            name: name.into(),
            commands: commands.into_iter().collect(),
        });
        self
    }

    /// Declares an exclusive text file and emits its NixOS file definition.
    /// Paths must be relative to `/etc`, with no empty, `.` or `..` components.
    /// A file also conflicts with another file beneath its destination.
    /// Deferred text is left to NixOS; the emitted `target` is always this path.
    /// With collision checks disabled, repeated paths remain separate NixOS
    /// definitions, allowing normal text merging and field priorities.
    pub fn etc_text(mut self, path: impl Into<String>, text: impl Into<NixValue>) -> Self {
        self.files.push(EtcFile {
            path: path.into(),
            text: text.into(),
        });
        self
    }

    /// Adds normal NixOS configuration without declaring exclusive ownership.
    /// Use this for settings such as overlays or assertions. Files and package
    /// exports supplied through this escape hatch are outside this contract.
    pub fn module(mut self, module: ModuleBody) -> Self {
        self.modules.push(module);
        self
    }

    /// Emits the already validated installation declarations and additional modules.
    fn into_module(self) -> ModuleBody {
        let packages = self
            .executables
            .into_iter()
            .map(|exe| exe.package.as_expression());
        let installed = ModuleBody::new(nix_record! {
            "environment": nix_record! {
                "systemPackages": NixValue::list(packages),
            },
        });
        let installed = self.files.into_iter().fold(installed, |body, file| {
            let definition = nix_record! { "text": file.text, "target": file.path.clone() };
            body.module(ModuleBody::new(nix_record! {
                "environment": nix_record! {
                    "etc": NixValue::record([(file.path, definition)]),
                },
            }))
        });

        self.modules.into_iter().fold(installed, ModuleBody::module)
    }
}

/// Whether one composition rejects competing installation claims in Rust.
/// This is a per-composition choice, independent of Cargo features and ordinary
/// NixOS module merging. Disabled checks provide no uniqueness guarantee.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CollisionChecks {
    /// Reject repeated application identities, command paths and file destinations.
    #[default]
    Enabled,
    /// Preserve duplicate declarations for normal NixOS evaluation and merging.
    /// NixOS or package assembly may still reject incompatible definitions.
    /// Malformed command names and paths are still rejected by Rusix.
    Disabled,
}

/// A malformed declaration or conflicting exclusive claim in one composition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallationError {
    /// An identity, command word or destination cannot denote a supported resource.
    Invalid {
        /// Target label supplied to [`compose`].
        scope: String,
        /// Application that supplied the invalid declaration.
        owner: String,
        /// Invalid name or destination as supplied by the author.
        resource: String,
        /// Constraint violated by this declaration.
        reason: &'static str,
    },
    /// Two declarations claim the same resource, or incompatible file destinations.
    Collision {
        /// Target label supplied to [`compose`].
        scope: String,
        /// Names of the competing resources, including their namespace.
        resource: String,
        /// Application that registered the earlier claim.
        first: String,
        /// Application that registered the later claim; may equal `first`.
        second: String,
    },
}

impl fmt::Display for InstallationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid {
                scope,
                owner,
                resource,
                reason,
            } => write!(
                formatter,
                "installation {scope:?}: {owner:?} declares {resource:?}: {reason}",
            ),
            Self::Collision {
                scope,
                resource,
                first,
                second,
            } => write!(
                formatter,
                "installation {scope:?}: {resource} is claimed by both {first:?} and {second:?}",
            ),
        }
    }
}

impl std::error::Error for InstallationError {}

/// Resource identity within one target's installation contract.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Resource {
    /// Application identity, kept separate from executable and file namespaces.
    Application(String),
    /// Executable followed by zero or more subcommands; words remain separate.
    Command(Vec<String>),
    /// Canonical relative `/etc` file destination.
    EtcFile(String),
}

impl fmt::Display for Resource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Application(owner) => write!(formatter, "application {owner:?}"),
            Self::Command(words) => {
                write!(formatter, "command {command:?}", command = words.join(" "))
            }
            Self::EtcFile(path) => write!(formatter, "file /etc/{path}"),
        }
    }
}

/// Rejects conflicting declarations, then emits their installation settings together.
/// Each call represents one target system. The same command or file may appear
/// in different calls, such as applications installed in separate VMs.
///
/// With checks enabled, duplicate command paths are rejected within an executable. Equal subcommand
/// names under different executables or parent commands are independent. Files
/// and executable exports are exclusive even when their definitions are equal.
/// Disabled checks leave duplicates to NixOS without choosing a winner or silently
/// dropping definitions. Name and path validation applies in both modes.
/// This runs during Rust configuration construction, before deferred Nix evaluation.
pub fn compose(
    scope: &str,
    checks: CollisionChecks,
    applications: impl IntoIterator<Item = Application>,
) -> Result<ModuleBody, InstallationError> {
    if scope.trim().is_empty() {
        return Err(InstallationError::Invalid {
            scope: scope.into(),
            owner: "<composition>".into(),
            resource: scope.into(),
            reason: "scope must be nonempty",
        });
    }

    let applications: Vec<_> = applications.into_iter().collect();
    let mut claims = BTreeMap::new();
    for app in &applications {
        let invalid = |resource: &str, reason| InstallationError::Invalid {
            scope: scope.into(),
            owner: app.owner.clone(),
            resource: resource.into(),
            reason,
        };
        if app.owner.trim().is_empty() {
            return Err(invalid(&app.owner, "application identity must be nonempty"));
        }

        claim(
            scope,
            checks,
            &mut claims,
            Resource::Application(app.owner.clone()),
            &app.owner,
        )?;
        for exe in &app.executables {
            if !command_word(&exe.name) {
                return Err(invalid(&exe.name, "expected a literal executable basename"));
            }
            claim(
                scope,
                checks,
                &mut claims,
                Resource::Command(vec![exe.name.clone()]),
                &app.owner,
            )?;
            for path in &exe.commands {
                if path.is_empty() || path.iter().any(|word| !command_word(word)) {
                    return Err(invalid(
                        &path.join(" "),
                        "expected a nonempty path of literal command words",
                    ));
                }
                let words = std::iter::once(exe.name.clone())
                    .chain(path.clone())
                    .collect();
                claim(
                    scope,
                    checks,
                    &mut claims,
                    Resource::Command(words),
                    &app.owner,
                )?;
            }
            for path in &exe.commands {
                if path.len() > 1
                    && !exe
                        .commands
                        .iter()
                        .any(|parent| parent == &path[..path.len() - 1])
                {
                    return Err(invalid(
                        &path.join(" "),
                        "parent command must also be declared",
                    ));
                }
            }
        }

        for file in &app.files {
            if file.path.chars().any(char::is_control)
                || file
                    .path
                    .split('/')
                    .any(|part| matches!(part, "" | "." | ".."))
            {
                return Err(invalid(
                    &file.path,
                    "expected a canonical relative /etc file path",
                ));
            }
            claim(
                scope,
                checks,
                &mut claims,
                Resource::EtcFile(file.path.clone()),
                &app.owner,
            )?;
        }
    }

    Ok(applications
        .into_iter()
        .fold(ModuleBody::empty(), |body, app| {
            body.module(app.into_module())
        }))
}

/// Accepts ordinary command names while rejecting paths, options and ambiguous words.
fn command_word(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !matches!(name, "." | "..")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.+-".contains(&byte))
}

/// Registers one exclusive claim, including file-versus-descendant conflicts.
fn claim(
    scope: &str,
    checks: CollisionChecks,
    claims: &mut BTreeMap<Resource, String>,
    resource: Resource,
    owner: &str,
) -> Result<(), InstallationError> {
    if checks == CollisionChecks::Disabled {
        return Ok(());
    }

    let collision = claims.iter().find(|(existing, _)| {
        if **existing == resource {
            return true;
        }
        match (existing, &resource) {
            (Resource::EtcFile(left), Resource::EtcFile(right)) => {
                left.strip_prefix(right)
                    .is_some_and(|suffix| suffix.starts_with('/'))
                    || right
                        .strip_prefix(left)
                        .is_some_and(|suffix| suffix.starts_with('/'))
            }
            _ => false,
        }
    });
    if let Some((existing, first)) = collision {
        let description = if existing == &resource {
            resource.to_string()
        } else {
            format!("{existing} and {resource}")
        };
        return Err(InstallationError::Collision {
            scope: scope.into(),
            resource: description,
            first: first.clone(),
            second: owner.into(),
        });
    }

    claims.insert(resource, owner.into());
    Ok(())
}
