#![cfg(feature = "evaluation")]

//! Shared sessions keep each evaluation's source, map and staged inputs together.
use rusix::{Config, Expr, nixos::NixosModule};
use rusix::{NixSession, compile, nixos::compile_module};
use std::{
    fs::{File, FileTimes},
    sync::{Arc, Barrier},
    thread,
    time::SystemTime,
};

fn ssh(port: i64) -> NixosModule {
    NixosModule::new(
        Config::new()
            .set_dynamic("services.openssh.enable", false)
            .set_dynamic("services.openssh.ports", vec![port]),
    )
    .import("nixos/modules/services/networking/ssh/sshd.nix")
}

#[test]
fn explicit_source_rejects_missing_checkouts_before_evaluation() {
    let missing = tempfile::tempdir().unwrap();
    let error = NixSession::with_nixpkgs(missing.path()).err().unwrap();
    assert_eq!(error.kind, rusix::DiagnosticKind::Tooling);
    assert!(error.reason.contains("NixSession::with_nixpkgs"));
}

#[test]
fn concurrent_calls_keep_values_and_diagnostics_with_their_artifacts() {
    let session = Arc::new(NixSession::new().unwrap());
    let barrier = Arc::new(Barrier::new(4));
    let workers: Vec<_> = (0..4)
        .map(|index| {
            let session = session.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                let failure = Expr::int(index).divide(Expr::int(0));
                let config = Config::new()
                    .set_dynamic("good", index)
                    .set_dynamic(format!("bad{index}"), failure);
                let expected = config.assignments[1].value.origin.clone();
                let generated = compile(&config).unwrap();
                let module = compile_module(&ssh(22 + index)).unwrap();
                barrier.wait();

                for _ in 0..2 {
                    assert_eq!(
                        session
                            .evaluate_attribute(&generated, "good")
                            .unwrap()
                            .value,
                        index
                    );
                    let error = session
                        .evaluate_attribute(&generated, &format!("bad{index}"))
                        .unwrap_err();
                    assert_eq!(error.primary, Some(expected.clone()));
                    assert!(
                        error
                            .related
                            .iter()
                            .any(|origin| { origin.purpose == format!("set bad{index}") })
                    );
                    assert_eq!(
                        session
                            .evaluate_nixos(&module, &["services", "openssh", "ports"], false)
                            .unwrap()
                            .value,
                        serde_json::json!([22 + index])
                    );
                }
            })
        })
        .collect();

    for worker in workers {
        worker.join().unwrap();
    }
}

#[test]
fn repeated_evaluations_preserve_staged_files_and_sessions_have_separate_inputs() {
    let module = compile_module(&ssh(22)).unwrap();
    let session = NixSession::new().unwrap();
    let selection = ["services", "openssh", "ports"];
    session.evaluate_nixos(&module, &selection, false).unwrap();

    let path = session.root().join("nixpkgs/lib/modules.nix");
    File::open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(SystemTime::UNIX_EPOCH))
        .unwrap();
    session.evaluate_nixos(&module, &selection, false).unwrap();
    assert_eq!(
        path.metadata().unwrap().modified().unwrap(),
        SystemTime::UNIX_EPOCH
    );

    let other = NixSession::new().unwrap();
    other.evaluate_nixos(&module, &selection, false).unwrap();
    assert_ne!(session.root(), other.root());
    assert_ne!(
        other
            .root()
            .join("nixpkgs/lib/modules.nix")
            .metadata()
            .unwrap()
            .modified()
            .unwrap(),
        SystemTime::UNIX_EPOCH
    );
}
