//! Sources and lookups for external Nix values.
use super::validation::validate_scoped;
use super::{Node, Origin, ValueKind};
use crate::ValidationError;
use crate::interop::raw::AttrPath;
use std::path::{Component, PathBuf};

/// Where Nix should obtain a referenced package, function or other value.
/// This is inspection data for backend authors. Normal configuration code
/// chooses a source through [`crate::interop::Nixpkgs`] or [`crate::interop::InputRef`] instead.
#[derive(Clone, Debug)]
pub enum Source {
    /// The Nix language's own builtin namespace, independent of nixpkgs or caller libraries.
    Builtins,
    /// The pinned standalone package set, with Nix applying overlays in order.
    Packages {
        /// Referenced or Rust-authored overlay expressions, ordered as supplied by the author.
        overlays: Vec<Node>,
    },
    /// The pinned nixpkgs library, independent of package-set overlays.
    Library,
    /// A file or directory in the pinned source tree, without importing or fetching it.
    PinnedPath {
        /// Relative path under nixpkgs; parent traversal is rejected.
        path: String,
    },
    /// The NixOS module's supplied `pkgs`, preserving its configuration and overlays.
    NixosPackages {
        /// Referenced or Rust-authored overlays applied to the supplied package set in order.
        overlays: Vec<Node>,
    },
    /// An existing NixOS module file in the pinned nixpkgs tree.
    ModuleFile {
        /// Relative path under `nixos/modules`, without parent traversal.
        path: String,
    },
    /// A local Nix expression supplying external objects; no fetching is implied.
    Input {
        /// Human-readable identity used in provenance and diagnostics.
        name: String,
        /// File imported by Nix; relative paths use Rust's working directory during lowering.
        file: PathBuf,
    },
}

/// A description of where Nix should look up an existing value.
/// It contains the source to load, the field names to select and the Rust
/// location to report on failure. It does not contain the evaluated Nix value.
/// Normal authoring uses category handles such as [`crate::interop::PackageRef`]; this metadata
/// is public for backend implementation and inspection.
#[derive(Clone, Debug)]
pub struct Reference {
    /// Package set, utility library or local file from which Nix obtains the value.
    pub source: Source,
    /// Field names to select from that source, or `None` to reference the source itself.
    pub path: Option<AttrPath>,
    /// Rust lookup operation to report if Nix cannot resolve the object.
    pub origin: Origin,
}

impl Reference {
    /// Check lookup paths, inputs and overlay expressions without opening files or evaluating Nix.
    /// Overlay callback parameters must stay within their function scopes; to
    /// validate a reference that captures an enclosing parameter, validate the
    /// enclosing [`Node`] instead.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_scoped(&[])
    }

    pub(crate) fn validate_scoped(&self, scope: &[u64]) -> Result<(), ValidationError> {
        if let Some(path) = &self.path {
            path.validate(&self.origin)?;
        }

        match &self.source {
            Source::Packages { overlays } | Source::NixosPackages { overlays } => {
                for overlay in overlays {
                    validate_scoped(overlay, scope)?;
                }
            }
            Source::ModuleFile { path } | Source::PinnedPath { path } => {
                if path.is_empty()
                    || path.contains('\0')
                    || PathBuf::from(path)
                        .components()
                        .any(|c| !matches!(c, Component::Normal(_)))
                {
                    return Err(ValidationError {
                        origin: self.origin.clone(),
                        message: if matches!(self.source, Source::PinnedPath { .. }) {
                            "source paths must be relative, nonempty paths without parent traversal"
                        } else {
                            "module paths must be relative, nonempty paths without parent traversal"
                        }
                        .into(),
                    });
                }
            }
            Source::Input { name, file } => {
                if name.is_empty()
                    || name.contains('\0')
                    || file
                        .to_str()
                        .is_none_or(|s| s.is_empty() || s.contains('\0'))
                {
                    return Err(ValidationError {
                        origin: self.origin.clone(),
                        message: "input identities and paths must be nonempty, UTF-8 and NUL-free"
                            .into(),
                    });
                }
            }
            Source::Library | Source::Builtins => {}
        }

        Ok(())
    }

    pub(crate) fn node(&self) -> Node {
        Node {
            origin: self.origin.clone(),
            kind: ValueKind::Reference(self.clone()),
        }
    }
}
