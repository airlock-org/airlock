//! Supply inert Qubes-shaped Python objects from Rust for binding regression tests.
//! This development-only extension replaces upstream modules inside isolated test processes.

#[path = "qubes_objects/image.rs"]
mod image;

#[path = "qubes_objects/storage.rs"]
mod storage;

use airlock_qubes::{descriptor, values};
use pyo3::{
    exceptions::{PyAttributeError, PyValueError},
    prelude::*,
    types::{PyDict, PyTuple},
};
use serde_json::Value;
use std::{io::Write, path::PathBuf};

/// Return one required positional argument, including the bound instance at index zero.
fn argument<'py>(args: &Bound<'py, PyTuple>, index: usize) -> PyResult<Bound<'py, PyAny>> {
    args.get_item(index)
}

/// Read a named argument or supply a declarative default without evaluating Python source.
fn keyword<'py>(
    py: Python<'py>,
    kwargs: Option<&Bound<'py, PyDict>>,
    name: &str,
    default: Value,
) -> PyResult<Bound<'py, PyAny>> {
    match kwargs
        .map(|values| values.get_item(name))
        .transpose()?
        .flatten()
    {
        Some(value) => Ok(value),
        None => values::python(py, &default),
    }
}

/// Convert observed Python fields to JSON while preserving upstream pool and volume identities.
fn plain(value: &Bound<'_, PyAny>) -> PyResult<Value> {
    values::plain(value).map_err(|error| PyValueError::new_err(error.to_string()))
}

/// Read a primitive field or identity for a fixture's persisted observation.
fn field(object: &Bound<'_, PyAny>, name: &str) -> PyResult<Value> {
    plain(&object.getattr(name)?)
}

/// Apply declarative fields to an ordinary Python instance with a writable dictionary.
fn fields(object: &Bound<'_, PyAny>, fields: &Value) -> PyResult<()> {
    for (name, value) in fields.as_object().expect("fixture fields are a record") {
        object.setattr(name, values::python(object.py(), value)?)?;
    }

    Ok(())
}

/// Define Python-visible methods and properties whose complete implementations live in Rust.
fn class<'py>(
    module: &Bound<'py, PyModule>,
    name: &str,
    body: descriptor::Body,
    methods: &[&str],
    deferred: &[&str],
    properties: &[&str],
) -> PyResult<Bound<'py, PyAny>> {
    let py = module.py();
    let namespace = PyDict::new(py);
    for name in methods {
        namespace.set_item(name, descriptor::method(py, name, body, false)?)?;
    }
    for name in deferred {
        namespace.set_item(name, descriptor::method(py, name, body, true)?)?;
    }
    for name in properties {
        namespace.set_item(name, descriptor::property(py, name, body)?)?;
    }

    let class = descriptor::subclass(
        py,
        name,
        py.import("builtins")?.getattr("object")?,
        &namespace,
    )?;
    class.setattr("__module__", module.name()?)?;
    module.add(name, &class)?;
    Ok(class)
}

/// Reject unimplemented fixture methods instead of silently accepting a changed binding contract.
fn unknown(name: &str) -> PyErr {
    PyAttributeError::new_err(name.to_owned())
}

/// Find an explicitly supplied observation path; fixtures never use the host Qubes database.
fn path(variable: &str) -> PyResult<PathBuf> {
    std::env::var_os(variable)
        .map(PathBuf::from)
        .ok_or_else(|| PyValueError::new_err(format!("missing fixture path: {variable}")))
}

/// Serialize one observation using Rust rather than calling Python file or JSON utilities.
fn write(variable: &str, value: &Value) -> PyResult<()> {
    std::fs::write(path(variable)?, value.to_string())?;
    Ok(())
}

/// Append a cleanup observation so failed image construction can prove every object was closed.
fn append(variable: &str, value: &Value) -> PyResult<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path(variable)?)?;
    writeln!(file, "{value}")?;
    Ok(())
}

/// Register a Python module without installing or generating any Python source files.
fn module<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyModule>> {
    let module = PyModule::new(py, name)?;
    py.import("sys")?
        .getattr("modules")?
        .set_item(name, &module)?;
    Ok(module)
}

/// Iterate VM objects as Qubes' VMCollection does, rather than iterating dictionary keys.
fn domains(
    _name: &str,
    args: &Bound<'_, PyTuple>,
    _kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    Ok(argument(args, 0)?
        .call_method0("values")?
        .call_method0("__iter__")?
        .unbind())
}

/// Construct a mutable VM collection with Qubes' object iteration semantics.
fn collection(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let namespace = PyDict::new(py);
    namespace.set_item(
        "__iter__",
        descriptor::method(py, "__iter__", domains, false)?,
    )?;
    descriptor::subclass(
        py,
        "Domains",
        py.get_type::<PyDict>().into_any(),
        &namespace,
    )?
    .call0()
}

/// Select the explicitly requested fixture family when the test imports `qubes.so`.
#[pymodule]
fn qubes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("__path__", pyo3::types::PyList::empty(module.py()))?;

    match std::env::var("AIRLOCK_QUBES_FIXTURE").as_deref() {
        Ok("image") => image::register(module),
        Ok("storage") => storage::register(module),
        _ => Err(PyValueError::new_err(
            "select an explicit Qubes object fixture",
        )),
    }
}
