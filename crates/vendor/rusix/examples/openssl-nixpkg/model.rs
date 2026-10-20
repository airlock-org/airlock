//! Selects one of the three OpenSSL releases in the pinned nixpkgs checkout.
//! Version numbers and source checksums are fixed Rust data shared by the package recipes.

/// An OpenSSL release to describe with the shared build recipe.
/// The labels describe historical release choices at this pin.
#[derive(Clone, Copy, Debug)]
pub enum Release {
    /// OpenSSL 1.1.1w, exported as `openssl_1_1` at this pin.
    Legacy,
    /// OpenSSL 3.0.15, the pin's long-term-support branch, exported as `openssl_3`.
    Lts,
    /// OpenSSL 3.3.2, this example's preview choice and the pin's default OpenSSL.
    Preview,
}

impl Release {
    /// All pinned releases, in the order used to assemble the family.
    pub const ALL: [Self; 3] = [Self::Legacy, Self::Lts, Self::Preview];

    /// Name of this release in the exported Nix package set, such as `openssl_3`.
    pub fn attribute(self) -> &'static str {
        match self {
            Self::Legacy => "openssl_1_1",
            Self::Lts => "openssl_3",
            Self::Preview => "openssl_3_3",
        }
    }

    /// Source release version used in the archive URL and recipe's version checks.
    pub fn version(self) -> &'static str {
        match self {
            Self::Legacy => "1.1.1w",
            Self::Lts => "3.0.15",
            Self::Preview => "3.3.2",
        }
    }

    /// Expected SHA-256 checksum of the source archive, in Nix's `sha256-...` format.
    /// Returning the checksum in Rust does not fetch or verify the archive.
    pub fn hash(self) -> &'static str {
        match self {
            Self::Legacy => "sha256-zzCYlQy02FOtlcCEHx+cbT3BAtzPys1SHZOSUgi3asg=",
            Self::Lts => "sha256-I8Zm0O3yDxQkmz2PA2isrumrWFsJ4d6CEHxm4fPslTM=",
            Self::Preview => "sha256-LopAsBl5r+i+C7+z3l3BxnCf7bRtbInBDaEUq1/D0oE=",
        }
    }
}
