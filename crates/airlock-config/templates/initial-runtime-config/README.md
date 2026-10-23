# Runtime configuration

Edit the VM declarations in `src/machine.rs` and host changes in `src/main.rs`.
The candidate uses the same validated VM identities and relationships as machine
assembly. `config-inputs.json` and the Cargo dependency select admitted local
source trees supplied by configuration-input transport.

Commit the Rust sources and dependency locks. `make prepare` compiles the
candidate into an ignored `target/nix/<hash>/` source tree and prints its `path:`
flake reference. `AIRLOCK_PREPARED_CACHE` overrides that location. The prepared tree
retains source identity and file hashes for inspection and reproducibility.
Preparation neither builds a system nor deploys it; the controller reviews the
committed source and validates its prepared interface before applying a change.
