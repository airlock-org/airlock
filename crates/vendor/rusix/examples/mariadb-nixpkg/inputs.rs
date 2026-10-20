//! Describes the dependencies and feature switches accepted by the pinned MariaDB Nix function.
//! Rust accessors generate references; Nix resolves their values only when the recipe needs them.

/// Declares the release, dependencies and features supplied to MariaDB's Nix function.
/// Accessors describe argument lookups without reading those values in Rust.
#[rusix::args]
pub mod args {
    use rusix::interop::{NixCallable, NixLibrary, Package, Stdenv, raw::NixValue};

    /// Deferred common and server dependencies plus the four upstream feature defaults.
    #[rusix(root)]
    struct Inputs {
        /// Release version used in the source archive URL and version-dependent recipe choices.
        version: String,
        /// Expected SHA-256 checksum of the source archive, checked when Nix fetches it.
        hash: String,
        /// Utility functions and package metadata supplied by the Nix caller.
        lib: NixLibrary,
        /// Standard build environment: compiler, platform information and build-recipe constructor.
        stdenv: Stdenv,
        /// Source fetcher that describes downloading an archive with an expected checksum.
        fetchurl: NixCallable<Package>,
        /// Existing NixOS integration tests selected for the chosen MariaDB release.
        nixos_tests: NixValue,
        /// Packages that run on the build machine, even when compiling for another platform.
        build_packages: NixValue,
        /// Parser generator used while building the server.
        bison: Package,
        /// C++ headers used while building the server.
        boost: Package,
        /// Build tool and setup hook that configure the client and server.
        cmake: Package,
        /// Build hook that repairs shared-library paths on macOS.
        fix_darwin_dylib_names: Package,
        /// Lexer generator used while building the server.
        flex: Package,
        /// Build hook that sets the program search path for the mytop monitoring script.
        make_wrapper: Package,
        /// Build tool that locates dependency headers and libraries.
        #[rusix(rename = "pkg-config")]
        pkg_config: Package,
        /// HTTP client library linked into the client and server recipes.
        curl: Package,
        /// Character-encoding conversion library used by both recipes.
        libiconv: Package,
        /// Terminal library used by the client and the mytop monitoring environment.
        ncurses: Package,
        /// TLS library linked into the client and server recipes.
        openssl: Package,
        /// Regular-expression library used by both recipes.
        pcre2: Package,
        /// Kerberos authentication library included on Linux.
        libkrb5: Package,
        /// Linux asynchronous-I/O library selected for releases older than 10.6.
        libaio: Package,
        /// Linux io_uring library selected for releases from 10.6 onward.
        liburing: Package,
        /// Linux service-manager dependency included by the shared recipe.
        systemd: Package,
        /// Apple CoreServices framework included on macOS.
        #[rusix(rename = "CoreServices")]
        core_services: Package,
        /// Apple toolchain utilities included by the macOS recipe.
        cctools: Package,
        /// Perl dependency included by the macOS recipe.
        perl: Package,
        /// Memory allocator used on platforms other than macOS.
        jemalloc: Package,
        /// Pager placed on the mytop monitoring script's program search path.
        less: Package,
        /// Line-editing library included on macOS.
        libedit: Package,
        /// Bzip2 compression library included in the server recipe.
        bzip2: Package,
        /// LZ4 compression library included in the server recipe.
        lz4: Package,
        /// LZO compression library included in the server recipe.
        lzo: Package,
        /// Snappy compression library included in the server recipe.
        snappy: Package,
        /// LZMA compression library included in the server recipe.
        xz: Package,
        /// Compression library shared by client and server; the recipe selects the system library.
        zlib: Package,
        /// Zstandard compression library included in the server recipe.
        zstd: Package,
        /// Password-checking library included in the server recipe.
        cracklib: Package,
        /// Array library included in the server recipe.
        judy: Package,
        /// Event-loop library included in the server recipe.
        libevent: Package,
        /// XML library included in the server recipe.
        libxml2: Package,
        /// Authentication-module library included in the Linux server recipe.
        #[rusix(rename = "linux-pam")]
        linux_pam: Package,
        /// NUMA library added and propagated when withNuma is enabled.
        numactl: Package,
        /// C++ formatting library included in both recipes from release 10.7 onward.
        #[rusix(rename = "fmt_8")]
        fmt_8: Package,
        /// Enables the Mroonga full-text storage engine and its dependencies; defaults to true.
        with_storage_mroonga: bool,
        /// Text-analysis dependency included when Mroonga is enabled.
        kytea: Package,
        /// Cryptography dependency included when Mroonga is enabled.
        libsodium: Package,
        /// MessagePack dependency included when Mroonga is enabled.
        msgpack: Package,
        /// Messaging dependency included when Mroonga is enabled.
        zeromq: Package,
        /// Enables the RocksDB storage engine; defaults to true in Nix.
        with_storage_rocks: bool,
        /// Enables the embedded server and retains development files; defaults to false.
        with_embedded: bool,
        /// Enables NUMA support and its numactl dependency; defaults to false in Nix.
        with_numa: bool,
    }
}

pub use args::Inputs;
