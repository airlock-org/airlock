//! Implement Qubes pool and volume callbacks with the native immutable-storage engine.

use crate::{argument, keyword, native};
use airlock_qubes::{descriptor, values};
use pyo3::{
    prelude::*,
    types::{PyDict, PyTuple},
};
use serde_json::{Value, json};

/// Read one object's primitive field without letting Python objects cross the engine boundary.
fn field(object: &Bound<'_, PyAny>, name: &str) -> PyResult<Value> {
    values::plain(&object.getattr(name)?)
        .map_err(|error| crate::exception(object.py(), "StoragePoolException", error))
}

/// Collect persisted view identity plus its current transient device path.
fn definition(object: &Bound<'_, PyAny>) -> PyResult<Value> {
    Ok(json!({
        "view_kind": field(object, "view_kind")?,
        "view_path": field(object, "path")?,
        "blob_map_path": field(object, "blob_map_path")?,
        "blob_logical_size": field(object, "blob_logical_size")?,
        "manifest_path": field(object, "manifest_path")?,
    }))
}

/// Run one complete native lifecycle operation and retain its returned device path.
fn lifecycle(object: &Bound<'_, PyAny>, action: &str) -> PyResult<()> {
    let mut request = definition(object)?;
    let pool = object.getattr("pool")?;
    let fields = json!({
        "operation": "volume-lifecycle", "action": action,
        "root": field(&pool, "root_dir")?, "domain": field(object, "domain_name")?,
        "generation": field(object, "generation")?, "name": field(object, "name")?,
    });
    request
        .as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    let result = native(object.py(), request)?;
    object.setattr("path", values::python(object.py(), &result["path"])?)?;
    Ok(())
}

/// Forward pool callbacks while keeping Qubes' base class responsible for its own fields.
fn pool(
    name: &str,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py = args.py();
    let object = argument(args, 0)?;
    let result = match name {
        "__init__" => {
            let options = PyDict::new(py);
            options.set_item("name", keyword(kwargs, "name")?)?;
            options.set_item(
                "revisions_to_keep",
                kwargs
                    .and_then(|k| k.get_item("revisions_to_keep").transpose())
                    .transpose()?
                    .unwrap_or(0_u64.into_pyobject(py)?.into_any()),
            )?;
            options.set_item(
                "ephemeral_volatile",
                kwargs
                    .and_then(|k| k.get_item("ephemeral_volatile").transpose())
                    .transpose()?
                    .unwrap_or(false.into_pyobject(py)?.to_owned().into_any()),
            )?;
            py.import("qubes.storage")?.getattr("Pool")?.call_method(
                "__init__",
                (&object,),
                Some(&options),
            )?;
            let root = kwargs
                .and_then(|k| k.get_item("root_dir").transpose())
                .transpose()?
                .map(|value| value.extract::<String>())
                .transpose()?
                .unwrap_or_else(|| "/var/lib/airlock/closure-store".into());
            let normalized = native(
                py,
                json!({"operation":"pool-root", "root":root, "create":false}),
            )?;
            object.setattr("root_dir", values::python(py, &normalized["root"])?)?;
            object.setattr("_volumes", PyDict::new(py))?;
            return Ok(py.None());
        }
        "config" => json!({
            "name": field(&object, "name")?, "driver":"airlock-closure",
            "root_dir": field(&object, "root_dir")?,
            "revisions_to_keep": field(&object, "revisions_to_keep")?,
            "ephemeral_volatile": field(&object, "ephemeral_volatile")?,
        }),
        "setup" => {
            native(
                py,
                json!({"operation":"pool-root", "root":field(&object,"root_dir")?, "create":true}),
            )?;
            return Ok(object.unbind());
        }
        "destroy" => return Ok(object.unbind()),
        "init_volume" => {
            let vm = argument(args, 1)?;
            let config = argument(args, 2)?.cast_into::<PyDict>()?.copy()?;
            if config.contains("pool")? {
                config.del_item("pool")?;
            }
            config.set_item("pool", &object)?;
            config.set_item("_domain", vm.getattr("name")?)?;
            let volume = py
                .import("airlock_qubes")?
                .getattr("ClosureVolume")?
                .call((), Some(&config))?;
            object
                .getattr("_volumes")?
                .set_item(volume.getattr("vid")?, &volume)?;
            return Ok(volume.unbind());
        }
        "list_volumes" => {
            return Ok(py
                .import("builtins")?
                .getattr("list")?
                .call1((object.getattr("_volumes")?.call_method0("values")?,))?
                .unbind());
        }
        "get_volume" => {
            return Ok(object
                .getattr("_volumes")?
                .get_item(argument(args, 1)?)?
                .unbind());
        }
        "size" | "usage" => {
            let result = native(
                py,
                json!({
                    "operation":"pool-stats", "root":field(&object,"root_dir")?,
                    "include_usage":name=="usage",
                }),
            )?;
            result[name].clone()
        }
        _ => return Err(pyo3::exceptions::PyAttributeError::new_err(name.to_owned())),
    };
    Ok(values::python(py, &result)?.unbind())
}

/// Forward immutable-volume callbacks without duplicating engine policy or mapper operations.
fn volume(
    name: &str,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py = args.py();
    let object = argument(args, 0)?;
    match name {
        "__init__" => {
            let pool = keyword(kwargs, "pool")?;
            let config = kwargs
                .ok_or_else(|| {
                    pyo3::exceptions::PyTypeError::new_err("missing volume configuration")
                })?
                .copy()?;
            config.del_item("pool")?;
            let domain = config
                .get_item("_domain")?
                .unwrap_or(py.None().into_bound(py));
            if config.contains("_domain")? {
                config.del_item("_domain")?;
            }
            let source = config
                .get_item("source")?
                .unwrap_or(py.None().into_bound(py));
            if config.contains("source")? {
                config.del_item("source")?;
            }
            let domain = values::plain(&domain)
                .map_err(|error| crate::exception(py, "StoragePoolException", error))?;
            let config = values::plain(config.as_any())
                .map_err(|error| crate::exception(py, "StoragePoolException", error))?;
            let normalized = native(
                py,
                json!({
                    "operation":"volume-config", "domain":domain,
                    "config":config,
                    "has_source":!source.is_none(),
                }),
            )?;
            let options = values::python(py, &normalized["config"])?.cast_into::<PyDict>()?;
            options.set_item("pool", pool)?;
            py.import("qubes.storage")?.getattr("Volume")?.call_method(
                "__init__",
                (&object,),
                Some(&options),
            )?;
            for (key, value) in normalized["fields"].as_object().unwrap() {
                object.setattr(
                    if key == "view_path" { "path" } else { key },
                    values::python(py, value)?,
                )?;
            }
            object.setattr("_persisted", values::python(py, &normalized["persisted"])?)?;
            Ok(py.None())
        }
        "config" => {
            let base = py
                .import("qubes.storage")?
                .getattr("Volume")?
                .getattr("config")?
                .call_method1("__get__", (&object, object.get_type()))?
                .cast_into::<PyDict>()?;
            base.update(
                object
                    .getattr("_persisted")?
                    .cast_into::<PyDict>()?
                    .as_mapping(),
            )?;
            Ok(base.into_any().unbind())
        }
        "definition" => Ok(values::python(py, &definition(&object)?)?.unbind()),
        "lifecycle" => {
            lifecycle(&object, &argument(args, 1)?.extract::<String>()?)?;
            Ok(object.unbind())
        }
        "create" | "start" | "stop" => {
            lifecycle(&object, name)?;
            Ok(object.unbind())
        }
        "remove" => {
            let volumes = object.getattr("pool")?.getattr("_volumes")?;
            let key = object.getattr("vid")?;
            if volumes.contains(&key)? {
                volumes.del_item(key)?;
            }
            Ok(object.unbind())
        }
        "verify" | "export" => {
            lifecycle(&object, "verify")?;
            if name == "export" {
                Ok(object.getattr("path")?.unbind())
            } else {
                Ok(true.into_pyobject(py)?.to_owned().into_any().unbind())
            }
        }
        "import_data" | "import_volume" | "resize" => {
            lifecycle(&object, "immutable")?;
            Ok(py.None())
        }
        "verify_definition" => {
            let mut request = definition(&object)?;
            request["operation"] = json!("verify-volume");
            native(py, request)?;
            Ok(true.into_pyobject(py)?.to_owned().into_any().unbind())
        }
        "is_dirty" | "is_outdated" => Ok(false.into_pyobject(py)?.to_owned().into_any().unbind()),
        "block_device" => {
            lifecycle(&object, "block-device")?;
            Ok(py
                .import("qubes.storage")?
                .getattr("Volume")?
                .call_method1("block_device", (&object,))?
                .unbind())
        }
        "revisions" => Ok(PyDict::new(py).into_any().unbind()),
        "size" | "usage" => {
            let mut request = definition(&object)?;
            request["operation"] = json!("volume-stats");
            request["fallback_size"] = field(&object, "_size")?;
            let result = native(py, request)?;
            if name == "size" {
                object.setattr("_size", values::python(py, &result["size"])?)?;
            }
            Ok(values::python(py, &result[name])?.unbind())
        }
        _ => Err(pyo3::exceptions::PyAttributeError::new_err(name.to_owned())),
    }
}

/// Publish genuine upstream subclasses with ordinary Python methods and property descriptors.
pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let storage = py.import("qubes.storage")?;
    let namespace = PyDict::new(py);
    namespace.set_item("driver", "airlock-closure")?;
    for name in ["__init__", "init_volume", "list_volumes", "get_volume"] {
        namespace.set_item(name, descriptor::method(py, name, pool, false)?)?;
    }
    for name in ["setup", "destroy"] {
        namespace.set_item(name, descriptor::method(py, name, pool, true)?)?;
    }
    for name in ["config", "size", "usage"] {
        namespace.set_item(name, descriptor::property(py, name, pool)?)?;
    }
    module.add(
        "ClosurePool",
        descriptor::subclass(py, "ClosurePool", storage.getattr("Pool")?, &namespace)?,
    )?;

    let namespace = PyDict::new(py);
    for name in [
        "__init__",
        "definition",
        "lifecycle",
        "is_dirty",
        "is_outdated",
        "block_device",
    ] {
        namespace.set_item(name, descriptor::method(py, name, volume, false)?)?;
    }
    for name in [
        "create",
        "remove",
        "export",
        "import_data",
        "import_volume",
        "resize",
        "start",
        "stop",
        "verify_definition",
        "verify",
    ] {
        namespace.set_item(name, descriptor::method(py, name, volume, true)?)?;
    }
    for name in ["config", "revisions", "size", "usage"] {
        namespace.set_item(name, descriptor::property(py, name, volume)?)?;
    }
    module.add(
        "ClosureVolume",
        descriptor::subclass(py, "ClosureVolume", storage.getattr("Volume")?, &namespace)?,
    )?;
    Ok(())
}
