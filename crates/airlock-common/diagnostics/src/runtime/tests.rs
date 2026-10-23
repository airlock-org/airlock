//! Exercise authorization before reads, real filesystem boundaries and bounded child lifetimes.

use super::*;
use std::{cell::RefCell, os::unix::fs::symlink};

/// A file-backed endpoint isolates authorization behavior from host services and privileges.
fn fixture(path: &Path) -> ServerConfig {
    ServerConfig {
        schema: CONFIG_SCHEMA.into(),
        target: "usr-home".parse().unwrap(),
        callers: vec!["usr-assistant".parse().unwrap()],
        approval: ApprovalMode::Automatic,
        max_bytes: 64,
        sources: vec![Source {
            id: "application".parse().unwrap(),
            description: "Application evidence".into(),
            reader: Reader::File {
                path: path.to_str().unwrap().into(),
                tail: false,
                owner: rustix::process::geteuid().as_raw(),
            },
        }],
    }
}

/// One exact read request whose identity is preserved through approval and dispatch.
fn query(config: &ServerConfig) -> Query {
    Query {
        schema: REQUEST_SCHEMA.into(),
        id: "request-test".parse().unwrap(),
        target: config.target.clone(),
        reason: "Investigate an application failure".into(),
        action: Operation::Read {
            source: "application".parse().unwrap(),
            options: ReadOptions {
                max_bytes: 8,
                ..ReadOptions::default()
            },
        },
    }
}

#[test]
fn approval_precedes_reads_and_binds_full_request() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("evidence");
    fs::write(&path, "old-evidence-new").unwrap();
    let mut config = fixture(&path);
    let caller: CallerId = "usr-assistant".parse().unwrap();
    let original = query(&config);
    let events = RefCell::new(Vec::new());
    let log = |event: &Value| {
        events.borrow_mut().push(event.clone());
        Ok(())
    };

    let response = handle(&config, &caller, original.clone(), log);
    assert_eq!(response.outcome, Outcome::Truncated);
    assert_eq!(response.stdout, "old-evid");
    assert_eq!(events.borrow()[0]["phase"], "authorized_automatic");
    assert_eq!(response.approval, ApprovalMode::Automatic);
    assert!(
        events
            .borrow()
            .iter()
            .all(|event| !event.to_string().contains("old-evid"))
    );

    let before = ReadPlan::check(&config, &caller, original.clone()).unwrap();
    let mut changed = original.clone();
    if let Operation::Read { options, .. } = &mut changed.action {
        options.lines += 1;
    }
    let after = ReadPlan::check(&config, &caller, changed).unwrap();
    assert_ne!(
        before.permit.request().binding,
        after.permit.request().binding
    );
    config.sources[0].reader = Reader::File {
        path: directory.path().join("other").to_str().unwrap().into(),
        tail: false,
        owner: rustix::process::geteuid().as_raw(),
    };
    let after = ReadPlan::check(&config, &caller, original.clone()).unwrap();
    assert_ne!(
        before.permit.request().binding,
        after.permit.request().binding
    );

    config.approval = ApprovalMode::Deny;
    let response = handle(&config, &caller, original.clone(), |_| Ok(()));
    assert_eq!(response.outcome, Outcome::Denied);
    assert!(response.stdout.is_empty());
    config.approval = ApprovalMode::Automatic;
    assert_eq!(
        handle(&config, &caller, original, |_| anyhow::bail!(
            "audit unavailable"
        ))
        .outcome,
        Outcome::Unavailable
    );
}

#[test]
fn identity_scope_limits_and_discovery_are_enforced() {
    let directory = tempfile::tempdir().unwrap();
    let config = fixture(&directory.path().join("missing"));
    let caller = "usr-assistant".parse().unwrap();
    let original = query(&config);
    assert!(ReadPlan::check(&config, &"usr-home".parse().unwrap(), original.clone()).is_err());
    let mut wrong = original.clone();
    wrong.target = "dom0".parse().unwrap();
    assert!(ReadPlan::check(&config, &caller, wrong).is_err());
    let mut wrong = original.clone();
    wrong.action = Operation::Read {
        source: "unknown".parse().unwrap(),
        options: ReadOptions::default(),
    };
    assert!(ReadPlan::check(&config, &caller, wrong).is_err());
    let mut wrong = original.clone();
    if let Operation::Read { options, .. } = &mut wrong.action {
        options.max_bytes = config.max_bytes + 1;
    }
    assert!(ReadPlan::check(&config, &caller, wrong).is_err());

    let mut discovery = original;
    discovery.action = Operation::Catalog {};
    assert!(ReadPlan::check(&config, &"sys-gui".parse().unwrap(), discovery.clone()).is_err());
    let response = handle(&config, &caller, discovery, |_| Ok(()));
    assert_eq!(response.outcome, Outcome::Complete);
    assert_eq!(response.sources, config.sources);
    assert!(response.boot.is_some());
}

#[test]
fn missing_stale_binary_and_tail_evidence_are_explicit() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("evidence");
    let mut config = fixture(&path);
    let caller = "usr-assistant".parse().unwrap();
    let original = query(&config);
    assert_eq!(
        handle(&config, &caller, original.clone(), |_| Ok(())).outcome,
        Outcome::Unavailable
    );
    fs::write(&path, b"0123456789\xff").unwrap();
    let response = handle(&config, &caller, original.clone(), |_| Ok(()));
    assert_eq!(response.outcome, Outcome::Truncated);
    assert!(!response.lossy);
    config.sources[0].reader = Reader::File {
        path: path.to_str().unwrap().into(),
        tail: true,
        owner: rustix::process::geteuid().as_raw(),
    };
    let response = handle(&config, &caller, original.clone(), |_| Ok(()));
    assert_eq!(response.outcome, Outcome::Truncated);
    assert!(response.lossy);
    assert!(response.stdout.starts_with("345"));
    let mut stale = original;
    if let Operation::Read { options, .. } = &mut stale.action {
        options.boot = Some("00000000-0000-0000-0000-000000000000".parse().unwrap());
    }
    let response = handle(&config, &caller, stale, |_| Ok(()));
    assert_eq!(response.outcome, Outcome::StaleBoot);
    assert!(response.stdout.is_empty());
}

#[test]
fn file_reader_rejects_symlinks_and_special_files() {
    let directory = tempfile::tempdir().unwrap();
    let real = directory.path().join("real");
    fs::create_dir(&real).unwrap();
    fs::write(real.join("log"), "private").unwrap();
    symlink(&real, directory.path().join("alias")).unwrap();
    symlink(real.join("log"), directory.path().join("link")).unwrap();
    assert!(regular(&directory.path().join("alias/log")).is_err());
    assert!(regular(&directory.path().join("link")).is_err());
    assert!(regular(Path::new("/dev/zero")).is_err());
    assert!(regular(&real).is_err());
    let owner = rustix::process::geteuid().as_raw();
    assert!(regular_owned(&real.join("log"), Some(owner.wrapping_add(1))).is_err());
    fs::hard_link(real.join("log"), real.join("alias-log")).unwrap();
    assert!(regular_owned(&real.join("log"), Some(owner)).is_err());
    rustix::fs::mknodat(
        rustix::fs::CWD,
        directory.path().join("fifo"),
        rustix::fs::FileType::Fifo,
        Mode::RUSR | Mode::WUSR,
        0,
    )
    .unwrap();
    assert!(regular(&directory.path().join("fifo")).is_err());
}

/// Cargo-owned helper process; its environment is set on children, never globally in tests.
#[test]
fn child_fixture() {
    let Ok(mode) = std::env::var("AIRLOCK_DIAGNOSTICS_FIXTURE") else {
        return;
    };
    match mode.as_str() {
        "stream" => {
            std::io::stdout().write_all(&vec![b'o'; 131072]).unwrap();
            std::io::stderr().write_all(&vec![b'e'; 131072]).unwrap();
        }
        "hang" => std::thread::sleep(Duration::from_secs(30)),
        "fail" => std::process::exit(7),
        "pipes" => {
            // The helper exits while a background waiter and its child still own the pipes.
            let mut child = fixture_command("hang").spawn().unwrap();
            std::thread::spawn(move || child.wait().unwrap());
        }
        _ => panic!("unknown fixture"),
    }
}

/// Run one helper through the same Cargo test executable and a separate child environment.
fn fixture_command(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "runtime::tests::child_fixture", "--nocapture"])
        .env("AIRLOCK_DIAGNOSTICS_FIXTURE", mode);
    command
}

#[test]
fn command_output_deadlines_failures_and_inherited_pipes_are_bounded() {
    let output = process::capture(
        &mut fixture_command("stream"),
        &[],
        4096,
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(output.truncated);
    assert_eq!(output.stdout.len() + output.stderr.len(), 4096);
    let output = process::capture(
        &mut fixture_command("fail"),
        &[],
        4096,
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(!output.success);
    for mode in ["hang", "pipes"] {
        let started = Instant::now();
        let output = process::capture(
            &mut fixture_command(mode),
            &[],
            4096,
            Duration::from_millis(100),
        )
        .unwrap();
        assert!(output.timed_out);
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}

#[test]
fn journal_filters_are_exact_and_cannot_become_options() {
    let options = ReadOptions {
        unit: Some("app@instance.service".parse().unwrap()),
        ..ReadOptions::default()
    };
    let command = reader_command(
        &Reader::UserJournal { uid: 1000 },
        &options,
        "00000000-0000-0000-0000-000000000000",
    )
    .unwrap();
    let args: Vec<_> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert!(args.contains(&"_UID=1000".into()));
    assert!(args.contains(&"_SYSTEMD_USER_UNIT=app@instance.service".into()));
    for value in [
        "--system.service",
        "*.service",
        "a.service + _UID=0",
        "../secret.service",
    ] {
        assert!(value.parse::<UnitName>().is_err());
    }
}

#[test]
fn input_and_slot_limits_survive_an_uncooperative_caller() {
    use std::os::unix::net::UnixStream;
    let directory = tempfile::tempdir().unwrap();
    let mut slots = Vec::new();
    for _ in 0..4 {
        slots.push(acquire_slot(directory.path()).unwrap());
    }
    assert!(acquire_slot(directory.path()).is_err());
    slots.pop();
    assert!(acquire_slot(directory.path()).is_ok());

    let (mut sender, receiver) = UnixStream::pair().unwrap();
    sender.write_all(&vec![b'x'; REQUEST_LIMIT + 1]).unwrap();
    drop(sender);
    assert!(read_request(&mut receiver.try_clone().unwrap(), &receiver).is_err());

    let (_sender, receiver) = UnixStream::pair().unwrap();
    let started = Instant::now();
    assert!(read_request(&mut receiver.try_clone().unwrap(), &receiver).is_err());
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn response_identity_includes_the_exact_query_and_successful_boot() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("evidence");
    fs::write(&file, "evidence").unwrap();
    let config = fixture(&file);
    let query = query(&config);
    let response = handle(
        &config,
        &"usr-assistant".parse().unwrap(),
        query.clone(),
        |_| Ok(()),
    );
    validate_response(&query, &response).unwrap();
    let mut changed = response.clone();
    changed.target = "dom0".parse().unwrap();
    assert!(validate_response(&query, &changed).is_err());
    let mut changed = response.clone();
    changed.action = Operation::Catalog {};
    assert!(validate_response(&query, &changed).is_err());
    let mut changed = response;
    changed.boot = None;
    assert!(validate_response(&query, &changed).is_err());
}
