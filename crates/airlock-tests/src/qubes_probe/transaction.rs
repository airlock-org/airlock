//! Observe real Qubes file locking and persistence using a disposable offline database.

use airlock_qubes::{snapshot, storage, values};
use anyhow::{Context, Result, ensure};
use pyo3::{prelude::*, types::PyDict};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    fs,
    io::Write,
    process::{Command, Stdio},
};

/// Attempt POSIX record locking from another process; same-process lock tests would be misleading.
pub(super) fn locked(py: Python<'_>, request: &Value) -> Result<Value> {
    let path = request["store"].as_str().context("Qubes lock probe path")?;
    let file = py
        .import("builtins")?
        .getattr("open")?
        .call1((path, "r+b"))?;
    let fcntl = py.import("fcntl")?;
    let flags =
        fcntl.getattr("LOCK_EX")?.extract::<i32>()? | fcntl.getattr("LOCK_NB")?.extract::<i32>()?;
    let result = fcntl.call_method1("lockf", (&file, flags));
    let held = match result {
        Ok(_) => false,
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyBlockingIOError>(py) => true,
        Err(error) => return Err(error.into()),
    };
    file.call_method0("close")?;
    Ok(json!({"locked":held}))
}

/// Run the lock contender in a new process so it cannot inherit the caller's POSIX lock ownership.
fn contended(store: &str) -> Result<bool> {
    let mut child = Command::new(std::env::current_exe()?)
        .arg("store-lock")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .context("lock contender input")?
        .write_all(&serde_json::to_vec(&json!({"store":store}))?)?;
    let output = child.wait_with_output()?;
    ensure!(
        output.status.success(),
        "Qubes lock contender failed: {error}",
        error = String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice::<Value>(&output.stdout)?["locked"] == true)
}

/// Open a deliberately offline database through the actual upstream constructor.
fn open<'py>(py: Python<'py>, store: &str, load: bool) -> PyResult<Bound<'py, PyAny>> {
    let kwargs = PyDict::new(py);
    kwargs.set_item("offline_mode", true)?;
    kwargs.set_item("load", load)?;
    py.import("qubes")?
        .getattr("Qubes")?
        .call((store,), Some(&kwargs))
}

/// Compare resolved identities and properties; Qubes reloads raw XML configuration scalars as strings.
fn resolved(snapshot: &Value) -> Result<Vec<Value>> {
    snapshot["volumes"]
        .as_array()
        .context("upstream volume records")?
        .iter()
        .map(|volume| {
            let mut properties = volume
                .as_object()
                .context("upstream volume properties")?
                .clone();
            properties.remove("config");
            Ok(Value::Object(properties))
        })
        .collect()
}

/// Exercise production rejection under the real database lock and preserve writable state on reload.
pub(super) fn observe(py: Python<'_>) -> Result<Value> {
    let directory = tempfile::tempdir()?;
    let root = directory.path().to_str().context("offline database root")?;
    let store = directory.path().join("qubes.xml");
    let store = store.to_str().context("offline database path")?;
    fs::create_dir(directory.path().join("vm-kernels"))?;
    let kernel = directory.path().join("vm-kernels/test-kernel");
    fs::create_dir(&kernel)?;
    for name in ["vmlinuz", "modules.img"] {
        fs::write(kernel.join(name), "fixture")?;
    }
    let config = py.import("qubes.config")?;
    config.setattr("qubes_base_dir", root)?;
    config.getattr("defaults")?.set_item(
        "pool_configs",
        values::python(
            py,
            &json!({
                "varlibqubes":{"name":"varlibqubes", "driver":"file", "dir_path":root},
                "linux-kernel":{
                    "name":"linux-kernel", "driver":"linux-kernel",
                    "dir_path":directory.path().join("vm-kernels"),
                },
            }),
        )?,
    )?;
    py.import("qubes.ext")?
        .setattr("get_extensions", py.import("builtins")?.getattr("set")?)?;
    py.import("qubes.vm.appvm")?;
    let runner = py.import("asyncio")?.getattr("Runner")?.call0()?;
    runner.call_method0("__enter__")?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("offline_mode", true)?;
    let app = py.import("qubes")?.getattr("Qubes")?.call_method(
        "create_empty_store",
        (store,),
        Some(&kwargs),
    )?;
    app.setattr("events_enabled", false)?;
    app.setattr("default_kernel", "")?;
    app.setattr("default_template", py.None())?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("name", "usr-home")?;
    kwargs.set_item("label", "green")?;
    let vm = app.call_method("add_new_vm", ("StandaloneVM",), Some(&kwargs))?;
    vm.setattr("events_enabled", false)?;
    vm.setattr("kernel", "test-kernel")?;
    vm.setattr("netvm", py.None())?;
    vm.setattr("guivm", py.None())?;
    fs::create_dir_all(vm.getattr("dir_path")?.extract::<std::path::PathBuf>()?)?;
    app.call_method0("save")?;
    app.call_method0("close")?;

    let app = open(py, store, true)?;
    let vm = app.getattr("domains")?.get_item("usr-home")?;
    let before = snapshot::read(&app, &vm).context("read initial upstream storage objects")?;
    app.call_method0("close")?;
    let mut volume = before["volumes"]
        .as_array()
        .context("upstream volumes")?
        .iter()
        .find(|volume| volume["name"] == "private")
        .context("upstream private volume")?["config"]
        .clone();
    volume["size"] = json!(1048576);
    let request = json!({"operation":"install", "domain":"usr-home", "expected":before,
        "pool":{"name":"airlock-closure", "driver":"airlock-closure", "root_dir":root, "revisions_to_keep":0}, "volumes":[volume]});
    let guards = Cell::new(0);
    let guard = |request: Value| -> Result<Value> {
        ensure!(
            contended(store)?,
            "Qubes released its database lock before the native guard"
        );
        guards.set(guards.get() + 1);
        airlock_dom0_service::storage_request(request)
    };
    let saved = fs::read(store)?;
    let app = open(py, store, false)?;
    let rejected_private = storage::on_app(&app, &request, &guard, &runner).is_err();
    app.call_method0("close")?;
    ensure!(rejected_private && saved == fs::read(store)?, "immutable attachment changed private storage");
    ensure!(
        !contended(store)?,
        "Qubes did not release its database lock"
    );

    let mut stale = request;
    stale["expected"]["running"] = json!(true);
    let app = open(py, store, false)?;
    let rejected = storage::on_app(
        &app,
        &stale,
        &guard,
        &runner,
    )
    .is_err();
    app.call_method0("close")?;
    ensure!(
        rejected && saved == fs::read(store)?,
        "stale storage request changed the database"
    );
    let app = open(py, store, true)?;
    let vm = app.getattr("domains")?.get_item("usr-home")?;
    let persisted = snapshot::read(&app, &vm)?;
    app.call_method0("close")?;
    runner.call_method0("close")?;
    ensure!(
        resolved(&before)? == resolved(&persisted)?,
        "Qubes resolved volume properties changed on reload"
    );
    Ok(
        json!({"guards_under_lock":guards.get(), "private_unchanged":true, "private_effect_rejected":rejected_private,
        "stale_rejected":rejected, "lock_released":true}),
    )
}
