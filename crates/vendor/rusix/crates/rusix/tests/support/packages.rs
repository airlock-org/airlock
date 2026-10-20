//! Shared evaluation harness; comparisons keep exact recipes and original diagnostics.
use rusix::interop::raw::{NixFunctionExt, NixpkgsExt};
use rusix::{
    Config,
    interop::{InputRef, Nixpkgs, raw::NixValue},
};
use rusix::{Generated, NixSession, compile};
use std::{
    fs,
    path::Path,
    sync::{Mutex, OnceLock},
};

pub fn session() -> &'static Mutex<NixSession> {
    static SESSION: OnceLock<Mutex<NixSession>> = OnceLock::new();
    SESSION.get_or_init(|| Mutex::new(NixSession::new().unwrap()))
}

pub fn artifact(
    suite: &str,
    fields: impl IntoIterator<Item = (&'static str, NixValue)>,
) -> Generated {
    let mut fields: Vec<_> = fields.into_iter().collect();
    fields.push(("nixpkgs", Nixpkgs::new().value("path")));
    let result = InputRef::local(
        suite,
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/{suite}-equivalence.nix")),
    )
    .function("compare")
    .call(NixValue::record(fields));
    compile(Config::new().set_dynamic("result", result)).unwrap()
}

pub fn compare(suite: &str, name: &str, generated: Generated) -> serde_json::Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/{suite}-equivalence/{name}"));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("generated.nix"), &generated.source).unwrap();
    let result = session()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .evaluate_interop(&generated)
        .unwrap_or_else(|e| panic!("{suite}/{name}: {}", e.reason))
        .value;
    fs::write(
        root.join("comparison.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    let a = &result["result"]["upstream"];
    let b = &result["result"]["candidate"];
    if a != b {
        let keys: Vec<_> = a
            .as_object()
            .unwrap()
            .keys()
            .filter(|k| a[*k] != b[*k])
            .collect();
        panic!(
            "{suite}/{name}: different {keys:?}; inspect {}",
            root.display()
        );
    }
    result["result"]["candidate"].clone()
}
