//! Observe PCI assignments through upstream Qubes objects without changing ownership.

use super::values::{optional, plain};
use anyhow::Result;
use pyo3::prelude::*;
use serde_json::{Value, json};

/// Read template-inherited graphics features and assignments at a lifecycle boundary.
pub fn vm(vm: &Bound<'_, PyAny>) -> Result<Value> {
    let features = vm.getattr("features")?;
    let feature = |name, default| features.call_method1("check_with_template", (name, default));
    let mut assignments = Vec::new();
    for item in vm
        .getattr("devices")?
        .get_item("pci")?
        .call_method0("get_assigned_devices")?
        .try_iter()?
    {
        let item = item?;
        let mut devices = Vec::new();
        for device in item.getattr("devices")?.try_iter()? {
            let device = device?;
            let text =
                |name| -> PyResult<String> { Ok(device.getattr(name)?.str()?.to_str()?.into()) };
            let segment = text("segment")?;
            let bus = text("bus")?;
            let slot = text("device")?;
            let function = text("function")?;
            devices.push(json!({
                "host_bdf": format!("{segment}:{bus}:{slot}.{function}"),
                "port_id": device.getattr("port_id")?.extract::<String>()?,
            }));
        }
        assignments.push(json!({
            "required": item.getattr("required")?.is_truthy()?,
            "backend_qid": item.getattr("backend_domain")?.getattr("qid")?.extract::<u32>()?,
            "devices": devices,
        }));
    }

    let py = vm.py();
    let video_model =
        feature("video-model", "".into_pyobject(py)?.into_any())?.extract::<String>()?;
    let uefi = feature("uefi", false.into_pyobject(py)?.to_owned().into_any())?.is_truthy()?;
    let linux_stubdom = feature(
        "linux-stubdom",
        true.into_pyobject(py)?.to_owned().into_any(),
    )?
    .is_truthy()?;
    let qemu_extra_args =
        feature("qemu-extra-args", "".into_pyobject(py)?.into_any())?.extract::<String>()?;

    Ok(json!({
        "name": vm.getattr("name")?.extract::<String>()?,
        "uuid": vm.getattr("uuid")?.str()?.to_str()?,
        "virt_mode": vm.getattr("virt_mode")?.extract::<String>()?,
        "features": {
            "video_model": video_model, "uefi": uefi,
            "linux_stubdom": linux_stubdom, "qemu_extra_args": qemu_extra_args,
        },
        "assignments": assignments,
    }))
}

/// Observe all VMs through the upstream Admin client, including resolved PCI port identity.
pub fn inventory(port: &str) -> Result<Value> {
    Python::attach(|py| {
        let app = py.import("qubesadmin")?.getattr("Qubes")?.call0()?;
        observe(&app, port)
    })
}

/// Read an existing Admin client object, also used by read-only upstream regression probes.
pub fn observe(app: &Bound<'_, PyAny>, port: &str) -> Result<Value> {
    let domains = app.getattr("domains")?;
    let resolved = if port.is_empty() {
        Value::Null
    } else {
        plain(
            &domains
                .get_item("dom0")?
                .getattr("devices")?
                .get_item("pci")?
                .get_item(port)?
                .getattr("data")?
                .call_method1("get", ("sbdf",))?,
        )?
    };
    let mut records = Vec::new();
    for vm in domains.try_iter()? {
        let vm = vm?;
        let pci = vm.getattr("devices")?.get_item("pci")?;
        let collect = |method| -> Result<Vec<Value>> {
            pci.call_method0(method)?.try_iter()?.map(|item| {
                let item = item?;
                let mode = optional(&item, "mode")?;
                Ok(json!({
                    "backend": item.getattr("backend_domain")?.getattr("name")?.extract::<String>()?,
                    "port": item.getattr("port_id")?.extract::<String>()?,
                    "mode": if mode.is_none() { Value::Null } else { plain(&optional(&mode, "value")?)? },
                    "device_id": plain(&optional(&item, "device_id")?)?,
                    "options": plain(&item.getattr("options")?)?,
                }))
            }).collect()
        };
        records.push(json!({
            "name": vm.getattr("name")?.extract::<String>()?,
            "running": vm.call_method0("is_running")?.is_truthy()?,
            "assignments": collect("get_assigned_devices")?,
            "attached": collect("get_attached_devices")?,
        }));
    }
    Ok(json!({"resolved": resolved, "domains": records}))
}
