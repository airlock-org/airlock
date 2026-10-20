//! Generate NixOS modules and evaluate selected configuration fields.
//!
//! A NixOS module contributes settings or declares options: configurable fields
//! with types, defaults and documentation. NixOS combines modules according to
//! those types and definition priorities. This backend generates ordinary modules
//! and retains Rust locations for type, merge and expression errors.
//!
//! Compilation does not run NixOS. Evaluation uses `NixSession`’s disposable
//! store and never builds packages or activates a system.
use crate::compiler::{
    ast::{NixExpr, NixKind},
    lower_value, render_with_options,
};
use crate::ir::ReferencedExpression;
use crate::{Diagnostic, Generated, RenderOptions, compiler::render::quote};

use crate::{DiagnosticKind, DiagnosticOrigin, OriginRole, Provenance};

#[cfg(feature = "evaluation")]
use crate::{Evaluation, NixSession};

use crate::{
    ir::{IntoNode, Origin},
    nixos::NixosModule,
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "evaluation")]
use sha2::{Digest, Sha256};

#[cfg(feature = "evaluation")]
use std::fs;

use std::{path::Path, sync::OnceLock};

#[cfg(all(test, feature = "evaluation"))]
#[path = "nixos_context_audit.rs"]
mod context_audit;

/// Exact nixpkgs submodule revision used for the minimal NixOS option
/// declarations and full offline package evaluation.
pub const NIXPKGS_REVISION: &str = "8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296";

/// Minimal module-evaluation driver for staged artifacts, not a full system evaluation.
/// Accepts `nixpkgs`, `module`, literal `selection` segments and `checkAssertions`.
/// Interop can supply `pkgs` and request package metadata with `packageSummary`.
/// Exposed for backend/CLI artifact inspection; ordinary evaluation uses `NixSession`.
pub const DRIVER: &str = include_str!("driver.nix");

/// Render a wrapper using `nixos-driver.nix`, `module.nix` and the staged minimal tree.
/// `selection` contains escaped literal attribute segments. This produces backend
/// compiler output for inspection, not an authoring escape hatch for raw Nix syntax.
pub fn evaluation_source(selection: &[&str], check_assertions: bool) -> String {
    let selection = selection
        .iter()
        .map(|s| quote(s))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "(import ./nixos-driver.nix) {{ nixpkgs = ./nixpkgs; module = ./module.nix; selection = [ {selection} ]; checkAssertions = {check_assertions}; }}\n"
    )
}

/// A configuration entry or import associated with a Rust source location.
/// NixOS can report errors while combining modules, after a value’s computation
/// has finished. This metadata lets Rusix still identify the defining or
/// importing Rust operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Boundary {
    /// The affected setting, assertion name or imported module path shown in diagnostics.
    /// Empty for intact configuration records whose option paths are reported by NixOS.
    pub path: String,
    /// Rust operation that introduced the definition or crossed into imported code.
    pub origin: Origin,
    /// Source label NixOS reports for this definition or import, when available.
    /// NixOS stores such labels as `_file` metadata to identify contributing modules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

/// A generated NixOS module together with its Rust-source error information.
/// The module can be evaluated alongside ordinary NixOS modules. Expression
/// locations identify failed computations; separate definition metadata identifies
/// settings involved in type errors or conflicts between modules. Keep both
/// source and metadata when saving the artifact.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NixosArtifact {
    /// Generated module source and Rust source map.
    pub module: Generated,
    /// Locations of contributed settings, used to explain rejected or conflicting NixOS values.
    pub definitions: Vec<Boundary>,
    /// Locations that declared option names, types, defaults and documentation.
    /// Kept separate so invalid values from other modules are not blamed on these declarations.
    #[serde(default)]
    pub declarations: Vec<Boundary>,
    /// Rust assertion locations and source markers carried in NixOS failure messages.
    pub assertions: Vec<Boundary>,
    /// Rust import locations used when errors originate in external NixOS files.
    pub imports: Vec<Boundary>,
}

impl NixosArtifact {
    /// Map an external evaluator's stderr back to this module's Rust definitions.
    /// `module_file` must identify the exact saved `module.source`; `nixpkgs`
    /// identifies the caller's source tree containing `lib/modules.nix`.
    /// This reads diagnostic evidence without launching Nix or choosing its inputs.
    /// Preserve the source and this artifact together when evaluating elsewhere.
    ///
    /// Pass the actual failure kind: syntax and tooling failures retain their
    /// original classification without blaming a Rust definition. Unknown NixOS
    /// message formats retain the original diagnostic and available source spans.
    pub fn diagnose(
        &self,
        kind: DiagnosticKind,
        stderr: &str,
        module_file: &Path,
        nixpkgs: &Path,
    ) -> Diagnostic {
        let diagnostic = Diagnostic::from_nix(kind, stderr, &self.module, module_file);
        if diagnostic.kind == DiagnosticKind::NixEval {
            translate(diagnostic, self, nixpkgs)
        } else {
            diagnostic
        }
    }
}

fn string(value: impl Into<String>) -> NixExpr {
    NixExpr::plain(NixKind::String(value.into()))
}

fn attrs(bindings: Vec<(&str, NixExpr)>) -> NixExpr {
    NixExpr::plain(NixKind::AttrSet(
        bindings
            .into_iter()
            .map(|(key, value)| (vec![key.into()], value))
            .collect(),
    ))
}

/// Generate a NixOS module from the supplied Rust contributions.
/// Independent settings, declarations and imports keep their identities so
/// NixOS can merge them and report each contributing Rust location. Final-option
/// references become `config.*` expressions. This runs Rusix validation, but
/// does not evaluate Nix or check option types against NixOS declarations.
pub fn compile_module(module: &NixosModule) -> Result<NixosArtifact, Box<Diagnostic>> {
    compile_module_with_options(module, RenderOptions::default())
}

/// Generate a validated module with optional origin comments for inspection.
/// Definition, declaration, assertion and import metadata is unchanged. The
/// module's spans correspond to the selected rendering, so save source and map
/// together. Normal [`compile_module`] output omits fine-grained origin comments.
pub fn compile_module_with_options(
    module: &NixosModule,
    options: RenderOptions,
) -> Result<NixosArtifact, Box<Diagnostic>> {
    let (ast, mut artifact) = lower_module(module)?;
    artifact.module = render_with_options(&ast, options);
    Ok(artifact)
}

fn lower_module(module: &NixosModule) -> Result<(NixExpr, NixosArtifact), Box<Diagnostic>> {
    module
        .config
        .validate()
        .map_err(|error| Diagnostic::validation(error.origin, error.message))?;

    module
        .options
        .validate()
        .map_err(|error| Diagnostic::validation(error.origin, error.message))?;

    let mut imports = Vec::new();

    for imported in &module.imports {
        if imported.path.is_empty()
            || imported.path.contains('\0')
            || Path::new(&imported.path)
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(Diagnostic::validation(
                imported.origin.clone(),
                "module paths must be relative, nonempty paths without parent traversal".into(),
            )
            .into());
        }

        // Paths are structured data, never injected Nix source. A string suffix
        // also permits spaces and other characters that aren't Nix path tokens.
        imports.push(NixExpr::attributed(
            NixKind::Binary(
                crate::compiler::ast::BinaryOp::Add,
                Box::new(NixExpr::plain(NixKind::Path("./nixpkgs".into()))),
                Box::new(string(format!("/{}", imported.path))),
            ),
            imported.origin.clone(),
        ));
    }

    for (module_ref, origin) in &module.opaque_imports {
        module_ref
            .reference()
            .validate()
            .map_err(|e| Diagnostic::validation(e.origin, e.message))?;
        imports.push(crate::compiler::interop::module(
            module_ref.reference(),
            origin,
        ));
    }

    let generated_imports: Vec<_> = module
        .generated_imports
        .iter()
        .map(|(value, origin)| (value.clone().into_node(origin.clone()), origin))
        .chain(module.contextual_modules.iter().map(|context| {
            (
                context.function.clone().into_node(context.origin.clone()),
                &context.origin,
            )
        }))
        .collect();

    for (value, origin) in &generated_imports {
        value
            .validate()
            .map_err(|error| Diagnostic::validation(error.origin, error.message))?;
        imports.push(NixExpr::attributed(
            NixKind::Group(Box::new(lower_value(value))),
            (*origin).clone(),
        ));
    }

    for declaration in &module.options.assignments {
        let body = attrs(vec![
            ("_file", string(declaration.origin.id.clone())),
            (
                "options",
                NixExpr::plain(NixKind::AttrSet(vec![(
                    declaration.path_segments().to_vec(),
                    lower_value(&declaration.value),
                )])),
            ),
        ]);
        imports.push(NixExpr::attributed(body.kind, declaration.origin.clone()));
    }

    for assignment in &module.config.assignments {
        let config = NixExpr::plain(NixKind::AttrSet(vec![(
            assignment.path_segments().to_vec(),
            match module.priority.override_priority() {
                None => lower_value(&assignment.value),
                Some(priority) => attrs(vec![
                    ("_type", string("override")),
                    (
                        "priority",
                        NixExpr::plain(NixKind::Int(i64::from(priority))),
                    ),
                    ("content", lower_value(&assignment.value)),
                ]),
            },
        )]));

        // One module per definition makes even unknown-option diagnostics carry
        // the precise introducing operation, without demanding the option value.
        let body = attrs(vec![
            ("_file", string(assignment.origin.id.clone())),
            ("config", config),
        ]);
        imports.push(NixExpr::attributed(body.kind, assignment.origin.clone()));
    }

    for assertion in &module.assertions {
        assertion
            .condition
            .validate()
            .map_err(|error| Diagnostic::validation(error.origin, error.message))?;

        if assertion.name.is_empty()
            || assertion.name.contains('\0')
            || assertion.message.contains('\0')
        {
            return Err(Diagnostic::validation(
                assertion.origin.clone(),
                "assertion name must be nonempty and assertion strings must not contain NUL".into(),
            )
            .into());
        }

        let value = attrs(vec![
            ("assertion", lower_value(&assertion.condition)),
            (
                "message",
                string(format!("[{}] {}", assertion.origin.id, assertion.message)),
            ),
        ]);
        let config = attrs(vec![(
            "assertions",
            NixExpr::plain(NixKind::List(vec![value])),
        )]);
        let body = attrs(vec![
            ("_file", string(assertion.origin.id.clone())),
            ("config", config),
        ]);
        imports.push(NixExpr::attributed(body.kind, assertion.origin.clone()));
    }

    // Each child validates independently. Duplicate paths across modules belong
    // to NixOS's merge semantics, not the single-Config duplicate-path check.
    let mut children = Vec::new();

    for child in &module.modules {
        let (ast, artifact) = lower_module(child)?;
        imports.push(ast);
        children.push(artifact);
    }

    let body = attrs(vec![("imports", NixExpr::plain(NixKind::List(imports)))]);
    let scoped_values: Vec<_> = module
        .config
        .assignments
        .iter()
        .map(|a| &a.value)
        .chain(module.options.assignments.iter().map(|a| &a.value))
        .chain(generated_imports.iter().map(|(v, _)| v))
        .chain(module.assertions.iter().map(|a| &a.condition))
        .collect();
    let needs_config = scoped_values
        .iter()
        .any(|v| crate::compiler::option_reference(v).is_some());
    let kind = if needs_config {
        let mut arguments = vec!["config".into()];
        if scoped_values
            .iter()
            .any(|v| crate::compiler::module_package_reference(v).is_some())
        {
            arguments.push("pkgs".into());
        }
        NixKind::Function(arguments, Box::new(body))
    } else {
        body.kind
    };

    let ast = NixExpr::attributed(kind, module.config.origin.clone());
    let mut artifact = NixosArtifact {
        module: Generated::default(),
        definitions: module
            .config
            .assignments
            .iter()
            .map(|a| Boundary {
                path: a.path.clone(),
                origin: a.origin.clone(),
                file: None,
            })
            .collect(),
        declarations: module
            .options
            .assignments
            .iter()
            .map(|a| Boundary {
                path: a.path.clone(),
                origin: a.origin.clone(),
                file: None,
            })
            .collect(),
        assertions: module
            .assertions
            .iter()
            .map(|a| Boundary {
                path: format!("assertions.{}", a.name),
                origin: a.origin.clone(),
                file: None,
            })
            .collect(),
        imports: module
            .imports
            .iter()
            .map(|i| Boundary {
                path: i.path.clone(),
                origin: i.origin.clone(),
                file: None,
            })
            .collect(),
    };

    for (module_ref, origin) in &module.opaque_imports {
        use crate::ir::Source;

        let (path, file) = match &module_ref.reference().source {
            Source::ModuleFile { path } => (
                format!("nixos/modules/{path}"),
                Some(format!("nixpkgs-full/nixos/modules/{path}")),
            ),
            Source::Input { file, .. } => {
                let path = std::path::absolute(file)
                    .expect("configuration working directory")
                    .to_string_lossy()
                    .into_owned();
                (path.clone(), Some(path))
            }
            _ => unreachable!("opaque module constructors use module files or inputs"),
        };
        artifact.imports.push(Boundary {
            path,
            file,
            origin: origin.clone(),
        });
    }
    for context in &module.contextual_modules {
        artifact.definitions.extend(context.definitions.clone());
    }

    for child in children {
        artifact.definitions.extend(child.definitions);
        artifact.declarations.extend(child.declarations);
        artifact.assertions.extend(child.assertions);
        artifact.imports.extend(child.imports);
    }
    Ok((ast, artifact))
}

#[derive(Deserialize)]
pub(crate) struct Pin {
    /// Git revision required for the offline nixpkgs source checkout.
    pub(crate) revision: String,
    /// Expected content hashes keyed by relative paths of the minimal evaluator files.
    #[cfg(feature = "evaluation")]
    files: std::collections::BTreeMap<String, String>,
    /// Checked upstream files whose contents identify supported backend diagnostics.
    pub(crate) diagnostic_files: std::collections::BTreeMap<String, String>,
}

// Both evaluation modes use this reviewed descriptor. The revision constant
// checks the expected upstream API; individual-file digests live in the manifest.
pub(crate) fn pin() -> Result<&'static Pin, String> {
    static PIN: OnceLock<Result<Pin, String>> = OnceLock::new();

    PIN.get_or_init(|| {
        let pin: Pin =
            serde_json::from_str(include_str!("nixpkgs-pin.json")).map_err(|e| e.to_string())?;
        if pin.revision != NIXPKGS_REVISION {
            return Err("nixpkgs pin revision mismatch".into());
        }
        Ok(pin)
    })
    .as_ref()
    .map_err(Clone::clone)
}

#[cfg(feature = "evaluation")]
type PinnedFiles = Vec<(String, Vec<u8>)>;

#[cfg(feature = "evaluation")]
fn pinned_files(root: &Path) -> Result<&'static PinnedFiles, String> {
    static SNAPSHOT: OnceLock<Result<PinnedFiles, String>> = OnceLock::new();

    SNAPSHOT
        .get_or_init(|| {
            pin()?
                .files
                .iter()
                .map(|(name, hash)| {
                    if Path::new(&name)
                        .components()
                        .any(|c| !matches!(c, std::path::Component::Normal(_)))
                    {
                        return Err("invalid snapshot path".into());
                    }
                    let bytes = fs::read(root.join(name))
                        .map_err(|e| format!("missing pinned {name}: {e}"))?;
                    if format!("{:x}", Sha256::digest(&bytes)) != *hash {
                        return Err(format!("pinned nixpkgs hash mismatch: {name}"));
                    }
                    Ok((name.clone(), bytes))
                })
                .collect()
        })
        .as_ref()
        .map_err(Clone::clone)
}

#[cfg(feature = "evaluation")]
impl NixSession {
    pub(crate) fn stage_pinned(&self) -> Result<std::path::PathBuf, Box<Diagnostic>> {
        let pin_root = self.root().join("nixpkgs");
        if self.pinned_staged.get().is_some() {
            return Ok(pin_root);
        }
        let source = self.nixpkgs_source()?;
        let files = pinned_files(&source.path).map_err(Diagnostic::tooling)?;

        for (name, bytes) in files {
            let path = pin_root.join(name);
            fs::create_dir_all(path.parent().unwrap())
                .map_err(|e| Diagnostic::tooling(e.to_string()))?;
            fs::write(path, bytes).map_err(|e| Diagnostic::tooling(e.to_string()))?;
        }

        let _ = self.pinned_staged.set(());
        Ok(pin_root)
    }

    /// Evaluate a module using a custom Nix expression that selects the result.
    /// Use this advanced testing API for a full NixOS evaluation or a comparison
    /// projection: an expression that selects only the data being compared.
    ///
    /// The module is staged as `module.nix` and parsed separately. Minimal and full
    /// pinned inputs are available inside the isolated session. `driver` must return
    /// JSON-compatible data; it is executable Nix supplied by the caller, not a
    /// restricted authoring API. Module failures retain the artifact’s Rust locations.
    pub fn evaluate_nixos_with_driver(
        &self,
        artifact: &NixosArtifact,
        driver: &Generated,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        self.with_evaluation_lock(|| {
            let pin_root = self.stage_interop()?;
            self.evaluate_module_driver(artifact, driver, &pin_root)
        })
    }

    /// Evaluate a selected configuration field using the minimal pinned NixOS harness.
    /// `selection` contains literal field names, such as `["services", "example",
    /// "port"]`. An empty selection requests all `config`; JSON conversion evaluates
    /// the requested data. `check_assertions` also checks NixOS’s assertion list.
    ///
    /// This harness loads only imported option declarations. Unrelated namespaces
    /// such as `systemd` and `environment` accept arbitrary fields here, so this is
    /// not a full-system validity check. [`Self::evaluate_nixos_interop`] adds the
    /// full package set and module files; use [`Self::evaluate_nixos_with_driver`]
    /// for a complete NixOS evaluation.
    pub fn evaluate_nixos(
        &self,
        artifact: &NixosArtifact,
        selection: &[&str],
        check_assertions: bool,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        self.evaluate_nixos_mode(artifact, selection, check_assertions, false, false)
    }

    /// Evaluate a configuration field with the full pinned package set available.
    /// This stages package inputs and module files offline but still uses the minimal
    /// NixOS harness; it does not load every NixOS module automatically. Selection and
    /// assertion behavior match [`Self::evaluate_nixos`]. Use
    /// [`Self::evaluate_nixos_with_driver`] for a full-system comparison.
    pub fn evaluate_nixos_interop(
        &self,
        artifact: &NixosArtifact,
        selection: &[&str],
        check_assertions: bool,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        self.evaluate_nixos_mode(artifact, selection, check_assertions, true, false)
    }

    /// Check the system package list and return package metadata as JSON.
    /// NixOS validates `environment.systemPackages` using its real package type.
    /// This selects metadata rather than output or derivation paths. Nix may create
    /// build recipes, but no package outputs are built.
    pub fn evaluate_system_packages(
        &self,
        artifact: &NixosArtifact,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        self.evaluate_nixos_mode(
            artifact,
            &["environment", "systemPackages"],
            false,
            true,
            true,
        )
    }

    fn evaluate_nixos_mode(
        &self,
        artifact: &NixosArtifact,
        selection: &[&str],
        check_assertions: bool,
        interop: bool,
        summaries: bool,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        self.with_evaluation_lock(|| {
            let pin_root = if interop {
                self.stage_interop()?
            } else {
                self.stage_pinned()?
            };
            fs::write(self.root().join("nixos-driver.nix"), DRIVER)
                .map_err(|e| Diagnostic::tooling(e.to_string()))?;

            if selection.iter().any(|s| s.contains('\0')) {
                return Err(Diagnostic::tooling("NUL in NixOS selection").into());
            }

            let source = if interop {
                let pkgs = crate::compiler::render(&crate::compiler::interop::source(
                    &rusix::ir::Source::Packages { overlays: vec![] },
                ))
                .source;
                format!(
                    "(import ./nixos-driver.nix) {{ nixpkgs = ./nixpkgs; module = ./module.nix; selection = [ {} ]; checkAssertions = {check_assertions}; pkgs = {pkgs}; packageSummary = {summaries}; }}\n",
                    selection
                        .iter()
                        .map(|s| quote(s))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            } else {
                evaluation_source(selection, check_assertions)
            };
            let wrapper = Generated {
                source,
                ..Generated::default()
            };

            self.evaluate_module_driver(artifact, &wrapper, &pin_root)
        })
    }

    // Called only while the complete evaluation holds the session lock.
    fn evaluate_module_driver(
        &self,
        artifact: &NixosArtifact,
        driver: &Generated,
        pin_root: &Path,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        let parse_stderr = self.validate_generated(&artifact.module, "module.nix")?;
        self.evaluate_staged(driver)
            .map(|mut evaluation| {
                evaluation.raw_nix = format!("{parse_stderr}{}", evaluation.raw_nix);
                evaluation
            })
            .map_err(|mut error| {
                error.raw_nix = format!("{parse_stderr}{}", error.raw_nix);
                if error.kind != DiagnosticKind::NixEval {
                    return error;
                }
                Box::new(artifact.diagnose(
                    DiagnosticKind::NixEval,
                    &error.raw_nix,
                    &self.root().join("module.nix"),
                    pin_root,
                ))
            })
    }
}

fn boundary_file(boundary: &Boundary, pin_root: &Path) -> std::path::PathBuf {
    match &boundary.file {
        Some(file) => pin_root.parent().unwrap_or(pin_root).join(file),
        None => pin_root.join(&boundary.path),
    }
}

fn matches_file(reported: &str, path: &Path) -> bool {
    [
        path.to_owned(),
        path.canonicalize().unwrap_or_else(|_| path.to_owned()),
    ]
    .iter()
    .any(|path| {
        let prefix = path.to_string_lossy();
        reported == prefix || reported.starts_with(&format!("{prefix}:"))
    })
}

fn translate(mut diagnostic: Diagnostic, artifact: &NixosArtifact, pin_root: &Path) -> Diagnostic {
    let evidence = crate::diagnostic::nix_evidence(&diagnostic.raw_nix);
    let in_module_system = evidence.iter().any(|(file, _)| {
        file.as_deref().is_some_and(|file| {
            [
                pin_root.to_owned(),
                pin_root.parent().unwrap_or(pin_root).join("nixpkgs-full"),
            ]
            .iter()
            .any(|root| matches_file(file, &root.join("lib/modules.nix")))
        })
    });

    if in_module_system && let Some(failure) = crate::diagnostic::module_failure(&diagnostic.reason)
    {
        diagnostic.kind = failure.kind;
        diagnostic.option_path = Some(failure.option.clone());
        let mut origins = Vec::new();

        for file in &failure.files {
            // A schema default is a definition only when Nix explicitly reports
            // our declaration identity. Never blame it for a foreign bad value.
            let source = if let Some(id) = crate::diagnostic::origin_id(file) {
                artifact
                    .definitions
                    .iter()
                    .chain(&artifact.declarations)
                    .find(|b| b.origin.id == id)
                    .map(|boundary| {
                        // Keep precise NixOS paths within opaque records. Only
                        // synthetic list-definition suffixes need the IR path.
                        if !boundary.path.is_empty()
                            && (!failure.option.starts_with(&format!("{}.", boundary.path))
                                || failure.option.contains("[definition "))
                        {
                            diagnostic.option_path = Some(boundary.path.clone());
                        }
                        DiagnosticOrigin {
                            origin: Some(boundary.origin.clone()),
                            role: if failure.kind == DiagnosticKind::NixosMerge {
                                OriginRole::ConflictingDefinition
                            } else {
                                OriginRole::ContributingDefinition
                            },
                            provenance: Provenance::ModuleDefinition,
                            nix_file: None,
                        }
                    })
            } else {
                let relative = Path::new(file)
                    .strip_prefix(pin_root)
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned());
                // Retain an upstream file even when no directly imported Rust
                // boundary matches it. Never map it as a generated source span.
                let boundary = artifact
                    .imports
                    .iter()
                    .find(|b| matches_file(file, &boundary_file(b, pin_root)));
                Some(DiagnosticOrigin {
                    origin: boundary.map(|b| b.origin.clone()),
                    role: OriginRole::ImportedBoundary,
                    provenance: if boundary.is_some() {
                        Provenance::ImportBoundary
                    } else {
                        Provenance::Unavailable
                    },
                    nix_file: Some(
                        boundary
                            .map(|b| b.path.clone())
                            .or(relative)
                            .unwrap_or_else(|| file.clone()),
                    ),
                })
            };

            if let Some(source) = source
                && !origins.contains(&source)
            {
                origins.push(source);
            }
        }

        if !origins.is_empty() {
            let first = origins
                .iter()
                .find(|o| o.provenance == Provenance::ModuleDefinition)
                .or_else(|| origins.iter().find(|o| o.origin.is_some()));
            diagnostic.primary = first.and_then(|o| o.origin.clone());
            diagnostic.provenance = first.map_or(Provenance::Unavailable, |o| o.provenance);
            diagnostic.related = origins.iter().filter_map(|o| o.origin.clone()).collect();
            diagnostic.origins = origins;
        }
    }

    for boundary in &artifact.definitions {
        diagnostic.reason = diagnostic.reason.replace(
            &boundary.origin.id,
            &format!(
                "Rust {}:{}:{}",
                boundary.origin.file, boundary.origin.line, boundary.origin.column
            ),
        );
    }

    if evidence
        .iter()
        .any(|(_, message)| message == "rusix-stage:nixos-assertions")
    {
        diagnostic.kind = DiagnosticKind::NixosAssertion;
        let mut origins = Vec::new();
        for boundary in &artifact.assertions {
            let marker = format!("[{}]", boundary.origin.id);
            if diagnostic.reason.contains(&marker) {
                origins.push(DiagnosticOrigin {
                    origin: Some(boundary.origin.clone()),
                    role: OriginRole::ContributingDefinition,
                    provenance: Provenance::AssertionMessage,
                    nix_file: None,
                });
                if origins.len() == 1 {
                    diagnostic.primary = Some(boundary.origin.clone());
                    diagnostic.option_path = Some(boundary.path.clone());
                    diagnostic.provenance = Provenance::AssertionMessage;
                }
                diagnostic.reason = diagnostic.reason.replace(&format!("{marker} "), "");
            }
        }

        if !origins.is_empty() {
            if origins.len() > 1 {
                diagnostic.option_path = Some("assertions".into());
            }
            diagnostic.related = origins.iter().filter_map(|o| o.origin.clone()).collect();
            diagnostic.origins = origins;
        }
    }

    if diagnostic.primary.is_none() {
        for (file, _) in &evidence {
            for boundary in &artifact.imports {
                let external = boundary_file(boundary, pin_root);
                if file
                    .as_deref()
                    .is_some_and(|file| matches_file(file, &external))
                {
                    diagnostic.kind = DiagnosticKind::ExternalNix;
                    diagnostic.primary = Some(boundary.origin.clone());
                    diagnostic.external_file = Some(boundary.path.clone());
                    diagnostic.provenance = Provenance::ImportBoundary;
                    diagnostic.origins = vec![DiagnosticOrigin {
                        origin: Some(boundary.origin.clone()),
                        role: OriginRole::ImportedBoundary,
                        provenance: Provenance::ImportBoundary,
                        nix_file: Some(boundary.path.clone()),
                    }];
                    break;
                }
            }
            if diagnostic.primary.is_some() {
                break;
            }
        }
    }

    if diagnostic.option_path.is_none() {
        diagnostic.option_path = diagnostic
            .related
            .iter()
            .rev()
            .find_map(|o| o.purpose.strip_prefix("set ").map(str::to_owned));
    }

    if diagnostic.option_path.is_none() {
        diagnostic.option_path = evidence.iter().find_map(|(_, message)| {
            message
                .strip_prefix("while evaluating the option `")
                .and_then(|s| s.split_once('\''))
                .map(|(path, _)| path.to_owned())
        });
    }

    // Remove disposable staging paths from display only. The exact original
    // diagnostic remains in raw_nix, including all external trace positions.
    diagnostic.reason = diagnostic
        .reason
        .replace(&format!("{}/", pin_root.display()), "");
    diagnostic.with_origin_set()
}
