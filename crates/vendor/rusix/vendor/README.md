# Pinned nixpkgs evaluation sources

`nixpkgs/` is a Git submodule of <https://github.com/NixOS/nixpkgs>, pinned to
release 24.11 commit `8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296` (the dereferenced
release tag). The upstream license is in `nixpkgs/COPYING`.

From the Rusix repository root, initialize it with:

```bash
git submodule update --init --depth=1
```

`.gitmodules` requests a shallow checkout. Initialization needs network access
unless the pinned commit is already available locally; evaluation never downloads
sources. A normal Rusix clone does not include nixpkgs data. Initializing the
submodule downloads its full source tree, without its full Git history.

The original Git package definition is directly browsable at
[`nixpkgs/pkgs/applications/version-management/git/default.nix`](nixpkgs/pkgs/applications/version-management/git/default.nix),
with patches and `update.sh` alongside it.

Before staging sources, Rusix checks that the checkout exists, HEAD matches the
compiled-in revision, and there are no modified, untracked, or ignored files.
Missing or mismatched submodules are tooling failures with initialization advice.
Git must be available on PATH. Verification is cached while sessions share a
source handle; keep the checkout unchanged while evaluation is running.

[`nixpkgs-pin.json`](../crates/rusix/src/nixos/nixpkgs-pin.json) records that same revision and individual SHA-256 digests for
232 upstream files (about 1 MB of content). The minimal module evaluator verifies
every listed file once per process and stages the checked subset once per
disposable session. Later evaluations reuse the staged files.
The manifest is reviewed repository data, not an upstream signature. Its
`diagnostic_files` hashes identify the supported upstream dependency validator
even when a library consumer supplies a checkout outside this repository.
Consumers select that existing checkout with `NixSession::with_nixpkgs(path)`;
revision and cleanliness checks remain the same.

The minimal selection is all of upstream `lib/`, root `.version`,
`.version-suffix`, `COPYING`, and these four modules:

- `nixos/modules/services/networking/ssh/sshd.nix`
- `nixos/modules/misc/assertions.nix`
- `nixos/modules/misc/label.nix`
- `nixos/modules/system/activation/top-level.nix`

The last file is a reference for assertion enforcement, never an evaluated
import. The minimal evaluator does not load a package set or system builder.

Interoperability sessions link the same full checkout as `nixpkgs-full`; there is
no archive extraction. Source files are shared, but stores are always disposable
and private. Dropping a session leaves the submodule intact. Interop imports
upstream `default.nix` with explicit system/config/overlays and uses the actual
`config/system-path.nix` option declarations. Metadata evaluation can create
`.drv` records and source paths exclusively in the disposable store. It does not
build outputs. Import-from-derivation is disabled; substitutions and builders
are empty. All Nix commands use the private offline helper.

To update nixpkgs deliberately, update the submodule gitlink, the backend's
`NIXPKGS_REVISION`, and this checksum manifest together, then run all repository
checks. No automatic update or downloading step runs during evaluation.
