//! Selects one of four MariaDB releases for the shared client/server recipe.
//! Version numbers and source checksums are fixed Rust data; package dependencies remain deferred.
use rusix::{
    IntoRusixValue,
    interop::{Nixpkgs, Package},
};

/// A MariaDB release from the pinned nixpkgs checkout to use with the shared recipe.
#[derive(Clone, Copy, Debug)]
pub enum Release {
    /// MariaDB 10.5.27, exported as `mariadb_105`.
    V105,
    /// MariaDB 10.6.20, exported as `mariadb_106`.
    V106,
    /// MariaDB 10.11.10, the pinned default, exported as `mariadb_1011`.
    V1011,
    /// MariaDB 11.4.4, exported as `mariadb_114`.
    V114,
}

impl Release {
    /// All pinned releases, in the order used to assemble the family.
    pub const ALL: [Self; 4] = [Self::V105, Self::V106, Self::V1011, Self::V114];

    /// Name of this release in the exported Nix package set, such as `mariadb_1011`.
    pub fn attribute(self) -> &'static str {
        match self {
            Self::V105 => "mariadb_105",
            Self::V106 => "mariadb_106",
            Self::V1011 => "mariadb_1011",
            Self::V114 => "mariadb_114",
        }
    }

    /// Source release version used in the archive URL and recipe's version checks.
    pub fn version(self) -> &'static str {
        match self {
            Self::V105 => "10.5.27",
            Self::V106 => "10.6.20",
            Self::V1011 => "10.11.10",
            Self::V114 => "11.4.4",
        }
    }

    /// Expected SHA-256 checksum of the source archive, in Nix's `sha256-...` format.
    /// Returning the checksum in Rust does not fetch or verify the archive.
    pub fn hash(self) -> &'static str {
        match self {
            Self::V105 => "sha256-76ZPpfczuCKrGmeV4evBecFgwIaQ7Ncduh6w9hru3RQ=",
            Self::V106 => "sha256-R/BavH2+uz8msx51AisbOWnbvEdSxK5wH4kgpgSJbUE=",
            Self::V1011 => "sha256-sGp0ZQuDoWqpqwmJhEgrAo51sABnSxH/KIdyxhmm8CI=",
            Self::V114 => "sha256-lvvS5uk/t+izc+6nXYW2/qV8DhEaAgkMu+/tUlmdx3s=",
        }
    }

    /// Supplies this release's version and checksum plus the macOS framework dependency.
    /// Nix selects the remaining function arguments from its package set through `callPackage`.
    pub fn arguments(self) -> Arguments {
        Arguments {
            version: self.version(),
            hash: self.hash(),
            core_services: Nixpkgs::new()
                .get("darwin.apple_sdk.frameworks.CoreServices")
                .into(),
        }
    }
}

/// Pinned release inputs; the framework stays a deferred package dependency.
#[derive(IntoRusixValue)]
pub struct Arguments {
    // Fixed source version supplied to the shared recipe.
    version: &'static str,
    // Expected checksum supplied to the source fetcher.
    hash: &'static str,
    // Reference to the macOS framework; Rust does not load or build it.
    #[rusix(rename = "CoreServices")]
    core_services: Package,
}
