//! Native smoke-test report extraction and canonical storage-sharing evidence, without a Python orchestration dependency.

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

/// Bound potentially noisy command logs before scanning them for a structured report.
fn read(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 32 * 1024 * 1024,
        "test report input exceeds limit"
    );
    Ok(bytes)
}

/// Select the first complete JSON object with the requested schema, tolerating unrelated command diagnostics.
fn extract(bytes: &[u8], schema: Option<&str>) -> Result<Value> {
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'{' {
            continue;
        }
        let Some(Ok(value)) = serde_json::Deserializer::from_slice(&bytes[index..])
            .into_iter::<Value>()
            .next()
        else {
            continue;
        };
        if value.is_object() && schema.is_none_or(|schema| value["schema"] == schema) {
            return Ok(value);
        }
    }
    anyhow::bail!("command did not emit a JSON object with the requested schema")
}

/// Keep the previous evidence file intact until a complete new report is ready to publish.
fn write(path: &Path, value: &Value) -> Result<()> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temporary, value)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    Ok(())
}

/// Preserve object extent order while refusing views that cannot prove canonical sharing.
fn objects(map: &Value) -> Result<BTreeMap<&str, Vec<&Value>>> {
    ensure!(
        map["view_backend"] == "erofs-objectmap",
        "not an erofs-objectmap view"
    );
    ensure!(
        map["external_data_format"] == "posix-tar-index",
        "not a tar-index EROFS external-data view"
    );
    ensure!(
        map.get("central_blob").is_none(),
        "closure view uses single-blob metadata"
    );
    let mut objects: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for extent in map["extents"].as_array().context("object extents")? {
        let name = extent["object"]
            .as_str()
            .context("extent object identity")?;
        if name != ".tar-eof" {
            objects.entry(name).or_default().push(extent);
        }
    }
    ensure!(!objects.is_empty(), "view has no object extents");
    Ok(objects)
}

/// Read a checked nonnegative byte or extent bound from native JSON.
fn number(value: &Value) -> Option<u64> {
    value.as_u64()
}

/// Prove that shared objects retain identical canonical extents while both closures also contain unique objects.
fn sharing(
    label: &str,
    left_vm: &str,
    right_vm: &str,
    left_path: &Path,
    right_path: &Path,
) -> Result<Value> {
    let left: Value = serde_json::from_slice(&read(left_path)?)?;
    let right: Value = serde_json::from_slice(&read(right_path)?)?;
    let left_extents = objects(&left)?;
    let right_extents = objects(&right)?;
    let common: Vec<_> = left_extents
        .keys()
        .filter(|name| right_extents.contains_key(**name))
        .copied()
        .collect();
    ensure!(
        !common.is_empty(),
        "closure views do not share any canonical objects"
    );
    let same = |a: &&Value, b: &&Value| {
        a["segment_id"].is_string()
            && a["segment_id"] == b["segment_id"]
            && number(&a["segment_offset"]).is_some()
            && number(&a["segment_offset"]) == number(&b["segment_offset"])
            && number(&a["length"]).is_some()
            && number(&a["length"]) == number(&b["length"])
            && a["object_sha256"].is_string()
            && a["object_sha256"] == b["object_sha256"]
    };
    let selected = common
        .iter()
        .find(|name| {
            let a = &left_extents[**name];
            let b = &right_extents[**name];
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
        })
        .context("common objects do not retain stable canonical extents")?;
    let left_unique: Vec<_> = left_extents
        .keys()
        .filter(|name| !right_extents.contains_key(**name))
        .copied()
        .collect();
    let right_unique: Vec<_> = right_extents
        .keys()
        .filter(|name| !left_extents.contains_key(**name))
        .copied()
        .collect();
    ensure!(
        !left_unique.is_empty() && !right_unique.is_empty(),
        "each closure must contain a unique object"
    );
    let mut blobs = BTreeSet::new();
    for map in [&left, &right] {
        for segment in map["segments"].as_array().context("canonical segments")? {
            blobs.insert(segment["blob"].as_str().context("segment blob")?);
        }
    }
    let mut size = 0u64;
    for path in &blobs {
        size = size
            .checked_add(fs::metadata(path)?.len())
            .context("segment byte count overflow")?;
    }
    Ok(
        json!({"schema": "airlock.nix-closure-domain-sharing-proof.v1", "status": "passed", "label": label,
        "left_domain": left_vm, "right_domain": right_vm, "left_blob_map": left_path, "right_blob_map": right_path,
        "left_segments": left["segments"], "right_segments": right["segments"], "unique_segment_count": blobs.len(),
        "segment_physical_size_total": size, "common_object_count": common.len(), "left_unique_object_count": left_unique.len(),
        "right_unique_object_count": right_unique.len(), "shared_object": selected, "shared_extents": left_extents[*selected],
        "left_unique_object": left_unique[0], "right_unique_object": right_unique[0], "left_logical_size": number(&left["logical_size"]).context("left logical size")?,
        "right_logical_size": number(&right["logical_size"]).context("right logical size")?, "same_canonical_segment_offset_and_hash": true}),
    )
}

/// Translate archived logical sources through the selected store, keeping offline evaluation inputs explicit.
fn sources(archive: &Path, store: &Path) -> Result<Value> {
    ensure!(store.is_absolute(), "source store must be absolute");
    let archive: Value = serde_json::from_slice(&read(archive)?)?;
    let machine = archive["inputs"].get("airlock-machine").unwrap_or(&archive);
    let inputs = machine["inputs"]
        .as_object()
        .context("archived machine inputs")?;
    let mut result = serde_json::Map::new();
    for (output, input) in [
        ("nixpkgsSource", "nixpkgs"),
        ("qubesSource", "nixos-qubes"),
        ("libvirtSource", "nixpkgs-libvirt_10_5"),
    ] {
        let local_qubes = format!(
            "{machine}/vendor/nixos-qubes",
            machine = machine["path"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!(
                    "{root}/airlock-machine",
                    root = archive["path"].as_str().unwrap_or("")
                ))
        );
        let logical = inputs
            .get(input)
            .and_then(|value| value["path"].as_str())
            .or_else(|| (input == "nixos-qubes").then_some(local_qubes.as_str()))
            .with_context(|| format!("missing archived input {input}"))?;
        ensure!(
            logical.starts_with("/nix/store/"),
            "unexpected source path: {logical}"
        );
        let physical = store.join(logical.trim_start_matches('/'));
        ensure!(physical.is_dir(), "archived source missing: {physical:?}");
        result.insert(output.into(), json!(physical));
    }
    Ok(Value::Object(result))
}

/// Accept explicit evidence paths and publish data only; this tool cannot control VMs or build packages.
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("extract") if args.len() == 4 => {
            let schema = (args[1] != "--any").then_some(args[1].as_str());
            write(
                Path::new(&args[3]),
                &extract(&read(Path::new(&args[2]))?, schema)?,
            )
        }
        Some("sources") if args.len() == 4 => write(
            Path::new(&args[3]),
            &sources(Path::new(&args[1]), Path::new(&args[2]))?,
        ),
        Some("sharing") if args.len() == 7 => {
            let report = sharing(
                &args[1],
                &args[2],
                &args[3],
                Path::new(&args[4]),
                Path::new(&args[5]),
            )?;
            write(Path::new(&args[6]), &report)
        }
        _ => anyhow::bail!(
            "usage: airlock-test-report extract SCHEMA|--any INPUT OUTPUT; sources ARCHIVE STORE OUTPUT; sharing LABEL LEFT-VM RIGHT-VM LEFT-MAP RIGHT-MAP OUTPUT"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Noisy, malformed and unrelated objects cannot replace the first complete report with the requested schema.
    #[test]
    fn extracts_complete_schema_matched_objects_without_requiring_clean_stdout() -> Result<()> {
        let input = br#"noise {broken
{"schema":"other","nested":{"keep":true}}
{"schema":"wanted","message":"literal } braces","nested":{"value":42}}
trailer"#;
        assert_eq!(
            extract(input, Some("wanted"))?,
            json!({"schema": "wanted", "message": "literal } braces", "nested": {"value": 42}})
        );
        assert_eq!(extract(input, None)?["schema"], "other");
        assert!(extract(input, Some("missing")).is_err());
        assert!(extract(br#"{"schema":"wanted","incomplete":["#, Some("wanted")).is_err());
        Ok(())
    }
}
