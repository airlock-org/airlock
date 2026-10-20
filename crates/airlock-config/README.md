# Airlock configuration

This crate authors VM systems, dom0 services, topology and deployment contracts,
flake assembly and installation media in Rust. Rusix emits ordinary Nix artifacts;
NixOS and upstream Qubes still own module evaluation, Xen, qrexec, QubesDB and
VM lifecycle.

Machine declarations belong to the standalone `airlock-machine/src/` crate.
Each block declares a VM, configures its typed handle, then registers it.
The authoring model checks identities, role compatibility and relationships before
freezing a snapshot. Runtime services reuse the same identity and connection
contracts. Components own their package recipes and NixOS contributions.

The controller candidate is also a Rust crate. Its authoring API separates
ordinary host package/file changes from arbitrary modules, whose automatic
activation is staged for the next boot. Commit Rust declarations and dependency
locks for review. Preparation never applies a machine change.

Generated Nix is an inspectable build interface kept in the ignored
`target/nix/<hash>/` directory of each checkout. `make prepare` emits this
content-addressed source tree and prints its `path:` flake reference. Set
`AIRLOCK_PREPARED_CACHE` to use another ignored directory or an external cache. Its receipt records the source revision, working source contents and
prepared file hashes. Nix evaluates that tree after Rust compilation finishes;
it never compiles the authoring program during evaluation. Build and deployment
records retain the required artifacts independently of disposable build output.
Source maps remain available through inspection exports. Evidence belongs in
`.airlock-logs/`.
