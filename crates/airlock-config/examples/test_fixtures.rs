//! Emit test modules and inspection queries through Rusix for integration tests.
//! Export scenarios also write snapshots into caller-supplied temporary directories.
//! Callers explicitly choose evaluation stores and build policy.

#[path = "test_authoring/mod.rs"]
mod fixtures;

use rusix::interop::{
    NixExpression, NixPath,
    raw::{NixRepresentation, NixValue},
};

/// Convert JSON input data into deferred literal values without accepting executable source.
fn data(value: serde_json::Value) -> NixValue {
    match value {
        serde_json::Value::Null => NixValue::null(),
        serde_json::Value::Bool(value) => value.into(),
        serde_json::Value::Number(value) => value
            .as_i64()
            .expect("fixture integers must fit i64")
            .into(),
        serde_json::Value::String(value) => value.into(),
        serde_json::Value::Array(values) => NixValue::list(values.into_iter().map(data)),
        serde_json::Value::Object(values) => {
            NixValue::record(values.into_iter().map(|(name, value)| (name, data(value))))
        }
    }
}

/// Preserve path values when appending source-relative filenames; other operands use Nix addition.
fn add(left: NixValue, right: NixValue) -> NixValue {
    let kind = NixValue::builtin("typeOf").call(left.clone());
    let text = NixValue::concat_text([left.clone().to_text(), right.clone().to_text()]);

    NixValue::if_else(
        kind.clone().equals("path"),
        NixPath::from_source_text(text.clone().into_expr::<String>()).as_expression(),
        NixValue::if_else(
            kind.equals("string"),
            text,
            NixValue::builtin("add").call(left).call(right),
        ),
    )
}

/// Tie a lazy recursive definition without evaluating its recursive argument in Rust.
fn recursive(build: impl Fn(NixValue) -> NixValue) -> NixValue {
    let step = NixValue::function(|value| build(value.clone().call(value)));
    step.clone().call(step)
}

/// Resolve a fixture-relative source path using the working directory selected by the caller.
fn source_path(path: &str) -> NixValue {
    let path = std::path::Path::new(path);
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .expect("fixture working directory")
            .join(path)
    };
    NixPath::from_source_text(absolute.to_string_lossy().into_owned()).as_expression()
}

/// Emit one named fixture from JSON data; generation never invokes Nix or provisions tools.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let name = arguments.next().ok_or("expected a fixture name")?;
    let settings: serde_json::Value =
        serde_json::from_str(&arguments.next().unwrap_or_else(|| "{}".into()))?;
    let inputs = data(settings.clone());
    let mut value = fixtures::fixture(&name, inputs, &settings)?;
    if ["topology_schema/", "system_schema/", "config_activation/"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        let source = settings["nixpkgs_source"]
            .as_str()
            .ok_or("schema flakes need a pinned nixpkgs source")?;
        value = NixValue::record([
            (
                "inputs",
                field(["nixpkgs", "url"], format!("path:{source}")),
            ),
            (
                "outputs",
                NixValue::function_attrs(["self", "nixpkgs"], |arguments| {
                    (
                        Vec::<(&str, NixValue)>::new(),
                        value
                            .select("outputs")
                            .call(field(["self"], arguments.select("self"))),
                    )
                }),
            ),
        ]);
    }
    let generated = rusix::compile_value(value)
        .map_err(|diagnostic| diagnostic.render(std::path::Path::new(".")))?;

    print!("{source}", source = generated.source);
    Ok(())
}

/// Rust expression handles shared by successive stages of an inspection scenario.
/// This table is constructed in Rust; it adds no options or bindings to the evaluated system.
struct Bindings {
    /// Named handles captured from the enclosing scenario, retained without Nix evaluation.
    values: std::collections::BTreeMap<&'static str, NixValue>,
}

impl Bindings {
    /// Collect the deferred handles needed by one inspection stage.
    fn new(values: impl IntoIterator<Item = (&'static str, NixValue)>) -> Self {
        Self {
            values: values.into_iter().collect(),
        }
    }

    /// Reuse a handle; an absent name indicates an incorrectly wired Rust test fixture.
    fn get(&self, name: &str) -> NixValue {
        self.values
            .get(name)
            .expect("missing Rust fixture capture")
            .clone()
    }
}

/// Place a deferred value beneath literal attribute segments, preserving dots inside a filename.
fn field<const N: usize>(path: [&str; N], value: impl Into<NixValue>) -> NixValue {
    path.into_iter().rev().fold(value.into(), |value, name| {
        NixValue::record([(name, value)])
    })
}
