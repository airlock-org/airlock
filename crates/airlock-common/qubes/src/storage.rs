//! Hold Qubes' own database lock across native comparison, volume construction and save.

use super::{snapshot, values};
use anyhow::{Context, Result, ensure};
use pyo3::{prelude::*, types::PyDict};
use serde_json::{Value, json};

/// Await upstream APIs that may return either synchronous values or coroutine objects.
pub fn complete(value: Bound<'_, PyAny>, runner: &Bound<'_, PyAny>) -> Result<()> {
    let py = value.py();
    if py
        .import("inspect")?
        .call_method1("isawaitable", (&value,))?
        .is_truthy()?
    {
        let bounded = py
            .import("asyncio")?
            .call_method1("wait_for", (value, 120))?;
        runner
            .call_method0("get_loop")?
            .call_method1("run_until_complete", (bounded,))?;
    }
    Ok(())
}

/// Read or install exact native volume definitions through upstream Qubes objects.
/// The caller supplies native guards; no lock is released between comparison and save.
/// Run this in a short-lived worker because Qubes extension singletons are process-global.
pub fn execute(request: &Value, guard: impl Fn(Value) -> Result<Value>) -> Result<Value> {
    ensure!(
        matches!(request["operation"].as_str(), Some("snapshot" | "install")),
        "unsupported Qubes object operation"
    );
    Python::attach(|py| {
        let runner = py.import("asyncio")?.getattr("Runner")?.call0()?;
        runner.call_method0("__enter__")?;
        let result = with_app(py, request, &guard, &runner);
        let closed = runner
            .call_method0("close")
            .context("close Qubes storage event loop");
        match result {
            Ok(value) => {
                closed?;
                Ok(value)
            }
            Err(error) => {
                if let Err(close) = closed {
                    eprintln!("Qubes event loop cleanup: {close:#}");
                }
                Err(error)
            }
        }
    })
}

/// Open upstream objects, close them on every result, and leave extension discovery to Qubes.
fn with_app(
    py: Python<'_>,
    request: &Value,
    guard: &impl Fn(Value) -> Result<Value>,
    runner: &Bound<'_, PyAny>,
) -> Result<Value> {
    let install = request["operation"] == "install";
    let options = PyDict::new(py);
    if install {
        options.set_item("load", false)?;
    }
    let app = py
        .import("qubes")?
        .getattr("Qubes")?
        .call((), Some(&options))?;
    let result = on_app(&app, request, guard, runner);
    let closed = app
        .call_method0("close")
        .context("close Qubes storage objects");
    match result {
        Ok(value) => {
            closed?;
            Ok(value)
        }
        Err(error) => {
            if let Err(close) = closed {
                eprintln!("Qubes object cleanup: {close:#}");
            }
            Err(error)
        }
    }
}

/// Read or update a caller-owned Qubes object graph using upstream database locking.
/// The caller closes the object graph and event loop after success or failure.
pub fn on_app(
    app: &Bound<'_, PyAny>,
    request: &Value,
    guard: &impl Fn(Value) -> Result<Value>,
    runner: &Bound<'_, PyAny>,
) -> Result<Value> {
    ensure!(
        matches!(request["operation"].as_str(), Some("snapshot" | "install")),
        "unsupported Qubes object operation"
    );
    let install = request["operation"] == "install";
    let py = app.py();
    let locked = PyDict::new(py);
    locked.set_item("lock", true)?;
    if install {
        app.call_method("load", (), Some(&locked))?;
    }
    let name = request["domain"]
        .as_str()
        .context("missing Qubes VM name")?;
    let vm = app.getattr("domains")?.get_item(name)?;
    if vm.getattr("storage")?.is_none() {
        vm.setattr(
            "storage",
            py.import("qubes.storage")?
                .getattr("Storage")?
                .call1((&vm,))?,
        )?;
    }
    let before = snapshot::read(app, &vm)?;
    if !install {
        return Ok(before);
    }
    guard(
        json!({"operation": "authorize-install", "snapshot": before, "expected": request["expected"]}),
    )?;

    let pool = &request["pool"];
    let pools = app.getattr("pools")?;
    if !pools.contains(pool["name"].as_str().context("missing pool name")?)? {
        let options = values::python(py, pool)?
            .cast_into::<PyDict>()
            .map_err(PyErr::from)?;
        complete(app.call_method("add_pool", (), Some(&options))?, runner)?;
    }
    for config in request["volumes"]
        .as_array()
        .context("missing Qubes volumes")?
    {
        let name = config["name"].as_str().context("missing volume name")?;
        vm.getattr("volume_config")?
            .set_item(name, values::python(py, config)?)?;
        let volume = vm
            .getattr("storage")?
            .call_method1("init_volume", (name, values::python(py, config)?))?;
        complete(volume.call_method0("create")?, runner)?;
    }

    guard(json!({
        "operation": "require-stopped",
        "klass": vm.getattr("klass")?.extract::<String>()?,
        "running": vm.call_method0("is_running")?.is_truthy()?,
    }))?;
    app.call_method("save", (), Some(&locked))?;
    snapshot::read(app, &vm)
}
