//! Describes the dependencies and feature switches accepted by the pinned OpenSSL Nix function.
//! Rust accessors generate references; Nix resolves their values only when the recipe needs them.

/// Declares the dependencies and feature arguments accepted by the OpenSSL Nix function.
/// Its views retain the caller's values for Nix to resolve when the recipe needs them.
#[rusix::args]
#[allow(dead_code)] // Upstream retains unused coreutils/writeShellScript parameters.
pub mod args {
    use rusix::interop::{NixCallable, NixLibrary, NixNullable, Package, Stdenv, raw::NixValue};

    /// Caller-owned dependencies and lazy policy arguments.
    #[rusix(root)]
    struct Inputs {
        /// Utility functions and package metadata supplied by the Nix caller.
        lib: NixLibrary,
        /// Standard build environment: compiler, platform information and build-recipe constructor.
        stdenv: Stdenv,
        /// Source fetcher that describes downloading an archive with an expected checksum.
        fetchurl: NixCallable<Package>,
        /// Packages that run on the build machine, even when compiling for another platform.
        build_packages: NixValue,
        /// Interpreter used to run OpenSSL's Configure script.
        perl: Package,
        /// Unused upstream parameter retained so existing Nix callers can still supply it.
        #[allow(dead_code)]
        coreutils: Package,
        /// Unused upstream script helper retained for compatibility with Nix callers.
        #[allow(dead_code)]
        write_shell_script: NixValue,
        /// Build hook that creates the c_rehash compatibility wrapper around openssl rehash.
        make_binary_wrapper: Package,
        /// Enables the cryptodev engine and adds its dependency; defaults to false in Nix.
        with_cryptodev: bool,
        /// Kernel-crypto interface used only when the cryptodev engine is enabled.
        cryptodev: Package,
        /// Enables compression through zlib; defaults to false in Nix.
        with_zlib: bool,
        /// Compression library used only when withZlib is enabled.
        zlib: Package,
        /// Requests the legacy SSL 2 protocol through the upstream configure flags.
        #[rusix(rename = "enableSSL2")]
        enable_ssl2: bool,
        /// Requests the legacy SSL 3 protocol through the upstream configure flags.
        #[rusix(rename = "enableSSL3")]
        enable_ssl3: bool,
        /// Requests the legacy MD2 digest through the upstream configure flags.
        #[rusix(rename = "enableMD2")]
        enable_md2: bool,
        /// Enables kernel TLS support; the Nix default follows the host platform's Linux flag.
        #[rusix(rename = "enableKTLS")]
        enable_ktls: bool,
        /// Selects static libraries; the Nix default follows the host platform's static-build flag.
        #[rusix(rename = "static")]
        static_build: bool,
        /// Optional openssl.cnf file copied into the configuration output; null keeps the default file.
        conf: NixNullable<NixValue>,
        /// Build tool that removes the library output's own store-path reference from static archives.
        remove_references_to: Package,
        /// nixpkgs test helpers used to check the final package's pkg-config metadata.
        testers: NixValue,
    }
}

pub use args::Inputs;
