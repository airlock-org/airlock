//! Observe actual Qubes event ordering with native extension callbacks and inert VM objects.

use airlock_qubes::{descriptor, values};
use anyhow::Result;
use pyo3::{
    prelude::*,
    types::{PyDict, PyTuple},
};
use serde_json::{Value, json};

/// Supply upstream inherited graphics defaults while intentionally assigning no PCI device.
#[pyfunction]
#[pyo3(signature=(key, default=None))]
fn feature(py: Python<'_>, key: &str, default: Option<Py<PyAny>>) -> PyResult<Py<PyAny>> {
    if key == "video-model" {
        return Ok("none".into_pyobject(py)?.into_any().unbind());
    }
    Ok(default.unwrap_or_else(|| py.None()))
}

/// No physical devices are involved in a dispatcher-ordering regression.
#[pyfunction]
fn assignments(py: Python<'_>) -> Py<PyAny> {
    pyo3::types::PyList::empty(py).into_any().unbind()
}

/// Supply only Emitter's instance state and a class handler that records whether dispatch reached it.
fn vm_method(
    name: &str,
    args: &Bound<'_, PyTuple>,
    _kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py = args.py();
    let vm = args.get_item(0)?;
    match name {
        "__init__" => {
            py.import("qubes.events")?
                .getattr("Emitter")?
                .call_method1("__init__", (&vm,))?;
            vm.setattr("events_enabled", true)?;
            vm.setattr("name", args.get_item(1)?)?;
            vm.setattr("running", args.get_item(2)?)?;
            vm.setattr("uuid", "a7e27839-b68f-4c21-8b91-6861db3ec112")?;
            vm.setattr("virt_mode", "hvm")?;
            vm.setattr("reached_device_handler", false)?;
            let environment = super::namespace(py, &json!({"filters":{}}))
                .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
            let app = super::namespace(py, &json!({}))
                .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
            app.setattr("env", environment)?;
            vm.setattr("app", app)?;
            let features = super::namespace(py, &json!({}))
                .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
            features.setattr("check_with_template", wrap_pyfunction!(feature, py)?)?;
            vm.setattr("features", features)?;
            let pci = super::namespace(py, &json!({}))
                .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
            pci.setattr("get_assigned_devices", wrap_pyfunction!(assignments, py)?)?;
            let devices = PyDict::new(py);
            devices.set_item("pci", pci)?;
            vm.setattr("devices", devices)?;
        }
        "is_running" => return Ok(vm.getattr("running")?.unbind()),
        "device_handler" => vm.setattr("reached_device_handler", true)?,
        _ => return Err(pyo3::exceptions::PyAttributeError::new_err(name.to_owned())),
    }
    Ok(py.None())
}

/// Report dispatch barriers through upstream Emitter, including ordinary unconfigured VMs.
pub(super) fn observe(py: Python<'_>) -> Result<Value> {
    let events = [
        "domain-pre-start",
        "device-pre-attach:pci",
        "device-pre-detach:pci",
    ];
    let namespace = PyDict::new(py);
    for name in ["__init__", "is_running"] {
        namespace.set_item(name, descriptor::method(py, name, vm_method, false)?)?;
    }
    let method = descriptor::method(py, "device_handler", vm_method, false)?;
    let decorate = py
        .import("qubes.events")?
        .getattr("handler")?
        .call(PyTuple::new(py, events)?, None)?;
    namespace.set_item("device_handler", decorate.call1((method,))?)?;
    let vm_class = descriptor::subclass(
        py,
        "ObservedVM",
        py.import("qubes.events")?.getattr("Emitter")?,
        &namespace,
    )?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("group", "qubes.ext")?;
    let entries =
        py.import("importlib.metadata")?
            .call_method("entry_points", (), Some(&kwargs))?;
    let mut extension_class = None;
    for entry in entries.try_iter()? {
        let entry = entry?;
        if entry.getattr("name")?.extract::<String>()? == "airlock-amd-vbios" {
            extension_class = Some(entry.call_method0("load")?);
        }
    }
    let extension = extension_class
        .ok_or_else(|| anyhow::anyhow!("native GPU entry point is missing"))?
        .call0()?;
    let exception = py.import("qubes.exc")?.getattr("QubesException")?;
    let runner = py.import("asyncio")?.getattr("Runner")?.call0()?;
    runner.call_method0("__enter__")?;
    let mut records = Vec::new();
    for event in events {
        let vm = vm_class.call1(("graphics", event != "domain-pre-start"))?;
        vm.setattr(
            "_airlock_amd_vbios",
            values::python(py, &json!({"stale":true}))?,
        )?;
        extension.call_method1("on_load", (&vm, "domain-load"))?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("pre_event", true)?;
        let pending = vm.call_method("fire_event_async", (event,), Some(&kwargs))?;
        let error = match runner.call_method1("run", (pending,)) {
            Ok(_) => None,
            Err(error) if error.value(py).is_instance(&exception)? => Some(error.to_string()),
            Err(error) => return Err(error.into()),
        };
        let ordinary = vm_class.call1(("ordinary", true))?;
        extension.call_method1("on_load", (&ordinary, "domain-load"))?;
        let pending = ordinary.call_method("fire_event_async", (event,), Some(&kwargs))?;
        runner.call_method1("run", (pending,))?;
        records.push(json!({"event":event,"error":error,
            "reached":vm.getattr("reached_device_handler")?.is_truthy()?,
            "stale_cleared":vm.getattr("_airlock_amd_vbios")?.is_none(),
            "ordinary_reached":ordinary.getattr("reached_device_handler")?.is_truthy()?}));
    }
    runner.call_method0("close")?;
    Ok(json!(records))
}
