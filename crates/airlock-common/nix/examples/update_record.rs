//! Publish a local review note only if the previously observed record is still current.
//! The writer lock prevents competing writers; the snapshot detects stale observations.

use airlock_nix_core::state::{OperationKind, OperationOwner, RecordPermissions, StateStore};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct ReviewNote {
    /// Human-readable context saved beside a local review; it grants no deployment authority.
    summary: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let store = StateStore::initialize(root.path())?;
    let record = store.record::<ReviewNote>("review-note".parse()?, 4096)?;
    let owner = OperationOwner::new(
        "Review example".into(),
        "Save a local review note".into(),
        OperationKind::Maintenance,
    )?;

    let mut writer = record.try_write(owner)?;
    let observed = writer.read()?;
    let note = ReviewNote {
        summary: "Check the planned VM restarts before approval.".into(),
    };
    writer.commit(&observed, &note, RecordPermissions::Private)?;

    if let Some(saved) = record.read()?.value() {
        println!("{}", saved.summary);
    }

    Ok(())
}
