# MariaDB authored in Rusix

This example defines MariaDB's client and server build recipes in Rust, sharing
one recipe across four pinned releases. Start with [model.rs](model.rs) to choose
a release, then [main.rs](main.rs) to see how it becomes generated Nix.

```rust
use rusix::interop::Nixpkgs;

let release = model::Release::V1011;
let factory = lowering::factory();
let mariadb = Nixpkgs::new()
    .try_call_package(&factory, release.arguments())
    .expect("fixed release arguments");
```

A package factory is a Nix function that returns a build recipe. The release
supplies its version and expected source archive checksum. `try_call_package`
checks the Rust argument record and describes a Nix `callPackage` call, which
fills other dependencies from the package set. Rust does not evaluate Nix or
build MariaDB; Nix resolves the actual dependencies and feature defaults later.
The resulting server package also exposes separate `client` and `server` members.

From the repository root:

```bash
cargo run --locked -p rusix --example mariadb-nixpkg
```

The executable prints the shared `factory`, a `family` containing all four
release packages, and `mariadb` selecting 10.11.10, the default at this pin.
To select another pinned release, change the enum variant in `main.rs`.

| File | Purpose |
| --- | --- |
| [model.rs](model.rs) | Release choices, fixed versions and source archive checksums |
| [inputs.rs](inputs.rs) | References to dependencies and feature arguments supplied later in Nix |
| [lowering.rs](lowering.rs) | Shared recipe and the client/server differences |
| [scripts.rs](scripts.rs) | Shell text for build phases; Rust does not execute these commands |
| [main.rs](main.rs) | Selects a release and prints the generated Nix |
| [mod.rs](mod.rs) | Exposes the factory and family for other Rust examples |

## Recipe and compatibility details

The authoritative recipe is `pkgs/servers/sql/mariadb/default.nix`, 269 lines at
nixpkgs `8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`. Its generic function has
51 arguments, including required version/hash and four lazy feature defaults.
The release policy includes 10.5.27, 10.6.20, 10.11.10 (the pinned default) and
11.4.4. The release arguments also supply the CoreServices framework for macOS.

The complete argument view supplies `args::argument_names()` to the factory and
family forwarders. The four lazy defaults are declared once and shared by both
interfaces. Only exceptional names such as `CoreServices`, `pkg-config`,
`linux-pam` and `fmt_8` need explicit naming annotations. Fixed common/client/server
refinements use `nix_record!`; dynamic fields retain ordinary Rust iteration.
Boolean conjunction uses `Expr<bool>::and`; native list concatenation uses
`NixList::concat` and `NixValue::concat_lists`. Version comparisons use the supplied
`NixLibrary::version_at_least` and `version_older`; test-name text replacement
uses `NixValue::replace_text`. These shared helpers retain lazy evaluation and
Rust call locations without local copies of their implementations.
Dependencies and source results retain `Package`; phase text uses `Expr<String>`,
and dependency/flag lists retain their element interfaces.
See [typed package authoring](../../docs/typed-package-values.md).

The common attributes are a derived Rust `Common` record, shared lazily through
a finite typed argument view using `try_bind_record`. The factory returns
`PackageFunction<Package>` and the family returns `NixAttrs<Package>`. Its lexical
factory binding retains `PackageFunction<Package>` without `as_value` conversion. This is ordinary Rust composition and Nix interop: no new derivation,
CMake, platform, dependency, metadata or shell framework was added to core.
The family export similarly shares one implementation and uses thin named-argument
forwarders so every member retains the real callPackage override interface.

The backend still supplies stdenv, fetchurl, CMake hooks, Perl's withPackages,
platform emulators, builders, package outputs and NixOS tests. No dependency
closure is rewritten and no derivations are built or fetched.

## Verification

Run `cargo test --locked -p rusix --test mariadb`. It compares exact server
and client recipes, identities, outputs, source recipes, patch bytes/store paths,
flags, phase bytes and contexts, metadata, default argument interfaces and family
membership. All 16 combinations of Mroonga, RocksDB, embedded and NUMA switches
are covered for all four releases, as are native Linux/i686/Darwin, supported
Linux/musl cross builds, source-version argument and attribute overrides,
excluded dependency laziness and client selection with invalid server-only inputs.
Evaluation artifacts live in `target/mariadb-equivalence/`.

Upstream's generic function does not use recursive finalAttrs. Its common
version/source/test selection is lexically captured. An argument override
reconstructs both client and server. An attribute override changes only the outer
server and drops the extra client/server members added after mkDerivation; the
suite explicitly compares that behavior rather than inventing recursive semantics.

Two pinned backend limitations are tested as paired upstream/Rusix rejections:
FreeBSD cross evaluation cannot supply a runnable emulator, and the
`mysql-autobackup` NixOS test defines nonexistent `nodes.machine.automysqlbackup`.
The other four real NixOS passthru tests are compared by exact test derivation
recipes. The broken test is preserved unchanged, and its raw diagnostics are
saved for both implementations. Solaris's existing libc rejection is also tested.

Patch comparisons inspect bytes and actual Nix store paths because the isolated
source symlink has a different filesystem spelling. Only metadata source
`position` is excluded. Derivation recipes are never normalized.
