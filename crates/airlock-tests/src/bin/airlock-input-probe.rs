//! Run the installed input regression only in the explicitly marked disposable VM.

#[path = "../input_receiver.rs"]
mod input_receiver;

/// Require an explicit invocation as well as the fixture marker checked by the probe.
fn main() -> anyhow::Result<()> {
    anyhow::ensure!(
        std::env::args().nth(1).as_deref() == Some("--inside-test-vm"),
        "run only inside the disposable input test VM"
    );
    input_receiver::run()
}
