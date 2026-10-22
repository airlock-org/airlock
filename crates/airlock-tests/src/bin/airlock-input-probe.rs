//! Run the installed input regression only in the explicitly marked disposable VM.

#[path = "../input_receiver.rs"]
mod input_receiver;

use clap::Parser;
use std::path::PathBuf;

/// Explicit VM identity and immutable tool path supplied by the test fixture.
#[derive(Parser)]
struct Args {
    // Required acknowledgement; a marker file inside the VM supplies the second guard.
    #[arg(long)]
    inside_test_vm: bool,
    // Absolute udev observer from the fixture's own store closure.
    #[arg(long)]
    udevadm: PathBuf,
}

/// Require an explicit invocation as well as the fixture marker checked by the probe.
fn main() -> anyhow::Result<()> {
    let arguments = Args::parse();
    anyhow::ensure!(
        arguments.inside_test_vm,
        "run only inside the disposable input test VM"
    );
    input_receiver::run(&arguments.udevadm)
}
