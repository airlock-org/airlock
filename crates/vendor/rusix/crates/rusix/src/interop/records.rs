//! Read platform information and final build attributes supplied by nixpkgs.
//! These views describe deferred field lookups, preserving the complete Nix records.
// Only the nested views are instantiated; the root declares their structure.
#[allow(dead_code)]
#[crate::args]
mod views {
    use crate::Expr;
    use crate::interop::{NixCallable, Package};

    #[rusix(root)]
    struct Inputs {
        /// Supplied machine description used to choose platform-specific build behavior.
        platform: Platform,
        /// Final build-recipe fields supplied by mkDerivation, including overridden values.
        final_attrs: FinalAttrs,
    }

    /// Information about a machine's CPU, operating system and compiler conventions.
    ///
    /// nixpkgs uses platform records to distinguish where build commands execute,
    /// where the built program runs, and where a compiler's generated code runs.
    /// Obtain these views from [`crate::interop::Stdenv::build_platform`],
    /// [`crate::interop::Stdenv::host_platform`] and
    /// [`crate::interop::Stdenv::target_platform`] respectively.
    /// Use the fields to describe platform-specific package inputs or commands.
    ///
    /// This view exposes commonly used properties of the supplied Nix record.
    /// Accessors construct expressions in Rust; Nix reads the properties later.
    /// It does not reconstruct a platform or guarantee every field exists.
    #[rusix(value)]
    struct Platform {
        /// Nix system name identifying the CPU and operating system, such as `x86_64-linux`.
        system: String,
        /// Toolchain target identifier, such as `x86_64-unknown-linux-gnu`.
        config: String,
        /// C library identifier, such as `glibc`, `musl`, or `libSystem`.
        libc: String,
        /// Whether the platform uses the Linux kernel.
        is_linux: bool,
        /// Whether the platform belongs to the Darwin family, including macOS and iOS.
        is_darwin: bool,
        /// Whether the platform uses Windows, including MinGW and Cygwin environments.
        is_windows: bool,
        /// Whether nixpkgs configured this platform for static linking, as with `pkgsStatic`.
        is_static: bool,
        /// Whether the platform uses the musl C library.
        is_musl: bool,
        /// Whether the Windows platform uses the Cygwin environment.
        is_cygwin: bool,
        /// Whether the platform uses FreeBSD.
        #[rusix(rename = "isFreeBSD")]
        is_free_bsd: bool,
        /// Whether the platform uses OpenBSD.
        #[rusix(rename = "isOpenBSD")]
        is_open_bsd: bool,
        /// Whether the platform uses the Solaris/SunOS kernel.
        #[rusix(rename = "isSunOS")]
        is_sun_os: bool,
        /// Whether the operating system belongs to the BSD family.
        #[rusix(rename = "isBSD")]
        is_bsd: bool,
        /// Whether the Windows platform uses the MinGW toolchain environment.
        #[rusix(rename = "isMinGW")]
        is_min_gw: bool,
        /// Whether the platform uses iOS.
        #[rusix(rename = "isiOS")]
        is_ios: bool,
        /// Whether executables use the ELF binary format, common on Linux and BSD.
        #[rusix(rename = "isElf")]
        is_elf: bool,
        /// Whether the CPU is in the 64-bit ARM family (AArch64).
        #[rusix(rename = "isAarch64")]
        is_aarch64: bool,
        /// Whether the parsed CPU type is specifically i686, a 32-bit x86 variant.
        #[rusix(rename = "isi686")]
        is_i686: bool,
        /// Whether the CPU is in the 64-bit x86 family.
        #[rusix(rename = "isx86_64")]
        is_x86_64: bool,
        /// Whether the CPU is in the 32-bit x86 family, including i686.
        #[rusix(rename = "isx86_32")]
        is_x86_32: bool,
        /// Whether the CPU belongs to the MIPS family, regardless of its bit width.
        #[rusix(rename = "isMips")]
        is_mips: bool,
        /// Whether the CPU is in the 32-bit MIPS family.
        #[rusix(rename = "isMips32")]
        is_mips32: bool,
        /// Whether a 64-bit MIPS CPU uses the n32 calling convention with 32-bit pointers.
        #[rusix(rename = "isMips64n32")]
        is_mips64n32: bool,
        /// Whether a 64-bit MIPS CPU uses the n64 calling convention with 64-bit pointers.
        #[rusix(rename = "isMips64n64")]
        is_mips64n64: bool,
        /// Whether the CPU belongs to the MicroBlaze family.
        #[rusix(rename = "isMicroBlaze")]
        is_micro_blaze: bool,
        /// Machine details extracted from the platform's toolchain identifier.
        /// `parsed.cpu.bits()` describes the hardware bit width, typically 32 or 64;
        /// it is not necessarily the pointer width used by the calling convention.
        parsed: Parsed,
        /// Conventional filename suffixes for this platform's build outputs.
        /// `extensions.shared_library()` describes the shared-library suffix,
        /// including the dot, such as `.so`, `.dylib`, or `.dll`.
        extensions: Extensions,
        /// Platform-specific settings for the GCC compiler.
        /// `gcc.arch()` describes the CPU architecture name used for flags such as
        /// `-march`. Some platforms omit it; `gcc.as_attrs().has("arch")` describes
        /// a Nix presence check that can guard a later selection.
        gcc: Gcc,
        /// Function taking a package set and returning the path to a program that
        /// runs this platform's executables, using an emulator when needed.
        /// The supplied package set determines available tools and their execution
        /// platform. Nix reports an error if it cannot choose a suitable program.
        emulator: NixCallable<Expr<String>>,
    }

    /// Machine details parsed by nixpkgs from a platform's toolchain identifier.
    struct Parsed {
        /// CPU properties, including its hardware bit width.
        cpu: Cpu,
    }

    /// CPU properties used to choose architecture-specific build behavior.
    struct Cpu {
        /// Hardware bit width, typically 32 or 64; it is not necessarily the pointer width.
        bits: i64,
    }

    /// Conventional filename suffixes used by this platform.
    struct Extensions {
        /// Shared-library filename suffix, including the dot, such as `.so`, `.dylib`, or `.dll`.
        shared_library: String,
    }

    /// Platform-specific settings for the GCC compiler.
    #[rusix(value)]
    struct Gcc {
        /// GCC CPU architecture name, used for architecture flags such as `-march`.
        /// This field may be absent; check `as_attrs().has("arch")` before selecting it.
        arch: String,
    }

    /// The build attributes after overrides, supplied to a `mkDerivation` callback.
    ///
    /// nixpkgs lets a recipe be a function of its final attributes, so one field
    /// can use another field's overridden value rather than its original value.
    /// The callback also receives the resulting package itself as `finalPackage`.
    /// Rust builds these references once; Nix resolves them lazily when needed.
    /// Avoid a field depending on itself without a terminating condition.
    /// Other fields remain accessible through `as_attrs()`.
    ///
    /// ```
    /// use rusix::interop::{FinalAttrs, NixAttrs, NixCallable, raw::NixValue};
    ///
    /// let recipe = NixCallable::<NixAttrs, FinalAttrs>::from_function(|final_attrs| {
    ///     NixAttrs::new([
    ///         ("pname", NixValue::from("demo")),
    ///         ("version", NixValue::from("1.0")),
    ///         ("name", final_attrs.version().with_prefix("demo-").into()),
    ///     ])
    /// });
    /// // Like Nix: finalAttrs: { pname = "demo"; version = "1.0";
    /// //                         name = "demo-" + finalAttrs.version; }.
    /// // Pass recipe to stdenv.builder().call(recipe) to describe the package.
    /// ```
    #[rusix(value)]
    struct FinalAttrs {
        /// The package produced from the final recipe, including its overridden fields.
        /// Use it to refer to the package's outputs or attach package-dependent data.
        final_package: Package,
        /// The recipe's final version string, including any `overrideAttrs` replacement.
        version: String,
    }
}

pub use views::{FinalAttrs, Platform};
