//! Declare dependencies and feature choices supplied to curl's Nix package function.
//! Accessors describe deferred lookups; Rust does not evaluate the arguments.

/// Declares named accessors for curl's Nix function arguments and the dependency fields it uses.
/// Rust constructs references through these views; Nix resolves the selected values later.
#[rusix::args]
pub(super) mod args {
    use rusix::interop::{NixCallable, Overridable, Package, raw::NixValue};

    /// Deferred package dependencies and features, resolved by callPackage or an ordinary caller.
    #[rusix(root)]
    struct Inputs {
        /// Utility functions and metadata from this exact package caller.
        lib: Lib,
        /// Standard builder, compiler, and build/host platform records.
        stdenv: Stdenv,
        /// Bootstrap source fetcher; constructing its derivation does not fetch.
        fetchurl: Overridable<NixCallable<Package>>,
        /// Apple framework packages used by the Darwin dependency branch.
        darwin: Darwin,
        /// Build-platform tool that supplies dependency flags.
        #[rusix(rename = "pkg-config")]
        pkg_config: Package,
        /// Build-platform interpreter used by curl scripts.
        perl: Package,
        /// Existing NixOS tests retained as passthru references.
        nixos_tests: NixosTests,
        /// Enables Brotli response decompression.
        brotli_support: bool,
        /// Brotli library propagated when that decompression feature is selected.
        brotli: Package,
        /// Selects asynchronous DNS resolution through c-ares.
        #[rusix(rename = "c-aresSupport")]
        c_ares_support: bool,
        /// Minimal c-ares package used by the asynchronous DNS branch.
        #[rusix(rename = "c-aresMinimal")]
        c_ares_minimal: Package,
        /// Selects GnuTLS; the native Nix assertion excludes other simultaneous TLS backends.
        gnutls_support: bool,
        /// GnuTLS package used for encrypted connections and compatibility library links.
        gnutls: Package,
        /// Enables authentication through GNU SASL.
        gsasl_support: bool,
        /// GNU SASL implementation propagated when its authentication feature is selected.
        gsasl: Package,
        /// Selects GSS authentication; its default depends on platform and cross-build state.
        gss_support: bool,
        /// Kerberos implementation and development output used by GSS authentication.
        libkrb5: Package,
        /// Enables the HTTP/2 library and defaults to true in Nix.
        http2_support: bool,
        /// HTTP/2 library propagated to libcurl consumers.
        nghttp2: Package,
        /// Enables both HTTP/3 and QUIC dependencies and their configure switches.
        http3_support: bool,
        /// HTTP/3 protocol library selected by http3Support.
        nghttp3: Package,
        /// QUIC transport library selected alongside nghttp3.
        ngtcp2: Package,
        /// Enables curl's experimental websocket protocol support.
        websocket_support: bool,
        /// Enables internationalized domain-name handling.
        idn_support: bool,
        /// Internationalized domain-name library and configure-time development output.
        libidn2: Package,
        /// Enables both LDAP and LDAPS protocol configure switches.
        ldap_support: bool,
        /// LDAP library propagated when LDAP support is enabled.
        openldap: Package,
        /// Selects OpenSSL; its lazy default follows zlibSupport.
        openssl_support: bool,
        /// Caller-selected OpenSSL package, also exposed unchanged in passthru.
        openssl: Package,
        /// Enables public-suffix handling, including the static linker workaround.
        psl_support: bool,
        /// Public suffix library propagated when PSL support is enabled.
        libpsl: Package,
        /// Enables RTMP streaming protocol support.
        rtmp_support: bool,
        /// RTMP implementation selected by the librtmp configure switch.
        rtmpdump: Package,
        /// Selects SCP; its default follows zlib and excludes SunOS/Cygwin.
        scp_support: bool,
        /// SSH library supplying SCP support and separate development/library outputs.
        libssh2: Package,
        /// Selects wolfSSL as one of the mutually exclusive TLS backends.
        wolfssl_support: bool,
        /// Caller-selected wolfSSL implementation and development output.
        wolfssl: Package,
        /// Selects Rustls and the platform-specific CA-bundle configure switch.
        rustls_support: bool,
        /// Existing Rustls FFI package; its implementation stays in nixpkgs.
        #[rusix(rename = "rustls-ffi")]
        rustls_ffi: Package,
        /// Enables zlib and controls the dependent OpenSSL/SCP defaults.
        zlib_support: bool,
        /// Compression library propagated when zlib support is selected.
        zlib: Package,
        /// Enables Zstandard response decompression.
        zstd_support: bool,
        /// Zstandard library propagated when that decompression feature is selected.
        zstd: Package,
        /// Existing consuming package overridden to use the eventual curl in a passthru check.
        coeurl: Package,
        /// Existing C++ curl wrapper overridden to consume the eventual package.
        curlpp: Package,
        /// Package scope containing the Haskell curl binding checked recursively.
        haskell_packages: HaskellPackages,
        /// Package scope containing the OCaml curly binding checked recursively.
        ocaml_packages: OcamlPackages,
        /// Extension scope containing the PHP curl binding checked recursively.
        php_extensions: PhpExtensions,
        /// Static package scope supplying the pinned static-curl passthru reference.
        pkgs_static: StaticPackages,
        /// Python package scope containing the pycurl binding checked recursively.
        python3: Python,
        /// nixpkgs regression-test scope containing the fetchpatch consumer check.
        tests: Tests,
        /// nixpkgs test helpers used to validate the final package's pkg-config metadata.
        testers: Testers,
        /// Required passthru dependency; never used to fetch curl source or patches.
        fetchpatch: Overridable<NixCallable<Package>>,
    }

    /// Existing builder and the finite compiler/platform properties this recipe needs.
    /// Whole-subtree access retains the entire supplied stdenv for build/host equality.
    #[rusix(value)]
    struct Stdenv {
        /// Real mkDerivation function, including recursive finalAttrs and overrides.
        mk_derivation: NixCallable<Package>,
        /// Platform where the resulting curl runs.
        #[rusix(expression)]
        host_platform: rusix::interop::Platform,
        /// Compiler metadata for the explicit C++ command names.
        cc: Compiler,
    }

    /// Compiler command naming remains controlled by stdenv.
    struct Compiler {
        /// Prefix before c++/c++ -E, including cross-toolchain names.
        target_prefix: String,
    }

    /// Caller-supplied library dispatch and package metadata.
    #[rusix(value)]
    struct Lib {
        /// The pinned package license record from nixpkgs.
        licenses: Licenses,
        /// Existing maintainer records, not Rust-maintained contact data.
        maintainers: Maintainers,
        /// Existing supported-platform lists.
        platforms: Platforms,
    }

    /// License metadata needed by curl.
    struct Licenses {
        /// curl's permissive license as described by the caller's library.
        curl: NixValue,
    }

    /// The upstream maintainer identity stays opaque.
    struct Maintainers {
        /// Maintainer listed by the pinned package.
        lovek323: NixValue,
    }

    /// Platform metadata stays owned by nixpkgs.
    struct Platforms {
        /// The same complete platform list used by upstream.
        all: NixValue,
    }

    /// Darwin package hierarchy used for framework dependencies.
    struct Darwin {
        /// The SDK selected by this package caller.
        #[rusix(rename = "apple_sdk")]
        apple_sdk: AppleSdk,
    }

    /// Only these SDK frameworks are required by curl.
    struct AppleSdk {
        /// Three propagated dependencies needed on Darwin.
        frameworks: Frameworks,
    }

    /// Framework packages remain Nix values rather than Rust SDK bindings.
    struct Frameworks {
        /// Core Foundation dependency.
        #[rusix(rename = "CoreFoundation")]
        core_foundation: Package,
        /// Core Services dependency.
        #[rusix(rename = "CoreServices")]
        core_services: Package,
        /// System configuration dependency.
        #[rusix(rename = "SystemConfiguration")]
        system_configuration: Package,
    }

    /// The HTTP/3 NixOS test is referenced, never executed by this example.
    struct NixosTests {
        /// Upstream cannot override this test; retain the original test unchanged.
        #[rusix(rename = "nginx-http3")]
        nginx_http3: NixValue,
    }

    /// Haskell package that tests consuming the final curl.
    struct HaskellPackages {
        /// Haskell curl binding, overridden with the final package in passthru.
        curl: Package,
    }

    /// OCaml package that tests consuming the final curl.
    struct OcamlPackages {
        /// OCaml curly binding, overridden with the final package in passthru.
        curly: Package,
    }

    /// PHP extension that tests consuming the final curl.
    struct PhpExtensions {
        /// PHP curl extension, overridden with the final package in passthru.
        curl: Package,
    }

    /// Separately maintained static package referenced by upstream's tests.
    struct StaticPackages {
        /// Existing static curl; it is not replaced with the package being authored.
        curl: Package,
    }

    /// Python packages used by curl's consuming-package test.
    struct Python {
        /// Packages associated with the caller's Python interpreter.
        pkgs: PythonPackages,
    }

    /// Only pycurl is needed from this package set.
    struct PythonPackages {
        /// Python binding overridden to use finalAttrs.finalPackage.
        pycurl: Package,
    }

    /// Fetcher test hierarchy supplied by the caller.
    struct Tests {
        /// Existing fetchpatch test variants.
        fetchpatch: FetchpatchTests,
    }

    /// Only the simple fetchpatch test participates in curl's passthru.
    struct FetchpatchTests {
        /// Test overridden with a fetchpatch that uses this curl through fetchurl.
        simple: Package,
    }

    /// Generic package test constructors remain opaque functions in nixpkgs.
    struct Testers {
        /// Checks libcurl's pkg-config metadata against finalAttrs.finalPackage.
        test_meta_pkg_config: NixValue,
    }
}

pub(super) use args::Inputs;
