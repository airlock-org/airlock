//! Data-only Qubes storage bridge for native attachment identity and daemon coordination scenarios.

use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path};

/// Project an upstream volume's supplied configuration into the snapshot consumed by native policy.
pub fn volume(config: &Value) -> Value {
    let mut result = json!({"source": null, "config": config});
    for field in [
        "name",
        "pool",
        "vid",
        "rw",
        "snap_on_start",
        "save_on_stop",
        "generation",
        "closure_hash",
        "blob_map_path",
        "size",
    ] {
        result[field] = config[field].clone();
    }
    result["path"] = config["view_path"].clone();
    result
}

/// Return literal fixture snapshots and record requested installs; policy and generation selection remain native.
pub fn execute(root: &Path) -> Result<()> {
    let request: Value = serde_json::from_reader(std::io::stdin())?;
    let path = root.join("qubes.json");
    let mut state: Value = serde_json::from_slice(&fs::read(&path)?)?;
    let mut calls = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("bridge-calls.jsonl"))?;
    writeln!(calls, "{request}")?;
    if request["operation"] == "install" {
        ensure!(
            std::env::var("AIRLOCK_TEST_STORAGE_FAIL_INSTALL").as_deref() != Ok("1"),
            "injected attachment failure"
        );
        ensure!(
            state == request["expected"],
            "Qubes state differs from expected snapshot"
        );
        ensure!(
            state["running"] == false,
            "cannot install into a running VM"
        );
        let pool = &request["pool"];
        state["pools"][pool["name"].as_str().unwrap()] = pool.clone();
        for config in request["volumes"].as_array().unwrap() {
            let volumes = state["volumes"].as_array_mut().unwrap();
            if let Some(index) = volumes.iter().position(|row| row["name"] == config["name"]) {
                volumes[index] = volume(config);
            } else {
                volumes.push(volume(config));
            }
        }
        fs::write(path, serde_json::to_vec(&state)?)?;
    }
    println!("{state}");
    Ok(())
}
