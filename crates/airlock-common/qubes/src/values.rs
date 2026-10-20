//! Convert declarative data at the Python boundary without evaluating source text.

use anyhow::{Result, bail};
use pyo3::{
    prelude::*,
    types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple},
};
use serde_json::{Value, json};

/// Construct ordinary Python values for upstream property descriptors and volume constructors.
pub fn python<'py>(py: Python<'py>, value: &Value) -> PyResult<Bound<'py, PyAny>> {
    Ok(match value {
        Value::Null => py.None().into_bound(py),
        Value::Bool(value) => value.into_pyobject(py)?.to_owned().into_any(),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                value.into_pyobject(py)?.into_any()
            } else if let Some(value) = value.as_u64() {
                value.into_pyobject(py)?.into_any()
            } else {
                value.as_f64().unwrap().into_pyobject(py)?.into_any()
            }
        }
        Value::String(value) => value.into_pyobject(py)?.into_any(),
        Value::Array(values) => {
            let items = values
                .iter()
                .map(|value| python(py, value))
                .collect::<PyResult<Vec<_>>>()?;
            PyList::new(py, items)?.into_any()
        }
        Value::Object(values) => {
            let dictionary = PyDict::new(py);
            for (key, value) in values {
                dictionary.set_item(key, python(py, value)?)?;
            }

            dictionary.into_any()
        }
    })
}

/// Replace Qubes pool/volume references with identities and reject unknown object types.
pub fn plain(value: &Bound<'_, PyAny>) -> Result<Value> {
    let py = value.py();
    if value.is_none() {
        return Ok(Value::Null);
    }
    if value.is_instance_of::<PyBool>() {
        return Ok(json!(value.extract::<bool>()?));
    }
    if value.is_instance_of::<PyInt>() {
        return Ok(match value.extract::<i64>() {
            Ok(value) => json!(value),
            Err(_) => json!(value.extract::<u64>()?),
        });
    }
    if value.is_instance_of::<PyFloat>() {
        let number = value.extract::<f64>()?;
        return serde_json::Number::from_f64(number)
            .map(Value::Number)
            .ok_or_else(|| anyhow::anyhow!("non-finite upstream number"));
    }
    if value.is_instance_of::<PyString>() {
        return Ok(json!(value.extract::<String>()?));
    }
    if let Ok(values) = value.cast::<PyDict>() {
        return values
            .iter()
            .map(|(key, value)| Ok((key.extract::<String>()?, plain(&value)?)))
            .collect::<Result<serde_json::Map<_, _>>>()
            .map(Value::Object);
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        return value
            .try_iter()?
            .map(|value| plain(&value?))
            .collect::<Result<Vec<_>>>()
            .map(Value::Array);
    }

    let storage = py.import("qubes.storage")?;
    if value.is_instance(&storage.getattr("Volume")?)? {
        return identity(value);
    }
    if value.is_instance(&storage.getattr("Pool")?)? {
        return Ok(json!(value.getattr("name")?.extract::<String>()?));
    }

    bail!(
        "unsupported upstream value: {kind}",
        kind = value.get_type().name()?
    );
}

/// Read stable storage identities without retaining references to upstream object graphs.
pub fn identity(volume: &Bound<'_, PyAny>) -> Result<Value> {
    Ok(json!({
        "pool": volume.getattr("pool")?.getattr("name")?.extract::<String>()?,
        "vid": volume.getattr("vid")?.extract::<String>()?,
    }))
}

/// Preserve absent optional attributes while allowing property getter failures to propagate.
pub fn optional<'py>(value: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    match value.getattr(name) {
        Ok(value) => Ok(value),
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(value.py()) => {
            Ok(value.py().None().into_bound(value.py()))
        }
        Err(error) => Err(error),
    }
}
