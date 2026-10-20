//! Describe recurring nixpkgs package operations without evaluating them in Rust.
//! nixpkgs supplies build environments, dependencies and builders; these helpers
//! construct expressions using the values supplied by the package or module caller.

use crate::{Expr, interop::raw::NixValue};

/// Compare the build and host platform values supplied by `stdenv`.
///
/// nixpkgs' standard build environment, `stdenv`, distinguishes the platform
/// running build tools (`buildPlatform`) from the platform where the resulting
/// package runs (`hostPlatform`). This helper constructs exactly:
///
/// ```nix
/// stdenv.buildPlatform == stdenv.hostPlatform
/// ```
///
/// Rust does not inspect either value. Nix compares the complete supplied values
/// later, using ordinary `==`, including its behavior for shared values and
/// function-valued platform fields. The records are neither rebuilt nor normalized.
///
/// This is deliberately narrower than a general "native build" test. It does not
/// compare `targetPlatform`, establish that host binaries can execute during the
/// build, compare only `.system` strings, or call `lib.systems.equals` (which
/// removes top-level function-valued fields). Negate the result with `!` for inequality.
///
/// ```
/// use rusix::{Expr, interop::raw::NixValue, package::build_host_equal};
///
/// # fn example(stdenv: NixValue) {
/// // `stdenv` is the value supplied by the Nix caller.
/// let equal: Expr<bool> = build_host_equal(stdenv);
/// let unequal = !equal;
/// # }
/// ```
#[track_caller]
pub fn build_host_equal(stdenv: NixValue) -> Expr<bool> {
    stdenv
        .clone()
        .select_segments(["buildPlatform"])
        .equals(stdenv.select_segments(["hostPlatform"]))
        .into_expr()
}
