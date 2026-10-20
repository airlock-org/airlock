//! Inert Qubes wire peers expose configured snapshots without implementing desktop policy or publication.

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

/// Return the actual Admin API framing, retaining absence as a typed error instead of process failure.
fn feature(value: Option<&str>) -> Vec<u8> {
    match value {
        Some(value) => format!("0\0{value}").into_bytes(),
        None => b"2\0QubesFeatureNotFoundError\0\0Feature not found\0".to_vec(),
    }
}

/// Model only QubesDB and qrexec; the production service performs all validation and writes every desktop file.
pub fn execute(configuration: &Path, program: &str, arguments: &[String]) -> Result<()> {
    let mut data: Value = serde_json::from_slice(&fs::read(configuration)?)?;
    let directory = configuration.parent().context("menu fixture directory")?;
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input)?;
    if program == "menu-qdb" {
        ensure!(arguments == ["/name"], "unexpected QubesDB key");
        println!(
            "{name}",
            name = data["local_name"].as_str().context("local name")?
        );
        return Ok(());
    }
    ensure!(
        arguments.len() == 3 && arguments[0] == "--",
        "unexpected qrexec arguments"
    );
    let name = &arguments[1];
    let (service, argument) = arguments[2].split_once('+').unwrap_or((&arguments[2], ""));
    let operation = match service {
        "admin.vm.List" => "inventory",
        "qubes.GetAppmenus" => "fetch",
        "qubes.GetImageRGBA" => "icon",
        _ => service,
    };
    let mut calls = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("calls.jsonl"))?;
    writeln!(
        calls,
        "{}",
        json!({"operation": operation, "names": [name], "args": arguments, "input": String::from_utf8_lossy(&input)})
    )?;
    ensure!(data["fail"] != operation, "Qubes API unavailable");
    let rows = data["domains"].as_array().context("VM observations")?;
    let row = rows.iter().find(|row| row["name"] == *name);
    let payload = match service {
        "admin.vm.List" => rows
            .iter()
            .map(|row| {
                format!(
                    "{name} class={class} state={state}\n",
                    name = row["name"].as_str().unwrap(),
                    class = row["class"].as_str().unwrap(),
                    state = if row["running"] == true {
                        "Running"
                    } else {
                        "Halted"
                    }
                )
            })
            .collect::<String>(),
        "admin.vm.property.Get" => {
            let row = row.context("unknown VM")?;
            let (kind, value) = match argument {
                "guivm" => ("vm", row["guivm"].as_str().unwrap().to_owned()),
                "uuid" => ("str", row["uuid"].as_str().unwrap().to_owned()),
                "label" => (
                    "label",
                    format!("label-{uuid}", uuid = row["uuid"].as_str().unwrap()),
                ),
                "auto_cleanup" => (
                    "bool",
                    if row["auto_cleanup"] == true {
                        "True"
                    } else {
                        "False"
                    }
                    .to_owned(),
                ),
                _ => bail!("unexpected property {argument}"),
            };
            format!("default=False type={kind} {value}")
        }
        "admin.label.Get" => rows
            .iter()
            .find(|row| argument == format!("label-{uuid}", uuid = row["uuid"].as_str().unwrap()))
            .context("unknown label")?["label"]
            .as_str()
            .unwrap()
            .to_owned(),
        "admin.vm.tag.List" => row.context("unknown VM")?["tags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tag| format!("{tag}\n", tag = tag.as_str().unwrap()))
            .collect(),
        "admin.vm.CurrentState" => format!(
            "power_state={state}",
            state = if row.context("unknown VM")?["running"] == true {
                "Running"
            } else {
                "Halted"
            }
        ),
        "admin.vm.feature.Get" => {
            let row = row.context("unknown VM")?;
            let value = match argument {
                "internal" => (row["internal"] == true).then_some("1"),
                "menu-items" => row["menu-items"].as_str(),
                "supported-rpc.airlock.AppSessionReady" => {
                    (data["supported"] == true).then_some("1")
                }
                _ => bail!("unexpected feature {argument}"),
            };
            std::io::stdout().write_all(&feature(value))?;
            return Ok(());
        }
        "qubes.GetAppmenus" => {
            ensure!(
                row.context("unknown VM")?["running"] == true,
                "policy denies autostart"
            );
            let wire = data["wire"].as_str().map(str::to_owned).unwrap_or_else(|| {
                let mut wire = String::new();
                for (id, fields) in data["menus"].as_object().unwrap() {
                    for (field, value) in fields.as_object().unwrap() {
                        wire.push_str(&format!(
                            "{id}:{field}={value}\n",
                            value = value.as_str().unwrap()
                        ));
                    }
                }
                wire
            });
            if data["stop_after_fetch"] == true {
                data["domains"][0]["running"] = json!(false);
                fs::write(configuration, serde_json::to_vec(&data)?)?;
            }
            std::io::stdout().write_all(wire.as_bytes())?;
            return Ok(());
        }
        "qubes.GetImageRGBA" => {
            ensure!(
                row.context("unknown VM")?["running"] == true,
                "policy denies autostart"
            );
            let pixels = data["icon_bytes"]
                .as_str()
                .map(str::as_bytes)
                .unwrap_or(b"1 1\n\xff\x20\x40\xff");
            let pixels = pixels.to_vec();
            if data["stop_after_icon"] == true {
                data["domains"][0]["running"] = json!(false);
                fs::write(configuration, serde_json::to_vec(&data)?)?;
            }
            std::io::stdout().write_all(&pixels)?;
            return Ok(());
        }
        _ => bail!("unexpected Qubes service {service}"),
    };
    std::io::stdout().write_all(format!("0\0{payload}").as_bytes())?;
    Ok(())
}
