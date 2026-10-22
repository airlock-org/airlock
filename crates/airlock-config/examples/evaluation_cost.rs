//! Measure generated Nix evaluation without building or activating a system.
//! Supply an already prepared, offline-resolvable source tree and GNU time explicitly.
//! Each sample includes source loading and disposable-store setup inside the Nix process;
//! filesystem caches remain warm. Nix's own counters are retained separately.

use rusix::{
    interop::{NixExpression, raw::NixValue},
    nix_record,
};
use serde_json::json;
use std::{fs, path::PathBuf, process::Command};

/// Run paired samples in alternating order and require matching results for every projection.
fn compare(arguments: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let [baseline, candidate, store, timer, logs, repetitions] = arguments else {
        return Err(
            "usage: evaluation_cost --compare BASELINE CANDIDATE STORE GNU_TIME LOGS REPETITIONS"
                .into(),
        );
    };
    let repetitions: usize = repetitions.to_str().ok_or("invalid repetitions")?.parse()?;
    if repetitions == 0 {
        return Err("at least one repetition is required".into());
    }
    let logs = PathBuf::from(logs);
    fs::create_dir_all(&logs)?;
    // Keep the executable path stable if Cargo replaces its artifact during a
    // long comparison. Final measurements should use a saved binary copy too.
    let executable = std::env::current_exe()?;
    let variants = [("baseline", baseline), ("candidate", candidate)];
    let mut measurements = Vec::new();
    for repetition in 0..repetitions {
        for case in [
            "output-names",
            "guest-config",
            "dom0-config",
            "guest-derivation",
            "gui-derivation",
            "network-derivation",
            "packages-and-check",
        ] {
            let mut results = Vec::new();
            for position in 0..2 {
                let (variant, source) = variants[(position + repetition) % 2];
                let directory = logs.join(format!("{case}-{repetition}-{variant}"));
                let output = Command::new(&executable)
                    .args([source, store, timer])
                    .arg(&directory)
                    .arg("1")
                    .env("AIRLOCK_EVALUATION_CASE", case)
                    .output()?;
                fs::write(directory.with_extension("stdout"), &output.stdout)?;
                fs::write(directory.with_extension("stderr"), &output.stderr)?;
                if !output.status.success() {
                    return Err(format!(
                        "{variant}/{case} failed; see {path}",
                        path = directory.display()
                    )
                    .into());
                }
                let values: Vec<serde_json::Value> =
                    serde_json::from_slice(&fs::read(directory.join("measurements.json"))?)?;
                let [mut measurement] =
                    <[_; 1]>::try_from(values).map_err(|_| "expected one sample")?;
                measurement["variant"] = json!(variant);
                measurement["repetition"] = json!(repetition);
                println!(
                    "{variant}/{case}/{repetition}: {wall}s, {rss} KiB",
                    wall = measurement["wall_seconds"],
                    rss = measurement["peak_rss_kib"]
                );
                results.push(measurement["result"].clone());
                measurements.push(measurement);
                fs::write(
                    logs.join("measurements.json"),
                    serde_json::to_vec_pretty(&measurements)?,
                )?;
            }
            if results[0] != results[1] {
                return Err(format!("baseline and candidate results differ for {case}").into());
            }
        }
    }

    Ok(())
}

/// Evaluate fixed projections in fresh processes and evaluation stores, retaining resource evidence.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|argument| argument == "--compare")
    {
        return compare(&arguments[1..]);
    }
    let [source, store, timer, logs, repetitions] = arguments.as_slice() else {
        return Err("usage: evaluation_cost SOURCE AIRLOCK_STORE GNU_TIME LOGS REPETITIONS".into());
    };
    let source = fs::canonicalize(source)?;
    let store = fs::canonicalize(store)?;
    if store == std::path::Path::new("/") || !store.join("nix/store").is_dir() {
        return Err("select the guarded Airlock store root".into());
    }
    let logs = PathBuf::from(logs);
    fs::create_dir_all(&logs)?;
    let repetitions: usize = repetitions.to_str().ok_or("invalid repetitions")?.parse()?;
    if repetitions == 0 {
        return Err("at least one repetition is required".into());
    }
    fs::write(
        logs.join("run.json"),
        serde_json::to_vec_pretty(&json!({
            "schema": "airlock.evaluation-cost.v1",
            "source": source, "store": store, "timer": timer,
            "repetitions": repetitions, "evaluation_cache": false,
            "fresh_evaluation_store_per_sample": true,
            "includes_source_loading": true, "builds_enabled": false,
        }))?,
    )?;

    let flake = NixValue::builtin("getFlake").call(format!("path:{path}", path = source.display()));
    let config = flake
        .clone()
        .select("nixosConfigurations.airlock-domain-usr-home.config");
    let dom0 = flake.clone().select("nixosConfigurations.usb-dev.config");
    let cases = [
        ("output-names", NixValue::builtin("attrNames").call(flake.clone())),
        (
            "guest-config",
            config.clone().bind(|config| nix_record! {
                "hostname": config.clone().select("networking.hostName"),
                "stateVersion": config.clone().select("system.stateVersion"),
                "services": NixValue::builtin("attrNames").call(config.select("systemd.services")),
            }),
        ),
        (
            "dom0-config",
            dom0.bind(|config| nix_record! {
                "hostname": config.clone().select("networking.hostName"),
                "stateVersion": config.clone().select("system.stateVersion"),
                "services": NixValue::builtin("attrNames").call(config.select("systemd.services")),
            }),
        ),
        (
            "guest-derivation",
            config.select("system.build.toplevel.drvPath"),
        ),
        (
            "gui-derivation",
            flake.clone().select("nixosConfigurations.airlock-domain-sys-gui.config.system.build.toplevel.drvPath"),
        ),
        (
            "packages-and-check",
            nix_record! {
                "package": flake.clone().select("packages.x86_64-linux.qubes-linux-utils.drvPath"),
                "check": flake.clone().select("checks.x86_64-linux.module-contract.drvPath"),
            },
        ),
        (
            "network-derivation",
            flake.select("nixosConfigurations.airlock-domain-sys-net.config.system.build.toplevel.drvPath"),
        ),
    ];
    let mut measurements = Vec::new();
    for (name, value) in cases {
        if std::env::var("AIRLOCK_EVALUATION_CASE").is_ok_and(|selected| selected != name) {
            continue;
        }
        let driver = logs.join(format!("{name}.nix"));
        let generated = rusix::compile_value(value).map_err(|error| error.render(&source))?;
        fs::write(&driver, generated.source)?;

        for repetition in 0..repetitions {
            let prefix = logs.join(format!("{name}-{repetition}"));
            let isolated = tempfile::tempdir()?;
            for directory in ["config", "cache", "state", "home"] {
                fs::create_dir(isolated.path().join(directory))?;
            }
            let mut command = Command::new(timer);
            command
                .args(["--format", "%e %U %S %M", "--output"])
                .arg(prefix.with_extension("resources"))
                .args(["nix", "--store"])
                .arg(&store)
                .args([
                    "--extra-experimental-features",
                    "nix-command flakes",
                    "--option",
                    "substituters",
                    "",
                    "--option",
                    "builders",
                    "",
                    "--option",
                    "build-users-group",
                    "",
                    "--option",
                    "allow-import-from-derivation",
                    "false",
                    "--option",
                    "eval-cache",
                    "false",
                    "--offline",
                    "eval",
                    "--impure",
                    "--json",
                    "--eval-store",
                ])
                .arg(isolated.path().join("store"))
                .arg("--file")
                .arg(&driver)
                .env_remove("NIX_REMOTE")
                .env_remove("NIX_STORE_DIR")
                .env_remove("NIX_STATE_DIR")
                .env_remove("NIX_DAEMON_SOCKET_PATH")
                .env("NIX_CONFIG", "")
                .env("NIX_PATH", "")
                .env("NIX_CONF_DIR", isolated.path().join("config"))
                .env(
                    "NIX_USER_CONF_FILES",
                    isolated.path().join("config/nix.conf"),
                )
                .env("XDG_CACHE_HOME", isolated.path().join("cache"))
                .env("XDG_STATE_HOME", isolated.path().join("state"))
                .env("XDG_DATA_HOME", isolated.path().join("state"))
                .env("HOME", isolated.path().join("home"))
                .env("NIX_SHOW_STATS", "1")
                .env("NIX_SHOW_STATS_PATH", prefix.with_extension("stats.json"));
            let arguments: Vec<_> = std::iter::once(command.get_program())
                .chain(command.get_args())
                .map(|value| value.to_string_lossy().into_owned())
                .collect();
            fs::write(
                prefix.with_extension("command.json"),
                serde_json::to_vec_pretty(&arguments)?,
            )?;

            let output = command.output()?;
            fs::write(prefix.with_extension("stdout"), &output.stdout)?;
            fs::write(prefix.with_extension("stderr"), &output.stderr)?;
            if !output.status.success() {
                return Err(format!(
                    "{name} failed; see {path}",
                    path = prefix.with_extension("stderr").display()
                )
                .into());
            }
            let resources = fs::read_to_string(prefix.with_extension("resources"))?;
            let fields: Vec<_> = resources.split_whitespace().collect();
            let [wall, user, system, rss] = fields.as_slice() else {
                return Err("GNU time returned an unexpected resource record".into());
            };
            let result: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            let measurement = json!({
                "case": name, "repetition": repetition,
                "wall_seconds": wall.parse::<f64>()?,
                "user_seconds": user.parse::<f64>()?,
                "system_seconds": system.parse::<f64>()?,
                "peak_rss_kib": rss.parse::<u64>()?,
                "result": result,
            });
            println!("{measurement}");
            measurements.push(measurement);
            fs::write(
                logs.join("measurements.json"),
                serde_json::to_vec_pretty(&measurements)?,
            )?;
        }
    }

    Ok(())
}
